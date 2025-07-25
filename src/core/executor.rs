use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs as async_fs;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::core::checksum::calculate_checksum;
use crate::core::hooks::{HookContext, HookManager, HookType};
use crate::core::plugin::{Plugin, PluginRegistry, PluginResult};
use crate::core::snapshot::SnapshotManager;
use crate::symbols::*;

/// Executes all plugins asynchronously and creates a snapshot
pub struct SnapshotExecutor {
    registry: Arc<PluginRegistry>,
    snapshot_manager: SnapshotManager,
    config: Option<Arc<Config>>,
}

impl SnapshotExecutor {
    pub fn with_config(
        registry: Arc<PluginRegistry>,
        base_path: PathBuf,
        config: Arc<Config>,
    ) -> Self {
        Self {
            registry,
            snapshot_manager: SnapshotManager::new(base_path),
            config: Some(config),
        }
    }

    /// Executes all plugins and creates a snapshot
    pub async fn execute_snapshot(&self) -> Result<PathBuf> {
        info!("Starting snapshot execution");

        // Create snapshot directory
        let snapshot_dir = self.snapshot_manager.create_snapshot_dir().await?;
        let snapshot_name = snapshot_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        info!("Created snapshot directory: {}", snapshot_dir.display());

        // Set up hooks manager and context
        let hooks_config = self
            .config
            .as_ref()
            .map(|c| c.get_hooks_config())
            .unwrap_or_default();
        let hook_manager = HookManager::new(hooks_config.clone());
        let hook_context =
            HookContext::new(snapshot_name, snapshot_dir.clone(), hooks_config.clone());

        // Execute pre-snapshot hooks (global)
        if let Some(config) = &self.config {
            let pre_snapshot_hooks = config.get_global_pre_snapshot_hooks();
            if !pre_snapshot_hooks.is_empty() {
                hook_manager
                    .execute_hooks(&pre_snapshot_hooks, &HookType::PreSnapshot, &hook_context)
                    .await;
            }
        }

        // Create initial metadata
        let mut metadata = self.snapshot_manager.create_metadata();

        // Execute all plugins concurrently
        let plugins = self.registry.plugins();
        let mut plugin_tasks = Vec::new();

        for (plugin_name, plugin) in plugins {
            let plugin_clone = Arc::clone(plugin);
            let plugin_name_clone = plugin_name.clone();
            let snapshot_dir_clone = snapshot_dir.clone();
            let snapshot_manager_clone = self.snapshot_manager.clone();
            let config_clone = self.config.clone();
            let hook_manager_clone = HookManager::new(hooks_config.clone());
            let hook_context_clone = hook_context.clone();

            let task = tokio::spawn(async move {
                Self::execute_plugin_with_hooks(
                    plugin_name_clone,
                    plugin_clone,
                    &snapshot_dir_clone,
                    &snapshot_manager_clone,
                    config_clone.as_deref(),
                    hook_manager_clone,
                    hook_context_clone,
                )
                .await
            });

            plugin_tasks.push(task);
        }

        // Wait for all plugins to complete
        let mut results = Vec::new();
        for task in plugin_tasks {
            match task.await {
                Ok(result) => {
                    match result {
                        Ok(plugin_result) => {
                            results.push(plugin_result);
                        }
                        Err(e) => {
                            error!("Plugin execution failed: {}", e);
                            // Create error result for failed plugin
                            results.push(PluginResult {
                                plugin_name: "unknown".to_string(),
                                content: String::new(),
                                checksum: String::new(),
                                success: false,
                                error_message: Some(e.to_string()),
                            });
                        }
                    }
                }
                Err(e) => {
                    error!("Plugin task failed: {}", e);
                }
            }
        }

        // Update metadata with plugin results
        for result in &results {
            if result.success {
                metadata
                    .checksums
                    .insert(result.plugin_name.clone(), result.checksum.clone());
            }
        }

        // Save metadata
        self.snapshot_manager
            .save_metadata(&snapshot_dir, &metadata)
            .await?;

        // Finalize snapshot (calculate directory checksum)
        self.snapshot_manager
            .finalize_snapshot(&snapshot_dir)
            .await?;

        // Execute post-snapshot hooks (global)
        if let Some(config) = &self.config {
            let post_snapshot_hooks = config.get_global_post_snapshot_hooks();
            if !post_snapshot_hooks.is_empty() {
                let final_context = hook_context.with_file_count(results.len());
                hook_manager
                    .execute_hooks(
                        &post_snapshot_hooks,
                        &HookType::PostSnapshot,
                        &final_context,
                    )
                    .await;
            }
        }

        info!("Snapshot execution completed: {}", snapshot_dir.display());
        Ok(snapshot_dir)
    }

    /// Executes a single plugin with hooks and checksum optimization
    async fn execute_plugin_with_hooks(
        plugin_name: String,
        plugin: Arc<dyn Plugin>,
        snapshot_dir: &Path,
        snapshot_manager: &SnapshotManager,
        _config: Option<&Config>,
        hook_manager: HookManager,
        hook_context: HookContext,
    ) -> Result<PluginResult> {
        info!(
            "{} Executing plugin: {}",
            SYMBOL_CONTENT_PACKAGE, plugin_name
        );

        // Create plugin-specific hook context
        let plugin_hook_context = hook_context.with_plugin(plugin_name.clone());

        // Execute pre-plugin hooks from plugin's own configuration
        let plugin_hooks = plugin.get_hooks();
        if !plugin_hooks.is_empty() {
            hook_manager
                .execute_hooks(&plugin_hooks, &HookType::PrePlugin, &plugin_hook_context)
                .await;
        }

        // Validate plugin can run
        if let Err(e) = plugin.validate().await {
            warn!("Plugin validation failed for {}: {}", plugin_name, e);
            return Ok(PluginResult {
                plugin_name: plugin_name.clone(),
                content: String::new(),
                checksum: String::new(),
                success: false,
                error_message: Some(format!("Validation failed: {e}")),
            });
        }

        // Set environment variable for snapshot directory (for plugins that need it)
        std::env::set_var("DOTSNAPSHOT_SNAPSHOT_DIR", snapshot_dir);

        // Execute plugin to get content
        let content = match plugin.execute().await {
            Ok(content) => content,
            Err(e) => {
                error!("Plugin execution failed for {}: {}", plugin_name, e);

                // Execute post-plugin hooks even on failure
                let plugin_hooks = plugin.get_hooks();
                if !plugin_hooks.is_empty() {
                    let error_context =
                        plugin_hook_context.with_variable("error".to_string(), e.to_string());
                    hook_manager
                        .execute_hooks(&plugin_hooks, &HookType::PostPlugin, &error_context)
                        .await;
                }

                return Ok(PluginResult {
                    plugin_name: plugin_name.clone(),
                    content: String::new(),
                    checksum: String::new(),
                    success: false,
                    error_message: Some(e.to_string()),
                });
            }
        };

        // Calculate checksum
        let checksum = calculate_checksum(&content);

        // Check if we can reuse existing file with same checksum
        let output_file_for_checksum =
            PluginRegistry::get_plugin_output_file_from_plugin(plugin.as_ref(), &plugin_name);
        if let Ok(Some(_existing_file)) = snapshot_manager
            .find_file_by_checksum(
                &plugin_name,
                &output_file_for_checksum,
                &checksum,
                snapshot_dir,
            )
            .await
        {
            info!(
                "Reusing existing file for plugin {} (checksum match)",
                plugin_name
            );

            // Copy file from latest snapshot
            if snapshot_manager
                .copy_from_latest(&plugin_name, &output_file_for_checksum, snapshot_dir)
                .await?
            {
                let result = PluginResult {
                    plugin_name: plugin_name.clone(),
                    content,
                    checksum,
                    success: true,
                    error_message: None,
                };

                // Execute post-plugin hooks for successful reuse
                let plugin_hooks = plugin.get_hooks();
                if !plugin_hooks.is_empty() {
                    let success_context = plugin_hook_context
                        .with_file_count(1)
                        .with_variable("reused".to_string(), "true".to_string());
                    hook_manager
                        .execute_hooks(&plugin_hooks, &HookType::PostPlugin, &success_context)
                        .await;
                }

                return Ok(result);
            }
        }

        // Determine output path for hooks (even if we don't save for static files)
        let output_file =
            PluginRegistry::get_plugin_output_file_from_plugin(plugin.as_ref(), &plugin_name);
        let output_path = if let Some(custom_path) = plugin.get_target_path() {
            snapshot_dir.join(custom_path).join(&output_file)
        } else {
            snapshot_dir.join(&output_file)
        };

        // Some plugins handle their own file operations, skip output file creation for those
        if !plugin.creates_own_output_files() {
            // Create parent directory if it doesn't exist
            if let Some(parent) = output_path.parent() {
                async_fs::create_dir_all(parent).await.context(format!(
                    "Failed to create parent directory for plugin {plugin_name}"
                ))?;
            }

            async_fs::write(&output_path, &content)
                .await
                .context(format!("Failed to write output for plugin {plugin_name}"))?;
        }

        info!(
            "{} Plugin {} completed successfully",
            SYMBOL_INDICATOR_SUCCESS, plugin_name
        );

        let result = PluginResult {
            plugin_name: plugin_name.clone(),
            content,
            checksum,
            success: true,
            error_message: None,
        };

        // Execute post-plugin hooks for successful completion
        let plugin_hooks = plugin.get_hooks();
        if !plugin_hooks.is_empty() {
            let success_context = plugin_hook_context.with_file_count(1).with_variable(
                "output_path".to_string(),
                output_path.to_string_lossy().to_string(),
            );
            hook_manager
                .execute_hooks(&plugin_hooks, &HookType::PostPlugin, &success_context)
                .await;
        }

        Ok(result)
    }
}

// Make SnapshotManager cloneable for use in async tasks
impl Clone for SnapshotManager {
    fn clone(&self) -> Self {
        Self::new(self.base_path().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use tempfile::TempDir;

    // Test-only symbol for mock plugins
    const SYMBOL_ACTION_TEST: &str = "🧪";

    /// Mock plugin implementation for testing executor functionality
    struct TestPlugin {
        content: String,
        should_fail: bool,
        validation_error: Option<String>,
        creates_own_files: bool,
    }

    impl TestPlugin {
        fn new(content: String) -> Self {
            Self {
                content,
                should_fail: false,
                validation_error: None,
                creates_own_files: false,
            }
        }

        fn with_validation_error(mut self, error: String) -> Self {
            self.validation_error = Some(error);
            self
        }

        fn with_execution_failure(mut self) -> Self {
            self.should_fail = true;
            self
        }

        fn with_custom_file_handling(mut self) -> Self {
            self.creates_own_files = true;
            self
        }
    }

    #[async_trait]
    impl Plugin for TestPlugin {
        fn description(&self) -> &str {
            "Test plugin for executor tests"
        }

        fn icon(&self) -> &str {
            SYMBOL_ACTION_TEST
        }

        async fn execute(&self) -> Result<String> {
            if self.should_fail {
                return Err(anyhow::anyhow!("Test plugin execution failure"));
            }
            Ok(self.content.clone())
        }

        async fn validate(&self) -> Result<()> {
            if let Some(ref error) = self.validation_error {
                return Err(anyhow::anyhow!(error.clone()));
            }
            Ok(())
        }

        fn get_target_path(&self) -> Option<String> {
            None
        }

        fn get_output_file(&self) -> Option<String> {
            None
        }

        fn creates_own_output_files(&self) -> bool {
            self.creates_own_files
        }
    }

    /// Test basic snapshot execution with a single plugin
    /// Verifies that the executor can create a snapshot directory,
    /// execute a plugin, save its output, and create metadata
    #[tokio::test]
    async fn test_execute_snapshot_basic() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "test_plugin".to_string(),
            Arc::new(TestPlugin::new("test content".to_string())),
        );

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        assert!(snapshot_dir.join("test_plugin.txt").exists());
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        let content = async_fs::read_to_string(snapshot_dir.join("test_plugin.txt")).await?;
        assert_eq!(content, "test content");

        Ok(())
    }

    /// Test snapshot execution when plugin validation fails
    /// Verifies that the executor gracefully handles validation failures
    /// and continues to create metadata even when plugins fail validation
    #[tokio::test]
    async fn test_execute_snapshot_with_validation_failure() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "failing_plugin".to_string(),
            Arc::new(
                TestPlugin::new("content".to_string())
                    .with_validation_error("Validation failed".to_string()),
            ),
        );

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        // Plugin file should not be created due to validation failure
        assert!(!snapshot_dir.join("failing_plugin.txt").exists());
        // But metadata should still be created
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        Ok(())
    }

    /// Test snapshot execution when plugin execution fails
    /// Verifies that the executor handles plugin execution failures gracefully
    /// and continues to create metadata even when plugins fail to execute
    #[tokio::test]
    async fn test_execute_snapshot_with_execution_failure() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "failing_plugin".to_string(),
            Arc::new(TestPlugin::new("content".to_string()).with_execution_failure()),
        );

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        // Plugin file should not be created due to execution failure
        assert!(!snapshot_dir.join("failing_plugin.txt").exists());
        // But metadata should still be created
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        Ok(())
    }

    /// Test snapshot execution with multiple plugins running concurrently
    /// Verifies that the executor can handle multiple plugins simultaneously
    /// and that all plugins produce their expected outputs
    #[tokio::test]
    async fn test_execute_snapshot_multiple_plugins() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "plugin1".to_string(),
            Arc::new(TestPlugin::new("content1".to_string())),
        );
        registry.add_plugin(
            "plugin2".to_string(),
            Arc::new(TestPlugin::new("content2".to_string())),
        );
        registry.add_plugin(
            "plugin3".to_string(),
            Arc::new(TestPlugin::new("content3".to_string())),
        );

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        assert!(snapshot_dir.join("plugin1.txt").exists());
        assert!(snapshot_dir.join("plugin2.txt").exists());
        assert!(snapshot_dir.join("plugin3.txt").exists());
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        let content1 = async_fs::read_to_string(snapshot_dir.join("plugin1.txt")).await?;
        let content2 = async_fs::read_to_string(snapshot_dir.join("plugin2.txt")).await?;
        let content3 = async_fs::read_to_string(snapshot_dir.join("plugin3.txt")).await?;

        assert_eq!(content1, "content1");
        assert_eq!(content2, "content2");
        assert_eq!(content3, "content3");

        Ok(())
    }

    /// Test snapshot execution with plugins that handle their own file operations
    /// Verifies that the executor respects plugins that create their own files
    /// and doesn't interfere with custom file handling logic
    #[tokio::test]
    async fn test_execute_snapshot_with_custom_file_handling() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "custom_plugin".to_string(),
            Arc::new(TestPlugin::new("custom content".to_string()).with_custom_file_handling()),
        );

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        // Plugin handles its own file creation, so no default .txt file should exist
        assert!(!snapshot_dir.join("custom_plugin.txt").exists());
        // But metadata should still be created
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        Ok(())
    }

    /// Test individual plugin execution with hooks integration
    /// Verifies that a single plugin can be executed with proper hook context
    /// and that the plugin result contains correct data
    #[tokio::test]
    async fn test_execute_plugin_with_hooks_basic() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let snapshot_dir = temp_dir.path().join("snapshot");
        async_fs::create_dir_all(&snapshot_dir).await?;

        let plugin = Arc::new(TestPlugin::new("test content".to_string()));
        let snapshot_manager = SnapshotManager::new(temp_dir.path().to_path_buf());
        let hook_manager = HookManager::new(Default::default());
        let hook_context = HookContext::new(
            "test_snapshot".to_string(),
            snapshot_dir.clone(),
            Default::default(),
        );

        let result = SnapshotExecutor::execute_plugin_with_hooks(
            "test_plugin".to_string(),
            plugin,
            &snapshot_dir,
            &snapshot_manager,
            None,
            hook_manager,
            hook_context,
        )
        .await?;

        assert!(result.success);
        assert_eq!(result.plugin_name, "test_plugin");
        assert_eq!(result.content, "test content");
        assert!(!result.checksum.is_empty());
        assert!(result.error_message.is_none());

        // Verify file was created
        assert!(snapshot_dir.join("test_plugin.txt").exists());
        let file_content = async_fs::read_to_string(snapshot_dir.join("test_plugin.txt")).await?;
        assert_eq!(file_content, "test content");

        Ok(())
    }

    /// Test that SnapshotManager can be cloned correctly
    /// Verifies that the Clone implementation preserves the base path
    /// and creates a functionally equivalent instance
    #[tokio::test]
    async fn test_snapshot_manager_clone() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();

        let original = SnapshotManager::new(base_path.clone());
        let cloned = original.clone();

        // Both should reference the same base path
        assert_eq!(original.base_path(), cloned.base_path());
    }
}

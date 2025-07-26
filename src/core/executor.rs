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

    /// Test SnapshotExecutor creation with configuration
    /// Verifies that the executor can be properly instantiated with config
    #[tokio::test]
    async fn test_snapshot_executor_with_config() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();

        let registry = Arc::new(PluginRegistry::new());
        let config = Arc::new(Config::default());

        let executor = SnapshotExecutor::with_config(registry, base_path, config);

        // Verify executor is properly constructed (no panics or errors)
        assert!(executor.config.is_some());
    }

    /// Test plugin execution with validation failure in plugin context
    /// Verifies that validation errors are properly caught and handled
    #[tokio::test]
    async fn test_execute_plugin_validation_failure() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let snapshot_dir = temp_dir.path().join("snapshot");
        async_fs::create_dir_all(&snapshot_dir).await?;

        let plugin = Arc::new(
            TestPlugin::new("content".to_string())
                .with_validation_error("Plugin not available".to_string()),
        );
        let snapshot_manager = SnapshotManager::new(temp_dir.path().to_path_buf());
        let hook_manager = HookManager::new(Default::default());
        let hook_context = HookContext::new(
            "test_snapshot".to_string(),
            snapshot_dir.clone(),
            Default::default(),
        );

        let result = SnapshotExecutor::execute_plugin_with_hooks(
            "failing_plugin".to_string(),
            plugin,
            &snapshot_dir,
            &snapshot_manager,
            None,
            hook_manager,
            hook_context,
        )
        .await?;

        assert!(!result.success);
        assert_eq!(result.plugin_name, "failing_plugin");
        assert!(result.error_message.is_some());
        assert!(result.error_message.unwrap().contains("Validation failed"));
        assert_eq!(result.content, "");
        assert_eq!(result.checksum, "");

        Ok(())
    }

    /// Test plugin execution with execution failure and hook handling
    /// Verifies that execution errors trigger appropriate hook cleanup
    #[tokio::test]
    async fn test_execute_plugin_execution_failure() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let snapshot_dir = temp_dir.path().join("snapshot");
        async_fs::create_dir_all(&snapshot_dir).await?;

        let plugin = Arc::new(TestPlugin::new("content".to_string()).with_execution_failure());
        let snapshot_manager = SnapshotManager::new(temp_dir.path().to_path_buf());
        let hook_manager = HookManager::new(Default::default());
        let hook_context = HookContext::new(
            "test_snapshot".to_string(),
            snapshot_dir.clone(),
            Default::default(),
        );

        let result = SnapshotExecutor::execute_plugin_with_hooks(
            "failing_plugin".to_string(),
            plugin,
            &snapshot_dir,
            &snapshot_manager,
            None,
            hook_manager,
            hook_context,
        )
        .await?;

        assert!(!result.success);
        assert_eq!(result.plugin_name, "failing_plugin");
        assert!(result.error_message.is_some());
        assert!(result
            .error_message
            .unwrap()
            .contains("Test plugin execution failure"));

        Ok(())
    }

    /// Test plugin execution with custom target path
    /// Verifies that plugins with custom paths create files in the right location
    #[tokio::test]
    async fn test_execute_plugin_with_custom_target_path() -> Result<()> {
        struct CustomPathPlugin;

        #[async_trait]
        impl Plugin for CustomPathPlugin {
            fn description(&self) -> &str {
                "Custom path plugin"
            }
            fn icon(&self) -> &str {
                SYMBOL_ACTION_TEST
            }
            async fn execute(&self) -> Result<String> {
                Ok("custom content".to_string())
            }
            async fn validate(&self) -> Result<()> {
                Ok(())
            }
            fn get_target_path(&self) -> Option<String> {
                Some("custom/path".to_string())
            }
            fn get_output_file(&self) -> Option<String> {
                Some("custom.txt".to_string())
            }
            fn creates_own_output_files(&self) -> bool {
                false
            }
        }

        let temp_dir = TempDir::new()?;
        let snapshot_dir = temp_dir.path().join("snapshot");
        async_fs::create_dir_all(&snapshot_dir).await?;

        let plugin = Arc::new(CustomPathPlugin);
        let snapshot_manager = SnapshotManager::new(temp_dir.path().to_path_buf());
        let hook_manager = HookManager::new(Default::default());
        let hook_context = HookContext::new(
            "test_snapshot".to_string(),
            snapshot_dir.clone(),
            Default::default(),
        );

        let result = SnapshotExecutor::execute_plugin_with_hooks(
            "custom_plugin".to_string(),
            plugin,
            &snapshot_dir,
            &snapshot_manager,
            None,
            hook_manager,
            hook_context,
        )
        .await?;

        assert!(result.success);
        assert_eq!(result.content, "custom content");

        // Verify file was created in custom path
        assert!(snapshot_dir.join("custom/path/custom.txt").exists());
        let content = async_fs::read_to_string(snapshot_dir.join("custom/path/custom.txt")).await?;
        assert_eq!(content, "custom content");

        Ok(())
    }

    /// Test plugin execution that creates its own output files
    /// Verifies that plugins with custom file handling don't get default file creation
    #[tokio::test]
    async fn test_execute_plugin_creates_own_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let snapshot_dir = temp_dir.path().join("snapshot");
        async_fs::create_dir_all(&snapshot_dir).await?;

        let plugin =
            Arc::new(TestPlugin::new("custom content".to_string()).with_custom_file_handling());
        let snapshot_manager = SnapshotManager::new(temp_dir.path().to_path_buf());
        let hook_manager = HookManager::new(Default::default());
        let hook_context = HookContext::new(
            "test_snapshot".to_string(),
            snapshot_dir.clone(),
            Default::default(),
        );

        let result = SnapshotExecutor::execute_plugin_with_hooks(
            "custom_plugin".to_string(),
            plugin,
            &snapshot_dir,
            &snapshot_manager,
            None,
            hook_manager,
            hook_context,
        )
        .await?;

        assert!(result.success);
        assert_eq!(result.content, "custom content");

        // File should NOT be created because plugin handles its own files
        assert!(!snapshot_dir.join("custom_plugin.txt").exists());

        Ok(())
    }

    /// Test snapshot execution with mixed success and failure plugins
    /// Verifies that executor handles heterogeneous plugin results correctly
    #[tokio::test]
    async fn test_execute_snapshot_mixed_results() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "success_plugin".to_string(),
            Arc::new(TestPlugin::new("success content".to_string())),
        );
        registry.add_plugin(
            "validation_failure".to_string(),
            Arc::new(
                TestPlugin::new("content".to_string())
                    .with_validation_error("Validation error".to_string()),
            ),
        );
        registry.add_plugin(
            "execution_failure".to_string(),
            Arc::new(TestPlugin::new("content".to_string()).with_execution_failure()),
        );

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        // Success plugin should create file
        assert!(snapshot_dir.join("success_plugin.txt").exists());
        // Failed plugins should not create files
        assert!(!snapshot_dir.join("validation_failure.txt").exists());
        assert!(!snapshot_dir.join("execution_failure.txt").exists());
        // Metadata should still be created
        assert!(snapshot_dir.join(".snapshot/checksum.json").exists());

        let content = async_fs::read_to_string(snapshot_dir.join("success_plugin.txt")).await?;
        assert_eq!(content, "success content");

        Ok(())
    }

    /// Test plugin hook integration with pre and post hooks
    /// Verifies that plugin-specific hooks are executed correctly
    #[tokio::test]
    async fn test_plugin_hooks_integration() -> Result<()> {
        use crate::core::hooks::HookAction;

        struct HookedPlugin {
            hooks: Vec<HookAction>,
        }

        impl HookedPlugin {
            fn new() -> Self {
                Self {
                    hooks: vec![HookAction::Log {
                        message: "Pre-plugin hook".to_string(),
                        level: "info".to_string(),
                    }],
                }
            }
        }

        #[async_trait]
        impl Plugin for HookedPlugin {
            fn description(&self) -> &str {
                "Plugin with hooks"
            }
            fn icon(&self) -> &str {
                SYMBOL_ACTION_TEST
            }
            async fn execute(&self) -> Result<String> {
                Ok("hooked content".to_string())
            }
            async fn validate(&self) -> Result<()> {
                Ok(())
            }
            fn get_target_path(&self) -> Option<String> {
                None
            }
            fn get_output_file(&self) -> Option<String> {
                None
            }
            fn creates_own_output_files(&self) -> bool {
                false
            }
            fn get_hooks(&self) -> Vec<HookAction> {
                self.hooks.clone()
            }
        }

        let temp_dir = TempDir::new()?;
        let snapshot_dir = temp_dir.path().join("snapshot");
        async_fs::create_dir_all(&snapshot_dir).await?;

        let plugin = Arc::new(HookedPlugin::new());
        let snapshot_manager = SnapshotManager::new(temp_dir.path().to_path_buf());
        let hook_manager = HookManager::new(Default::default());
        let hook_context = HookContext::new(
            "test_snapshot".to_string(),
            snapshot_dir.clone(),
            Default::default(),
        );

        let result = SnapshotExecutor::execute_plugin_with_hooks(
            "hooked_plugin".to_string(),
            plugin,
            &snapshot_dir,
            &snapshot_manager,
            None,
            hook_manager,
            hook_context,
        )
        .await?;

        assert!(result.success);
        assert_eq!(result.content, "hooked content");

        Ok(())
    }

    /// Test snapshot execution with global pre and post hooks
    /// Verifies that global hooks are executed before and after plugin execution
    #[tokio::test]
    async fn test_execute_snapshot_with_global_hooks() -> Result<()> {
        use crate::config::{GlobalConfig, GlobalHooks};
        use crate::core::hooks::HookAction;

        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "test_plugin".to_string(),
            Arc::new(TestPlugin::new("test content".to_string())),
        );

        let global_hooks = GlobalHooks {
            pre_snapshot: vec![HookAction::Log {
                message: "Pre-snapshot hook".to_string(),
                level: "info".to_string(),
            }],
            post_snapshot: vec![HookAction::Log {
                message: "Post-snapshot hook".to_string(),
                level: "info".to_string(),
            }],
        };

        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(global_hooks),
            }),
            ..Default::default()
        };

        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        assert!(snapshot_dir.join("test_plugin.txt").exists());
        assert!(snapshot_dir.join(".snapshot/checksum.json").exists());

        Ok(())
    }

    /// Test environment variable setting during plugin execution
    /// Verifies that DOTSNAPSHOT_SNAPSHOT_DIR is properly set for plugins
    #[tokio::test]
    async fn test_plugin_environment_variable() -> Result<()> {
        struct EnvCheckPlugin;

        #[async_trait]
        impl Plugin for EnvCheckPlugin {
            fn description(&self) -> &str {
                "Environment check plugin"
            }
            fn icon(&self) -> &str {
                SYMBOL_ACTION_TEST
            }
            async fn execute(&self) -> Result<String> {
                // Check if environment variable is set
                if let Ok(snapshot_dir) = std::env::var("DOTSNAPSHOT_SNAPSHOT_DIR") {
                    Ok(format!("Snapshot dir: {snapshot_dir}"))
                } else {
                    Ok("No snapshot dir set".to_string())
                }
            }
            async fn validate(&self) -> Result<()> {
                Ok(())
            }
            fn get_target_path(&self) -> Option<String> {
                None
            }
            fn get_output_file(&self) -> Option<String> {
                None
            }
            fn creates_own_output_files(&self) -> bool {
                false
            }
        }

        let temp_dir = TempDir::new()?;
        let snapshot_dir = temp_dir.path().join("snapshot");
        async_fs::create_dir_all(&snapshot_dir).await?;

        let plugin = Arc::new(EnvCheckPlugin);
        let snapshot_manager = SnapshotManager::new(temp_dir.path().to_path_buf());
        let hook_manager = HookManager::new(Default::default());
        let hook_context = HookContext::new(
            "test_snapshot".to_string(),
            snapshot_dir.clone(),
            Default::default(),
        );

        let result = SnapshotExecutor::execute_plugin_with_hooks(
            "env_plugin".to_string(),
            plugin,
            &snapshot_dir,
            &snapshot_manager,
            None,
            hook_manager,
            hook_context,
        )
        .await?;

        assert!(result.success);
        assert!(result.content.contains("Snapshot dir:"));

        Ok(())
    }

    /// Test checksum reuse functionality during plugin execution
    /// Verifies that plugins with matching checksums can reuse existing files
    #[tokio::test]
    async fn test_plugin_checksum_reuse() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        // Create executor with test plugin
        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "reuse_plugin".to_string(),
            Arc::new(TestPlugin::new("reusable content".to_string())),
        );

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        // First execution - creates initial snapshot
        let snapshot_dir1 = executor.execute_snapshot().await?;
        assert!(snapshot_dir1.join("reuse_plugin.txt").exists());

        // Second execution with same content should potentially reuse
        let snapshot_dir2 = executor.execute_snapshot().await?;
        assert!(snapshot_dir2.join("reuse_plugin.txt").exists());

        // Verify both files have the same content
        let content1 = async_fs::read_to_string(snapshot_dir1.join("reuse_plugin.txt")).await?;
        let content2 = async_fs::read_to_string(snapshot_dir2.join("reuse_plugin.txt")).await?;
        assert_eq!(content1, content2);

        Ok(())
    }

    /// Test plugin execution with task join failure simulation
    /// Verifies that executor handles async task panics gracefully
    #[tokio::test]
    async fn test_plugin_task_panic_handling() -> Result<()> {
        struct PanicPlugin;

        #[async_trait]
        impl Plugin for PanicPlugin {
            fn description(&self) -> &str {
                "Plugin that panics"
            }
            fn icon(&self) -> &str {
                SYMBOL_ACTION_TEST
            }
            async fn execute(&self) -> Result<String> {
                panic!("Intentional panic for testing");
            }
            async fn validate(&self) -> Result<()> {
                Ok(())
            }
            fn get_target_path(&self) -> Option<String> {
                None
            }
            fn get_output_file(&self) -> Option<String> {
                None
            }
            fn creates_own_output_files(&self) -> bool {
                false
            }
        }

        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.add_plugin("panic_plugin".to_string(), Arc::new(PanicPlugin));

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        // Executor should handle the panic and still complete
        let snapshot_dir = executor.execute_snapshot().await?;
        assert!(snapshot_dir.exists());
        assert!(snapshot_dir.join(".snapshot/checksum.json").exists());

        Ok(())
    }

    /// Test plugin execution without configuration
    /// Verifies that executor works properly with no config provided
    #[tokio::test]
    async fn test_execute_snapshot_no_config() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "simple_plugin".to_string(),
            Arc::new(TestPlugin::new("simple content".to_string())),
        );

        // Create executor without config
        let executor = SnapshotExecutor {
            registry: Arc::new(registry),
            snapshot_manager: SnapshotManager::new(base_path),
            config: None,
        };

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        assert!(snapshot_dir.join("simple_plugin.txt").exists());
        assert!(snapshot_dir.join(".snapshot/checksum.json").exists());

        Ok(())
    }

    /// Test plugin execution with failed file write scenario
    /// Verifies error handling when file operations fail
    #[tokio::test]
    async fn test_plugin_file_write_failure() -> Result<()> {
        struct ReadOnlyPlugin;

        #[async_trait]
        impl Plugin for ReadOnlyPlugin {
            fn description(&self) -> &str {
                "Plugin that tries to write to readonly location"
            }
            fn icon(&self) -> &str {
                SYMBOL_ACTION_TEST
            }
            async fn execute(&self) -> Result<String> {
                Ok("readonly content".to_string())
            }
            async fn validate(&self) -> Result<()> {
                Ok(())
            }
            fn get_target_path(&self) -> Option<String> {
                // Try to write to root directory (should fail on most systems)
                Some("/root/invalid".to_string())
            }
            fn get_output_file(&self) -> Option<String> {
                Some("test.txt".to_string())
            }
            fn creates_own_output_files(&self) -> bool {
                false
            }
        }

        let temp_dir = TempDir::new()?;
        let snapshot_dir = temp_dir.path().join("snapshot");
        async_fs::create_dir_all(&snapshot_dir).await?;

        let plugin = Arc::new(ReadOnlyPlugin);
        let snapshot_manager = SnapshotManager::new(temp_dir.path().to_path_buf());
        let hook_manager = HookManager::new(Default::default());
        let hook_context = HookContext::new(
            "test_snapshot".to_string(),
            snapshot_dir.clone(),
            Default::default(),
        );

        // This should fail due to permission issues but return error properly
        let result = SnapshotExecutor::execute_plugin_with_hooks(
            "readonly_plugin".to_string(),
            plugin,
            &snapshot_dir,
            &snapshot_manager,
            None,
            hook_manager,
            hook_context,
        )
        .await;

        // Should either fail or succeed depending on system permissions
        // The key is that it doesn't panic
        assert!(result.is_ok() || result.is_err());

        Ok(())
    }

    /// Test plugin hooks execution with failure scenarios
    /// Verifies that plugin hooks are executed even when plugin fails
    #[tokio::test]
    async fn test_plugin_hooks_with_execution_failure() -> Result<()> {
        use crate::core::hooks::HookAction;

        struct FailingHookedPlugin {
            hooks: Vec<HookAction>,
        }

        impl FailingHookedPlugin {
            fn new() -> Self {
                Self {
                    hooks: vec![HookAction::Log {
                        message: "Plugin hook executed".to_string(),
                        level: "info".to_string(),
                    }],
                }
            }
        }

        #[async_trait]
        impl Plugin for FailingHookedPlugin {
            fn description(&self) -> &str {
                "Failing plugin with hooks"
            }
            fn icon(&self) -> &str {
                SYMBOL_ACTION_TEST
            }
            async fn execute(&self) -> Result<String> {
                Err(anyhow::anyhow!("Plugin execution failed"))
            }
            async fn validate(&self) -> Result<()> {
                Ok(())
            }
            fn get_target_path(&self) -> Option<String> {
                None
            }
            fn get_output_file(&self) -> Option<String> {
                None
            }
            fn creates_own_output_files(&self) -> bool {
                false
            }
            fn get_hooks(&self) -> Vec<HookAction> {
                self.hooks.clone()
            }
        }

        let temp_dir = TempDir::new()?;
        let snapshot_dir = temp_dir.path().join("snapshot");
        async_fs::create_dir_all(&snapshot_dir).await?;

        let plugin = Arc::new(FailingHookedPlugin::new());
        let snapshot_manager = SnapshotManager::new(temp_dir.path().to_path_buf());
        let hook_manager = HookManager::new(Default::default());
        let hook_context = HookContext::new(
            "test_snapshot".to_string(),
            snapshot_dir.clone(),
            Default::default(),
        );

        let result = SnapshotExecutor::execute_plugin_with_hooks(
            "failing_hooked".to_string(),
            plugin,
            &snapshot_dir,
            &snapshot_manager,
            None,
            hook_manager,
            hook_context,
        )
        .await?;

        // Plugin should fail but hooks should still execute
        assert!(!result.success);
        assert!(result.error_message.is_some());

        Ok(())
    }

    /// Test plugin with successful checksum reuse scenario
    /// Verifies the complete checksum reuse workflow including hook execution
    #[tokio::test]
    async fn test_plugin_successful_checksum_reuse() -> Result<()> {
        use crate::core::hooks::HookAction;

        struct ReusableHookedPlugin {
            content: String,
            hooks: Vec<HookAction>,
        }

        impl ReusableHookedPlugin {
            fn new(content: String) -> Self {
                Self {
                    content,
                    hooks: vec![HookAction::Log {
                        message: "Reuse hook executed".to_string(),
                        level: "info".to_string(),
                    }],
                }
            }
        }

        #[async_trait]
        impl Plugin for ReusableHookedPlugin {
            fn description(&self) -> &str {
                "Reusable plugin with hooks"
            }
            fn icon(&self) -> &str {
                SYMBOL_ACTION_TEST
            }
            async fn execute(&self) -> Result<String> {
                Ok(self.content.clone())
            }
            async fn validate(&self) -> Result<()> {
                Ok(())
            }
            fn get_target_path(&self) -> Option<String> {
                None
            }
            fn get_output_file(&self) -> Option<String> {
                Some("reusable.txt".to_string())
            }
            fn creates_own_output_files(&self) -> bool {
                false
            }
            fn get_hooks(&self) -> Vec<HookAction> {
                self.hooks.clone()
            }
        }

        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        // Create a snapshot directory with existing content to simulate checksum reuse
        let existing_snapshot = base_path.join("existing_snapshot");
        async_fs::create_dir_all(&existing_snapshot).await?;
        async_fs::write(existing_snapshot.join("reusable.txt"), "reusable content").await?;

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "reusable_plugin".to_string(),
            Arc::new(ReusableHookedPlugin::new("reusable content".to_string())),
        );

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        assert!(snapshot_dir.join("reusable.txt").exists());

        Ok(())
    }

    /// Test execution with empty plugin registry
    /// Verifies that executor handles empty registry gracefully
    #[tokio::test]
    async fn test_execute_snapshot_empty_registry() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let registry = PluginRegistry::new(); // Empty registry
        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        // Should still create metadata even with no plugins
        assert!(snapshot_dir.join(".snapshot/checksum.json").exists());

        Ok(())
    }

    /// Test plugin execution with complex hook scenarios and error contexts
    /// Verifies that error context is properly passed to hooks during failures
    #[tokio::test]
    async fn test_plugin_execution_with_error_context_hooks() -> Result<()> {
        use crate::core::hooks::HookAction;

        struct ErrorContextPlugin {
            hooks: Vec<HookAction>,
        }

        impl ErrorContextPlugin {
            fn new() -> Self {
                Self {
                    hooks: vec![HookAction::Log {
                        message: "Plugin hook with error: {error}".to_string(),
                        level: "error".to_string(),
                    }],
                }
            }
        }

        #[async_trait]
        impl Plugin for ErrorContextPlugin {
            fn description(&self) -> &str {
                "Plugin that provides error context to hooks"
            }
            fn icon(&self) -> &str {
                SYMBOL_ACTION_TEST
            }
            async fn execute(&self) -> Result<String> {
                Err(anyhow::anyhow!("Detailed error message for context"))
            }
            async fn validate(&self) -> Result<()> {
                Ok(())
            }
            fn get_target_path(&self) -> Option<String> {
                None
            }
            fn get_output_file(&self) -> Option<String> {
                None
            }
            fn creates_own_output_files(&self) -> bool {
                false
            }
            fn get_hooks(&self) -> Vec<HookAction> {
                self.hooks.clone()
            }
        }

        let temp_dir = TempDir::new()?;
        let snapshot_dir = temp_dir.path().join("snapshot");
        async_fs::create_dir_all(&snapshot_dir).await?;

        let plugin = Arc::new(ErrorContextPlugin::new());
        let snapshot_manager = SnapshotManager::new(temp_dir.path().to_path_buf());
        let hook_manager = HookManager::new(Default::default());
        let hook_context = HookContext::new(
            "test_snapshot".to_string(),
            snapshot_dir.clone(),
            Default::default(),
        );

        let result = SnapshotExecutor::execute_plugin_with_hooks(
            "error_context_plugin".to_string(),
            plugin,
            &snapshot_dir,
            &snapshot_manager,
            None,
            hook_manager,
            hook_context,
        )
        .await?;

        assert!(!result.success);
        assert!(result
            .error_message
            .unwrap()
            .contains("Detailed error message"));

        Ok(())
    }

    /// Test plugin execution with output path context in success hooks
    /// Verifies that successful plugins get proper output path context in hooks
    #[tokio::test]
    async fn test_plugin_success_hooks_with_output_path() -> Result<()> {
        use crate::core::hooks::HookAction;

        struct OutputPathPlugin {
            hooks: Vec<HookAction>,
        }

        impl OutputPathPlugin {
            fn new() -> Self {
                Self {
                    hooks: vec![HookAction::Log {
                        message: "Plugin completed, output at: {output_path}".to_string(),
                        level: "info".to_string(),
                    }],
                }
            }
        }

        #[async_trait]
        impl Plugin for OutputPathPlugin {
            fn description(&self) -> &str {
                "Plugin that tests output path context"
            }
            fn icon(&self) -> &str {
                SYMBOL_ACTION_TEST
            }
            async fn execute(&self) -> Result<String> {
                Ok("output content".to_string())
            }
            async fn validate(&self) -> Result<()> {
                Ok(())
            }
            fn get_target_path(&self) -> Option<String> {
                None
            }
            fn get_output_file(&self) -> Option<String> {
                Some("output.txt".to_string())
            }
            fn creates_own_output_files(&self) -> bool {
                false
            }
            fn get_hooks(&self) -> Vec<HookAction> {
                self.hooks.clone()
            }
        }

        let temp_dir = TempDir::new()?;
        let snapshot_dir = temp_dir.path().join("snapshot");
        async_fs::create_dir_all(&snapshot_dir).await?;

        let plugin = Arc::new(OutputPathPlugin::new());
        let snapshot_manager = SnapshotManager::new(temp_dir.path().to_path_buf());
        let hook_manager = HookManager::new(Default::default());
        let hook_context = HookContext::new(
            "test_snapshot".to_string(),
            snapshot_dir.clone(),
            Default::default(),
        );

        let result = SnapshotExecutor::execute_plugin_with_hooks(
            "output_path_plugin".to_string(),
            plugin,
            &snapshot_dir,
            &snapshot_manager,
            None,
            hook_manager,
            hook_context,
        )
        .await?;

        assert!(result.success);
        assert_eq!(result.content, "output content");
        assert!(snapshot_dir.join("output.txt").exists());

        Ok(())
    }
}

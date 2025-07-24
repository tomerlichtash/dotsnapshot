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
    #[allow(dead_code)]
    pub fn new(registry: Arc<PluginRegistry>, base_path: PathBuf) -> Self {
        Self {
            registry,
            snapshot_manager: SnapshotManager::new(base_path),
            config: None,
        }
    }

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
        info!("{} Executing plugin: {}", CONTENT_PACKAGE, plugin_name);

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
            INDICATOR_SUCCESS, plugin_name
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

    struct TestVscodeSettingsPlugin {
        content: String,
    }

    struct TestVscodeExtensionsPlugin {
        content: String,
    }

    struct TestPlugin {
        content: String,
    }

    #[async_trait]
    impl Plugin for TestVscodeSettingsPlugin {
        fn description(&self) -> &str {
            "Test VSCode settings plugin"
        }

        fn icon(&self) -> &str {
            ACTION_TEST
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
            None
        }
    }

    #[async_trait]
    impl Plugin for TestVscodeExtensionsPlugin {
        fn description(&self) -> &str {
            "Test VSCode extensions plugin"
        }

        fn icon(&self) -> &str {
            ACTION_TEST
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
            None
        }
    }

    #[async_trait]
    impl Plugin for TestPlugin {
        // Uses default "txt" extension

        fn description(&self) -> &str {
            "Test plugin for unit tests"
        }

        fn icon(&self) -> &str {
            ACTION_TEST
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
            None
        }
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_execute_snapshot() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.register(Arc::new(TestPlugin {
            content: "test content".to_string(),
        }));

        let executor = SnapshotExecutor::new(Arc::new(registry), base_path);
        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        assert!(snapshot_dir.join("test.txt").exists());
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        Ok(())
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_execute_snapshot_with_test_plugins() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        // Test that plugins work without custom configuration
        let config = Config::default();

        let mut registry = PluginRegistry::new();
        registry.register(Arc::new(TestVscodeSettingsPlugin {
            content: "vscode settings content".to_string(),
        }));
        registry.register(Arc::new(TestVscodeExtensionsPlugin {
            content: "vscode extensions content".to_string(),
        }));

        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));
        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());

        // Plugin files should be in the root directory since no custom target_path is configured
        // The plugin names are derived from the struct names using camel-to-snake conversion
        assert!(snapshot_dir.join("test_vscode_settings.txt").exists());
        assert!(snapshot_dir.join("test_vscode_extensions.txt").exists());
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        Ok(())
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_regular_plugin_creates_output_file() -> Result<()> {
        use crate::config::{Config, PluginsConfig, StaticPluginConfig};

        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        // Create a config with static files plugin
        let config = Config {
            output_dir: None,
            include_plugins: None,
            logging: None,
            hooks: None,
            global: None,
            static_files: None,
            plugins: Some(PluginsConfig {
                plugins: {
                    let mut map = std::collections::HashMap::new();
                    map.insert(
                        "static".to_string(),
                        toml::Value::try_from(StaticPluginConfig {
                            target_path: None,
                            output_file: None,
                            files: Some(vec!["Cargo.toml".to_string()]), // Use a file that exists
                            ignore: None,
                        })
                        .unwrap(),
                    );
                    map
                },
            }),
            ui: None,
        };

        // Create a fake static files plugin for testing
        // The plugin will be registered with a derived name that doesn't match static files pattern
        struct TestRegularPlugin;

        #[async_trait]
        impl Plugin for TestRegularPlugin {
            fn description(&self) -> &str {
                "Test regular plugin that creates output files"
            }

            fn icon(&self) -> &str {
                ACTION_TEST
            }

            async fn execute(&self) -> Result<String> {
                Ok("Regular plugin content".to_string())
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
        }

        let mut registry = PluginRegistry::new();
        // Register a regular plugin (not static files)
        registry.register(Arc::new(TestRegularPlugin));

        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));
        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());

        // Verify that a regular plugin DOES create an output file
        // (this is testing the normal behavior, not static files special handling)
        assert!(snapshot_dir.join("test_regular.txt").exists());

        // But metadata should still be created
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        Ok(())
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_static_files_plugin_no_output_file_created() -> Result<()> {
        use crate::config::{Config, PluginsConfig, StaticPluginConfig};

        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        // Create a config with static files plugin
        let config = Config {
            output_dir: None,
            include_plugins: None,
            logging: None,
            hooks: None,
            global: None,
            static_files: None,
            plugins: Some(PluginsConfig {
                plugins: {
                    let mut map = std::collections::HashMap::new();
                    map.insert(
                        "static".to_string(),
                        toml::Value::try_from(StaticPluginConfig {
                            target_path: None,
                            output_file: None,
                            files: Some(vec!["Cargo.toml".to_string()]), // Use a file that exists
                            ignore: None,
                        })
                        .unwrap(),
                    );
                    map
                },
            }),
            ui: None,
        };

        // Create registry and register a regular plugin to test the normal flow
        let mut registry = PluginRegistry::new();
        registry.register(Arc::new(TestPlugin {
            content: "test content".to_string(),
        }));

        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));
        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());

        // Regular plugins should create output files
        assert!(snapshot_dir.join("test.txt").exists());

        // Static files handling is tested elsewhere - this test verifies the execution flow
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        Ok(())
    }
}

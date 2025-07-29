//! Tests for individual plugin execution within the executor

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::core::executor::SnapshotExecutor;
    use crate::core::plugin::{Plugin, PluginRegistry};
    use anyhow::Result;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::fs as async_fs;

    use crate::core::executor::tests::TestPlugin;

    /// Test plugin execution with validation failure in plugin context
    /// Verifies that validation errors are properly caught and handled
    #[tokio::test]
    async fn test_execute_plugin_validation_failure() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "test_plugin".to_string(),
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
        assert!(!snapshot_dir.join("test_plugin.txt").exists());
        // But metadata should still be created
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        Ok(())
    }

    /// Test plugin execution with execution failure
    /// Verifies that plugin execution failures are handled gracefully
    #[tokio::test]
    async fn test_execute_plugin_execution_failure() -> Result<()> {
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

    /// Test plugin execution with custom target path
    /// Verifies that plugins can specify custom target paths for their output
    #[tokio::test]
    async fn test_execute_plugin_with_custom_target_path() -> Result<()> {
        struct CustomPathPlugin;

        #[async_trait]
        impl Plugin for CustomPathPlugin {
            fn description(&self) -> &str {
                "Custom path plugin"
            }

            fn icon(&self) -> &str {
                "📁"
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
        }

        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.add_plugin("custom_plugin".to_string(), Arc::new(CustomPathPlugin));

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        // Check that the file was created in the custom path
        let custom_file_path = snapshot_dir.join("custom").join("path").join("custom.txt");
        assert!(custom_file_path.exists());

        let content = async_fs::read_to_string(custom_file_path).await?;
        assert_eq!(content, "custom content");

        Ok(())
    }

    /// Test plugin that creates its own output files
    /// Verifies that plugins with creates_own_output_files() work correctly
    #[tokio::test]
    async fn test_execute_plugin_creates_own_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "custom_file_plugin".to_string(),
            Arc::new(TestPlugin::new("file content".to_string()).with_custom_file_handling()),
        );

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        // No default file should be created for plugins that handle their own files
        assert!(!snapshot_dir.join("custom_file_plugin.txt").exists());
        // But metadata should still be created
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        Ok(())
    }

    /// Test plugin environment variable setup
    /// Verifies that the snapshot directory is made available to plugins via environment
    #[tokio::test]
    async fn test_plugin_environment_variable() -> Result<()> {
        struct EnvTestPlugin;

        #[async_trait]
        impl Plugin for EnvTestPlugin {
            fn description(&self) -> &str {
                "Environment test plugin"
            }

            fn icon(&self) -> &str {
                "🌍"
            }

            async fn execute(&self) -> Result<String> {
                let snapshot_dir = std::env::var("DOTSNAPSHOT_SNAPSHOT_DIR")
                    .map_err(|_| anyhow::anyhow!("Environment variable not set"))?;
                Ok(format!("Snapshot dir: {snapshot_dir}"))
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

        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.add_plugin("env_plugin".to_string(), Arc::new(EnvTestPlugin));

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        assert!(snapshot_dir.join("env_plugin.txt").exists());

        let content = async_fs::read_to_string(snapshot_dir.join("env_plugin.txt")).await?;
        assert!(content.starts_with("Snapshot dir:"));
        assert!(content.contains(&snapshot_dir.to_string_lossy().to_string()));

        Ok(())
    }

    /// Test plugin file write failure handling
    /// Verifies that file system errors during plugin output writing are handled
    #[tokio::test]
    async fn test_plugin_file_write_failure() -> Result<()> {
        struct ReadOnlyPlugin;

        #[async_trait]
        impl Plugin for ReadOnlyPlugin {
            fn description(&self) -> &str {
                "Read-only test plugin"
            }

            fn icon(&self) -> &str {
                "🔒"
            }

            async fn execute(&self) -> Result<String> {
                Ok("content".to_string())
            }

            async fn validate(&self) -> Result<()> {
                Ok(())
            }

            fn get_target_path(&self) -> Option<String> {
                // Try to write to a path that should fail (system directory)
                Some("/root/readonly".to_string())
            }

            fn get_output_file(&self) -> Option<String> {
                Some("readonly.txt".to_string())
            }
        }

        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.add_plugin("readonly_plugin".to_string(), Arc::new(ReadOnlyPlugin));

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        // This should complete without panicking, even if the plugin file write fails
        let result = executor.execute_snapshot().await;

        // The snapshot should still be created, but the plugin might fail
        // The important thing is that the executor doesn't crash
        match result {
            Ok(snapshot_dir) => {
                assert!(snapshot_dir.exists());
                // Check that metadata was still created
                assert!(snapshot_dir
                    .join(".snapshot")
                    .join("checksum.json")
                    .exists());
            }
            Err(_) => {
                // It's also acceptable for the entire operation to fail gracefully
                // due to permission issues
            }
        }

        Ok(())
    }

    /// Test plugin task panic handling
    /// Verifies that panics in plugin tasks don't crash the executor
    #[tokio::test]
    async fn test_plugin_task_panic_handling() -> Result<()> {
        struct PanicPlugin;

        #[async_trait]
        impl Plugin for PanicPlugin {
            fn description(&self) -> &str {
                "Panic test plugin"
            }

            fn icon(&self) -> &str {
                "💥"
            }

            async fn execute(&self) -> Result<String> {
                panic!("Test panic in plugin execution");
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

        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.add_plugin("panic_plugin".to_string(), Arc::new(PanicPlugin));
        // Add a normal plugin to ensure other plugins still work
        registry.add_plugin(
            "normal_plugin".to_string(),
            Arc::new(TestPlugin::new("normal content".to_string())),
        );

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        // The normal plugin should still work
        assert!(snapshot_dir.join("normal_plugin.txt").exists());
        // The panic plugin file should not be created
        assert!(!snapshot_dir.join("panic_plugin.txt").exists());
        // Metadata should still be created
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        Ok(())
    }
}

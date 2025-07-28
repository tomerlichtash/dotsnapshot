//! Tests for snapshot execution and management

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::core::executor::SnapshotExecutor;
    use crate::core::plugin::PluginRegistry;
    use crate::core::snapshot::SnapshotManager;
    use anyhow::Result;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::fs as async_fs;

    use crate::core::executor::tests::TestPlugin;

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
    /// Verifies that the executor gracefully handles execution failures
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

    /// Test snapshot execution with multiple plugins
    /// Verifies that the executor can handle multiple plugins concurrently
    /// and that all plugin outputs are properly saved and checksummed
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

        // Check that all plugin files were created
        assert!(snapshot_dir.join("plugin1.txt").exists());
        assert!(snapshot_dir.join("plugin2.txt").exists());
        assert!(snapshot_dir.join("plugin3.txt").exists());

        // Check content of each file
        let content1 = async_fs::read_to_string(snapshot_dir.join("plugin1.txt")).await?;
        let content2 = async_fs::read_to_string(snapshot_dir.join("plugin2.txt")).await?;
        let content3 = async_fs::read_to_string(snapshot_dir.join("plugin3.txt")).await?;

        assert_eq!(content1, "content1");
        assert_eq!(content2, "content2");
        assert_eq!(content3, "content3");

        // Check metadata was created
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        Ok(())
    }

    /// Test snapshot execution with custom file handling plugins
    /// Verifies that plugins with creates_own_output_files work correctly
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
        // Plugin should not create a default output file
        assert!(!snapshot_dir.join("custom_plugin.txt").exists());
        // But metadata should still be created
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        Ok(())
    }

    /// Test snapshot execution with mixed success and failure results
    /// Verifies that successful plugins still work when others fail
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
            "validation_fail_plugin".to_string(),
            Arc::new(
                TestPlugin::new("validation content".to_string())
                    .with_validation_error("Validation error".to_string()),
            ),
        );
        registry.add_plugin(
            "execution_fail_plugin".to_string(),
            Arc::new(TestPlugin::new("execution content".to_string()).with_execution_failure()),
        );

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());

        // Successful plugin should create file
        assert!(snapshot_dir.join("success_plugin.txt").exists());
        let content = async_fs::read_to_string(snapshot_dir.join("success_plugin.txt")).await?;
        assert_eq!(content, "success content");

        // Failed plugins should not create files
        assert!(!snapshot_dir.join("validation_fail_plugin.txt").exists());
        assert!(!snapshot_dir.join("execution_fail_plugin.txt").exists());

        // Metadata should still be created
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        Ok(())
    }

    /// Test snapshot execution with empty plugin registry
    /// Verifies that the executor can handle empty plugin registries gracefully
    #[tokio::test]
    async fn test_execute_snapshot_empty_registry() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let registry = PluginRegistry::new();
        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        // No plugin files should be created
        let mut entries = async_fs::read_dir(&snapshot_dir).await?;
        let mut file_count = 0;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                file_count += 1;
            }
        }
        assert_eq!(file_count, 0);

        // But metadata directory should still be created
        assert!(snapshot_dir.join(".snapshot").exists());

        Ok(())
    }

    /// Test snapshot execution without config
    /// Verifies that the executor works without configuration
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
        let executor = SnapshotExecutor::new(Arc::new(registry), base_path);

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        assert!(snapshot_dir.join("simple_plugin.txt").exists());
        assert!(snapshot_dir.join(".snapshot/checksum.json").exists());

        let content = async_fs::read_to_string(snapshot_dir.join("simple_plugin.txt")).await?;
        assert_eq!(content, "simple content");

        Ok(())
    }

    /// Test snapshot manager clone functionality
    /// Verifies that the SnapshotManager can be cloned for use in async tasks
    #[tokio::test]
    async fn test_snapshot_manager_clone() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();

        let snapshot_manager = SnapshotManager::new(base_path.clone());
        let cloned_manager = snapshot_manager.clone();

        // Both managers should have the same base path
        assert_eq!(snapshot_manager.base_path(), cloned_manager.base_path());
        assert_eq!(*cloned_manager.base_path(), base_path);
    }

    /// Test snapshot executor with config
    /// Verifies that the executor can be created with configuration
    #[tokio::test]
    async fn test_snapshot_executor_with_config() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();

        let registry = PluginRegistry::new();
        let config = Config::default();

        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        // Verify executor is properly constructed (no panics or errors)
        assert!(executor.config().is_some());
    }
}

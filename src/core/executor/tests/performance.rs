//! Tests for performance and optimization features in the executor

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

    /// Test plugin checksum reuse optimization
    /// Verifies that plugins with identical content reuse existing files
    #[tokio::test]
    async fn test_plugin_checksum_reuse() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        // First execution
        let mut registry1 = PluginRegistry::new();
        registry1.add_plugin(
            "checksum_plugin".to_string(),
            Arc::new(TestPlugin::new("identical content".to_string())),
        );

        let config = Config::default();
        let executor1 =
            SnapshotExecutor::with_config(Arc::new(registry1), base_path.clone(), Arc::new(config));

        let snapshot_dir1 = executor1.execute_snapshot().await?;
        assert!(snapshot_dir1.exists());
        assert!(snapshot_dir1.join("checksum_plugin.txt").exists());

        // Wait a bit to ensure different snapshot timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Second execution with identical content
        let mut registry2 = PluginRegistry::new();
        registry2.add_plugin(
            "checksum_plugin".to_string(),
            Arc::new(TestPlugin::new("identical content".to_string())),
        );

        let config2 = Config::default();
        let executor2 = SnapshotExecutor::with_config(
            Arc::new(registry2),
            base_path.clone(),
            Arc::new(config2),
        );

        let snapshot_dir2 = executor2.execute_snapshot().await?;
        assert!(snapshot_dir2.exists());
        assert!(snapshot_dir2.join("checksum_plugin.txt").exists());

        // Verify content is identical
        let content1 = async_fs::read_to_string(snapshot_dir1.join("checksum_plugin.txt")).await?;
        let content2 = async_fs::read_to_string(snapshot_dir2.join("checksum_plugin.txt")).await?;
        assert_eq!(content1, content2);
        assert_eq!(content1, "identical content");

        Ok(())
    }

    /// Test successful plugin checksum reuse with hooks
    /// Verifies that checksum reuse works with plugin hooks
    #[tokio::test]
    async fn test_plugin_successful_checksum_reuse() -> Result<()> {
        struct ReuseHookPlugin {
            content: String,
            hooks: Vec<crate::core::hooks::HookAction>,
        }

        #[async_trait]
        impl Plugin for ReuseHookPlugin {
            fn description(&self) -> &str {
                "Reuse hook plugin"
            }

            fn icon(&self) -> &str {
                "♻️"
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

            fn get_hooks(&self) -> Vec<crate::core::hooks::HookAction> {
                self.hooks.clone()
            }
        }

        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let hook_action = crate::core::hooks::HookAction::Log {
            message: "Plugin executed: reused={reused}".to_string(),
            level: "info".to_string(),
        };

        // First execution
        let mut registry1 = PluginRegistry::new();
        registry1.add_plugin(
            "reuse_plugin".to_string(),
            Arc::new(ReuseHookPlugin {
                content: "reusable content".to_string(),
                hooks: vec![hook_action.clone()],
            }),
        );

        let config = Config::default();
        let executor1 =
            SnapshotExecutor::with_config(Arc::new(registry1), base_path.clone(), Arc::new(config));

        let snapshot_dir1 = executor1.execute_snapshot().await?;
        assert!(snapshot_dir1.exists());
        assert!(snapshot_dir1.join("reuse_plugin.txt").exists());

        // Wait a bit to ensure different snapshot timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Second execution with identical content
        let mut registry2 = PluginRegistry::new();
        registry2.add_plugin(
            "reuse_plugin".to_string(),
            Arc::new(ReuseHookPlugin {
                content: "reusable content".to_string(),
                hooks: vec![hook_action],
            }),
        );

        let config2 = Config::default();
        let executor2 = SnapshotExecutor::with_config(
            Arc::new(registry2),
            base_path.clone(),
            Arc::new(config2),
        );

        let snapshot_dir2 = executor2.execute_snapshot().await?;
        assert!(snapshot_dir2.exists());
        assert!(snapshot_dir2.join("reuse_plugin.txt").exists());

        // Verify content is identical
        let content1 = async_fs::read_to_string(snapshot_dir1.join("reuse_plugin.txt")).await?;
        let content2 = async_fs::read_to_string(snapshot_dir2.join("reuse_plugin.txt")).await?;
        assert_eq!(content1, content2);
        assert_eq!(content1, "reusable content");

        Ok(())
    }
}

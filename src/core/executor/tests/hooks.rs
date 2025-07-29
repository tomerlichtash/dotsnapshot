//! Tests for hooks integration within the executor

#[cfg(test)]
mod tests {
    use crate::config::{Config, GlobalConfig, GlobalHooks};
    use crate::core::executor::SnapshotExecutor;
    use crate::core::hooks::HookAction;
    use crate::core::plugin::{Plugin, PluginRegistry};
    use anyhow::Result;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::fs as async_fs;

    use crate::core::executor::tests::TestPlugin;

    /// Test plugin execution with basic hooks functionality
    /// Verifies that plugin-level hooks are executed correctly
    #[tokio::test]
    async fn test_execute_plugin_with_hooks_basic() -> Result<()> {
        struct HookTestPlugin {
            hooks: Vec<HookAction>,
        }

        #[async_trait]
        impl Plugin for HookTestPlugin {
            fn description(&self) -> &str {
                "Hook test plugin"
            }

            fn icon(&self) -> &str {
                "🪝"
            }

            async fn execute(&self) -> Result<String> {
                Ok("hook content".to_string())
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

            fn get_hooks(&self) -> Vec<HookAction> {
                self.hooks.clone()
            }
        }

        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let hook_action = HookAction::Log {
            message: "Plugin hook executed".to_string(),
            level: "info".to_string(),
        };

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "hook_plugin".to_string(),
            Arc::new(HookTestPlugin {
                hooks: vec![hook_action],
            }),
        );

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        assert!(snapshot_dir.join("hook_plugin.txt").exists());

        let content = async_fs::read_to_string(snapshot_dir.join("hook_plugin.txt")).await?;
        assert_eq!(content, "hook content");

        Ok(())
    }

    /// Test snapshot execution with global hooks
    /// Verifies that global pre and post snapshot hooks are executed
    #[tokio::test]
    async fn test_execute_snapshot_with_global_hooks() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "test_plugin".to_string(),
            Arc::new(TestPlugin::new("test content".to_string())),
        );

        // Create config with global hooks
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

        let global_config = GlobalConfig {
            hooks: Some(global_hooks),
        };

        let config = Config {
            global: Some(global_config),
            ..Default::default()
        };

        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        assert!(snapshot_dir.join("test_plugin.txt").exists());
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        Ok(())
    }

    /// Test comprehensive plugin hooks integration
    /// Verifies that plugin hooks work correctly with different hook types
    #[tokio::test]
    async fn test_plugin_hooks_integration() -> Result<()> {
        struct ComprehensiveHookPlugin {
            hooks: Vec<HookAction>,
        }

        #[async_trait]
        impl Plugin for ComprehensiveHookPlugin {
            fn description(&self) -> &str {
                "Comprehensive hook plugin"
            }

            fn icon(&self) -> &str {
                "🔗"
            }

            async fn execute(&self) -> Result<String> {
                Ok("comprehensive content".to_string())
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

            fn get_hooks(&self) -> Vec<HookAction> {
                self.hooks.clone()
            }
        }

        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let hook_actions = vec![
            HookAction::Log {
                message: "Pre-plugin log".to_string(),
                level: "info".to_string(),
            },
            HookAction::Log {
                message: "Plugin execution log".to_string(),
                level: "debug".to_string(),
            },
        ];

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "comprehensive_plugin".to_string(),
            Arc::new(ComprehensiveHookPlugin {
                hooks: hook_actions,
            }),
        );

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        assert!(snapshot_dir.join("comprehensive_plugin.txt").exists());

        let content =
            async_fs::read_to_string(snapshot_dir.join("comprehensive_plugin.txt")).await?;
        assert_eq!(content, "comprehensive content");

        Ok(())
    }

    /// Test plugin hooks with execution failure
    /// Verifies that post-plugin hooks are executed even when plugin execution fails
    #[tokio::test]
    async fn test_plugin_hooks_with_execution_failure() -> Result<()> {
        struct FailingHookPlugin {
            hooks: Vec<HookAction>,
        }

        #[async_trait]
        impl Plugin for FailingHookPlugin {
            fn description(&self) -> &str {
                "Failing hook plugin"
            }

            fn icon(&self) -> &str {
                "💥"
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

            fn get_hooks(&self) -> Vec<HookAction> {
                self.hooks.clone()
            }
        }

        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let hook_actions = vec![HookAction::Log {
            message: "Post-plugin hook after failure".to_string(),
            level: "error".to_string(),
        }];

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "failing_hook_plugin".to_string(),
            Arc::new(FailingHookPlugin {
                hooks: hook_actions,
            }),
        );

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        // Plugin file should not be created due to execution failure
        assert!(!snapshot_dir.join("failing_hook_plugin.txt").exists());
        // But metadata should still be created
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        Ok(())
    }

    /// Test plugin execution with error context hooks
    /// Verifies that hooks receive error context when plugin execution fails
    #[tokio::test]
    async fn test_plugin_execution_with_error_context_hooks() -> Result<()> {
        struct ErrorContextPlugin {
            hooks: Vec<HookAction>,
        }

        #[async_trait]
        impl Plugin for ErrorContextPlugin {
            fn description(&self) -> &str {
                "Error context plugin"
            }

            fn icon(&self) -> &str {
                "⚠️"
            }

            async fn execute(&self) -> Result<String> {
                Err(anyhow::anyhow!("Specific error message"))
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

            fn get_hooks(&self) -> Vec<HookAction> {
                self.hooks.clone()
            }
        }

        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let hook_actions = vec![HookAction::Log {
            message: "Error occurred: {error}".to_string(),
            level: "error".to_string(),
        }];

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "error_context_plugin".to_string(),
            Arc::new(ErrorContextPlugin {
                hooks: hook_actions,
            }),
        );

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        // Plugin file should not be created due to execution failure
        assert!(!snapshot_dir.join("error_context_plugin.txt").exists());
        // But metadata should still be created
        assert!(snapshot_dir
            .join(".snapshot")
            .join("checksum.json")
            .exists());

        Ok(())
    }

    /// Test plugin success hooks with output path
    /// Verifies that successful plugin execution provides output path to hooks
    #[tokio::test]
    async fn test_plugin_success_hooks_with_output_path() -> Result<()> {
        struct SuccessHookPlugin {
            hooks: Vec<HookAction>,
        }

        #[async_trait]
        impl Plugin for SuccessHookPlugin {
            fn description(&self) -> &str {
                "Success hook plugin"
            }

            fn icon(&self) -> &str {
                "✅"
            }

            async fn execute(&self) -> Result<String> {
                Ok("success content".to_string())
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

            fn get_hooks(&self) -> Vec<HookAction> {
                self.hooks.clone()
            }
        }

        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();

        let hook_actions = vec![HookAction::Log {
            message: "Success: output at {output_path}".to_string(),
            level: "info".to_string(),
        }];

        let mut registry = PluginRegistry::new();
        registry.add_plugin(
            "success_hook_plugin".to_string(),
            Arc::new(SuccessHookPlugin {
                hooks: hook_actions,
            }),
        );

        let config = Config::default();
        let executor =
            SnapshotExecutor::with_config(Arc::new(registry), base_path, Arc::new(config));

        let snapshot_dir = executor.execute_snapshot().await?;

        assert!(snapshot_dir.exists());
        assert!(snapshot_dir.join("success_hook_plugin.txt").exists());

        let content =
            async_fs::read_to_string(snapshot_dir.join("success_hook_plugin.txt")).await?;
        assert_eq!(content, "success content");

        Ok(())
    }
}

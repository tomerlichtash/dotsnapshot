//! Tests for plugin hook management through CLI commands

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use crate::cli::hooks::{
        ensure_plugin_config, handle_plugin_hook_removal, modify_plugin_config,
    };
    use crate::config::{Config, PluginHooks};
    use crate::core::hooks::HookAction;
    use anyhow::Result;
    use std::collections::HashMap;

    /// Test handle_plugin_hook_removal with various removal scenarios
    /// Verifies that plugin hooks can be removed by index, all, or script name
    #[tokio::test]
    async fn test_handle_plugin_hook_removal_comprehensive() -> Result<()> {
        // Setup config with plugin containing hooks
        let (_temp_dir, config_path) = create_test_environment();

        let mut config = Config::default();
        ensure_plugin_config(&mut config, "test_plugin");

        // Add some hooks to the plugin
        modify_plugin_config(&mut config, "test_plugin", |plugin_config| {
            plugin_config.hooks = Some(PluginHooks {
                pre_plugin: vec![
                    HookAction::Script {
                        command: "script1.sh".to_string(),
                        args: vec![],
                        timeout: 30,
                        working_dir: None,
                        env_vars: HashMap::new(),
                    },
                    HookAction::Script {
                        command: "script2.sh".to_string(),
                        args: vec![],
                        timeout: 30,
                        working_dir: None,
                        env_vars: HashMap::new(),
                    },
                    HookAction::Log {
                        message: "Test log".to_string(),
                        level: "info".to_string(),
                    },
                ],
                post_plugin: vec![HookAction::Notify {
                    message: "Done".to_string(),
                    title: None,
                }],
            });
        });

        config.save_to_file(&config_path).await?;

        // Test removing hook by index
        handle_plugin_hook_removal(
            &mut config,
            "test_plugin",
            "pre-plugin",
            Some(0),
            false,
            None,
            Some(config_path.clone()),
        )
        .await?;

        // Test removing all hooks
        handle_plugin_hook_removal(
            &mut config,
            "test_plugin",
            "pre-plugin",
            None,
            true,
            None,
            Some(config_path.clone()),
        )
        .await?;

        // Test removing hooks by script name
        let mut config = Config::default();
        ensure_plugin_config(&mut config, "test_plugin2");
        modify_plugin_config(&mut config, "test_plugin2", |plugin_config| {
            plugin_config.hooks = Some(PluginHooks {
                pre_plugin: vec![HookAction::Script {
                    command: "remove_me.sh".to_string(),
                    args: vec![],
                    timeout: 30,
                    working_dir: None,
                    env_vars: HashMap::new(),
                }],
                post_plugin: vec![],
            });
        });

        handle_plugin_hook_removal(
            &mut config,
            "test_plugin2",
            "pre-plugin",
            None,
            false,
            Some("remove_me".to_string()),
            Some(config_path.clone()),
        )
        .await?;

        Ok(())
    }

    /// Test handle_plugin_hook_removal with non-existent plugin
    /// Verifies graceful handling when plugin doesn't exist
    #[tokio::test]
    async fn test_handle_plugin_hook_removal_nonexistent_plugin() {
        let (_temp_dir, config_path) = create_test_environment();

        // Create config without the plugin
        let config = create_empty_config();
        setup_config_file(&config, &config_path).await;

        let mut config = Config::load_from_file(&config_path).await.unwrap();
        let result = handle_plugin_hook_removal(
            &mut config,
            "nonexistent_plugin",
            "pre-plugin",
            Some(0),
            false,
            None,
            Some(config_path),
        )
        .await;
        assert!(result.is_ok()); // Should handle gracefully
    }
}

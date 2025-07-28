//! Tests for listing hooks through CLI commands

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use crate::cli::hooks::{ensure_plugin_config, handle_list_hooks, modify_plugin_config};
    use crate::config::PluginHooks;
    use crate::core::hooks::HookAction;

    /// Test handle_list_hooks function with global hooks
    /// Verifies that listing hooks works correctly
    #[tokio::test]
    async fn test_handle_list_hooks_global() {
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_config_with_pre_snapshot_hooks();
        setup_config_file(&config, &config_path).await;

        let result = handle_list_hooks(
            None,  // plugin
            false, // pre_plugin
            false, // post_plugin
            true,  // pre_snapshot
            false, // post_snapshot
            false, // verbose
            Some(config_path),
        )
        .await;
        assert!(result.is_ok());
    }

    /// Test handle_list_hooks with all hook types selected
    /// Verifies listing all hook types at once
    #[tokio::test]
    async fn test_handle_list_hooks_all_types() {
        let (_temp_dir, config_path) = create_test_environment();

        // Create config with various hooks
        let mut config = create_config_with_all_hook_types();

        // Add plugin hooks
        ensure_plugin_config(&mut config, "test_plugin");
        modify_plugin_config(&mut config, "test_plugin", |plugin_config| {
            plugin_config.hooks = Some(PluginHooks {
                pre_plugin: vec![HookAction::Log {
                    message: "Pre plugin".to_string(),
                    level: "info".to_string(),
                }],
                post_plugin: vec![HookAction::Log {
                    message: "Post plugin".to_string(),
                    level: "info".to_string(),
                }],
            });
        });

        setup_config_file(&config, &config_path).await;

        let result = handle_list_hooks(
            None, // plugin
            true, // pre_plugin
            true, // post_plugin
            true, // pre_snapshot
            true, // post_snapshot
            true, // verbose
            Some(config_path),
        )
        .await;
        assert!(result.is_ok());
    }
}

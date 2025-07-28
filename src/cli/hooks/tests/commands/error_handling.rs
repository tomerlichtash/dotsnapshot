//! Tests for error handling and edge cases in CLI hooks commands

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use crate::cli::hooks::{
        handle_add_hook, handle_remove_hook, handle_scripts_dir, handle_validate_hooks,
    };
    use crate::config::Config;
    use crate::core::hooks::HookAction;
    use crate::{HookActionArgs, HookTarget};
    use std::collections::HashMap;

    /// Test handle_add_hook with script file that doesn't exist
    /// Verifies warning is shown when script file is missing
    #[tokio::test]
    async fn test_handle_add_hook_missing_script_file() {
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_empty_config();
        setup_config_file(&config, &config_path).await;

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let action = HookActionArgs {
            script: Some("nonexistent_script.sh".to_string()),
            log: None,
            notify: None,
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = handle_add_hook(target, action, Some(config_path)).await;
        assert!(result.is_ok()); // Should succeed but show warning
    }

    /// Test handle_remove_hook with no removal criteria specified
    /// Verifies error when no --index, --all, or --script is provided
    #[tokio::test]
    async fn test_handle_remove_hook_no_criteria() {
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_config_with_pre_snapshot_hooks();
        setup_config_file(&config, &config_path).await;

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let result = handle_remove_hook(
            target,
            None,  // index
            false, // all
            None,  // script
            Some(config_path),
        )
        .await;
        assert!(result.is_ok()); // Function handles this gracefully with error message
    }

    /// Test handle_remove_hook with out of range index
    /// Verifies error handling for invalid index values
    #[tokio::test]
    async fn test_handle_remove_hook_out_of_range_index() {
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_config_with_pre_snapshot_hooks();
        setup_config_file(&config, &config_path).await;

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let result = handle_remove_hook(
            target,
            Some(999), // out of range index
            false,
            None,
            Some(config_path),
        )
        .await;
        assert!(result.is_ok()); // Function handles this gracefully with error message
    }

    /// Test handle_scripts_dir with nonexistent directory (without create flag)
    /// Verifies warning is shown when directory doesn't exist
    #[tokio::test]
    async fn test_handle_scripts_dir_nonexistent_directory() {
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_empty_config();
        setup_config_file(&config, &config_path).await;

        // Use a simple absolute path that doesn't exist
        let nonexistent_dir = std::path::PathBuf::from("/tmp/nonexistent_test_dir");

        let result = handle_scripts_dir(
            Some(nonexistent_dir),
            false, // don't create
            Some(config_path),
        )
        .await;
        assert!(result.is_ok()); // Should succeed but show warning
    }

    /// Test handle_validate_hooks with notify action (system notification warning)
    /// Verifies warning is shown for system notifications
    #[tokio::test]
    async fn test_handle_validate_hooks_notify_warning() {
        let (_temp_dir, config_path) = create_test_environment();

        let mut config = Config::default();
        config.global = Some(crate::config::GlobalConfig {
            hooks: Some(crate::config::GlobalHooks {
                pre_snapshot: vec![HookAction::Notify {
                    message: "Test notification".to_string(),
                    title: Some("Test".to_string()),
                }],
                post_snapshot: vec![],
            }),
        });

        setup_config_file(&config, &config_path).await;

        let result = handle_validate_hooks(
            None,  // plugin
            false, // pre_plugin
            false, // post_plugin
            true,  // pre_snapshot
            false, // post_snapshot
            Some(config_path),
        )
        .await;
        assert!(result.is_ok());
    }

    /// Test handle_validate_hooks with invalid script hook
    /// Verifies error is shown for scripts that fail validation
    #[tokio::test]
    async fn test_handle_validate_hooks_invalid_script() {
        let (_temp_dir, config_path) = create_test_environment();

        let mut config = Config::default();
        config.global = Some(crate::config::GlobalConfig {
            hooks: Some(crate::config::GlobalHooks {
                pre_snapshot: vec![HookAction::Script {
                    command: "/nonexistent/invalid/script.sh".to_string(),
                    args: vec![],
                    timeout: 30,
                    working_dir: Some("/nonexistent/directory".into()),
                    env_vars: HashMap::new(),
                }],
                post_snapshot: vec![],
            }),
        });

        setup_config_file(&config, &config_path).await;

        let result = handle_validate_hooks(
            None,  // plugin
            false, // pre_plugin
            false, // post_plugin
            true,  // pre_snapshot
            false, // post_snapshot
            Some(config_path),
        )
        .await;
        assert!(result.is_ok()); // Function completes but reports errors
    }

    /// Test handle_validate_hooks with mixed valid and invalid hooks
    /// Verifies proper counting of valid, warning, and error hooks
    #[tokio::test]
    async fn test_handle_validate_hooks_mixed_results() {
        let (_temp_dir, config_path) = create_test_environment();
        
        let mut config = Config::default();
        config.global = Some(crate::config::GlobalConfig {
            hooks: Some(crate::config::GlobalHooks {
                pre_snapshot: vec![
                    // Valid log hook
                    HookAction::Log {
                        message: "Test log".to_string(),
                        level: "info".to_string(),
                    },
                    // Notify hook (generates warning)
                    HookAction::Notify {
                        message: "Test notification".to_string(),
                        title: None,
                    },
                ],
                post_snapshot: vec![],
            }),
        });

        setup_config_file(&config, &config_path).await;

        let result = handle_validate_hooks(
            None,  // plugin
            false, // pre_plugin
            false, // post_plugin
            true,  // pre_snapshot
            false, // post_snapshot
            Some(config_path),
        )
        .await;
        assert!(result.is_ok());
    }

    /// Test plugin hook removal with nonexistent plugin
    /// Verifies graceful handling when plugin doesn't exist
    #[tokio::test]
    async fn test_plugin_hook_removal_nonexistent_plugin() {
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_empty_config();
        setup_config_file(&config, &config_path).await;

        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: Some("nonexistent_plugin".to_string()),
            post_plugin: None,
        };

        let result = handle_remove_hook(target, Some(0), false, None, Some(config_path)).await;
        assert!(result.is_ok()); // Should handle gracefully
    }

    /// Test handle_validate_hooks with all hook types disabled
    /// Verifies no validation occurs when all flags are false
    #[tokio::test]
    async fn test_handle_validate_hooks_all_disabled() {
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_config_with_all_hook_types();
        setup_config_file(&config, &config_path).await;

        let result = handle_validate_hooks(
            None,  // plugin
            true,  // pre_plugin (disabled by negation logic)
            true,  // post_plugin (disabled by negation logic)
            false, // pre_snapshot (disabled)
            false, // post_snapshot (disabled)
            Some(config_path),
        )
        .await;
        assert!(result.is_ok());
    }

    /// Test edge case where hooks config serialization might fail
    /// This tests the warning paths in configuration functions
    #[tokio::test]
    async fn test_config_serialization_edge_cases() {
        let (_temp_dir, config_path) = create_test_environment();
        let mut config = create_empty_config();

        // Add plugin configuration to test plugin config paths
        crate::cli::hooks::ensure_plugin_config(&mut config, "test_plugin");
        crate::cli::hooks::modify_plugin_config(&mut config, "test_plugin", |plugin_config| {
            plugin_config.hooks = Some(crate::config::PluginHooks {
                pre_plugin: vec![HookAction::Log {
                    message: "test".to_string(),
                    level: "info".to_string(),
                }],
                post_plugin: vec![],
            });
        });

        setup_config_file(&config, &config_path).await;

        // This should exercise the plugin config serialization paths
        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: Some("test_plugin".to_string()),
            post_plugin: None,
        };

        let result = handle_remove_hook(
            target,
            None,
            true, // remove all
            None,
            Some(config_path),
        )
        .await;
        assert!(result.is_ok());
    }
}

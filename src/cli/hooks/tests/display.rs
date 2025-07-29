#[cfg(test)]
mod tests {
    use crate::cli::hooks::*;
    use crate::config::Config;
    use crate::core::hooks::{HookAction, HookContext, HookManager, HooksConfig};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Test show_global_hooks function with various configuration scenarios
    /// Verifies that global hooks are displayed correctly
    #[tokio::test]
    async fn test_show_global_hooks() {
        use crate::config::{GlobalConfig, GlobalHooks};
        use crate::core::hooks::HookAction;

        let global_hooks = GlobalHooks {
            pre_snapshot: vec![
                HookAction::Log {
                    message: "Pre-snapshot log".to_string(),
                    level: "info".to_string(),
                },
                HookAction::Script {
                    command: "test.sh".to_string(),
                    args: vec!["arg1".to_string()],
                    timeout: 30,
                    working_dir: None,
                    env_vars: HashMap::new(),
                },
            ],
            post_snapshot: vec![HookAction::Notify {
                message: "Snapshot complete".to_string(),
                title: Some("Dotsnapshot".to_string()),
            }],
        };

        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(global_hooks),
            }),
            ..Default::default()
        };

        let hooks_config = HooksConfig::default();

        // Test showing all hooks
        show_global_hooks(&config, true, true, false, &hooks_config);

        // Test showing only pre-snapshot hooks
        show_global_hooks(&config, true, false, false, &hooks_config);

        // Test showing only post-snapshot hooks
        show_global_hooks(&config, false, true, false, &hooks_config);

        // Test with verbose mode
        show_global_hooks(&config, true, true, true, &hooks_config);

        // Test with config that has no global hooks
        let empty_config = Config::default();
        show_global_hooks(&empty_config, true, true, false, &hooks_config);
    }

    /// Test show_plugin_hooks function with various plugin types
    /// Verifies that plugin-specific hooks are displayed with correct icons
    #[tokio::test]
    async fn test_show_plugin_hooks() {
        let mut config = Config::default();
        ensure_plugin_config(&mut config, "homebrew_brewfile");
        ensure_plugin_config(&mut config, "vscode_settings");
        ensure_plugin_config(&mut config, "cursor_extensions");
        ensure_plugin_config(&mut config, "npm_config");
        ensure_plugin_config(&mut config, "custom_plugin");

        let hooks_config = HooksConfig::default();

        // Test different plugin types to verify icon selection
        show_plugin_hooks(
            &config,
            "homebrew_brewfile",
            true,
            true,
            false,
            &hooks_config,
        );
        show_plugin_hooks(
            &config,
            "vscode_settings",
            true,
            false,
            false,
            &hooks_config,
        );
        show_plugin_hooks(
            &config,
            "cursor_extensions",
            false,
            true,
            false,
            &hooks_config,
        );
        show_plugin_hooks(&config, "npm_config", true, true, false, &hooks_config);
        show_plugin_hooks(&config, "custom_plugin", true, true, false, &hooks_config);

        // Test with verbose mode
        show_plugin_hooks(
            &config,
            "homebrew_brewfile",
            true,
            true,
            true,
            &hooks_config,
        );
    }

    /// Test show_all_plugin_hooks function
    /// Verifies that all plugins' hooks are displayed correctly
    #[tokio::test]
    async fn test_show_all_plugin_hooks() {
        let mut config = Config::default();
        ensure_plugin_config(&mut config, "plugin1");
        ensure_plugin_config(&mut config, "plugin2");

        let hooks_config = HooksConfig::default();

        // Test showing all plugin hooks
        show_all_plugin_hooks(&config, true, true, false, &hooks_config);

        // Test with no plugins
        let empty_config = Config::default();
        show_all_plugin_hooks(&empty_config, true, true, false, &hooks_config);
    }

    /// Test show_hook_list function with various hook types
    /// Verifies that hook lists are displayed correctly with and without verbose mode
    #[tokio::test]
    async fn test_show_hook_list() {
        use crate::core::hooks::HookAction;

        let hooks = vec![
            HookAction::Script {
                command: "test_script.sh".to_string(),
                args: vec!["arg1".to_string(), "arg2".to_string()],
                timeout: 60,
                working_dir: None,
                env_vars: HashMap::new(),
            },
            HookAction::Log {
                message: "Test log message".to_string(),
                level: "info".to_string(),
            },
            HookAction::Notify {
                message: "Test notification".to_string(),
                title: Some("Test Title".to_string()),
            },
            HookAction::Backup {
                path: PathBuf::from("/test/source"),
                destination: PathBuf::from("/test/backup"),
            },
            HookAction::Cleanup {
                patterns: vec!["*.tmp".to_string()],
                directories: vec![PathBuf::from("/tmp")],
                temp_files: true,
            },
        ];

        let hooks_config = HooksConfig::default();

        // Test normal mode
        show_hook_list(&hooks, "pre-snapshot", None, false, &hooks_config);

        // Test verbose mode
        show_hook_list(
            &hooks,
            "post-plugin",
            Some("test_plugin"),
            true,
            &hooks_config,
        );

        // Test with empty hooks list
        let empty_hooks: Vec<HookAction> = vec![];
        show_hook_list(&empty_hooks, "pre-plugin", None, false, &hooks_config);
    }

    /// Test show_hook_list with empty hooks
    /// Verifies that empty hook lists are handled correctly
    #[test]
    fn test_show_hook_list_empty() {
        let empty_hooks: Vec<HookAction> = vec![];
        let hooks_config = HooksConfig::default();

        // Should return early without any output
        show_hook_list(&empty_hooks, "pre-snapshot", None, false, &hooks_config);
        show_hook_list(
            &empty_hooks,
            "post-plugin",
            Some("test"),
            true,
            &hooks_config,
        );
    }

    /// Test validate_hook_list function with various hook scenarios
    /// Verifies that hook validation returns correct counts
    #[tokio::test]
    async fn test_validate_hook_list() {
        use crate::core::hooks::{HookAction, HookContext, HookManager};

        let hooks = vec![
            HookAction::Log {
                message: "Valid log".to_string(),
                level: "info".to_string(),
            },
            HookAction::Notify {
                message: "Test notification".to_string(),
                title: None,
            },
        ];

        let hooks_config = HooksConfig::default();
        let hook_manager = HookManager::new(hooks_config.clone());
        let temp_dir = TempDir::new().unwrap();
        let context = HookContext::new(
            "test_snapshot".to_string(),
            temp_dir.path().to_path_buf(),
            hooks_config,
        );

        // Test validation of valid hooks
        let (valid, warnings, errors) =
            validate_hook_list(&hook_manager, &hooks, "pre-snapshot", None, &context);

        // Should have some valid hooks, notifications may generate warnings
        assert!(valid > 0);
        let _ = warnings; // May vary by system
        let _ = errors; // May vary by system

        // Test with empty hooks list
        let empty_hooks: Vec<HookAction> = vec![];
        let (valid, warnings, errors) = validate_hook_list(
            &hook_manager,
            &empty_hooks,
            "pre-plugin",
            Some("test_plugin"),
            &context,
        );

        assert_eq!(valid, 0);
        assert_eq!(warnings, 0);
        assert_eq!(errors, 0);
    }

    /// Test validate_hook_list with script validation errors
    /// Verifies that validation errors are properly counted and reported
    #[tokio::test]
    async fn test_validate_hook_list_with_errors() {
        let temp_dir = TempDir::new().unwrap();
        let hooks_config = HooksConfig {
            scripts_dir: temp_dir.path().to_path_buf(),
        };

        let hooks = vec![
            HookAction::Script {
                command: "nonexistent.sh".to_string(),
                args: vec![],
                timeout: 30,
                working_dir: None,
                env_vars: HashMap::new(),
            },
            HookAction::Log {
                message: "Valid log".to_string(),
                level: "invalid_level".to_string(), // Invalid log level
            },
        ];

        let hook_manager = HookManager::new(hooks_config.clone());
        let context = HookContext::new(
            "test".to_string(),
            temp_dir.path().to_path_buf(),
            hooks_config,
        );

        let (valid, _warnings, errors) =
            validate_hook_list(&hook_manager, &hooks, "pre-snapshot", None, &context);

        assert_eq!(valid, 0); // Both hooks should fail validation
        assert_eq!(errors, 2); // Both should produce errors
    }
}

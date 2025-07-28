//! Tests for hook action conversion and target handling functionality

#[cfg(test)]
mod tests {
    use crate::cli::hooks::*;
    use crate::core::hooks::HookAction;
    use crate::{HookActionArgs, HookTarget};
    use std::path::PathBuf;

    /// Test conversion from CLI args to hook actions
    /// Verifies that different hook action types are converted correctly
    #[test]
    fn test_convert_action_args_to_hook_action() {
        // Test script action
        let script_args = HookActionArgs {
            script: Some("test.sh".to_string()),
            args: Some("arg1,arg2".to_string()),
            timeout: Some(30),
            log: None,
            notify: None,
            backup: false,
            cleanup: false,
            level: None,
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(script_args).unwrap();
        match result {
            HookAction::Script {
                command,
                args,
                timeout,
                ..
            } => {
                assert_eq!(command, "test.sh");
                assert_eq!(args, vec!["arg1", "arg2"]);
                assert_eq!(timeout, 30);
            }
            _ => panic!("Expected script action"),
        }

        // Test log action
        let log_args = HookActionArgs {
            script: None,
            log: Some("Test message".to_string()),
            level: Some("info".to_string()),
            notify: None,
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(log_args).unwrap();
        match result {
            HookAction::Log { message, level } => {
                assert_eq!(message, "Test message");
                assert_eq!(level, "info");
            }
            _ => panic!("Expected log action"),
        }

        // Test notify action
        let notify_args = HookActionArgs {
            script: None,
            log: None,
            notify: Some("Notification message".to_string()),
            title: Some("Test Title".to_string()),
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(notify_args).unwrap();
        match result {
            HookAction::Notify { message, title } => {
                assert_eq!(message, "Notification message");
                assert_eq!(title, Some("Test Title".to_string()));
            }
            _ => panic!("Expected notify action"),
        }

        // Test backup action
        let backup_args = HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: true,
            path: Some(PathBuf::from("/source")),
            destination: Some(PathBuf::from("/backup")),
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(backup_args).unwrap();
        match result {
            HookAction::Backup { path, destination } => {
                assert_eq!(path, PathBuf::from("/source"));
                assert_eq!(destination, PathBuf::from("/backup"));
            }
            _ => panic!("Expected backup action"),
        }

        // Test cleanup action
        let cleanup_args = HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: false,
            cleanup: true,
            patterns: Some("*.tmp,*.log".to_string()),
            directories: Some("/tmp,/var/tmp".to_string()),
            temp_files: true,
            args: None,
            timeout: None,
            level: None,
            title: None,
            path: None,
            destination: None,
        };

        let result = convert_action_args_to_hook_action(cleanup_args).unwrap();
        match result {
            HookAction::Cleanup {
                patterns,
                directories,
                temp_files,
            } => {
                assert_eq!(patterns, vec!["*.tmp", "*.log"]);
                assert_eq!(
                    directories,
                    vec![PathBuf::from("/tmp"), PathBuf::from("/var/tmp")]
                );
                assert!(temp_files);
            }
            _ => panic!("Expected cleanup action"),
        }
    }

    /// Test error cases for action conversion
    /// Verifies that invalid hook action arguments produce appropriate errors
    #[test]
    fn test_convert_action_args_to_hook_action_errors() {
        // Test missing action (no action specified)
        let empty_args = HookActionArgs {
            script: None,
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

        let result = convert_action_args_to_hook_action(empty_args);
        assert!(result.is_err());

        // Test backup action without required paths
        let incomplete_backup = HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: true,
            path: None, // Missing path
            destination: Some(PathBuf::from("/backup")),
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(incomplete_backup);
        assert!(result.is_err());
    }

    /// Test determination of hook targets from CLI arguments
    /// Verifies that hook targets are parsed correctly
    #[test]
    fn test_determine_hook_target() {
        // Test pre-snapshot target
        let pre_snapshot_target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let (hook_type, plugin_name) = determine_hook_target(&pre_snapshot_target).unwrap();
        assert_eq!(hook_type, "pre-snapshot");
        assert_eq!(plugin_name, None);

        // Test post-snapshot target
        let post_snapshot_target = HookTarget {
            pre_snapshot: false,
            post_snapshot: true,
            pre_plugin: None,
            post_plugin: None,
        };

        let (hook_type, plugin_name) = determine_hook_target(&post_snapshot_target).unwrap();
        assert_eq!(hook_type, "post-snapshot");
        assert_eq!(plugin_name, None);

        // Test pre-plugin target
        let pre_plugin_target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: Some("vscode_settings".to_string()),
            post_plugin: None,
        };

        let (hook_type, plugin_name) = determine_hook_target(&pre_plugin_target).unwrap();
        assert_eq!(hook_type, "pre-plugin");
        assert_eq!(plugin_name, Some("vscode_settings".to_string()));

        // Test post-plugin target
        let post_plugin_target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: Some("homebrew_brewfile".to_string()),
        };

        let (hook_type, plugin_name) = determine_hook_target(&post_plugin_target).unwrap();
        assert_eq!(hook_type, "post-plugin");
        assert_eq!(plugin_name, Some("homebrew_brewfile".to_string()));
    }

    /// Test error cases for hook target determination
    /// Verifies that invalid targets produce appropriate errors
    #[test]
    fn test_determine_hook_target_errors() {
        // Test no target specified
        let no_target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let result = determine_hook_target(&no_target);
        assert!(result.is_err());

        // Note: Multiple targets are prevented by clap's group constraints,
        // so the function actually processes them sequentially and would return
        // the first match. This is correct behavior since clap prevents multiple targets.
    }

    /// Test determine_hook_target with multiple targets
    /// Verifies that first target takes precedence when multiple are set
    #[test]
    fn test_determine_hook_target_multiple_targets_first_wins() {
        // Test with multiple targets set - pre_snapshot should win
        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: true,
            pre_plugin: None,
            post_plugin: None,
        };
        let result = determine_hook_target(&target).unwrap();
        assert_eq!(result.0, "pre-snapshot");
        assert_eq!(result.1, None);
    }

    /// Test determine_hook_target with no targets
    /// Verifies that error is returned when no target is specified
    #[test]
    fn test_determine_hook_target_no_targets_error() {
        // Test with no targets set
        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };
        let result = determine_hook_target(&target);
        assert!(result.is_err());
    }

    /// Test determine_hook_target with multiple plugin targets
    /// Verifies that first plugin target takes precedence when multiple are set
    #[test]
    fn test_determine_hook_target_multiple_plugins_first_wins() {
        // Test with plugin target but multiple plugins - pre_plugin should win
        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: Some("plugin1".to_string()),
            post_plugin: Some("plugin2".to_string()),
        };
        let result = determine_hook_target(&target).unwrap();
        assert_eq!(result.0, "pre-plugin");
        assert_eq!(result.1, Some("plugin1".to_string()));
    }

    /// Test convert_action_args_to_hook_action with script
    /// Verifies that script action conversion works correctly
    #[test]
    fn test_convert_action_args_script() {
        let args = HookActionArgs {
            script: Some("test.sh".to_string()),
            log: None,
            notify: None,
            backup: false,
            cleanup: false,
            args: Some("arg1,arg2".to_string()),
            timeout: Some(60),
            level: None,
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(args).unwrap();
        match result {
            HookAction::Script {
                command,
                args,
                timeout,
                ..
            } => {
                assert_eq!(command, "test.sh");
                assert_eq!(args, vec!["arg1".to_string(), "arg2".to_string()]);
                assert_eq!(timeout, 60);
            }
            _ => panic!("Expected script hook"),
        }
    }

    /// Test convert_action_args_to_hook_action with log
    /// Verifies that log action conversion works correctly
    #[test]
    fn test_convert_action_args_log() {
        let args = HookActionArgs {
            script: None,
            log: Some("Test log message".to_string()),
            notify: None,
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            level: Some("warn".to_string()),
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(args).unwrap();
        match result {
            HookAction::Log { message, level } => {
                assert_eq!(message, "Test log message");
                assert_eq!(level, "warn");
            }
            _ => panic!("Expected log hook"),
        }
    }

    /// Test determine_hook_target with valid targets
    /// Verifies that hook target determination works correctly
    #[test]
    fn test_determine_hook_target_valid() {
        // Test pre-snapshot global target
        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };
        let result = determine_hook_target(&target).unwrap();
        assert_eq!(result.0, "pre-snapshot");
        assert_eq!(result.1, None);

        // Test post-snapshot global target
        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: true,
            pre_plugin: None,
            post_plugin: None,
        };
        let result = determine_hook_target(&target).unwrap();
        assert_eq!(result.0, "post-snapshot");
        assert_eq!(result.1, None);

        // Test pre-plugin target
        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: Some("test_plugin".to_string()),
            post_plugin: None,
        };
        let result = determine_hook_target(&target).unwrap();
        assert_eq!(result.0, "pre-plugin");
        assert_eq!(result.1, Some("test_plugin".to_string()));

        // Test post-plugin target
        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: Some("test_plugin".to_string()),
        };
        let result = determine_hook_target(&target).unwrap();
        assert_eq!(result.0, "post-plugin");
        assert_eq!(result.1, Some("test_plugin".to_string()));
    }

    /// Test convert_action_args_to_hook_action with multiple actions
    /// Verifies that first action takes precedence when multiple are specified
    #[test]
    fn test_convert_action_args_multiple_actions_first_wins() {
        let args = HookActionArgs {
            script: Some("test.sh".to_string()),
            log: Some("test log".to_string()),
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
        let result = convert_action_args_to_hook_action(args).unwrap();
        // Script should take precedence over log
        match result {
            HookAction::Script { command, .. } => {
                assert_eq!(command, "test.sh");
            }
            _ => panic!("Expected script action to take precedence"),
        }
    }

    /// Test convert_action_args_to_hook_action with no action
    /// Verifies that error is returned when no action is specified
    #[test]
    fn test_convert_action_args_no_action_error() {
        let args = HookActionArgs {
            script: None,
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
        let result = convert_action_args_to_hook_action(args);
        assert!(result.is_err());
    }

    /// Test error handling in convert_action_args_to_hook_action
    /// Verifies that conversion handles missing required fields correctly
    #[tokio::test]
    async fn test_convert_action_args_error_handling_comprehensive() {
        use std::path::PathBuf;

        // Test backup action missing destination
        let backup_args_missing_dest = HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: true,
            path: Some(PathBuf::from("/source")),
            destination: None, // Missing required field
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(backup_args_missing_dest);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("destination"));

        // Test backup action missing path
        let backup_args_missing_path = HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: true,
            path: None, // Missing required field
            destination: Some(PathBuf::from("/backup")),
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(backup_args_missing_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("path"));
    }
}

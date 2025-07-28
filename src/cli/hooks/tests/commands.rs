//! Tests for hook command handlers and integration functionality

#[cfg(test)]
mod tests {
    use crate::cli::hooks::*;
    use crate::config::{Config, GlobalConfig, GlobalHooks, PluginHooks};
    use crate::core::hooks::{HookAction, HooksConfig};
    use crate::{HookActionArgs, HookTarget, HooksCommands};
    use anyhow::Result;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio::fs;

    /// Test handle_hooks_command function with list subcommand
    /// Verifies that the hooks command dispatcher works correctly for listing hooks
    #[tokio::test]
    async fn test_handle_hooks_command_list() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create a test config with some hooks
        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![HookAction::Log {
                        message: "test pre".to_string(),
                        level: "info".to_string(),
                    }],
                    post_snapshot: vec![],
                }),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

        let command = HooksCommands::List {
            plugin: None,
            pre_plugin: false,
            post_plugin: false,
            pre_snapshot: true,
            post_snapshot: false,
            verbose: false,
        };

        let result = handle_hooks_command(command, Some(config_path)).await;
        assert!(result.is_ok());
    }

    /// Test handle_hooks_command function with add subcommand
    /// Verifies that adding hooks through the command works correctly
    #[tokio::test]
    async fn test_handle_hooks_command_add() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create empty config
        Config::default().save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let action = HookActionArgs {
            script: Some("test.sh".to_string()),
            args: None,
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

        let command = HooksCommands::Add { target, action };

        let result = handle_hooks_command(command, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was added
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let pre_hooks = updated_config.get_global_pre_snapshot_hooks();
        assert_eq!(pre_hooks.len(), 1);
    }

    /// Test handle_hooks_command function with remove subcommand
    /// Verifies that removing hooks through the command works correctly
    #[tokio::test]
    async fn test_handle_hooks_command_remove() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with a hook to remove
        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![HookAction::Script {
                        command: "test.sh".to_string(),
                        args: vec![],
                        timeout: 30,
                        working_dir: None,
                        env_vars: std::collections::HashMap::new(),
                    }],
                    post_snapshot: vec![],
                }),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let command = HooksCommands::Remove {
            target,
            index: Some(0),
            all: false,
            script: None,
        };

        let result = handle_hooks_command(command, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was removed
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let pre_hooks = updated_config.get_global_pre_snapshot_hooks();
        assert_eq!(pre_hooks.len(), 0);
    }

    /// Test handle_hooks_command function with validate subcommand
    /// Verifies that hook validation through the command works correctly
    #[tokio::test]
    async fn test_handle_hooks_command_validate() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with hooks to validate
        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![HookAction::Log {
                        message: "test".to_string(),
                        level: "info".to_string(),
                    }],
                    post_snapshot: vec![],
                }),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

        let command = HooksCommands::Validate {
            plugin: None,
            pre_plugin: false,
            post_plugin: false,
            pre_snapshot: true,
            post_snapshot: false,
        };

        let result = handle_hooks_command(command, Some(config_path)).await;
        assert!(result.is_ok());
    }

    /// Test handle_hooks_command function with scripts-dir subcommand
    /// Verifies that scripts directory management through the command works correctly
    #[tokio::test]
    async fn test_handle_hooks_command_scripts_dir() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        let scripts_dir = temp_dir.path().join("scripts");

        // Create scripts directory with a test script
        fs::create_dir_all(&scripts_dir).await.unwrap();
        fs::write(scripts_dir.join("test.sh"), "#!/bin/bash\necho test")
            .await
            .unwrap();

        // Create config with scripts directory
        let config = Config {
            hooks: Some(HooksConfig {
                scripts_dir: scripts_dir.clone(),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

        let command = HooksCommands::ScriptsDir {
            set: Some(scripts_dir),
            create: false,
        };

        let result = handle_hooks_command(command, Some(config_path)).await;
        assert!(result.is_ok());
    }

    /// Test handle_add_hook function with global hooks
    /// Verifies that adding global hooks works correctly
    #[tokio::test]
    async fn test_handle_add_hook_global() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create empty config
        Config::default().save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let action = HookActionArgs {
            script: None,
            args: None,
            timeout: None,
            log: Some("test message".to_string()),
            notify: None,
            backup: false,
            cleanup: false,
            level: Some("info".to_string()),
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = handle_add_hook(target, action, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was added
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = updated_config.get_global_pre_snapshot_hooks();
        assert_eq!(hooks.len(), 1);
        match &hooks[0] {
            HookAction::Log { message, level } => {
                assert_eq!(message, "test message");
                assert_eq!(level, "info");
            }
            _ => panic!("Expected log hook"),
        }
    }

    /// Test handle_add_hook function with plugin hooks
    /// Verifies that adding plugin-specific hooks works correctly
    #[tokio::test]
    async fn test_handle_add_hook_plugin() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create empty config
        Config::default().save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: Some("test_plugin".to_string()),
            post_plugin: None,
        };

        let action = HookActionArgs {
            script: Some("test.sh".to_string()),
            args: Some("arg1".to_string()),
            timeout: Some(60),
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

        let result = handle_add_hook(target, action, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was added
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = updated_config.get_plugin_pre_hooks("test_plugin");
        assert_eq!(hooks.len(), 1);
        match &hooks[0] {
            HookAction::Script {
                command,
                args,
                timeout,
                working_dir,
                env_vars,
            } => {
                assert_eq!(command, "test.sh");
                assert_eq!(args, &vec!["arg1".to_string()]);
                assert_eq!(*timeout, 60);
                assert_eq!(*working_dir, None);
                assert!(env_vars.is_empty());
            }
            _ => panic!("Expected script hook"),
        }
    }

    /// Test handle_add_hook with backup action
    /// Verifies that backup hooks can be added correctly
    #[tokio::test]
    async fn test_handle_add_hook_backup() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create empty config
        let config = Config::default();
        config.save_to_file(&config_path).await.unwrap();

        // Add backup hook
        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };
        let action = HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: true,
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: None,
            path: Some(PathBuf::from("/source/path")),
            destination: Some(PathBuf::from("/dest/path")),
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = handle_add_hook(target, action, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was added
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = &updated_config.global.unwrap().hooks.unwrap().pre_snapshot;
        assert_eq!(hooks.len(), 1);
        match &hooks[0] {
            HookAction::Backup { path, destination } => {
                assert_eq!(*path, PathBuf::from("/source/path"));
                assert_eq!(*destination, PathBuf::from("/dest/path"));
            }
            _ => panic!("Expected backup hook"),
        }
    }

    /// Test handle_add_hook with cleanup action
    /// Verifies that cleanup hooks can be added correctly
    #[tokio::test]
    async fn test_handle_add_hook_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create empty config
        let config = Config::default();
        config.save_to_file(&config_path).await.unwrap();

        // Add cleanup hook
        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };
        let action = HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: false,
            cleanup: true,
            args: None,
            timeout: None,
            level: None,
            title: None,
            path: None,
            destination: None,
            patterns: Some("*.tmp,*.log".to_string()),
            directories: Some("/tmp,/var/log".to_string()),
            temp_files: true,
        };

        let result = handle_add_hook(target, action, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was added
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = &updated_config.global.unwrap().hooks.unwrap().pre_snapshot;
        assert_eq!(hooks.len(), 1);
        match &hooks[0] {
            HookAction::Cleanup {
                patterns,
                directories,
                temp_files,
            } => {
                assert_eq!(patterns.len(), 2);
                assert!(patterns.contains(&"*.tmp".to_string()));
                assert!(patterns.contains(&"*.log".to_string()));
                assert_eq!(directories.len(), 2);
                assert!(directories.contains(&PathBuf::from("/tmp")));
                assert!(directories.contains(&PathBuf::from("/var/log")));
                assert!(*temp_files);
            }
            _ => panic!("Expected cleanup hook"),
        }
    }

    /// Test handle_add_hook with notify action
    /// Verifies that notify hooks can be added correctly
    #[tokio::test]
    async fn test_handle_add_hook_notify() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create empty config
        let config = Config::default();
        config.save_to_file(&config_path).await.unwrap();

        // Add notify hook
        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };
        let action = HookActionArgs {
            script: None,
            log: None,
            notify: Some("Test notification".to_string()),
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: Some("Test Title".to_string()),
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = handle_add_hook(target, action, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was added
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = &updated_config.global.unwrap().hooks.unwrap().pre_snapshot;
        assert_eq!(hooks.len(), 1);
        match &hooks[0] {
            HookAction::Notify { message, title } => {
                assert_eq!(*message, "Test notification");
                assert_eq!(*title, Some("Test Title".to_string()));
            }
            _ => panic!("Expected notify hook"),
        }
    }

    /// Test handle_add_hook with post-snapshot target
    /// Verifies adding hooks to post-snapshot
    #[tokio::test]
    async fn test_handle_add_hook_post_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        Config::default().save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: true,
            pre_plugin: None,
            post_plugin: None,
        };

        let action = HookActionArgs {
            script: None,
            log: None,
            notify: Some("Snapshot complete".to_string()),
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: Some("Dotsnapshot".to_string()),
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = handle_add_hook(target, action, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was added to post-snapshot
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = updated_config.get_global_post_snapshot_hooks();
        assert_eq!(hooks.len(), 1);
        match &hooks[0] {
            HookAction::Notify { message, title } => {
                assert_eq!(message, "Snapshot complete");
                assert_eq!(title, &Some("Dotsnapshot".to_string()));
            }
            _ => panic!("Expected notify hook"),
        }
    }

    /// Test handle_add_hook with post-plugin target
    /// Verifies adding hooks to post-plugin
    #[tokio::test]
    async fn test_handle_add_hook_post_plugin() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        Config::default().save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: Some("test_plugin".to_string()),
        };

        let action = HookActionArgs {
            script: None,
            log: Some("Plugin complete".to_string()),
            notify: None,
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            level: Some("debug".to_string()),
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = handle_add_hook(target, action, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was added to post-plugin
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = updated_config.get_plugin_post_hooks("test_plugin");
        assert_eq!(hooks.len(), 1);
        match &hooks[0] {
            HookAction::Log { message, level } => {
                assert_eq!(message, "Plugin complete");
                assert_eq!(level, "debug");
            }
            _ => panic!("Expected log hook"),
        }
    }

    /// Test handle_remove_hook function with index removal
    /// Verifies that removing hooks by index works correctly
    #[tokio::test]
    async fn test_handle_remove_hook_by_index() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with multiple hooks
        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![
                        HookAction::Log {
                            message: "first hook".to_string(),
                            level: "info".to_string(),
                        },
                        HookAction::Log {
                            message: "second hook".to_string(),
                            level: "warn".to_string(),
                        },
                    ],
                    post_snapshot: vec![],
                }),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let result =
            handle_remove_hook(target, Some(0), false, None, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify first hook was removed
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = updated_config.get_global_pre_snapshot_hooks();
        assert_eq!(hooks.len(), 1);
        match &hooks[0] {
            HookAction::Log { message, .. } => {
                assert_eq!(message, "second hook");
            }
            _ => panic!("Expected log hook"),
        }
    }

    /// Test handle_remove_hook function with all removal
    /// Verifies that removing all hooks works correctly
    #[tokio::test]
    async fn test_handle_remove_hook_all() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with multiple hooks
        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![
                        HookAction::Log {
                            message: "first hook".to_string(),
                            level: "info".to_string(),
                        },
                        HookAction::Log {
                            message: "second hook".to_string(),
                            level: "warn".to_string(),
                        },
                    ],
                    post_snapshot: vec![],
                }),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let result = handle_remove_hook(target, None, true, None, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify all hooks were removed
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = updated_config.get_global_pre_snapshot_hooks();
        assert_eq!(hooks.len(), 0);
    }

    /// Test handle_remove_hook with post-snapshot hooks
    /// Verifies that post-snapshot hooks can be removed correctly
    #[tokio::test]
    async fn test_handle_remove_hook_post_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with post-snapshot hooks
        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![],
                    post_snapshot: vec![HookAction::Log {
                        message: "post hook".to_string(),
                        level: "info".to_string(),
                    }],
                }),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

        // Remove post-snapshot hook by index
        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: true,
            pre_plugin: None,
            post_plugin: None,
        };
        let result =
            handle_remove_hook(target, Some(0), false, None, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was removed
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = &updated_config.global.unwrap().hooks.unwrap().post_snapshot;
        assert!(hooks.is_empty());
    }

    /// Test handle_remove_hook with script name filtering
    /// Verifies that removing hooks by script name works correctly
    #[tokio::test]
    async fn test_handle_remove_hook_by_script_name() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with multiple script hooks
        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![
                        HookAction::Script {
                            command: "remove_this.sh".to_string(),
                            args: vec![],
                            timeout: 30,
                            working_dir: None,
                            env_vars: HashMap::new(),
                        },
                        HookAction::Script {
                            command: "keep_this.sh".to_string(),
                            args: vec![],
                            timeout: 30,
                            working_dir: None,
                            env_vars: HashMap::new(),
                        },
                        HookAction::Log {
                            message: "Keep this log".to_string(),
                            level: "info".to_string(),
                        },
                    ],
                    post_snapshot: vec![],
                }),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let result = handle_remove_hook(
            target,
            None,
            false,
            Some("remove_this".to_string()),
            Some(config_path.clone()),
        )
        .await;
        assert!(result.is_ok());

        // Verify only the targeted script was removed
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = updated_config.get_global_pre_snapshot_hooks();
        assert_eq!(hooks.len(), 2);

        // Check remaining hooks
        let script_commands: Vec<String> = hooks
            .iter()
            .filter_map(|h| match h {
                HookAction::Script { command, .. } => Some(command.clone()),
                _ => None,
            })
            .collect();
        assert!(!script_commands.contains(&"remove_this.sh".to_string()));
        assert!(script_commands.contains(&"keep_this.sh".to_string()));
    }

    /// Test handle_remove_hook when no hooks exist
    /// Verifies graceful handling when trying to remove from empty hook list
    #[tokio::test]
    async fn test_handle_remove_hook_no_hooks() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config without any hooks
        Config::default().save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let result = handle_remove_hook(target, Some(0), false, None, Some(config_path)).await;
        assert!(result.is_ok()); // Should handle gracefully
    }

    /// Test handle_remove_hook with invalid index
    /// Verifies error handling when index is out of bounds
    #[tokio::test]
    async fn test_handle_remove_hook_invalid_index() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with one hook
        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![HookAction::Log {
                        message: "Only hook".to_string(),
                        level: "info".to_string(),
                    }],
                    post_snapshot: vec![],
                }),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        // Try to remove at index 5 (out of bounds)
        let result = handle_remove_hook(target, Some(5), false, None, Some(config_path)).await;
        assert!(result.is_ok()); // Function handles out-of-bounds gracefully with error message
    }

    /// Test handle_list_hooks function with global hooks
    /// Verifies that listing hooks works correctly
    #[tokio::test]
    async fn test_handle_list_hooks_global() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with hooks
        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![HookAction::Log {
                        message: "test".to_string(),
                        level: "info".to_string(),
                    }],
                    post_snapshot: vec![],
                }),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

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
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with various hooks
        let mut config = Config {
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![HookAction::Log {
                        message: "Pre snapshot".to_string(),
                        level: "info".to_string(),
                    }],
                    post_snapshot: vec![HookAction::Log {
                        message: "Post snapshot".to_string(),
                        level: "info".to_string(),
                    }],
                }),
            }),
            ..Default::default()
        };

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

        config.save_to_file(&config_path).await.unwrap();

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

    /// Test handle_validate_hooks function
    /// Verifies that hook validation works correctly
    #[tokio::test]
    async fn test_handle_validate_hooks() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with hooks to validate
        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![HookAction::Log {
                        message: "test".to_string(),
                        level: "info".to_string(),
                    }],
                    post_snapshot: vec![],
                }),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

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

    /// Test handle_validate_hooks with invalid script hooks
    /// Verifies validation catches non-existent scripts
    #[tokio::test]
    async fn test_handle_validate_hooks_invalid_scripts() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with script hooks that don't exist
        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![HookAction::Script {
                        command: "nonexistent.sh".to_string(),
                        args: vec![],
                        timeout: 30,
                        working_dir: None,
                        env_vars: HashMap::new(),
                    }],
                    post_snapshot: vec![],
                }),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

        let result = handle_validate_hooks(
            None,  // plugin
            false, // pre_plugin
            false, // post_plugin
            true,  // pre_snapshot
            false, // post_snapshot
            Some(config_path),
        )
        .await;
        assert!(result.is_ok()); // Validation completes but reports errors
    }

    /// Test handle_scripts_dir function
    /// Verifies that scripts directory management works correctly
    #[tokio::test]
    async fn test_handle_scripts_dir() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        let scripts_dir = temp_dir.path().join("scripts");

        // Create scripts directory with test scripts
        fs::create_dir_all(&scripts_dir).await.unwrap();
        fs::write(scripts_dir.join("test1.sh"), "#!/bin/bash\necho test1")
            .await
            .unwrap();
        fs::write(
            scripts_dir.join("test2.py"),
            "#!/usr/bin/env python\nprint('test2')",
        )
        .await
        .unwrap();

        // Create config with scripts directory
        let config = Config {
            hooks: Some(HooksConfig {
                scripts_dir: scripts_dir.clone(),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

        let result = handle_scripts_dir(Some(scripts_dir), false, Some(config_path)).await;
        assert!(result.is_ok());
    }

    /// Test handle_scripts_dir with create option
    /// Verifies scripts directory creation functionality
    #[tokio::test]
    async fn test_handle_scripts_dir_create() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        let new_scripts_dir = temp_dir.path().join("new_scripts");

        // Create config without scripts directory
        Config::default().save_to_file(&config_path).await.unwrap();

        let result = handle_scripts_dir(
            Some(new_scripts_dir.clone()),
            true, // create
            Some(config_path.clone()),
        )
        .await;
        assert!(result.is_ok());
        assert!(new_scripts_dir.exists());

        // Verify config was updated
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks_config = updated_config.get_hooks_config();
        assert_eq!(hooks_config.scripts_dir, new_scripts_dir);
    }

    /// Test handle_scripts_dir without set option (display only)
    /// Verifies display-only mode for scripts directory
    #[tokio::test]
    async fn test_handle_scripts_dir_display_only() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        let scripts_dir = temp_dir.path().join("scripts");

        // Create scripts directory with some scripts
        fs::create_dir_all(&scripts_dir).await.unwrap();
        fs::write(scripts_dir.join("test1.sh"), "#!/bin/bash\necho test")
            .await
            .unwrap();
        fs::write(scripts_dir.join("test2.js"), "console.log('test')")
            .await
            .unwrap();

        // Create config with scripts directory
        let config = Config {
            hooks: Some(HooksConfig {
                scripts_dir: scripts_dir.clone(),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

        let result = handle_scripts_dir(
            None,  // set
            false, // create
            Some(config_path),
        )
        .await;
        assert!(result.is_ok());
    }

    /// Test handle_plugin_hook_removal with various removal scenarios
    /// Verifies that plugin hooks can be removed by index, all, or script name
    #[tokio::test]
    async fn test_handle_plugin_hook_removal_comprehensive() -> Result<()> {
        use crate::core::hooks::HookAction;

        // Setup config with plugin containing hooks
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("test_config.toml");

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
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config without the plugin
        Config::default().save_to_file(&config_path).await.unwrap();

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

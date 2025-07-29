//! Tests for adding hooks through CLI commands

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use crate::cli::hooks::handle_add_hook;
    use crate::core::hooks::HookAction;
    use crate::{HookActionArgs, HookTarget};
    use std::path::PathBuf;

    /// Test handle_add_hook function with global hooks
    /// Verifies that adding global hooks works correctly
    #[tokio::test]
    async fn test_handle_add_hook_global() {
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

        // Verify hook was added with correct content
        let updated_config = crate::config::Config::load_from_file(&config_path)
            .await
            .unwrap();
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
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_empty_config();
        setup_config_file(&config, &config_path).await;

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

        // Verify hook was added with correct script configuration
        let updated_config = crate::config::Config::load_from_file(&config_path)
            .await
            .unwrap();
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

        // Verify backup hook was added with correct paths
        let updated_config = crate::config::Config::load_from_file(&config_path)
            .await
            .unwrap();
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

        // Verify cleanup hook was added with correct patterns
        let updated_config = crate::config::Config::load_from_file(&config_path)
            .await
            .unwrap();
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

        // Verify notify hook was added with correct message and title
        let updated_config = crate::config::Config::load_from_file(&config_path)
            .await
            .unwrap();
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
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_empty_config();
        setup_config_file(&config, &config_path).await;

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
        let updated_config = crate::config::Config::load_from_file(&config_path)
            .await
            .unwrap();
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
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_empty_config();
        setup_config_file(&config, &config_path).await;

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
        let updated_config = crate::config::Config::load_from_file(&config_path)
            .await
            .unwrap();
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
}

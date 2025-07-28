//! Tests for hook command dispatching functionality

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use crate::cli::hooks::handle_hooks_command;
    use crate::{HookActionArgs, HookTarget, HooksCommands};

    /// Test handle_hooks_command function with list subcommand
    /// Verifies that the hooks command dispatcher works correctly for listing hooks
    #[tokio::test]
    async fn test_handle_hooks_command_list() {
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_config_with_pre_snapshot_hooks();
        setup_config_file(&config, &config_path).await;

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

        verify_hook_added(&config_path, 1).await;
    }

    /// Test handle_hooks_command function with remove subcommand
    /// Verifies that removing hooks through the command works correctly
    #[tokio::test]
    async fn test_handle_hooks_command_remove() {
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_config_with_pre_snapshot_hooks();
        setup_config_file(&config, &config_path).await;

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

        verify_hooks_removed(&config_path, 0).await;
    }

    /// Test handle_hooks_command function with validate subcommand
    /// Verifies that hook validation through the command works correctly
    #[tokio::test]
    async fn test_handle_hooks_command_validate() {
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_config_with_pre_snapshot_hooks();
        setup_config_file(&config, &config_path).await;

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
        let (_temp_dir, config_path, scripts_dir) = create_test_environment_with_scripts().await;
        let config = create_config_with_scripts_dir(scripts_dir.clone());
        setup_config_file(&config, &config_path).await;

        let command = HooksCommands::ScriptsDir {
            set: Some(scripts_dir),
            create: false,
        };

        let result = handle_hooks_command(command, Some(config_path)).await;
        assert!(result.is_ok());
    }
}

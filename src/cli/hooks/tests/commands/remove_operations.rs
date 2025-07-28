//! Tests for removing hooks through CLI commands

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use crate::cli::hooks::handle_remove_hook;
    use crate::core::hooks::HookAction;
    use crate::HookTarget;

    /// Test handle_remove_hook function with index removal
    /// Verifies that removing hooks by index works correctly
    #[tokio::test]
    async fn test_handle_remove_hook_by_index() {
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_config_with_multiple_hooks();
        setup_config_file(&config, &config_path).await;

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let result =
            handle_remove_hook(target, Some(0), false, None, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify first hook was removed, second remains
        let updated_config = crate::config::Config::load_from_file(&config_path)
            .await
            .unwrap();
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
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_config_with_multiple_hooks();
        setup_config_file(&config, &config_path).await;

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let result = handle_remove_hook(target, None, true, None, Some(config_path.clone())).await;
        assert!(result.is_ok());

        verify_hooks_removed(&config_path, 0).await;
    }

    /// Test handle_remove_hook with post-snapshot hooks
    /// Verifies that post-snapshot hooks can be removed correctly
    #[tokio::test]
    async fn test_handle_remove_hook_post_snapshot() {
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_config_with_post_snapshot_hooks();
        setup_config_file(&config, &config_path).await;

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
        let updated_config = crate::config::Config::load_from_file(&config_path)
            .await
            .unwrap();
        let hooks = &updated_config.global.unwrap().hooks.unwrap().post_snapshot;
        assert!(hooks.is_empty());
    }

    /// Test handle_remove_hook with script name filtering
    /// Verifies that removing hooks by script name works correctly
    #[tokio::test]
    async fn test_handle_remove_hook_by_script_name() {
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_config_with_script_hooks();
        setup_config_file(&config, &config_path).await;

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
        let updated_config = crate::config::Config::load_from_file(&config_path)
            .await
            .unwrap();
        let hooks = updated_config.get_global_pre_snapshot_hooks();
        assert_eq!(hooks.len(), 2);

        // Check remaining hooks don't include the removed script
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
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_empty_config();
        setup_config_file(&config, &config_path).await;

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
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_config_with_pre_snapshot_hooks();
        setup_config_file(&config, &config_path).await;

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
}

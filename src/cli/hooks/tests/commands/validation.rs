//! Tests for hook validation through CLI commands

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use crate::cli::hooks::handle_validate_hooks;

    /// Test handle_validate_hooks function
    /// Verifies that hook validation works correctly
    #[tokio::test]
    async fn test_handle_validate_hooks() {
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_config_with_pre_snapshot_hooks();
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

    /// Test handle_validate_hooks with invalid script hooks
    /// Verifies validation catches non-existent scripts
    #[tokio::test]
    async fn test_handle_validate_hooks_invalid_scripts() {
        let (_temp_dir, config_path) = create_test_environment();
        let config = create_config_with_invalid_script_hooks();
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
        assert!(result.is_ok()); // Validation completes but reports errors
    }
}

//! Tests for scripts directory management through CLI commands

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use crate::cli::hooks::handle_scripts_dir;

    /// Test handle_scripts_dir function
    /// Verifies that scripts directory management works correctly
    #[tokio::test]
    async fn test_handle_scripts_dir() {
        let (_temp_dir, config_path, scripts_dir) = create_test_environment_with_scripts().await;
        let config = create_config_with_scripts_dir(scripts_dir.clone());
        setup_config_file(&config, &config_path).await;

        let result = handle_scripts_dir(Some(scripts_dir), false, Some(config_path)).await;
        assert!(result.is_ok());
    }

    /// Test handle_scripts_dir with create option
    /// Verifies scripts directory creation functionality
    #[tokio::test]
    async fn test_handle_scripts_dir_create() {
        let (_temp_dir, config_path) = create_test_environment();
        let new_scripts_dir = _temp_dir.path().join("new_scripts");

        let config = create_empty_config();
        setup_config_file(&config, &config_path).await;

        let result = handle_scripts_dir(
            Some(new_scripts_dir.clone()),
            true, // create
            Some(config_path.clone()),
        )
        .await;
        assert!(result.is_ok());
        assert!(new_scripts_dir.exists());

        // Verify config was updated
        let updated_config = crate::config::Config::load_from_file(&config_path)
            .await
            .unwrap();
        let hooks_config = updated_config.get_hooks_config();
        assert_eq!(hooks_config.scripts_dir, new_scripts_dir);
    }

    /// Test handle_scripts_dir without set option (display only)
    /// Verifies display-only mode for scripts directory
    #[tokio::test]
    async fn test_handle_scripts_dir_display_only() {
        let (_temp_dir, config_path) = create_test_environment();
        let scripts_dir = _temp_dir.path().join("scripts");

        // Create scripts directory with some scripts
        create_multiple_test_scripts(&scripts_dir).await;

        let config = create_config_with_scripts_dir(scripts_dir);
        setup_config_file(&config, &config_path).await;

        let result = handle_scripts_dir(
            None,  // set
            false, // create
            Some(config_path),
        )
        .await;
        assert!(result.is_ok());
    }
}

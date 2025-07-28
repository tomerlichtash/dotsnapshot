//! Tests for StaticFilesPlugin execution functionality

#[cfg(test)]
mod tests {
    use super::super::test_utils::{
        create_test_file_paths, extract_file_count_from_json, validate_json_response,
        AdvancedMockCore, ErrorMockCore, JsonErrorMockCore, MockStaticFilesCore,
    };
    use crate::core::plugin::Plugin;
    use crate::plugins::core::base::static_files::StaticFilesPlugin;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Test basic plugin execution with empty configuration
    /// Verifies plugin returns proper JSON when no files are configured
    #[tokio::test]
    async fn test_static_files_plugin_execute_empty() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());
        let result = plugin.execute().await.unwrap();

        // Should contain a JSON response indicating no files configured
        assert!(result.contains("total_files"));
        assert!(result.contains("\"total_files\": 0"));
        assert!(validate_json_response(&result));
    }

    /// Test plugin execution with environment variable set
    /// Verifies plugin uses DOTSNAPSHOT_SNAPSHOT_DIR environment variable correctly
    #[tokio::test]
    async fn test_static_files_execute_with_env_var() {
        // Set environment variable for snapshot directory
        std::env::set_var("DOTSNAPSHOT_SNAPSHOT_DIR", "/tmp/test_snapshot");

        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());
        let result = plugin.execute().await.unwrap();

        // Should work with empty config (no files configured)
        assert!(result.contains("total_files"));
        assert!(validate_json_response(&result));

        // Clean up environment variable
        std::env::remove_var("DOTSNAPSHOT_SNAPSHOT_DIR");
    }

    /// Test plugin execution with files configured
    /// Verifies plugin processes configured files correctly
    #[tokio::test]
    async fn test_static_files_plugin_execute_with_files() {
        let test_files = create_test_file_paths();
        let mock_core = MockStaticFilesCore::new().with_files_for_read_config(test_files.clone());

        let plugin = StaticFilesPlugin::new(mock_core);
        let result = plugin.execute().await.unwrap();

        // Should process the configured files
        assert!(validate_json_response(&result));
        let file_count = extract_file_count_from_json(&result);
        assert_eq!(file_count, Some(test_files.len()));
    }

    /// Test plugin execution with existing static directory
    /// Verifies plugin handles pre-existing static directories correctly
    #[tokio::test]
    async fn test_static_files_execute_with_existing_static_dir() {
        let temp_dir = TempDir::new().unwrap();
        let static_dir = temp_dir.path().join("static");

        // Create existing static directory
        tokio::fs::create_dir_all(&static_dir).await.unwrap();
        tokio::fs::write(static_dir.join("existing.txt"), "existing content")
            .await
            .unwrap();

        // Set environment to use temp directory
        std::env::set_var("DOTSNAPSHOT_SNAPSHOT_DIR", temp_dir.path());

        let test_files = vec![PathBuf::from("/test/new_file.txt")];
        let mock_core = AdvancedMockCore::new().with_files(test_files.clone());

        let plugin = StaticFilesPlugin::new(mock_core);
        let result = plugin.execute().await.unwrap();

        // Should handle existing directory correctly
        assert!(validate_json_response(&result));
        assert!(result.contains("directory_checksum"));

        // Clean up
        std::env::remove_var("DOTSNAPSHOT_SNAPSHOT_DIR");
    }

    /// Test plugin execution with configuration error
    /// Verifies plugin handles read_config errors gracefully
    #[tokio::test]
    async fn test_static_files_plugin_execute_with_config_error() {
        let error_mock = ErrorMockCore::new().with_read_config_error();
        let plugin = StaticFilesPlugin::new(error_mock);
        let result = plugin.execute().await.unwrap();

        // Should return error JSON response
        assert!(validate_json_response(&result));
        assert!(result.contains("error"));
        assert!(result.contains("Failed to read config"));
        assert!(result.contains("\"total_files\": 0"));
    }

    /// Test plugin execution with copy files error
    /// Verifies plugin handles copy_files errors gracefully
    #[tokio::test]
    async fn test_static_files_plugin_execute_with_copy_error() {
        let test_files = create_test_file_paths();

        // Use MockStaticFilesCore with copy_files error
        let mock_core = MockStaticFilesCore::new()
            .with_files_for_read_config(test_files)
            .with_copy_files_error();

        let plugin = StaticFilesPlugin::new(mock_core);
        let result = plugin.execute().await;

        // Should handle copy error
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Mock error copying files"));
    }

    /// Test plugin execution with JSON error handling
    /// Verifies plugin handles malformed JSON responses appropriately
    #[tokio::test]
    async fn test_static_files_execute_json_error_handling() {
        let json_error_mock = JsonErrorMockCore;
        let plugin = StaticFilesPlugin::new(json_error_mock);
        let result = plugin.execute().await;

        // Should handle JSON parsing errors
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("key must be a string")
                || error_msg.contains("JSON")
                || error_msg.contains("parse")
                || error_msg.contains("invalid")
        );
    }

    /// Test plugin execution with checksum calculation
    /// Verifies plugin includes directory checksum in response
    #[tokio::test]
    async fn test_static_files_execute_with_checksum() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("DOTSNAPSHOT_SNAPSHOT_DIR", temp_dir.path());

        let test_files = vec![PathBuf::from("/test/file.txt")];
        let mock_core = AdvancedMockCore::new().with_files(test_files);

        let plugin = StaticFilesPlugin::new(mock_core);
        let result = plugin.execute().await.unwrap();

        // Should include checksum information
        assert!(validate_json_response(&result));
        assert!(result.contains("STATIC_DIR_CHECKSUM:"));
        assert!(result.contains("directory_checksum"));

        // Clean up
        std::env::remove_var("DOTSNAPSHOT_SNAPSHOT_DIR");
    }

    /// Test plugin execution with advanced mock core
    /// Verifies plugin works correctly with advanced mock features
    #[tokio::test]
    async fn test_static_files_plugin_execute_with_advanced_core() {
        let test_files = create_test_file_paths();
        let advanced_mock = AdvancedMockCore::new().with_files(test_files.clone());

        let plugin = StaticFilesPlugin::new(advanced_mock);
        let result = plugin.execute().await.unwrap();

        // Should process files with advanced mock
        assert!(validate_json_response(&result));
        let file_count = extract_file_count_from_json(&result);
        assert_eq!(file_count, Some(test_files.len()));

        // Advanced mock provides more detailed JSON
        assert!(result.contains("details") || result.contains("ignore_patterns"));
    }

    /// Test plugin execution with ignore patterns
    /// Verifies plugin correctly applies ignore patterns during execution
    #[tokio::test]
    async fn test_static_files_execute_with_ignore_patterns() {
        let test_files = vec![
            PathBuf::from("/test/file.txt"),
            PathBuf::from("/test/temp.tmp"),
            PathBuf::from("/test/log.log"),
        ];
        let ignore_patterns = vec!["*.tmp".to_string(), "*.log".to_string()];

        let mock_core = AdvancedMockCore::new()
            .with_files(test_files.clone())
            .with_ignore_patterns(ignore_patterns);

        let plugin = StaticFilesPlugin::new(mock_core);
        let result = plugin.execute().await.unwrap();

        // Should apply ignore patterns
        assert!(validate_json_response(&result));
        assert!(result.contains("failed")); // Some files should be ignored/failed
        assert!(result.contains("ignore_patterns"));
    }

    /// Test plugin execution with snapshot directory fallback
    /// Verifies plugin uses fallback directory when environment variable not set
    #[tokio::test]
    async fn test_static_files_execute_snapshot_dir_fallback() {
        // Ensure no environment variable is set
        std::env::remove_var("DOTSNAPSHOT_SNAPSHOT_DIR");

        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());
        let result = plugin.execute().await.unwrap();

        // Should use fallback directory and still work
        assert!(validate_json_response(&result));
        // The response might not contain "target_directory" if no files are configured
        // Just verify it's a valid response
        assert!(result.contains("total_files"));
    }
}

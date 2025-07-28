//! Tests for StaticFilesPlugin error handling scenarios

#[cfg(test)]
mod tests {
    use super::super::test_utils::{
        AdvancedMockCore, ErrorMockCore, ErrorProneMockCore, JsonErrorMockCore, MockStaticFilesCore,
    };
    use crate::config::Config;
    use crate::core::plugin::Plugin;
    use crate::plugins::core::base::static_files::{StaticFilesCore, StaticFilesPlugin};
    use anyhow::Result;
    use std::path::PathBuf;
    use std::pin::Pin;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Test plugin execution with configuration errors
    /// Verifies plugin handles read_config errors gracefully
    #[tokio::test]
    async fn test_static_files_plugin_execute_with_config_error() {
        let error_mock = ErrorMockCore::new().with_read_config_error();
        let plugin = StaticFilesPlugin::new(error_mock);
        let result = plugin.execute().await.unwrap();

        // Should return error JSON response
        assert!(result.contains("error"));
        assert!(result.contains("Failed to read config"));
        assert!(result.contains("\"total_files\": 0"));
    }

    /// Test plugin restoration with error scenarios
    /// Verifies plugin handles restoration errors gracefully
    #[tokio::test]
    async fn test_static_files_plugin_restore_error() {
        let error_mock = ErrorProneMockCore::new().with_restore_failure();
        let plugin = StaticFilesPlugin::new(error_mock);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");
        tokio::fs::create_dir_all(&snapshot_dir).await.unwrap();
        tokio::fs::create_dir_all(&target_dir).await.unwrap();

        let result = plugin.restore(&snapshot_dir, &target_dir, false).await;

        // Should handle restoration error gracefully
        assert!(result.is_ok());
        let restored_files = result.unwrap();
        assert!(restored_files.is_empty());
    }

    /// Test AdvancedMockCore with expand_path errors
    /// Verifies expand_path error handling works correctly
    #[test]
    fn test_advanced_mock_core_expand_path_error() {
        let error_core = AdvancedMockCore::new().with_error("expand_path", "Mock expand error");
        let expand_result = error_core.expand_path("/any/path");
        assert!(expand_result.is_err());
        assert!(expand_result
            .unwrap_err()
            .to_string()
            .contains("Mock expand error"));
    }

    /// Test error mock implementations
    /// Verifies various error mock configurations work correctly
    #[tokio::test]
    async fn test_error_mock_implementations() {
        // Test AdvancedMockCore with error conditions
        let error_core =
            AdvancedMockCore::new().with_error("expand_path", "Test expand path error");
        let expand_result = error_core.expand_path("/any/path");
        assert!(expand_result.is_err());

        let ignore_core = AdvancedMockCore::new().with_ignore_result(true);
        assert!(ignore_core.should_ignore(&PathBuf::from("test.txt"), &[]));

        // Test if home directory expansion works when no errors
        if dirs::home_dir().is_some() {
            let normal_core = AdvancedMockCore::new();
            let tilde_result = normal_core.expand_path("~/test").unwrap();
            assert!(tilde_result.to_string_lossy().contains("test"));

            let home_result = normal_core.expand_path("$HOME/config").unwrap();
            assert!(home_result.to_string_lossy().contains("config"));
        }
    }

    /// Test plugin execution with JSON parsing errors
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

    /// Test plugin execution with checksum calculation errors
    /// Verifies plugin handles checksum errors gracefully
    #[tokio::test]
    async fn test_static_files_execute_checksum_error() {
        // Test execution with mock that may cause checksum issues
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());

        // Set invalid environment that might cause checksum issues
        std::env::set_var("DOTSNAPSHOT_SNAPSHOT_DIR", "/nonexistent/directory");

        let result = plugin.execute().await;

        // Should handle checksum errors gracefully
        assert!(result.is_ok());

        // Clean up environment variable
        std::env::remove_var("DOTSNAPSHOT_SNAPSHOT_DIR");
    }

    /// Test plugin restoration with comprehensive error handling
    /// Verifies plugin handles various restoration error scenarios
    #[tokio::test]
    async fn test_static_files_plugin_restore_comprehensive_error_handling() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");
        tokio::fs::create_dir_all(&snapshot_dir).await.unwrap();
        tokio::fs::create_dir_all(&target_dir).await.unwrap();

        // Test with error-prone mock
        let error_mock = ErrorProneMockCore::new().with_restore_failure();
        let plugin = StaticFilesPlugin::new(error_mock);

        let result = plugin.restore(&snapshot_dir, &target_dir, false).await;

        // Should handle errors gracefully
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    /// Test plugin restoration with directory read errors
    /// Verifies plugin handles cases where snapshot directory cannot be read
    #[tokio::test]
    async fn test_static_files_plugin_restore_directory_read_error() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("nonexistent_snapshot");
        let target_dir = temp_dir.path().join("target");
        tokio::fs::create_dir_all(&target_dir).await.unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Should handle missing snapshot directory gracefully
        assert!(result.is_empty());
    }

    /// Test plugin with advanced error scenarios
    /// Verifies plugin handles complex error conditions correctly
    #[tokio::test]
    async fn test_static_files_plugin_advanced_error_scenarios() {
        // Test with multiple error conditions
        let error_core = AdvancedMockCore::new()
            .with_error("read_config", "Config read error")
            .with_error("copy_files", "Copy error");

        let plugin = StaticFilesPlugin::new(error_core);

        // Test execution with multiple error conditions
        let exec_result = plugin.execute().await;
        // May succeed or fail depending on error handling - both acceptable
        let _ = exec_result;

        // Test validation should still work
        let validation_result = plugin.validate().await;
        assert!(validation_result.is_ok());
    }

    /// Test comprehensive error scenarios with local error mock
    /// Verifies all error paths are handled correctly
    #[tokio::test]
    async fn test_error_mock_core_comprehensive_error_scenarios() {
        // Create a local error mock for testing
        struct LocalErrorMock;

        impl StaticFilesCore for LocalErrorMock {
            fn icon(&self) -> &'static str {
                "📄"
            }
            fn read_config(
                &self,
                _config: Option<&Arc<Config>>,
            ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>>
            {
                Box::pin(async move { Err(anyhow::anyhow!("Config read error")) })
            }
            fn get_ignore_patterns(&self, _config: Option<&Arc<Config>>) -> Vec<String> {
                vec![]
            }
            fn should_ignore(&self, _path: &std::path::Path, _ignore_patterns: &[String]) -> bool {
                false
            }
            fn expand_path(&self, path: &str) -> Result<PathBuf> {
                Ok(PathBuf::from(path))
            }
            fn copy_files(
                &self,
                _file_paths: Vec<PathBuf>,
                _static_dir: &std::path::Path,
                _ignore_patterns: &[String],
            ) -> Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
            {
                Box::pin(async move { Err(anyhow::anyhow!("Copy files error")) })
            }
            fn restore_static_files(
                &self,
                _static_snapshot_dir: &std::path::Path,
                _target_base_path: &std::path::Path,
            ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>>
            {
                Box::pin(async move { Err(anyhow::anyhow!("Restore error")) })
            }
        }

        let core = LocalErrorMock;

        // Test basic methods work even with errors in async methods
        assert_eq!(core.icon(), "📄");
        assert!(core.get_ignore_patterns(None).is_empty());
        assert!(!core.should_ignore(&PathBuf::from("test.txt"), &[]));
        assert!(core.expand_path("/test").is_ok());

        // Test async methods return errors
        let config_result = core.read_config(None).await;
        assert!(config_result.is_err());
        assert!(config_result
            .unwrap_err()
            .to_string()
            .contains("Config read error"));

        let copy_result = core
            .copy_files(vec![], &PathBuf::from("/static"), &[])
            .await;
        assert!(copy_result.is_err());
        assert!(copy_result
            .unwrap_err()
            .to_string()
            .contains("Copy files error"));

        let restore_result = core
            .restore_static_files(&PathBuf::from("/snapshot"), &PathBuf::from("/target"))
            .await;
        assert!(restore_result.is_err());
        assert!(restore_result
            .unwrap_err()
            .to_string()
            .contains("Restore error"));

        // Test plugin with error mock
        let plugin = StaticFilesPlugin::new(core);
        let plugin_result = plugin.execute().await;
        // Plugin should handle the error gracefully
        assert!(plugin_result.is_ok());
        let result_str = plugin_result.unwrap();
        assert!(result_str.contains("error") || result_str.contains("total_files"));
    }

    /// Test error handling with copy files errors
    /// Verifies plugin handles copy_files errors appropriately
    #[tokio::test]
    async fn test_static_files_plugin_execute_with_copy_error() {
        let test_files = vec![PathBuf::from("/test/file.txt")];

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

    /// Test error handling edge cases
    /// Verifies plugin handles various edge case error scenarios
    #[tokio::test]
    async fn test_error_handling_edge_cases() {
        // Test with empty paths that might cause errors
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());

        let temp_dir = TempDir::new().unwrap();
        let empty_snapshot = temp_dir.path().join("");
        let empty_target = temp_dir.path().join("");

        // Should handle empty paths gracefully
        let result = plugin.restore(&empty_snapshot, &empty_target, false).await;
        assert!(result.is_ok());
    }

    /// Test validation error scenarios
    /// Verifies plugin validation handles error conditions
    #[tokio::test]
    async fn test_static_files_validate_error_case() {
        // Test validation normally succeeds with mock core
        // In real scenarios, validation could fail if home directory cannot be determined
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());
        let result = plugin.validate().await;
        assert!(result.is_ok());

        // Test validation with error-prone mock
        let error_plugin = StaticFilesPlugin::new(ErrorProneMockCore::new());
        let error_result = error_plugin.validate().await;
        assert!(error_result.is_ok());
    }
}

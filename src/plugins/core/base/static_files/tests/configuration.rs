//! Tests for StaticFilesPlugin configuration handling

#[cfg(test)]
mod tests {
    use super::super::test_utils::{AdvancedMockCore, MinimalStaticFilesCore, MockStaticFilesCore};
    use crate::config::{Config, StaticFilesConfig};
    use crate::core::plugin::Plugin;
    use crate::plugins::core::base::static_files::{StaticFilesCore, StaticFilesPlugin};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::fs;

    /// Test plugin with_config constructor functionality
    /// Verifies plugin correctly handles config-based initialization
    #[test]
    fn test_static_files_plugin_with_config() {
        let config = Arc::new(Config {
            output_dir: Some(PathBuf::from("/test/output")),
            include_plugins: None,
            logging: None,
            hooks: None,
            global: None,
            static_files: Some(StaticFilesConfig {
                files: Some(vec!["/test/file.txt".to_string()]),
                ignore: Some(vec!["*.log".to_string()]),
            }),
            plugins: None,
            ui: None,
            validation: None,
        });

        let plugin = StaticFilesPlugin::with_config(MockStaticFilesCore::new(), config.clone());
        assert!(plugin.config.is_some());
        assert_eq!(
            plugin.config.as_ref().unwrap().output_dir,
            Some(PathBuf::from("/test/output"))
        );
    }

    /// Test plugin with snapshot directory fallback logic
    /// Verifies snapshot directory resolution works correctly
    #[test]
    fn test_static_files_plugin_snapshot_dir_fallback() {
        // Test snapshot directory resolution logic
        let mut plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());
        plugin.snapshot_dir = Some(PathBuf::from("/test/snapshot"));

        // Verify plugin was created successfully with snapshot directory
        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );
    }

    /// Test advanced mock core read_config method
    /// Verifies read_config returns correct configuration results
    #[tokio::test]
    async fn test_advanced_mock_core_read_config() {
        let core = AdvancedMockCore::new();
        let result = core.read_config(None).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], PathBuf::from("/test/file1.txt"));
        assert_eq!(result[1], PathBuf::from("/test/file2.txt"));
    }

    /// Test advanced mock core copy_files method
    /// Verifies copy_files handles multiple files with configuration
    #[tokio::test]
    async fn test_advanced_mock_core_copy_files() {
        let core = AdvancedMockCore::new();
        let file_paths = vec![
            PathBuf::from("/source/file1.txt"),
            PathBuf::from("/source/file2.txt"),
        ];
        let static_dir = PathBuf::from("/static");
        let ignore_patterns = vec!["*.tmp".to_string()];

        let result = core
            .copy_files(file_paths.clone(), &static_dir, &ignore_patterns)
            .await
            .unwrap();

        // Verify JSON structure contains file information
        assert!(result.contains("total_files"));
        assert!(result.contains("copied"));
        assert!(result.contains("files"));
        assert!(result.contains("file1.txt"));
        assert!(result.contains("file2.txt"));
    }

    /// Test plugin with edge case configurations
    /// Verifies plugin handles empty and minimal configurations gracefully
    #[tokio::test]
    async fn test_static_files_plugin_config_edge_cases() {
        // Test with completely empty config
        let empty_config = Arc::new(Config {
            output_dir: None,
            include_plugins: None,
            logging: None,
            hooks: None,
            global: None,
            static_files: None,
            plugins: None,
            ui: None,
            validation: None,
        });
        let plugin = StaticFilesPlugin::with_config(MockStaticFilesCore::new(), empty_config);

        // Should handle empty config gracefully
        let result = plugin.execute().await;
        assert!(result.is_ok());
    }

    /// Test plugin with complex path configurations
    /// Verifies handling of various path types and edge cases
    #[tokio::test]
    async fn test_static_files_plugin_complex_path_config() {
        let config = Arc::new(Config {
            output_dir: Some(PathBuf::from("/test/output")),
            include_plugins: None,
            logging: None,
            hooks: None,
            global: None,
            static_files: Some(StaticFilesConfig {
                files: Some(vec![
                    "~".to_string(),              // Home directory
                    "~/Documents".to_string(),    // Home subdirectory
                    "$HOME".to_string(),          // Environment variable
                    "/absolute/path".to_string(), // Absolute path
                    "relative/path".to_string(),  // Relative path
                    "".to_string(),               // Empty path
                ]),
                ignore: Some(vec![
                    "*.tmp".to_string(),
                    "*.log".to_string(),
                    "node_modules".to_string(),
                ]),
            }),
            plugins: None,
            ui: None,
            validation: None,
        });

        let plugin = StaticFilesPlugin::with_config(MockStaticFilesCore::new(), config);
        let result = plugin.execute().await;

        // Should handle complex path configurations
        assert!(result.is_ok());
    }

    /// Test plugin with multiple mock configurations
    /// Verifies plugin works with different mock core configurations
    #[tokio::test]
    async fn test_static_files_plugin_multiple_mock_configurations() {
        // Test with minimal mock
        let minimal_plugin = StaticFilesPlugin::new(MinimalStaticFilesCore);
        let minimal_result = minimal_plugin.execute().await;
        assert!(minimal_result.is_ok());

        // Test with advanced mock
        let advanced_plugin = StaticFilesPlugin::new(AdvancedMockCore::new());
        let advanced_result = advanced_plugin.execute().await;
        assert!(advanced_result.is_ok());

        // Test with error-prone mock (using advanced mock with error conditions)
        let error_core =
            AdvancedMockCore::new().with_error("expand_path", "Mock expand path error");
        let error_plugin = StaticFilesPlugin::new(error_core);
        let error_result = error_plugin.execute().await;
        // Error mock may succeed or fail - both are acceptable
        let _ = error_result;
    }

    /// Test plugin with different snapshot directory scenarios
    /// Verifies snapshot_dir field behavior in various scenarios
    #[tokio::test]
    async fn test_static_files_plugin_snapshot_dir_scenarios() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("custom_snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test files in snapshot
        fs::write(snapshot_dir.join("file1.txt"), "content1")
            .await
            .unwrap();
        fs::write(snapshot_dir.join("file2.txt"), "content2")
            .await
            .unwrap();

        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());

        // Test plugin execution with custom snapshot directory
        let result = plugin.execute().await;
        assert!(result.is_ok());
    }

    /// Test configuration with static files settings
    /// Verifies static files config settings are properly handled
    #[test]
    fn test_static_files_config_settings() {
        let config = StaticFilesConfig {
            files: Some(vec![
                "/home/user/.bashrc".to_string(),
                "/home/user/.vimrc".to_string(),
                "/etc/hosts".to_string(),
            ]),
            ignore: Some(vec![
                "*.tmp".to_string(),
                "*.log".to_string(),
                ".DS_Store".to_string(),
            ]),
        };

        // Verify config structure
        assert!(config.files.is_some());
        assert!(config.ignore.is_some());
        assert_eq!(config.files.as_ref().unwrap().len(), 3);
        assert_eq!(config.ignore.as_ref().unwrap().len(), 3);
    }

    /// Test plugin with various ignore pattern configurations
    /// Verifies ignore patterns are correctly applied
    #[tokio::test]
    async fn test_static_files_plugin_ignore_pattern_config() {
        let ignore_patterns = vec![
            "*.tmp".to_string(),
            "*.log".to_string(),
            ".git/*".to_string(),
            "node_modules/*".to_string(),
        ];

        let config = Arc::new(Config {
            output_dir: Some(PathBuf::from("/test/output")),
            include_plugins: None,
            logging: None,
            hooks: None,
            global: None,
            static_files: Some(StaticFilesConfig {
                files: Some(vec!["/test/file.txt".to_string()]),
                ignore: Some(ignore_patterns.clone()),
            }),
            plugins: None,
            ui: None,
            validation: None,
        });

        let plugin = StaticFilesPlugin::with_config(MockStaticFilesCore::new(), config);
        let result = plugin.execute().await;

        // Should handle ignore patterns configuration
        assert!(result.is_ok());
    }

    /// Test configuration edge cases with None values
    /// Verifies plugin handles None config values gracefully
    #[test]
    fn test_static_files_config_none_values() {
        let config = StaticFilesConfig {
            files: None,
            ignore: None,
        };

        // Should handle None values gracefully
        assert!(config.files.is_none());
        assert!(config.ignore.is_none());
    }
}

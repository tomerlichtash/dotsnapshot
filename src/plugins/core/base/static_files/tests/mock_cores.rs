//! Tests for various mock core implementations used in testing

#[cfg(test)]
mod tests {
    use super::super::test_utils::{AdvancedMockCore, MinimalStaticFilesCore, MockStaticFilesCore};
    use crate::core::plugin::Plugin;
    use crate::plugins::core::base::static_files::{StaticFilesCore, StaticFilesPlugin};
    use std::path::PathBuf;

    /// Test basic MockStaticFilesCore functionality
    /// Verifies all core methods work correctly with default mock implementation
    #[test]
    fn test_mock_static_files_core_methods() {
        let core = MockStaticFilesCore::new();

        assert_eq!(core.icon(), "📄");

        let ignore_patterns = core.get_ignore_patterns(None);
        assert_eq!(ignore_patterns, vec![] as Vec<String>);

        // Test should_ignore method with non-empty patterns
        let test_patterns = vec!["*.tmp".to_string()];
        let temp_path = PathBuf::from("test.tmp");
        assert!(core.should_ignore(&temp_path, &test_patterns));

        let normal_path = PathBuf::from("test.txt");
        assert!(!core.should_ignore(&normal_path, &test_patterns));

        // Test expand_path method
        let expanded = core.expand_path("/test/path").unwrap();
        assert_eq!(expanded, PathBuf::from("/test/path"));
    }

    /// Test AdvancedMockCore icon functionality
    /// Verifies advanced mock returns custom icon
    #[test]
    fn test_advanced_mock_core_icon() {
        let core = AdvancedMockCore::new();
        assert_eq!(core.icon(), "🔧");
    }

    /// Test AdvancedMockCore ignore patterns functionality
    /// Verifies advanced mock returns multiple ignore patterns
    #[test]
    fn test_advanced_mock_core_ignore_patterns() {
        let core = AdvancedMockCore::new();
        let patterns = core.get_ignore_patterns(None);
        assert_eq!(patterns, vec!["*.tmp".to_string(), "*.log".to_string()]);
    }

    /// Test AdvancedMockCore should_ignore logic
    /// Verifies pattern matching works correctly with different pattern types
    #[test]
    fn test_advanced_mock_core_should_ignore_logic() {
        let core = AdvancedMockCore::new();
        let patterns = vec!["*.log".to_string(), "node_modules/".to_string()];

        // Test glob pattern matching
        assert!(core.should_ignore(&PathBuf::from("debug.log"), &patterns));
        assert!(core.should_ignore(&PathBuf::from("app.log"), &patterns));

        // Test directory pattern matching
        assert!(core.should_ignore(&PathBuf::from("project/node_modules/package"), &patterns));

        // Test non-matching paths
        assert!(!core.should_ignore(&PathBuf::from("readme.txt"), &patterns));
        assert!(!core.should_ignore(&PathBuf::from("src/main.rs"), &patterns));
    }

    /// Test AdvancedMockCore force ignore functionality
    /// Verifies should_ignore can be configured to always return true
    #[test]
    fn test_advanced_mock_core_should_ignore_force_true() {
        let core = AdvancedMockCore::new().with_ignore_result(true);
        let patterns = vec!["*.txt".to_string()];

        // Should ignore any path when forced
        assert!(core.should_ignore(&PathBuf::from("any_file.rs"), &patterns));
        assert!(core.should_ignore(&PathBuf::from("another.md"), &patterns));
    }

    /// Test AdvancedMockCore home directory expansion
    /// Verifies tilde expansion works correctly
    #[test]
    fn test_advanced_mock_core_expand_path_home_tilde() {
        let core = AdvancedMockCore::new();
        let expanded = core.expand_path("~/Documents").unwrap();
        // Should expand tilde to some home directory path
        assert!(expanded.to_string_lossy().contains("Documents"));
        assert!(!expanded.to_string_lossy().starts_with("~"));
    }

    /// Test AdvancedMockCore environment variable expansion
    /// Verifies $HOME expansion works correctly
    #[test]
    fn test_advanced_mock_core_expand_path_home_env() {
        let core = AdvancedMockCore::new();
        let expanded = core.expand_path("$HOME/test").unwrap();
        // Should expand $HOME to actual home directory
        assert!(expanded.to_string_lossy().contains("test"));
        assert!(!expanded.to_string_lossy().contains("$HOME"));
    }

    /// Test AdvancedMockCore regular path handling
    /// Verifies regular paths are returned unchanged
    #[test]
    fn test_advanced_mock_core_expand_path_regular() {
        let core = AdvancedMockCore::new();
        let expanded = core.expand_path("/regular/path").unwrap();
        assert_eq!(expanded, PathBuf::from("/regular/path"));
    }

    /// Test AdvancedMockCore error handling
    /// Verifies expand_path can be configured to return errors
    #[test]
    fn test_advanced_mock_core_expand_path_error() {
        let core = AdvancedMockCore::new().with_error("expand_path", "Mock expand error");
        let result = core.expand_path("/test/path");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Mock expand error"));
    }

    /// Test MinimalStaticFilesCore basic functionality
    /// Verifies minimal implementation provides basic trait coverage
    #[test]
    fn test_minimal_static_files_core() {
        let core = MinimalStaticFilesCore;

        assert_eq!(core.icon(), "📁");
        assert_eq!(core.get_ignore_patterns(None), Vec::<String>::new());
        assert!(!core.should_ignore(&PathBuf::from("any_file.txt"), &["*.log".to_string()]));

        let expanded = core.expand_path("/test/path").unwrap();
        assert_eq!(expanded, PathBuf::from("/test/path"));
    }

    /// Test MinimalStaticFilesCore async methods
    /// Verifies minimal implementation provides working async trait methods
    #[tokio::test]
    async fn test_minimal_static_files_core_async_methods() {
        let core = MinimalStaticFilesCore;

        let config_result = core.read_config(None).await.unwrap();
        assert!(config_result.is_empty());

        let copy_result = core
            .copy_files(vec![], &PathBuf::from("/static"), &[])
            .await
            .unwrap();
        // MinimalStaticFilesCore returns JSON, not empty string
        assert!(copy_result.contains("total_files"));

        let restore_result = core
            .restore_static_files(&PathBuf::from("/snapshot"), &PathBuf::from("/target"))
            .await
            .unwrap();
        assert!(restore_result.is_empty());
    }

    /// Test StaticFilesPlugin with MinimalStaticFilesCore
    /// Verifies plugin works correctly with minimal core implementation
    #[tokio::test]
    async fn test_static_files_plugin_with_minimal_core() {
        let plugin = StaticFilesPlugin::new(MinimalStaticFilesCore);

        assert_eq!(plugin.icon(), "📁");

        // Test plugin execution with minimal core
        let result = plugin.execute().await;
        assert!(result.is_ok());
    }

    /// Test all AdvancedMockCore methods comprehensively
    /// Verifies complete functionality of advanced mock implementation
    #[tokio::test]
    async fn test_all_advanced_mock_core_methods() {
        let core = AdvancedMockCore::new();

        // Test synchronous methods
        assert_eq!(core.icon(), "🔧");
        assert!(!core.get_ignore_patterns(None).is_empty());
        assert!(core.expand_path("/test").is_ok());

        // Test async methods
        let config_result = core.read_config(None).await;
        assert!(config_result.is_ok());

        let copy_result = core
            .copy_files(
                vec![PathBuf::from("/test/file")],
                &PathBuf::from("/static"),
                &[],
            )
            .await;
        assert!(copy_result.is_ok());

        let restore_result = core
            .restore_static_files(&PathBuf::from("/snapshot"), &PathBuf::from("/target"))
            .await;
        assert!(restore_result.is_ok());
    }

    /// Test all MinimalStaticFilesCore methods comprehensively
    /// Verifies complete functionality of minimal mock implementation
    #[tokio::test]
    async fn test_all_minimal_static_files_core_methods() {
        let core = MinimalStaticFilesCore;

        // Test synchronous methods
        assert_eq!(core.icon(), "📁");
        assert!(core.get_ignore_patterns(None).is_empty());
        assert!(core.expand_path("/test").is_ok());
        assert!(!core.should_ignore(&PathBuf::from("test.txt"), &[]));

        // Test async methods
        let config_result = core.read_config(None).await;
        assert!(config_result.is_ok());
        assert!(config_result.unwrap().is_empty());

        let copy_result = core
            .copy_files(vec![], &PathBuf::from("/static"), &[])
            .await;
        assert!(copy_result.is_ok());
        // MinimalStaticFilesCore returns JSON, not empty string
        assert!(copy_result.unwrap().contains("total_files"));

        let restore_result = core
            .restore_static_files(&PathBuf::from("/snapshot"), &PathBuf::from("/target"))
            .await;
        assert!(restore_result.is_ok());
        assert!(restore_result.unwrap().is_empty());
    }

    /// Test AdvancedMockCore constructor variations
    /// Verifies different construction patterns work correctly
    #[test]
    fn test_advanced_mock_core_constructors() {
        // Test default constructor
        let default_core = AdvancedMockCore::new();
        assert_eq!(default_core.files.len(), 2);

        // Test with custom files
        let custom_files = vec![PathBuf::from("/custom1"), PathBuf::from("/custom2")];
        let custom_core = AdvancedMockCore::new().with_files(custom_files.clone());
        assert_eq!(custom_core.files, custom_files);

        // Test with custom ignore patterns
        let ignore_patterns = vec!["*.bak".to_string(), "*.old".to_string()];
        let ignore_core = AdvancedMockCore::new().with_ignore_patterns(ignore_patterns.clone());
        assert_eq!(ignore_core.ignore_patterns, ignore_patterns);
    }

    /// Test AdvancedMockCore path expansion edge cases
    /// Verifies path expansion handles various edge cases correctly
    #[test]
    fn test_advanced_mock_core_path_expansion_edge_cases() {
        let core = AdvancedMockCore::new();

        // Test empty path
        let empty_result = core.expand_path("");
        assert!(empty_result.is_ok());
        assert_eq!(empty_result.unwrap(), PathBuf::from(""));

        // Test path with spaces
        let space_result = core.expand_path("/path with spaces");
        assert!(space_result.is_ok());
        assert_eq!(space_result.unwrap(), PathBuf::from("/path with spaces"));

        // Test relative path
        let relative_result = core.expand_path("relative/path");
        assert!(relative_result.is_ok());
        assert_eq!(relative_result.unwrap(), PathBuf::from("relative/path"));
    }

    /// Test AdvancedMockCore comprehensive functionality
    /// Verifies all advanced features work together correctly
    #[tokio::test]
    async fn test_advanced_mock_core_comprehensive_functionality() {
        let test_files = vec![
            PathBuf::from("/advanced/file1.txt"),
            PathBuf::from("/advanced/file2.txt"),
        ];
        let ignore_patterns = vec!["*.tmp".to_string(), "*.backup".to_string()];
        let restore_results = vec![
            PathBuf::from("/restored/file1.txt"),
            PathBuf::from("/restored/file2.txt"),
        ];

        let core = AdvancedMockCore::new()
            .with_files(test_files.clone())
            .with_ignore_patterns(ignore_patterns.clone())
            .with_restore_results(restore_results.clone());

        // Test that configuration was applied
        assert_eq!(core.files, test_files);
        assert_eq!(core.ignore_patterns, ignore_patterns);
        assert_eq!(core.restore_results, restore_results);

        // Test functionality with configuration
        let config_result = core.read_config(None).await.unwrap();
        assert_eq!(config_result, test_files);

        let restore_result = core
            .restore_static_files(&PathBuf::from("/snapshot"), &PathBuf::from("/target"))
            .await
            .unwrap();
        assert_eq!(restore_result, restore_results);
    }

    /// Test MinimalStaticFilesCore comprehensive functionality
    /// Verifies minimal implementation handles all scenarios correctly
    #[tokio::test]
    async fn test_minimal_core_comprehensive_functionality() {
        let core = MinimalStaticFilesCore;

        // Test all sync methods return appropriate defaults
        assert_eq!(core.icon(), "📁");
        assert!(core.get_ignore_patterns(None).is_empty());
        assert!(!core.should_ignore(&PathBuf::from("any_file"), &["*.tmp".to_string()]));
        assert_eq!(
            core.expand_path("/any/path").unwrap(),
            PathBuf::from("/any/path")
        );

        // Test all async methods return appropriate results
        assert!(core.read_config(None).await.unwrap().is_empty());
        assert!(core
            .copy_files(vec![PathBuf::from("/file")], &PathBuf::from("/static"), &[])
            .await
            .unwrap()
            .contains("total_files"));
        assert!(core
            .restore_static_files(&PathBuf::from("/snapshot"), &PathBuf::from("/target"))
            .await
            .unwrap()
            .is_empty());

        // Test with plugin
        let plugin = StaticFilesPlugin::new(core);
        let execution_result = plugin.execute().await;
        assert!(execution_result.is_ok());
    }
}

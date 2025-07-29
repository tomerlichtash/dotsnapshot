//! Integration tests for StaticFilesPlugin comprehensive functionality

#[cfg(test)]
mod tests {
    use super::super::test_utils::helpers::validate_json_response;
    use super::super::test_utils::{
        create_mock_snapshot_dir, AdvancedMockCore, MinimalStaticFilesCore, MockStaticFilesCore,
    };
    use crate::config::Config;
    use crate::core::plugin::Plugin;
    use crate::plugins::core::base::static_files::{StaticFilesCore, StaticFilesPlugin};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::fs;

    /// Test StaticFilesCore trait comprehensive coverage
    /// Verifies all trait methods work correctly with various configurations
    #[tokio::test]
    async fn test_static_files_core_trait_comprehensive_coverage() {
        let core = AdvancedMockCore::new();

        // Test read_config with actual config
        let config = Arc::new(Config::default());
        let config_result = core.read_config(Some(&config)).await.unwrap();
        assert_eq!(config_result.len(), 2);

        // Test read_config with None
        let config_result_none = core.read_config(None).await.unwrap();
        assert_eq!(config_result_none.len(), 2);

        // Test get_ignore_patterns with config
        let patterns_with_config = core.get_ignore_patterns(Some(&config));
        assert_eq!(patterns_with_config.len(), 2); // *.tmp, *.log

        // Test get_ignore_patterns with None
        let patterns_none = core.get_ignore_patterns(None);
        assert_eq!(patterns_none.len(), 2);

        // Test should_ignore with empty patterns
        assert!(!core.should_ignore(&PathBuf::from("test.txt"), &[]));

        // Test expand_path with various paths
        assert_eq!(
            core.expand_path("/absolute/path").unwrap(),
            PathBuf::from("/absolute/path")
        );
        assert_eq!(
            core.expand_path("relative/path").unwrap(),
            PathBuf::from("relative/path")
        );

        // Test copy_files with empty list
        let copy_empty = core
            .copy_files(vec![], &PathBuf::from("/static"), &[])
            .await
            .unwrap();
        assert!(copy_empty.contains("total_files"));

        // Test restore_static_files with different paths
        let restore_result = core
            .restore_static_files(&PathBuf::from("/snapshot1"), &PathBuf::from("/target1"))
            .await
            .unwrap();
        assert_eq!(restore_result.len(), 3); // Should be 3 because path contains "snapshot"

        let restore_result2 = core
            .restore_static_files(&PathBuf::from("/snapshot2"), &PathBuf::from("/target2"))
            .await
            .unwrap();
        assert_eq!(restore_result2.len(), 3); // Should be 3 because path contains "snapshot"
    }

    /// Test StaticFilesPlugin trait methods comprehensively
    /// Verifies complete Plugin trait implementation functionality
    #[tokio::test]
    async fn test_static_files_plugin_trait_comprehensive() {
        let plugin = StaticFilesPlugin::new(AdvancedMockCore::new());

        // Test all Plugin trait methods
        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );
        assert_eq!(plugin.icon(), "🔧");
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert!(plugin.creates_own_output_files());
        assert_eq!(plugin.get_restore_target_dir(), None);

        let default_target = plugin.get_default_restore_target_dir().unwrap();
        assert_eq!(default_target, PathBuf::from("/"));

        let validation_result = plugin.validate().await;
        assert!(validation_result.is_ok());

        let execution_result = plugin.execute().await;
        assert!(execution_result.is_ok());

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");
        let static_dir = snapshot_dir.join("static");

        fs::create_dir_all(&static_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();
        fs::write(static_dir.join("file.txt"), "content")
            .await
            .unwrap();

        let restore_result = plugin.restore(&snapshot_dir, &target_dir, false).await;
        assert!(restore_result.is_ok());
    }

    /// Test plugin with additional comprehensive scenarios
    /// Verifies plugin handles complex real-world usage patterns
    #[tokio::test]
    async fn test_static_files_plugin_additional_scenarios() {
        let plugin = StaticFilesPlugin::new(AdvancedMockCore::new());

        // Test validation
        let validation_result = plugin.validate().await;
        assert!(validation_result.is_ok());

        // Test plugin trait methods
        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );
        assert_eq!(plugin.icon(), "🔧");
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert_eq!(plugin.get_restore_target_dir(), None);
        assert!(plugin.creates_own_output_files());

        let default_target = plugin.get_default_restore_target_dir().unwrap();
        assert_eq!(default_target, PathBuf::from("/"));

        // Test execution with files
        let exec_result = plugin.execute().await.unwrap();
        assert!(exec_result.contains("STATIC_DIR_CHECKSUM:"));
        assert!(exec_result.contains("total_files"));
    }

    /// Test execution with existing static directory integration
    /// Verifies complete workflow when static directory already exists
    #[tokio::test]
    async fn test_static_files_execute_with_existing_static_dir_integration() {
        let plugin = StaticFilesPlugin::new(AdvancedMockCore::new());

        // Set environment variable to control static directory location
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let static_dir = snapshot_dir.join("static");
        fs::create_dir_all(&static_dir).await.unwrap();

        // Create some test files in static directory
        fs::write(static_dir.join("test1.txt"), "content1")
            .await
            .unwrap();
        fs::write(static_dir.join("test2.txt"), "content2")
            .await
            .unwrap();

        std::env::set_var("DOTSNAPSHOT_SNAPSHOT_DIR", snapshot_dir);

        let result = plugin.execute().await.unwrap();

        // Should contain checksum and file information
        assert!(result.contains("STATIC_DIR_CHECKSUM:"));
        assert!(result.contains("directory_checksum"));
        assert!(validate_json_response(&result));

        // Clean up environment variable
        std::env::remove_var("DOTSNAPSHOT_SNAPSHOT_DIR");
    }

    /// Test plugin with multiple mock configurations integration
    /// Verifies plugin works correctly across different mock implementations
    #[tokio::test]
    async fn test_static_files_plugin_multiple_mock_configurations_integration() {
        // Test with minimal mock
        let minimal_plugin = StaticFilesPlugin::new(MinimalStaticFilesCore);
        let minimal_validation = minimal_plugin.validate().await;
        assert!(minimal_validation.is_ok());
        let minimal_execution = minimal_plugin.execute().await;
        assert!(minimal_execution.is_ok());

        // Test with advanced mock
        let advanced_plugin = StaticFilesPlugin::new(AdvancedMockCore::new());
        let advanced_validation = advanced_plugin.validate().await;
        assert!(advanced_validation.is_ok());
        let advanced_execution = advanced_plugin.execute().await;
        assert!(advanced_execution.is_ok());

        // Test with basic mock
        let basic_plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());
        let basic_validation = basic_plugin.validate().await;
        assert!(basic_validation.is_ok());
        let basic_execution = basic_plugin.execute().await;
        assert!(basic_execution.is_ok());
    }

    /// Test plugin snapshot directory scenarios integration
    /// Verifies complete snapshot directory workflow scenarios
    #[tokio::test]
    async fn test_static_files_plugin_snapshot_dir_scenarios_integration() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = create_mock_snapshot_dir(&temp_dir).await;
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir).await.unwrap();

        // Test with MockStaticFilesCore
        {
            let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());
            let validation_result = plugin.validate().await;
            assert!(validation_result.is_ok());
            let execution_result = plugin.execute().await;
            assert!(execution_result.is_ok());
            let restore_result = plugin.restore(&snapshot_dir, &target_dir, false).await;
            assert!(restore_result.is_ok());
            let dry_run_result = plugin.restore(&snapshot_dir, &target_dir, true).await;
            assert!(dry_run_result.is_ok());
        }

        // Test with AdvancedMockCore
        {
            let plugin = StaticFilesPlugin::new(AdvancedMockCore::new());
            let validation_result = plugin.validate().await;
            assert!(validation_result.is_ok());
            let execution_result = plugin.execute().await;
            assert!(execution_result.is_ok());
            let restore_result = plugin.restore(&snapshot_dir, &target_dir, false).await;
            assert!(restore_result.is_ok());
            let dry_run_result = plugin.restore(&snapshot_dir, &target_dir, true).await;
            assert!(dry_run_result.is_ok());
        }

        // Test with MinimalStaticFilesCore
        {
            let plugin = StaticFilesPlugin::new(MinimalStaticFilesCore);
            let validation_result = plugin.validate().await;
            assert!(validation_result.is_ok());
            let execution_result = plugin.execute().await;
            assert!(execution_result.is_ok());
            let restore_result = plugin.restore(&snapshot_dir, &target_dir, false).await;
            assert!(restore_result.is_ok());
            let dry_run_result = plugin.restore(&snapshot_dir, &target_dir, true).await;
            assert!(dry_run_result.is_ok());
        }
    }

    /// Test complex path scenarios integration
    /// Verifies plugin handles complex path configurations end-to-end
    #[tokio::test]
    async fn test_static_files_plugin_complex_paths_integration() {
        use crate::config::StaticFilesConfig;

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

        let plugin = StaticFilesPlugin::with_config(AdvancedMockCore::new(), config);

        // Test complete workflow with complex paths
        let validation_result = plugin.validate().await;
        assert!(validation_result.is_ok());

        let execution_result = plugin.execute().await;
        assert!(execution_result.is_ok());

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = create_mock_snapshot_dir(&temp_dir).await;
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir).await.unwrap();

        let restore_result = plugin.restore(&snapshot_dir, &target_dir, false).await;
        assert!(restore_result.is_ok());
    }

    /// Test advanced mock core functionality integration
    /// Verifies complete AdvancedMockCore functionality in realistic scenarios
    #[tokio::test]
    async fn test_advanced_mock_core_functionality_integration() {
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

        let plugin = StaticFilesPlugin::new(core);

        // Test complete plugin workflow with configured advanced mock
        let validation_result = plugin.validate().await;
        assert!(validation_result.is_ok());

        let execution_result = plugin.execute().await.unwrap();
        assert!(execution_result.contains("total_files"));
        assert!(validate_json_response(&execution_result));

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = create_mock_snapshot_dir(&temp_dir).await;
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir).await.unwrap();

        let restore_result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(restore_result, restore_results);
    }

    /// Test minimal core functionality integration
    /// Verifies complete MinimalStaticFilesCore functionality in realistic scenarios
    #[tokio::test]
    async fn test_minimal_core_functionality_integration() {
        let core = MinimalStaticFilesCore;
        let plugin = StaticFilesPlugin::new(core);

        // Test complete plugin workflow with minimal core
        let validation_result = plugin.validate().await;
        assert!(validation_result.is_ok());

        let execution_result = plugin.execute().await.unwrap();
        assert!(execution_result.contains("total_files"));
        assert!(validate_json_response(&execution_result));

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = create_mock_snapshot_dir(&temp_dir).await;
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir).await.unwrap();

        let restore_result = plugin.restore(&snapshot_dir, &target_dir, false).await;
        assert!(restore_result.is_ok());
        assert!(restore_result.unwrap().is_empty());

        // Test dry-run mode
        let dry_run_result = plugin.restore(&snapshot_dir, &target_dir, true).await;
        assert!(dry_run_result.is_ok());
    }

    /// Test end-to-end workflow integration
    /// Verifies complete plugin lifecycle from creation to restoration
    #[tokio::test]
    async fn test_end_to_end_workflow_integration() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let static_dir = snapshot_dir.join("static");
        let target_dir = temp_dir.path().join("target");

        // Create realistic directory structure
        fs::create_dir_all(&static_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test files
        fs::write(static_dir.join("config.json"), r#"{"setting": "value"}"#)
            .await
            .unwrap();
        fs::write(static_dir.join("data.txt"), "important data")
            .await
            .unwrap();
        fs::write(static_dir.join("script.sh"), "#!/bin/bash\necho hello")
            .await
            .unwrap();

        let plugin = StaticFilesPlugin::new(AdvancedMockCore::new());

        // Step 1: Validate plugin setup
        let validation_result = plugin.validate().await;
        assert!(
            validation_result.is_ok(),
            "Plugin validation should succeed"
        );

        // Step 2: Execute plugin (simulate snapshot creation)
        std::env::set_var("DOTSNAPSHOT_SNAPSHOT_DIR", &snapshot_dir);
        let execution_result = plugin.execute().await;
        assert!(execution_result.is_ok(), "Plugin execution should succeed");

        let exec_output = execution_result.unwrap();
        assert!(
            exec_output.contains("total_files"),
            "Execution should report file count"
        );
        assert!(
            validate_json_response(&exec_output),
            "Execution should return valid JSON"
        );

        // Step 3: Restore files (simulate restoration)
        let restore_result = plugin.restore(&snapshot_dir, &target_dir, false).await;
        assert!(restore_result.is_ok(), "Plugin restoration should succeed");

        let restored_files = restore_result.unwrap();
        assert!(
            !restored_files.is_empty(),
            "Restoration should return file paths"
        );

        // Step 4: Test dry-run restoration
        let dry_run_result = plugin.restore(&snapshot_dir, &target_dir, true).await;
        assert!(dry_run_result.is_ok(), "Dry-run restoration should succeed");

        // Clean up environment variable
        std::env::remove_var("DOTSNAPSHOT_SNAPSHOT_DIR");
    }

    /// Test multi-core integration scenarios
    /// Verifies plugin works correctly when switching between different core implementations
    #[tokio::test]
    async fn test_multi_core_integration_scenarios() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = create_mock_snapshot_dir(&temp_dir).await;
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir).await.unwrap();

        // Test MockStaticFilesCore
        {
            let core_name = "MockStaticFilesCore";
            let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());

            let validation = plugin.validate().await;
            assert!(validation.is_ok(), "{core_name} validation should succeed");

            let execution = plugin.execute().await;
            assert!(execution.is_ok(), "{core_name} execution should succeed");

            let exec_result = execution.unwrap();
            assert!(
                exec_result.contains("total_files"),
                "{core_name} should report total_files"
            );
            assert!(
                validate_json_response(&exec_result),
                "{core_name} should return valid JSON"
            );

            let restoration = plugin.restore(&snapshot_dir, &target_dir, false).await;
            assert!(
                restoration.is_ok(),
                "{core_name} restoration should succeed"
            );

            let dry_run = plugin.restore(&snapshot_dir, &target_dir, true).await;
            assert!(dry_run.is_ok(), "{core_name} dry-run should succeed");
        }

        // Test AdvancedMockCore
        {
            let core_name = "AdvancedMockCore";
            let plugin = StaticFilesPlugin::new(AdvancedMockCore::new());

            let validation = plugin.validate().await;
            assert!(validation.is_ok(), "{core_name} validation should succeed");

            let execution = plugin.execute().await;
            assert!(execution.is_ok(), "{core_name} execution should succeed");

            let exec_result = execution.unwrap();
            assert!(
                exec_result.contains("total_files"),
                "{core_name} should report total_files"
            );
            assert!(
                validate_json_response(&exec_result),
                "{core_name} should return valid JSON"
            );

            let restoration = plugin.restore(&snapshot_dir, &target_dir, false).await;
            assert!(
                restoration.is_ok(),
                "{core_name} restoration should succeed"
            );

            let dry_run = plugin.restore(&snapshot_dir, &target_dir, true).await;
            assert!(dry_run.is_ok(), "{core_name} dry-run should succeed");
        }

        // Test MinimalStaticFilesCore
        {
            let core_name = "MinimalStaticFilesCore";
            let plugin = StaticFilesPlugin::new(MinimalStaticFilesCore);

            let validation = plugin.validate().await;
            assert!(validation.is_ok(), "{core_name} validation should succeed");

            let execution = plugin.execute().await;
            assert!(execution.is_ok(), "{core_name} execution should succeed");

            let exec_result = execution.unwrap();
            assert!(
                exec_result.contains("total_files"),
                "{core_name} should report total_files"
            );
            assert!(
                validate_json_response(&exec_result),
                "{core_name} should return valid JSON"
            );

            let restoration = plugin.restore(&snapshot_dir, &target_dir, false).await;
            assert!(
                restoration.is_ok(),
                "{core_name} restoration should succeed"
            );

            let dry_run = plugin.restore(&snapshot_dir, &target_dir, true).await;
            assert!(dry_run.is_ok(), "{core_name} dry-run should succeed");
        }
    }
}

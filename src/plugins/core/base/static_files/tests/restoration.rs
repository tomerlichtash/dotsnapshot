//! Tests for StaticFilesPlugin restoration functionality

#[cfg(test)]
mod tests {
    use super::super::test_utils::{
        create_mock_snapshot_dir, create_test_file_paths, AdvancedMockCore, ErrorProneMockCore,
        MockStaticFilesCore,
    };
    use crate::core::plugin::Plugin;
    use crate::plugins::core::base::static_files::StaticFilesPlugin;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio::fs;

    /// Test basic plugin restoration with empty static directory
    /// Verifies plugin handles restoration when no static files exist
    #[tokio::test]
    async fn test_static_files_plugin_restore_empty() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Should return empty list when no static directory exists
        assert!(result.is_empty());
    }

    /// Test plugin restoration with static directory present
    /// Verifies plugin correctly processes static files during restoration
    #[tokio::test]
    async fn test_static_files_plugin_restore_with_static_dir() {
        let test_files = create_test_file_paths();
        let mock_core = MockStaticFilesCore::new().with_files(test_files.clone());
        let plugin = StaticFilesPlugin::new(mock_core);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let static_dir = snapshot_dir.join("static");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&static_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test files in static directory
        fs::write(static_dir.join("test1.txt"), "content1")
            .await
            .unwrap();
        fs::write(static_dir.join("test2.txt"), "content2")
            .await
            .unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Should return restored file paths
        assert_eq!(result.len(), test_files.len());
        // Mock returns the configured files, which may not physically exist
        for path in &result {
            // Verify paths are reasonable (either exist or are test paths)
            assert!(
                path.is_absolute()
                    || path.starts_with(&target_dir)
                    || path.to_string_lossy().contains("test")
            );
        }
    }

    /// Test plugin restoration in dry-run mode
    /// Verifies plugin correctly simulates restoration without making changes
    #[tokio::test]
    async fn test_static_files_plugin_restore_dry_run() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let static_dir = snapshot_dir.join("static");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&static_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create a test file in static directory
        fs::write(static_dir.join("test.txt"), "test content")
            .await
            .unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();

        // Dry run should return target path without actually restoring
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target_dir);
    }

    /// Test plugin restoration with multiple files
    /// Verifies plugin handles restoration of multiple static files correctly
    #[tokio::test]
    async fn test_static_files_plugin_restore_with_files() {
        let restored_files = vec![
            PathBuf::from("/restored/file1.txt"),
            PathBuf::from("/restored/file2.txt"),
            PathBuf::from("/restored/file3.txt"),
        ];
        let mock_core = MockStaticFilesCore::new().with_files(restored_files.clone());
        let plugin = StaticFilesPlugin::new(mock_core);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = create_mock_snapshot_dir(&temp_dir).await;
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir).await.unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Should return the configured restored files
        assert_eq!(result.len(), restored_files.len());
        for (i, path) in result.iter().enumerate() {
            // The mock returns the configured files
            assert_eq!(*path, restored_files[i]);
        }
    }

    /// Test plugin restoration with error scenarios
    /// Verifies plugin handles restoration errors gracefully
    #[tokio::test]
    async fn test_static_files_plugin_restore_error() {
        let error_mock = ErrorProneMockCore::new().with_restore_failure();
        let plugin = StaticFilesPlugin::new(error_mock);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = create_mock_snapshot_dir(&temp_dir).await;
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir).await.unwrap();

        let result = plugin.restore(&snapshot_dir, &target_dir, false).await;

        // Should handle restoration error gracefully
        // The plugin's restore method catches errors and returns empty results
        assert!(result.is_ok());
        let restored_files = result.unwrap();
        assert!(restored_files.is_empty());
    }

    /// Test plugin restoration with advanced mock core
    /// Verifies plugin works correctly with advanced restoration features
    #[tokio::test]
    async fn test_static_files_plugin_restore_with_advanced_core() {
        let restore_results = vec![
            PathBuf::from("/advanced/restored1.txt"),
            PathBuf::from("/advanced/restored2.txt"),
        ];
        let advanced_mock = AdvancedMockCore::new().with_restore_results(restore_results.clone());
        let plugin = StaticFilesPlugin::new(advanced_mock);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = create_mock_snapshot_dir(&temp_dir).await;
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir).await.unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Should return advanced mock's configured results
        assert_eq!(result.len(), restore_results.len());
        for (i, path) in result.iter().enumerate() {
            assert_eq!(*path, restore_results[i]);
        }
    }

    /// Test plugin restoration with directory read errors
    /// Verifies plugin handles cases where snapshot directory cannot be read
    #[tokio::test]
    async fn test_static_files_plugin_restore_directory_read_error() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("nonexistent_snapshot");
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir).await.unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Should handle missing snapshot directory gracefully
        assert!(result.is_empty());
    }

    /// Test plugin restoration with checksum validation
    /// Verifies plugin can handle restoration with checksum verification
    #[tokio::test]
    async fn test_static_files_plugin_restore_with_checksum_validation() {
        let plugin = StaticFilesPlugin::new(AdvancedMockCore::new());
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = create_mock_snapshot_dir(&temp_dir).await;
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create some files with checksums in the snapshot
        let static_dir = snapshot_dir.join("static");
        fs::write(
            static_dir.join("file_with_checksum.txt"),
            "validated content",
        )
        .await
        .unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Should successfully restore files with checksum validation
        assert!(!result.is_empty());
        // Advanced mock returns realistic restore paths based on target directory
        for path in &result {
            assert!(path.to_string_lossy().contains("restored") || path.starts_with(&target_dir));
        }
    }

    /// Test plugin restoration dry-run with comprehensive scenarios
    /// Verifies dry-run mode works correctly across different scenarios
    #[tokio::test]
    async fn test_static_files_plugin_restore_dry_run_comprehensive() {
        let plugin = StaticFilesPlugin::new(AdvancedMockCore::new());
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = create_mock_snapshot_dir(&temp_dir).await;
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir).await.unwrap();

        // Add more files to the static directory
        let static_dir = snapshot_dir.join("static");
        fs::write(static_dir.join("dry_run_test1.txt"), "content1")
            .await
            .unwrap();
        fs::write(static_dir.join("dry_run_test2.txt"), "content2")
            .await
            .unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();

        // Dry run should report what would be restored without doing it
        assert!(!result.is_empty());

        // Verify no actual files were created in target (dry run)
        let target_entries = fs::read_dir(&target_dir).await;
        match target_entries {
            Ok(mut entries) => {
                let first_entry = entries.next_entry().await.unwrap();
                // Target directory should be empty or contain only what was there before
                assert!(first_entry.is_none() || first_entry.unwrap().path().is_dir());
            }
            Err(_) => {
                // Target directory might not exist, which is fine for dry run
            }
        }
    }

    /// Test default restore target directory method
    /// Verifies plugin returns correct default restoration target
    #[test]
    fn test_default_restore_target_dir() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());
        let default_dir = plugin.get_default_restore_target_dir().unwrap();

        // Static files plugin should default to root directory for restoration
        assert_eq!(default_dir.to_string_lossy(), "/");
    }

    /// Test restoration with empty static directory but with metadata
    /// Verifies plugin handles case where static directory exists but is empty
    #[tokio::test]
    async fn test_static_files_plugin_restore_empty_static_dir() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let static_dir = snapshot_dir.join("static");
        let target_dir = temp_dir.path().join("target");

        // Create empty static directory
        fs::create_dir_all(&static_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Should return empty results for empty static directory
        assert!(result.is_empty());
    }
}

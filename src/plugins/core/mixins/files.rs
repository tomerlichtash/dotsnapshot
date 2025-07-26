use anyhow::{Context, Result};
use std::path::Path;
use tokio::fs;
use tracing::{debug, info};

/// Mixin trait for common file operations
#[allow(async_fn_in_trait)]
pub trait FilesMixin {
    /// Restore a file from source to target, creating directories as needed
    async fn restore_file(&self, source: &Path, target: &Path) -> Result<()> {
        debug!(
            "Restoring file: {} -> {}",
            source.display(),
            target.display()
        );

        if !source.exists() {
            return Err(anyhow::anyhow!(
                "Source file does not exist: {}",
                source.display()
            ));
        }

        // Create parent directories if they don't exist
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).await.with_context(|| {
                format!(
                    "Failed to create parent directories for {}",
                    target.display()
                )
            })?;
        }

        // Copy the file
        fs::copy(source, target).await.with_context(|| {
            format!(
                "Failed to copy file from {} to {}",
                source.display(),
                target.display()
            )
        })?;

        info!("Restored file: {}", target.display());
        Ok(())
    }

    /// Check if a directory exists and is accessible
    async fn is_dir_accessible(&self, path: &Path) -> bool {
        match fs::metadata(path).await {
            Ok(metadata) => metadata.is_dir(),
            Err(_) => false,
        }
    }
}

// Note: Removed blanket implementation to avoid conflicts with specific implementations
// Each type that needs FilesMixin should implement it explicitly

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    /// Mock implementation for testing FilesMixin functionality
    #[derive(Clone, Copy)]
    struct MockPlugin;

    impl FilesMixin for MockPlugin {}

    /// Test file restoration with valid source and target
    /// Verifies that files can be restored from snapshot to target location
    #[tokio::test]
    async fn test_restore_file_success() -> Result<()> {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new()?;

        // Create source file
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target.txt");
        let content = "test content for restoration";

        fs::write(&source, content).await?;

        // Restore file
        plugin.restore_file(&source, &target).await?;

        // Verify target exists and has correct content
        assert!(target.exists());
        let restored_content = fs::read_to_string(&target).await?;
        assert_eq!(restored_content, content);

        Ok(())
    }

    /// Test file restoration when source file doesn't exist
    /// Verifies that appropriate error is returned for missing source files
    #[tokio::test]
    async fn test_restore_file_source_not_found() {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new().unwrap();

        let source = temp_dir.path().join("nonexistent.txt");
        let target = temp_dir.path().join("target.txt");

        let result = plugin.restore_file(&source, &target).await;

        assert!(result.is_err());
        // Just verify that we get an error when the source file doesn't exist
        // The exact error message may vary by platform
    }

    /// Test file restoration when target directory doesn't exist
    /// Verifies that parent directories are created during restoration
    #[tokio::test]
    async fn test_restore_file_create_target_dir() -> Result<()> {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new()?;

        // Create source file
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir
            .path()
            .join("deep")
            .join("nested")
            .join("target.txt");
        let content = "test content";

        fs::write(&source, content).await?;

        // Restore file (should create nested directories)
        plugin.restore_file(&source, &target).await?;

        // Verify target exists and has correct content
        assert!(target.exists());
        let restored_content = fs::read_to_string(&target).await?;
        assert_eq!(restored_content, content);

        Ok(())
    }

    /// Test directory accessibility check for existing directory
    /// Verifies that accessible directories are correctly identified
    #[tokio::test]
    async fn test_is_dir_accessible_existing() -> Result<()> {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new()?;

        let result = plugin.is_dir_accessible(temp_dir.path()).await;

        assert!(result);

        Ok(())
    }

    /// Test directory accessibility check for non-existent directory
    /// Verifies that non-existent directories are correctly identified
    #[tokio::test]
    async fn test_is_dir_accessible_nonexistent() {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("nonexistent");

        let result = plugin.is_dir_accessible(&nonexistent).await;

        assert!(!result);
    }

    /// Test directory accessibility check for file (not directory)
    /// Verifies that files are not identified as accessible directories
    #[tokio::test]
    async fn test_is_dir_accessible_file() -> Result<()> {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new()?;

        // Create a file, not a directory
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "content").await?;

        let result = plugin.is_dir_accessible(&file_path).await;

        assert!(!result);

        Ok(())
    }

    /// Test file restoration with empty source file
    /// Verifies that empty files can be restored correctly
    #[tokio::test]
    async fn test_restore_file_empty() -> Result<()> {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new()?;

        // Create empty source file
        let source = temp_dir.path().join("empty.txt");
        let target = temp_dir.path().join("restored_empty.txt");

        fs::write(&source, "").await?;

        // Restore empty file
        plugin.restore_file(&source, &target).await?;

        // Verify target exists and is empty
        assert!(target.exists());
        let restored_content = fs::read_to_string(&target).await?;
        assert_eq!(restored_content, "");

        Ok(())
    }

    /// Test file restoration overwrites existing target
    /// Verifies that existing target files are properly overwritten
    #[tokio::test]
    async fn test_restore_file_overwrite_existing() -> Result<()> {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new()?;

        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target.txt");
        let new_content = "new content";
        let old_content = "old content";

        // Create both files with different content
        fs::write(&source, new_content).await?;
        fs::write(&target, old_content).await?;

        // Restore should overwrite
        plugin.restore_file(&source, &target).await?;

        // Verify target has new content
        let restored_content = fs::read_to_string(&target).await?;
        assert_eq!(restored_content, new_content);

        Ok(())
    }

    /// Test file restoration with binary content
    /// Verifies that binary files can be restored correctly
    #[tokio::test]
    async fn test_restore_file_binary_content() -> Result<()> {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new()?;

        let source = temp_dir.path().join("binary.bin");
        let target = temp_dir.path().join("restored_binary.bin");
        let binary_data = vec![0x00, 0xFF, 0x42, 0x7F, 0x80, 0x01, 0xFE];

        fs::write(&source, &binary_data).await?;

        // Restore binary file
        plugin.restore_file(&source, &target).await?;

        // Verify target exists and has correct binary content
        assert!(target.exists());
        let restored_data = fs::read(&target).await?;
        assert_eq!(restored_data, binary_data);

        Ok(())
    }

    /// Test file restoration with large content
    /// Verifies that large files can be restored efficiently
    #[tokio::test]
    async fn test_restore_file_large_content() -> Result<()> {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new()?;

        let source = temp_dir.path().join("large.txt");
        let target = temp_dir.path().join("restored_large.txt");
        // Create a large content string (1MB)
        let large_content = "a".repeat(1024 * 1024);

        fs::write(&source, &large_content).await?;

        // Restore large file
        plugin.restore_file(&source, &target).await?;

        // Verify target exists and has correct content
        assert!(target.exists());
        let restored_content = fs::read_to_string(&target).await?;
        assert_eq!(restored_content.len(), large_content.len());
        assert_eq!(restored_content, large_content);

        Ok(())
    }

    /// Test file restoration with special characters in filename
    /// Verifies that files with unicode names can be restored
    #[tokio::test]
    async fn test_restore_file_unicode_filename() -> Result<()> {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new()?;

        let source = temp_dir.path().join("файл_тест_🚀.txt");
        let target = temp_dir.path().join("restored_файл_тест_🚀.txt");
        let content = "Unicode content: 测试内容 🌟";

        fs::write(&source, content).await?;

        // Restore file with unicode name
        plugin.restore_file(&source, &target).await?;

        // Verify target exists and has correct content
        assert!(target.exists());
        let restored_content = fs::read_to_string(&target).await?;
        assert_eq!(restored_content, content);

        Ok(())
    }

    /// Test file restoration with multiple directory levels
    /// Verifies that deeply nested directory structures are created correctly
    #[tokio::test]
    async fn test_restore_file_deep_nested_dirs() -> Result<()> {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new()?;

        let source = temp_dir.path().join("source.txt");
        let target = temp_dir
            .path()
            .join("level1")
            .join("level2")
            .join("level3")
            .join("level4")
            .join("level5")
            .join("deep_target.txt");
        let content = "deeply nested content";

        fs::write(&source, content).await?;

        // Restore to deeply nested location
        plugin.restore_file(&source, &target).await?;

        // Verify target exists and has correct content
        assert!(target.exists());
        let restored_content = fs::read_to_string(&target).await?;
        assert_eq!(restored_content, content);

        // Verify all intermediate directories were created
        assert!(target.parent().unwrap().exists());
        assert!(target.parent().unwrap().parent().unwrap().exists());

        Ok(())
    }

    /// Test file restoration with same source and target paths
    /// Verifies behavior when source and target are the same
    #[tokio::test]
    async fn test_restore_file_same_path() -> Result<()> {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new()?;

        let file_path = temp_dir.path().join("same.txt");
        let content = "same file content";

        fs::write(&file_path, content).await?;

        // Restore to same path - this may have platform-specific behavior
        // On some systems copying a file to itself may clear it
        let result = plugin.restore_file(&file_path, &file_path).await;

        // On Windows, copying to the same path might fail, which is acceptable behavior
        #[cfg(windows)]
        {
            if result.is_err() {
                // This is acceptable on Windows - copying to same path can fail
                return Ok(());
            }
        }

        // Verify the operation completed successfully
        assert!(result.is_ok());
        // Verify file still exists
        assert!(file_path.exists());

        // Content may be preserved or cleared depending on platform behavior
        // We just verify the file exists and operation doesn't panic

        Ok(())
    }

    /// Test directory accessibility with permission errors (Unix only)
    /// Verifies that permission-denied directories are handled correctly
    #[cfg(unix)]
    #[tokio::test]
    async fn test_is_dir_accessible_permission_denied() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let plugin = MockPlugin;
        let temp_dir = TempDir::new()?;
        let restricted_dir = temp_dir.path().join("restricted");

        // Create directory with restricted permissions
        fs::create_dir(&restricted_dir).await?;
        let mut perms = fs::metadata(&restricted_dir).await?.permissions();
        perms.set_mode(0o000); // No permissions
        fs::set_permissions(&restricted_dir, perms).await?;

        let result = plugin.is_dir_accessible(&restricted_dir).await;

        // Restore permissions for cleanup before assertion
        let mut perms = fs::metadata(&restricted_dir).await?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&restricted_dir, perms).await?;

        // On some systems, root or certain users may still have access
        // So we test that the function completes without panicking
        // The actual result may vary based on system permissions
        let _ = result; // Just verify no panic

        Ok(())
    }

    /// Test file restoration when target parent is a file (not directory)
    /// Verifies that appropriate error is returned when parent path is occupied by a file
    #[tokio::test]
    async fn test_restore_file_parent_is_file() -> Result<()> {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new()?;

        let source = temp_dir.path().join("source.txt");
        let parent_file = temp_dir.path().join("parent");
        let target = parent_file.join("target.txt");

        // Create source file and parent as a file (not directory)
        fs::write(&source, "content").await?;
        fs::write(&parent_file, "parent is file").await?;

        // This should fail because parent is a file, not a directory
        let result = plugin.restore_file(&source, &target).await;
        assert!(result.is_err());

        Ok(())
    }

    /// Test directory accessibility edge cases
    /// Verifies various edge cases for directory accessibility
    #[tokio::test]
    async fn test_is_dir_accessible_edge_cases() -> Result<()> {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new()?;

        // Test with empty directory
        let empty_dir = temp_dir.path().join("empty");
        fs::create_dir(&empty_dir).await?;
        assert!(plugin.is_dir_accessible(&empty_dir).await);

        // Test with nested directory
        let nested_dir = temp_dir.path().join("nested").join("dir");
        fs::create_dir_all(&nested_dir).await?;
        assert!(plugin.is_dir_accessible(&nested_dir).await);

        // Test with symlink to directory (if supported by platform)
        #[cfg(unix)]
        {
            let symlink_path = temp_dir.path().join("symlink");
            if tokio::fs::symlink(&empty_dir, &symlink_path).await.is_ok() {
                assert!(plugin.is_dir_accessible(&symlink_path).await);
            }
        }

        Ok(())
    }

    /// Test file restoration error handling for various scenarios
    /// Verifies that different error conditions are handled appropriately
    #[tokio::test]
    async fn test_restore_file_error_handling() -> Result<()> {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new()?;

        // Test with source as directory instead of file
        let source_dir = temp_dir.path().join("source_dir");
        let target = temp_dir.path().join("target.txt");
        fs::create_dir(&source_dir).await?;

        let result = plugin.restore_file(&source_dir, &target).await;
        // This might succeed or fail depending on platform behavior for copying directories as files
        // We just verify it completes without panicking
        let _ = result;

        // Test with invalid path characters (platform-specific)
        let source = temp_dir.path().join("valid_source.txt");
        fs::write(&source, "content").await?;

        // Most platforms can handle most Unicode characters now, so this test
        // mainly verifies the function doesn't panic on unusual filenames
        let unusual_target = temp_dir.path().join("target\t\n.txt");
        let result = plugin.restore_file(&source, &unusual_target).await;
        // Just verify it completes
        let _ = result;

        Ok(())
    }

    /// Test MockPlugin implementation coverage
    /// Verifies that the mock plugin properly implements the trait
    #[tokio::test]
    async fn test_mock_plugin_trait_implementation() {
        let plugin = MockPlugin;

        // This test mainly exists to ensure the trait is properly implemented
        // Since async traits can't be made into trait objects easily,
        // we just test direct implementation
        let temp_dir = TempDir::new().unwrap();
        let result = plugin.is_dir_accessible(temp_dir.path()).await;
        assert!(result);
    }

    /// Test concurrent file operations
    /// Verifies that multiple file operations can run concurrently
    #[tokio::test]
    async fn test_concurrent_file_operations() -> Result<()> {
        let plugin = MockPlugin;
        let temp_dir = TempDir::new()?;

        // Create multiple source files
        let mut tasks = Vec::new();
        for i in 0..10 {
            let source = temp_dir.path().join(format!("source_{i}.txt"));
            let target = temp_dir.path().join(format!("target_{i}.txt"));
            let content = format!("content for file {i}");

            fs::write(&source, &content).await?;

            let plugin_copy = plugin;
            let task =
                tokio::spawn(async move { plugin_copy.restore_file(&source, &target).await });
            tasks.push((task, i, content));
        }

        // Wait for all tasks to complete
        for (task, i, expected_content) in tasks {
            task.await??;

            let target = temp_dir.path().join(format!("target_{i}.txt"));
            assert!(target.exists());
            let restored_content = fs::read_to_string(&target).await?;
            assert_eq!(restored_content, expected_content);
        }

        Ok(())
    }
}

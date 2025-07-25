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
}

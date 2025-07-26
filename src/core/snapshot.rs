use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::fs as async_fs;

use crate::core::checksum::{calculate_directory_checksum, checksums_equal};

/// Metadata for a snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub timestamp: DateTime<Utc>,
    pub version: String,
    pub checksums: HashMap<String, String>, // plugin_name -> checksum
    pub directory_checksum: String,
}

/// Manages snapshot creation and validation
pub struct SnapshotManager {
    base_path: PathBuf,
}

impl SnapshotManager {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    pub fn base_path(&self) -> &PathBuf {
        &self.base_path
    }

    /// Creates a new snapshot directory with timestamp
    pub async fn create_snapshot_dir(&self) -> Result<PathBuf> {
        let timestamp = Utc::now();
        let snapshot_name = timestamp.format("%Y%m%d_%H%M%S").to_string();
        let snapshot_dir = self.base_path.join(snapshot_name);

        async_fs::create_dir_all(&snapshot_dir)
            .await
            .context("Failed to create snapshot directory")?;

        Ok(snapshot_dir)
    }

    /// Saves snapshot metadata to the snapshot directory
    pub async fn save_metadata(
        &self,
        snapshot_dir: &Path,
        metadata: &SnapshotMetadata,
    ) -> Result<()> {
        // Create .snapshot subdirectory for metadata files
        let snapshot_meta_dir = snapshot_dir.join(".snapshot");
        async_fs::create_dir_all(&snapshot_meta_dir)
            .await
            .context("Failed to create .snapshot directory")?;

        let metadata_path = snapshot_meta_dir.join("checksum.json");
        let json = serde_json::to_string_pretty(metadata)?;

        async_fs::write(&metadata_path, json)
            .await
            .context("Failed to save snapshot metadata")?;

        Ok(())
    }

    /// Loads snapshot metadata from a snapshot directory
    pub async fn load_metadata(&self, snapshot_dir: &Path) -> Result<SnapshotMetadata> {
        // Try new location first (.snapshot/checksum.json)
        let new_metadata_path = snapshot_dir.join(".snapshot").join("checksum.json");
        let old_metadata_path = snapshot_dir.join("metadata.json");

        let metadata_path = if new_metadata_path.exists() {
            new_metadata_path
        } else if old_metadata_path.exists() {
            // Fallback to old location for backward compatibility
            old_metadata_path
        } else {
            return Err(anyhow::anyhow!("Metadata file not found"));
        };

        let json = async_fs::read_to_string(&metadata_path).await?;
        let metadata: SnapshotMetadata = serde_json::from_str(&json)?;

        Ok(metadata)
    }

    /// Finds the most recent snapshot directory excluding a specific directory
    pub fn find_latest_snapshot_excluding(&self, exclude_dir: &Path) -> Result<Option<PathBuf>> {
        if !self.base_path.exists() {
            return Ok(None);
        }

        let mut snapshots = Vec::new();

        for entry in fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() && path != exclude_dir {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    // Check if directory name matches timestamp format
                    if name.len() == 15 && name.chars().nth(8) == Some('_') {
                        snapshots.push(path);
                    }
                }
            }
        }

        // Sort by name (which is timestamp-based)
        snapshots.sort();

        Ok(snapshots.last().cloned())
    }

    /// Checks if a file with the given checksum exists in the latest snapshot
    pub async fn find_file_by_checksum(
        &self,
        plugin_name: &str,
        filename: &str,
        checksum: &str,
        exclude_dir: &Path,
    ) -> Result<Option<PathBuf>> {
        let latest_snapshot = match self.find_latest_snapshot_excluding(exclude_dir)? {
            Some(path) => path,
            None => return Ok(None),
        };

        let metadata = self.load_metadata(&latest_snapshot).await?;

        if let Some(stored_checksum) = metadata.checksums.get(plugin_name) {
            if checksums_equal(checksum, stored_checksum) {
                // Try .snapshot subdirectory first (for static plugin and other metadata files)
                let snapshot_subdir_path = latest_snapshot.join(".snapshot").join(filename);
                if snapshot_subdir_path.exists() {
                    return Ok(Some(snapshot_subdir_path));
                }

                // Fallback to root directory (for other plugins)
                let root_path = latest_snapshot.join(filename);
                if root_path.exists() {
                    return Ok(Some(root_path));
                }
            }
        }

        Ok(None)
    }

    /// Copies a file from the latest snapshot to the current snapshot
    pub async fn copy_from_latest(
        &self,
        _plugin_name: &str,
        filename: &str,
        target_dir: &Path,
    ) -> Result<bool> {
        let latest_snapshot = match self.find_latest_snapshot_excluding(target_dir)? {
            Some(path) => path,
            None => return Ok(false),
        };

        // Try .snapshot subdirectory first
        let snapshot_subdir_source = latest_snapshot.join(".snapshot").join(filename);
        let snapshot_subdir_target = target_dir.join(".snapshot").join(filename);

        if snapshot_subdir_source.exists() {
            // Create target .snapshot directory if it doesn't exist
            if let Some(parent) = snapshot_subdir_target.parent() {
                async_fs::create_dir_all(parent)
                    .await
                    .context("Failed to create .snapshot directory")?;
            }

            async_fs::copy(&snapshot_subdir_source, &snapshot_subdir_target)
                .await
                .context("Failed to copy file from latest snapshot")?;
            return Ok(true);
        }

        // Fallback to root directory
        let root_source = latest_snapshot.join(filename);
        let root_target = target_dir.join(filename);

        if root_source.exists() {
            async_fs::copy(&root_source, &root_target)
                .await
                .context("Failed to copy file from latest snapshot")?;
            return Ok(true);
        }

        Ok(false)
    }

    /// Calculates and updates the directory checksum for a snapshot
    pub async fn finalize_snapshot(&self, snapshot_dir: &Path) -> Result<()> {
        let directory_checksum = calculate_directory_checksum(snapshot_dir)?;

        // Update metadata with directory checksum
        let mut metadata = self.load_metadata(snapshot_dir).await?;
        metadata.directory_checksum = directory_checksum;

        self.save_metadata(snapshot_dir, &metadata).await?;

        Ok(())
    }

    /// Creates initial metadata for a new snapshot
    pub fn create_metadata(&self) -> SnapshotMetadata {
        SnapshotMetadata {
            timestamp: Utc::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            checksums: HashMap::new(),
            directory_checksum: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_snapshot_dir() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());

        let snapshot_dir = manager.create_snapshot_dir().await?;

        assert!(snapshot_dir.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_save_and_load_metadata() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());

        let snapshot_dir = manager.create_snapshot_dir().await?;
        let metadata = manager.create_metadata();

        manager.save_metadata(&snapshot_dir, &metadata).await?;
        let loaded_metadata = manager.load_metadata(&snapshot_dir).await?;

        assert_eq!(metadata.version, loaded_metadata.version);
        assert_eq!(metadata.checksums, loaded_metadata.checksums);

        Ok(())
    }

    /// Test loading metadata from old location (backward compatibility)
    #[tokio::test]
    async fn test_load_metadata_backward_compatibility() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());
        let snapshot_dir = manager.create_snapshot_dir().await?;

        // Create metadata in old location
        let old_metadata = SnapshotMetadata {
            timestamp: Utc::now(),
            version: "1.0.0".to_string(),
            checksums: HashMap::new(),
            directory_checksum: "old_checksum".to_string(),
        };
        let old_path = snapshot_dir.join("metadata.json");
        let json = serde_json::to_string_pretty(&old_metadata)?;
        async_fs::write(&old_path, json).await?;

        // Should load from old location
        let loaded_metadata = manager.load_metadata(&snapshot_dir).await?;
        assert_eq!(loaded_metadata.version, "1.0.0");
        assert_eq!(loaded_metadata.directory_checksum, "old_checksum");

        Ok(())
    }

    /// Test loading metadata when file doesn't exist
    #[tokio::test]
    async fn test_load_metadata_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());
        let snapshot_dir = manager.create_snapshot_dir().await.unwrap();

        // Try to load metadata that doesn't exist
        let result = manager.load_metadata(&snapshot_dir).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Metadata file not found"));
    }

    /// Test find_latest_snapshot_excluding with no snapshots
    #[test]
    fn test_find_latest_snapshot_excluding_empty() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());
        let exclude_dir = temp_dir.path().join("exclude");

        let result = manager
            .find_latest_snapshot_excluding(&exclude_dir)
            .unwrap();
        assert!(result.is_none());
    }

    /// Test find_latest_snapshot_excluding with multiple snapshots
    #[test]
    fn test_find_latest_snapshot_excluding_multiple() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());

        // Create multiple snapshot directories
        let snapshot1 = temp_dir.path().join("20240115_100000");
        let snapshot2 = temp_dir.path().join("20240116_100000");
        let snapshot3 = temp_dir.path().join("20240117_100000");
        let exclude_dir = temp_dir.path().join("20240118_100000");
        let non_snapshot = temp_dir.path().join("not_a_snapshot");

        fs::create_dir_all(&snapshot1)?;
        fs::create_dir_all(&snapshot2)?;
        fs::create_dir_all(&snapshot3)?;
        fs::create_dir_all(&exclude_dir)?;
        fs::create_dir_all(&non_snapshot)?;

        // Should return the latest snapshot excluding the specified one
        let result = manager.find_latest_snapshot_excluding(&exclude_dir)?;
        assert_eq!(result, Some(snapshot3.clone()));

        // Exclude snapshot3, should return the exclude_dir (which is actually the latest)
        let result2 = manager.find_latest_snapshot_excluding(&snapshot3)?;
        assert_eq!(result2, Some(exclude_dir));

        Ok(())
    }

    /// Test find_latest_snapshot_excluding with nonexistent base path
    #[test]
    fn test_find_latest_snapshot_excluding_nonexistent_base() {
        let manager = SnapshotManager::new(PathBuf::from("/nonexistent/path"));
        let exclude_dir = PathBuf::from("/some/dir");

        let result = manager
            .find_latest_snapshot_excluding(&exclude_dir)
            .unwrap();
        assert!(result.is_none());
    }

    /// Test find_file_by_checksum when no latest snapshot exists
    #[tokio::test]
    async fn test_find_file_by_checksum_no_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());
        let exclude_dir = temp_dir.path().join("exclude");

        let result = manager
            .find_file_by_checksum("plugin", "file.txt", "checksum123", &exclude_dir)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    /// Test find_file_by_checksum with matching checksum in .snapshot subdirectory
    #[tokio::test]
    async fn test_find_file_by_checksum_in_snapshot_subdir() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());

        // Create a snapshot with metadata
        let snapshot_dir = temp_dir.path().join("20240117_100000");
        fs::create_dir_all(&snapshot_dir)?;

        // Create .snapshot subdirectory with file
        let snapshot_subdir = snapshot_dir.join(".snapshot");
        fs::create_dir_all(&snapshot_subdir)?;
        fs::write(snapshot_subdir.join("config.json"), "test content")?;

        // Create metadata with matching checksum
        let mut checksums = HashMap::new();
        checksums.insert("static".to_string(), "abc123".to_string());
        let metadata = SnapshotMetadata {
            timestamp: Utc::now(),
            version: "1.0.0".to_string(),
            checksums,
            directory_checksum: String::new(),
        };
        manager.save_metadata(&snapshot_dir, &metadata).await?;

        // Find file by checksum
        let exclude_dir = temp_dir.path().join("exclude");
        let result = manager
            .find_file_by_checksum("static", "config.json", "abc123", &exclude_dir)
            .await?;

        assert_eq!(result, Some(snapshot_subdir.join("config.json")));

        Ok(())
    }

    /// Test find_file_by_checksum with matching checksum in root directory
    #[tokio::test]
    async fn test_find_file_by_checksum_in_root() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());

        // Create a snapshot with metadata
        let snapshot_dir = temp_dir.path().join("20240117_100000");
        fs::create_dir_all(&snapshot_dir)?;

        // Create file in root directory
        fs::write(snapshot_dir.join("data.txt"), "test content")?;

        // Create metadata with matching checksum
        let mut checksums = HashMap::new();
        checksums.insert("plugin".to_string(), "xyz789".to_string());
        let metadata = SnapshotMetadata {
            timestamp: Utc::now(),
            version: "1.0.0".to_string(),
            checksums,
            directory_checksum: String::new(),
        };
        manager.save_metadata(&snapshot_dir, &metadata).await?;

        // Find file by checksum
        let exclude_dir = temp_dir.path().join("exclude");
        let result = manager
            .find_file_by_checksum("plugin", "data.txt", "xyz789", &exclude_dir)
            .await?;

        assert_eq!(result, Some(snapshot_dir.join("data.txt")));

        Ok(())
    }

    /// Test find_file_by_checksum with non-matching checksum
    #[tokio::test]
    async fn test_find_file_by_checksum_no_match() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());

        // Create a snapshot with metadata
        let snapshot_dir = temp_dir.path().join("20240117_100000");
        fs::create_dir_all(&snapshot_dir)?;

        // Create metadata with different checksum
        let mut checksums = HashMap::new();
        checksums.insert("plugin".to_string(), "different_checksum".to_string());
        let metadata = SnapshotMetadata {
            timestamp: Utc::now(),
            version: "1.0.0".to_string(),
            checksums,
            directory_checksum: String::new(),
        };
        manager.save_metadata(&snapshot_dir, &metadata).await?;

        // Find file by checksum
        let exclude_dir = temp_dir.path().join("exclude");
        let result = manager
            .find_file_by_checksum("plugin", "data.txt", "xyz789", &exclude_dir)
            .await?;

        assert!(result.is_none());

        Ok(())
    }

    /// Test find_file_by_checksum when file doesn't exist
    #[tokio::test]
    async fn test_find_file_by_checksum_file_not_found() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());

        // Create a snapshot with metadata
        let snapshot_dir = temp_dir.path().join("20240117_100000");
        fs::create_dir_all(&snapshot_dir)?;

        // Create metadata with matching checksum but no file
        let mut checksums = HashMap::new();
        checksums.insert("plugin".to_string(), "abc123".to_string());
        let metadata = SnapshotMetadata {
            timestamp: Utc::now(),
            version: "1.0.0".to_string(),
            checksums,
            directory_checksum: String::new(),
        };
        manager.save_metadata(&snapshot_dir, &metadata).await?;

        // Find file by checksum
        let exclude_dir = temp_dir.path().join("exclude");
        let result = manager
            .find_file_by_checksum("plugin", "missing.txt", "abc123", &exclude_dir)
            .await?;

        assert!(result.is_none());

        Ok(())
    }

    /// Test copy_from_latest when no latest snapshot exists
    #[tokio::test]
    async fn test_copy_from_latest_no_snapshot() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir)?;

        let result = manager
            .copy_from_latest("plugin", "file.txt", &target_dir)
            .await?;
        assert!(!result);

        Ok(())
    }

    /// Test copy_from_latest with file in .snapshot subdirectory
    #[tokio::test]
    async fn test_copy_from_latest_snapshot_subdir() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());

        // Create source snapshot
        let source_dir = temp_dir.path().join("20240117_100000");
        let source_snapshot_dir = source_dir.join(".snapshot");
        fs::create_dir_all(&source_snapshot_dir)?;
        fs::write(source_snapshot_dir.join("config.json"), "config content")?;

        // Create target directory
        let target_dir = temp_dir.path().join("20240118_100000");
        fs::create_dir_all(&target_dir)?;

        // Copy file
        let result = manager
            .copy_from_latest("static", "config.json", &target_dir)
            .await?;
        assert!(result);

        // Verify file was copied
        let target_file = target_dir.join(".snapshot").join("config.json");
        assert!(target_file.exists());
        let content = fs::read_to_string(&target_file)?;
        assert_eq!(content, "config content");

        Ok(())
    }

    /// Test copy_from_latest with file in root directory
    #[tokio::test]
    async fn test_copy_from_latest_root_dir() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());

        // Create source snapshot
        let source_dir = temp_dir.path().join("20240117_100000");
        fs::create_dir_all(&source_dir)?;
        fs::write(source_dir.join("data.txt"), "data content")?;

        // Create target directory
        let target_dir = temp_dir.path().join("20240118_100000");
        fs::create_dir_all(&target_dir)?;

        // Copy file
        let result = manager
            .copy_from_latest("plugin", "data.txt", &target_dir)
            .await?;
        assert!(result);

        // Verify file was copied
        let target_file = target_dir.join("data.txt");
        assert!(target_file.exists());
        let content = fs::read_to_string(&target_file)?;
        assert_eq!(content, "data content");

        Ok(())
    }

    /// Test copy_from_latest when file doesn't exist in source
    #[tokio::test]
    async fn test_copy_from_latest_file_not_found() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());

        // Create source snapshot without the file
        let source_dir = temp_dir.path().join("20240117_100000");
        fs::create_dir_all(&source_dir)?;

        // Create target directory
        let target_dir = temp_dir.path().join("20240118_100000");
        fs::create_dir_all(&target_dir)?;

        // Try to copy non-existent file
        let result = manager
            .copy_from_latest("plugin", "missing.txt", &target_dir)
            .await?;
        assert!(!result);

        Ok(())
    }

    /// Test finalize_snapshot
    #[tokio::test]
    async fn test_finalize_snapshot() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());

        // Create snapshot with some files
        let snapshot_dir = manager.create_snapshot_dir().await?;
        fs::write(snapshot_dir.join("file1.txt"), "content1")?;
        fs::write(snapshot_dir.join("file2.txt"), "content2")?;

        // Save initial metadata
        let metadata = manager.create_metadata();
        manager.save_metadata(&snapshot_dir, &metadata).await?;

        // Finalize snapshot
        manager.finalize_snapshot(&snapshot_dir).await?;

        // Load metadata and verify directory checksum was updated
        let updated_metadata = manager.load_metadata(&snapshot_dir).await?;
        assert!(!updated_metadata.directory_checksum.is_empty());
        assert_ne!(
            updated_metadata.directory_checksum,
            metadata.directory_checksum
        );

        Ok(())
    }

    /// Test create_metadata
    #[test]
    fn test_create_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());

        let metadata = manager.create_metadata();
        assert_eq!(metadata.version, env!("CARGO_PKG_VERSION"));
        assert!(metadata.checksums.is_empty());
        assert!(metadata.directory_checksum.is_empty());
        // Timestamp should be recent
        let now = Utc::now();
        let diff = now.signed_duration_since(metadata.timestamp);
        assert!(diff.num_seconds() < 2);
    }

    /// Test base_path getter
    #[test]
    fn test_base_path() {
        let base_path = PathBuf::from("/test/path");
        let manager = SnapshotManager::new(base_path.clone());
        assert_eq!(manager.base_path(), &base_path);
    }

    /// Test snapshot directory name format
    #[tokio::test]
    async fn test_snapshot_dir_name_format() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = SnapshotManager::new(temp_dir.path().to_path_buf());

        let snapshot_dir = manager.create_snapshot_dir().await?;
        let dir_name = snapshot_dir.file_name().unwrap().to_str().unwrap();

        // Check format: YYYYMMDD_HHMMSS
        assert_eq!(dir_name.len(), 15);
        assert_eq!(dir_name.chars().nth(8), Some('_'));
        assert!(dir_name[0..8].chars().all(|c| c.is_ascii_digit()));
        assert!(dir_name[9..15].chars().all(|c| c.is_ascii_digit()));

        Ok(())
    }
}

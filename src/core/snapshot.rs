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
        
        async_fs::create_dir_all(&snapshot_dir).await
            .context("Failed to create snapshot directory")?;
        
        Ok(snapshot_dir)
    }
    
    /// Saves snapshot metadata to the snapshot directory
    pub async fn save_metadata(&self, snapshot_dir: &Path, metadata: &SnapshotMetadata) -> Result<()> {
        let metadata_path = snapshot_dir.join("metadata.json");
        let json = serde_json::to_string_pretty(metadata)?;
        
        async_fs::write(&metadata_path, json).await
            .context("Failed to save snapshot metadata")?;
        
        Ok(())
    }
    
    /// Loads snapshot metadata from a snapshot directory
    pub async fn load_metadata(&self, snapshot_dir: &Path) -> Result<SnapshotMetadata> {
        let metadata_path = snapshot_dir.join("metadata.json");
        
        if !metadata_path.exists() {
            return Err(anyhow::anyhow!("Metadata file not found"));
        }
        
        let json = async_fs::read_to_string(&metadata_path).await?;
        let metadata: SnapshotMetadata = serde_json::from_str(&json)?;
        
        Ok(metadata)
    }
    
    /// Finds the most recent snapshot directory
    #[allow(dead_code)]
    pub fn find_latest_snapshot(&self) -> Result<Option<PathBuf>> {
        self.find_latest_snapshot_excluding(&PathBuf::new())
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
    pub async fn find_file_by_checksum(&self, plugin_name: &str, filename: &str, checksum: &str, exclude_dir: &Path) -> Result<Option<PathBuf>> {
        let latest_snapshot = match self.find_latest_snapshot_excluding(exclude_dir)? {
            Some(path) => path,
            None => return Ok(None),
        };
        
        let metadata = self.load_metadata(&latest_snapshot).await?;
        
        if let Some(stored_checksum) = metadata.checksums.get(plugin_name) {
            if checksums_equal(checksum, stored_checksum) {
                let file_path = latest_snapshot.join(filename);
                if file_path.exists() {
                    return Ok(Some(file_path));
                }
            }
        }
        
        Ok(None)
    }
    
    /// Copies a file from the latest snapshot to the current snapshot
    pub async fn copy_from_latest(&self, _plugin_name: &str, filename: &str, target_dir: &Path) -> Result<bool> {
        let latest_snapshot = match self.find_latest_snapshot_excluding(target_dir)? {
            Some(path) => path,
            None => return Ok(false),
        };
        
        let source_file = latest_snapshot.join(filename);
        let target_file = target_dir.join(filename);
        
        if source_file.exists() {
            async_fs::copy(&source_file, &target_file).await
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
}
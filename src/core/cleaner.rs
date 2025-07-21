use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{debug, info, warn};

/// Represents a snapshot directory with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInfo {
    pub name: String,
    pub path: PathBuf,
    pub created_at: DateTime<Local>,
    pub size_bytes: u64,
    pub plugin_count: usize,
}

/// Snapshot metadata structure (from metadata.json)
#[derive(Debug, Deserialize)]
struct SnapshotMetadata {
    pub timestamp: String,
    pub plugins: Vec<PluginMetadata>,
}

#[derive(Debug, Deserialize)]
struct PluginMetadata {
    pub name: String,
}

/// Manages snapshot cleanup operations
pub struct SnapshotCleaner {
    snapshots_dir: PathBuf,
}

impl SnapshotCleaner {
    pub fn new(snapshots_dir: PathBuf) -> Self {
        Self { snapshots_dir }
    }

    /// List all snapshots in the snapshots directory
    pub async fn list_snapshots(&self) -> Result<Vec<SnapshotInfo>> {
        let mut snapshots = Vec::new();

        if !self.snapshots_dir.exists() {
            warn!(
                "Snapshots directory does not exist: {}",
                self.snapshots_dir.display()
            );
            return Ok(snapshots);
        }

        let mut entries = fs::read_dir(&self.snapshots_dir).with_context(|| {
            format!(
                "Failed to read snapshots directory: {}",
                self.snapshots_dir.display()
            )
        })?;

        while let Some(entry) = entries.next().transpose()? {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(snapshot_info) = self.analyze_snapshot(&path).await {
                    snapshots.push(snapshot_info);
                } else {
                    debug!(
                        "Skipping directory that doesn't appear to be a snapshot: {}",
                        path.display()
                    );
                }
            }
        }

        // Sort by creation time (newest first)
        snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(snapshots)
    }

    /// Analyze a snapshot directory to extract metadata
    async fn analyze_snapshot(&self, snapshot_path: &Path) -> Result<SnapshotInfo> {
        let name = snapshot_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Try to read metadata.json first
        let metadata_path = snapshot_path.join("metadata.json");
        let (created_at, plugin_count) = if metadata_path.exists() {
            match self.read_snapshot_metadata(&metadata_path).await {
                Ok(metadata) => {
                    let created_at = self.parse_timestamp(&metadata.timestamp)?;
                    let plugin_count = metadata
                        .plugins
                        .iter()
                        .filter(|p| !p.name.is_empty())
                        .count();
                    (created_at, plugin_count)
                }
                Err(e) => {
                    debug!("Failed to read metadata for {}: {}", name, e);
                    // Fallback to directory modification time
                    let created_at = self.get_directory_creation_time(snapshot_path)?;
                    (created_at, 0)
                }
            }
        } else {
            // Fallback to directory modification time
            let created_at = self.get_directory_creation_time(snapshot_path)?;
            (created_at, 0)
        };

        // Calculate directory size
        let size_bytes = self.calculate_directory_size(snapshot_path)?;

        Ok(SnapshotInfo {
            name,
            path: snapshot_path.to_path_buf(),
            created_at,
            size_bytes,
            plugin_count,
        })
    }

    /// Read and parse snapshot metadata.json
    async fn read_snapshot_metadata(&self, metadata_path: &Path) -> Result<SnapshotMetadata> {
        let content = tokio::fs::read_to_string(metadata_path)
            .await
            .with_context(|| {
                format!("Failed to read metadata file: {}", metadata_path.display())
            })?;

        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse metadata file: {}", metadata_path.display()))
    }

    /// Parse timestamp from metadata
    fn parse_timestamp(&self, timestamp: &str) -> Result<DateTime<Local>> {
        // Try parsing the timestamp format used in snapshot creation
        if let Ok(naive_dt) = NaiveDateTime::parse_from_str(timestamp, "%Y%m%d_%H%M%S") {
            return Ok(DateTime::from_naive_utc_and_offset(
                naive_dt,
                Local::now().offset().clone(),
            ));
        }

        // Try RFC3339 format as fallback
        if let Ok(dt) = DateTime::parse_from_rfc3339(timestamp) {
            return Ok(dt.with_timezone(&Local));
        }

        anyhow::bail!("Could not parse timestamp: {}", timestamp);
    }

    /// Get directory creation time as fallback
    fn get_directory_creation_time(&self, path: &Path) -> Result<DateTime<Local>> {
        let metadata = fs::metadata(path)
            .with_context(|| format!("Failed to get metadata for: {}", path.display()))?;

        let created = metadata
            .modified()
            .or_else(|_| metadata.created())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        let datetime = DateTime::<Local>::from(created);
        Ok(datetime)
    }

    /// Calculate total size of directory in bytes
    fn calculate_directory_size(&self, dir_path: &Path) -> Result<u64> {
        let mut total_size = 0u64;

        fn visit_dir(dir: &Path, total: &mut u64) -> Result<()> {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    visit_dir(&path, total)?;
                } else {
                    *total += entry.metadata()?.len();
                }
            }
            Ok(())
        }

        visit_dir(dir_path, &mut total_size)?;
        Ok(total_size)
    }

    /// Clean snapshots by name
    pub async fn clean_by_name(&self, name: &str, dry_run: bool) -> Result<bool> {
        let snapshot_path = self.snapshots_dir.join(name);

        if !snapshot_path.exists() {
            warn!("Snapshot '{}' not found", name);
            return Ok(false);
        }

        if !snapshot_path.is_dir() {
            warn!("'{}' is not a directory", name);
            return Ok(false);
        }

        if dry_run {
            info!("Would delete snapshot: {}", name);
            return Ok(true);
        }

        info!("Deleting snapshot: {}", name);
        tokio::fs::remove_dir_all(&snapshot_path)
            .await
            .with_context(|| format!("Failed to delete snapshot: {}", name))?;

        Ok(true)
    }

    /// Clean snapshots older than specified days
    pub async fn clean_by_retention(&self, days: u32, dry_run: bool) -> Result<Vec<String>> {
        let snapshots = self.list_snapshots().await?;
        let cutoff_date = Local::now() - chrono::Duration::days(days as i64);
        let mut cleaned = Vec::new();

        for snapshot in snapshots {
            if snapshot.created_at < cutoff_date {
                if dry_run {
                    info!(
                        "Would delete snapshot: {} (created: {})",
                        snapshot.name,
                        snapshot.created_at.format("%Y-%m-%d %H:%M:%S")
                    );
                } else {
                    info!(
                        "Deleting snapshot: {} (created: {})",
                        snapshot.name,
                        snapshot.created_at.format("%Y-%m-%d %H:%M:%S")
                    );
                    tokio::fs::remove_dir_all(&snapshot.path)
                        .await
                        .with_context(|| format!("Failed to delete snapshot: {}", snapshot.name))?;
                }
                cleaned.push(snapshot.name);
            }
        }

        Ok(cleaned)
    }

    /// Format file size for human readable output
    pub fn format_size(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", bytes, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_list_empty_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let cleaner = SnapshotCleaner::new(temp_dir.path().to_path_buf());

        let snapshots = cleaner.list_snapshots().await?;
        assert!(snapshots.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_format_size() {
        assert_eq!(SnapshotCleaner::format_size(500), "500 B");
        assert_eq!(SnapshotCleaner::format_size(1024), "1.0 KB");
        assert_eq!(SnapshotCleaner::format_size(1536), "1.5 KB");
        assert_eq!(SnapshotCleaner::format_size(1048576), "1.0 MB");
    }

    #[tokio::test]
    async fn test_clean_by_name_nonexistent() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let cleaner = SnapshotCleaner::new(temp_dir.path().to_path_buf());

        let result = cleaner.clean_by_name("nonexistent", false).await?;
        assert!(!result);

        Ok(())
    }

    #[tokio::test]
    async fn test_clean_by_name_dry_run() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let snapshot_dir = temp_dir.path().join("test_snapshot");
        fs::create_dir_all(&snapshot_dir).await?;
        fs::write(snapshot_dir.join("test.txt"), "content").await?;

        let cleaner = SnapshotCleaner::new(temp_dir.path().to_path_buf());

        let result = cleaner.clean_by_name("test_snapshot", true).await?;
        assert!(result);
        assert!(snapshot_dir.exists()); // Should still exist in dry run

        Ok(())
    }
}

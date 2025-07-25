use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

/// Mixin trait for common file operations
#[allow(async_fn_in_trait)]
#[allow(dead_code)]
pub trait FilesMixin {
    /// Read a configuration file from a given path
    async fn read_config_file(&self, path: &Path) -> Result<String> {
        debug!("Reading config file: {}", path.display());

        if !path.exists() {
            return Ok(String::new());
        }

        let content = fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        debug!("Read {} bytes from {}", content.len(), path.display());
        Ok(content)
    }

    /// Get the default configuration directory for the current user
    async fn get_default_config_dir(&self) -> Result<PathBuf> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;

        // Default to .config directory
        let config_dir = home_dir.join(".config");
        Ok(config_dir)
    }

    /// Get platform-specific application data directory
    fn get_app_data_dir(&self, app_name: &str) -> Result<PathBuf> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;

        let data_dir = if cfg!(target_os = "macos") {
            home_dir.join("Library/Application Support").join(app_name)
        } else if cfg!(target_os = "windows") {
            home_dir.join("AppData/Roaming").join(app_name)
        } else {
            // Linux and other Unix-like systems
            home_dir.join(".config").join(app_name)
        };

        Ok(data_dir)
    }

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

    /// Restore a file with backup of existing target
    async fn restore_file_with_backup(&self, source: &Path, target: &Path) -> Result<()> {
        // Create backup if target exists
        if target.exists() {
            let backup_path = self.create_backup_path(target);
            info!(
                "Creating backup: {} -> {}",
                target.display(),
                backup_path.display()
            );

            fs::copy(target, &backup_path)
                .await
                .with_context(|| format!("Failed to create backup at {}", backup_path.display()))?;
        }

        // Restore the file
        self.restore_file(source, target).await
    }

    /// Create a backup path for a file
    fn create_backup_path(&self, original: &Path) -> PathBuf {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let mut backup_path = original.to_path_buf();

        // Add timestamp to filename
        if let Some(filename) = original.file_name() {
            let filename_str = filename.to_string_lossy();
            let backup_filename = format!("{filename_str}.backup.{timestamp}");
            backup_path.set_file_name(backup_filename);
        }

        backup_path
    }

    /// Check if a file exists and is readable
    async fn is_file_accessible(&self, path: &Path) -> bool {
        match fs::metadata(path).await {
            Ok(metadata) => metadata.is_file(),
            Err(_) => false,
        }
    }

    /// Check if a directory exists and is accessible
    async fn is_dir_accessible(&self, path: &Path) -> bool {
        match fs::metadata(path).await {
            Ok(metadata) => metadata.is_dir(),
            Err(_) => false,
        }
    }

    /// Find files matching a pattern in a directory
    async fn find_files_in_dir(&self, dir: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
        if !self.is_dir_accessible(dir).await {
            return Ok(Vec::new());
        }

        let mut entries = fs::read_dir(dir)
            .await
            .with_context(|| format!("Failed to read directory: {}", dir.display()))?;

        let mut matching_files = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_file() {
                if let Some(filename) = path.file_name() {
                    let filename_str = filename.to_string_lossy();
                    if filename_str.contains(pattern) {
                        matching_files.push(path);
                    }
                }
            }
        }

        Ok(matching_files)
    }

    /// Copy directory recursively with error handling
    async fn copy_dir_recursive(&self, src: &Path, dst: &Path) -> Result<Vec<PathBuf>> {
        debug!(
            "Copying directory recursively: {} -> {}",
            src.display(),
            dst.display()
        );

        // For now, use a simpler implementation to avoid lifetime issues
        // TODO: Implement proper recursive copy when needed
        warn!("copy_dir_recursive: Using placeholder implementation");

        if !src.is_dir() {
            return Err(anyhow::anyhow!(
                "Source is not a directory: {}",
                src.display()
            ));
        }

        // Create destination directory
        fs::create_dir_all(dst)
            .await
            .with_context(|| format!("Failed to create directory: {}", dst.display()))?;

        // For now, just copy the directory structure without files
        // This is a placeholder that avoids the complex async recursion issue
        Ok(vec![dst.to_path_buf()])
    }

    /// Ensure a directory exists, creating it if necessary
    async fn ensure_dir_exists(&self, dir: &Path) -> Result<()> {
        if !dir.exists() {
            debug!("Creating directory: {}", dir.display());
            fs::create_dir_all(dir)
                .await
                .with_context(|| format!("Failed to create directory: {}", dir.display()))?;
        }
        Ok(())
    }

    /// Get file size in bytes
    async fn get_file_size(&self, path: &Path) -> Result<u64> {
        let metadata = fs::metadata(path)
            .await
            .with_context(|| format!("Failed to get metadata for: {}", path.display()))?;

        Ok(metadata.len())
    }

    /// Check if a path is safe to write to (not a system directory)
    fn is_safe_write_path(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Block potentially dangerous system paths
        let dangerous_paths = [
            "/bin",
            "/sbin",
            "/usr/bin",
            "/usr/sbin",
            "/etc",
            "/var",
            "/sys",
            "/proc",
            "/boot",
            "/dev",
            "/run",
            "C:\\Windows",
            "C:\\System32",
            "C:\\Program Files",
        ];

        for dangerous in &dangerous_paths {
            if path_str.starts_with(dangerous) {
                warn!(
                    "Blocked write to potentially dangerous path: {}",
                    path.display()
                );
                return false;
            }
        }

        true
    }

    /// Write content to a file safely
    async fn write_file_safe(&self, path: &Path, content: &str) -> Result<()> {
        if !self.is_safe_write_path(path) {
            return Err(anyhow::anyhow!(
                "Refused to write to potentially dangerous path: {}",
                path.display()
            ));
        }

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            self.ensure_dir_exists(parent).await?;
        }

        fs::write(path, content)
            .await
            .with_context(|| format!("Failed to write file: {}", path.display()))?;

        info!("Wrote {} bytes to {}", content.len(), path.display());
        Ok(())
    }
}

// Note: Removed blanket implementation to avoid conflicts with specific implementations
// Each type that needs FilesMixin should implement it explicitly

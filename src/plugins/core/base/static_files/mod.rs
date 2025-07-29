use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

use crate::config::Config;
use crate::core::plugin::Plugin;
use crate::plugins::core::mixins::{CommandMixin, FilesMixin};

/// Core trait for static files-specific functionality
///
/// This trait handles the unique requirements of static file plugins:
/// - Uses Arc<Config> instead of toml::Value for configuration
/// - Handles complex file operations with ignore patterns
/// - Manages custom directory structures and restoration logic
pub trait StaticFilesCore: Send + Sync {
    /// Get the icon for this static files implementation
    fn icon(&self) -> &'static str;

    /// Read configuration and return list of file paths to copy
    fn read_config<'a>(
        &'a self,
        config: Option<&'a Arc<Config>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + 'a>>;

    /// Get ignore patterns from configuration
    fn get_ignore_patterns(&self, config: Option<&Arc<Config>>) -> Vec<String>;

    /// Check if a path should be ignored based on ignore patterns
    fn should_ignore(&self, path: &std::path::Path, ignore_patterns: &[String]) -> bool;

    /// Expand path variables like ~, $HOME, etc.
    fn expand_path(&self, path: &str) -> Result<PathBuf>;

    /// Copy files to static folder and return a JSON summary
    fn copy_files<'a>(
        &'a self,
        file_paths: Vec<PathBuf>,
        static_dir: &'a std::path::Path,
        ignore_patterns: &'a [String],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + 'a>>;

    /// Restore static files from snapshot back to their original locations
    fn restore_static_files<'a>(
        &'a self,
        static_snapshot_dir: &'a std::path::Path,
        target_base_path: &'a std::path::Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + 'a>>;
}

/// Generic static files plugin that uses mixins for common functionality
///
/// Unlike other base plugins, this uses Arc<Config> instead of StandardConfig
/// because static files plugins need access to the full application configuration.
pub struct StaticFilesPlugin<T: StaticFilesCore> {
    core: T,
    config: Option<Arc<Config>>,
    snapshot_dir: Option<PathBuf>,
}

impl<T: StaticFilesCore> StaticFilesPlugin<T> {
    /// Create a new static files plugin with the given core implementation
    pub fn new(core: T) -> Self {
        Self {
            core,
            config: None,
            snapshot_dir: None,
        }
    }

    /// Create a new static files plugin with configuration
    #[cfg(test)]
    pub fn with_config(core: T, config: Arc<Config>) -> Self {
        Self {
            core,
            config: Some(config),
            snapshot_dir: None,
        }
    }

    /// Get the default restore target directory for static files
    pub fn get_default_restore_target_dir(&self) -> Result<PathBuf> {
        // Static files are restored to their original locations,
        // preserving the directory structure from the snapshot
        Ok(PathBuf::from("/"))
    }
}

#[async_trait]
impl<T: StaticFilesCore> Plugin for StaticFilesPlugin<T> {
    fn description(&self) -> &str {
        "Copies arbitrary static files and directories based on configuration"
    }

    fn icon(&self) -> &str {
        self.core.icon()
    }

    async fn execute(&self) -> Result<String> {
        let file_paths = match self.core.read_config(self.config.as_ref()).await {
            Ok(paths) => paths,
            Err(e) => {
                return Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "error": format!("Failed to read config: {}", e),
                    "summary": {
                        "total_files": 0,
                        "copied": 0,
                        "failed": 0
                    }
                }))?);
            }
        };

        if file_paths.is_empty() {
            return Ok(serde_json::to_string_pretty(&serde_json::json!({
                "summary": {
                    "total_files": 0,
                    "copied": 0,
                    "failed": 0,
                    "message": "No files configured or config file not found"
                }
            }))?);
        }

        // Get snapshot directory from environment variable set by executor
        let static_dir = if let Ok(snapshot_dir_str) = std::env::var("DOTSNAPSHOT_SNAPSHOT_DIR") {
            let snapshot_dir = PathBuf::from(snapshot_dir_str);
            snapshot_dir.join("static")
        } else if let Some(snapshot_dir) = &self.snapshot_dir {
            snapshot_dir.join("static")
        } else {
            // Fallback: create static directory in current directory
            std::env::current_dir()?.join("static")
        };

        // Get ignore patterns
        let ignore_patterns = self.core.get_ignore_patterns(self.config.as_ref());

        let summary = self
            .core
            .copy_files(file_paths, &static_dir, &ignore_patterns)
            .await?;

        // Calculate checksum of the static directory contents for better change detection
        let directory_checksum = if static_dir.exists() {
            crate::core::checksum::calculate_directory_checksum(&static_dir)
                .unwrap_or_else(|_| "error_calculating_checksum".to_string())
        } else {
            "no_static_directory".to_string()
        };

        // Parse the summary JSON and add the directory checksum
        let mut summary_json: serde_json::Value = serde_json::from_str(&summary)?;
        if let Some(summary_obj) = summary_json.get_mut("summary") {
            summary_obj["directory_checksum"] =
                serde_json::Value::String(directory_checksum.clone());
        }

        // Create the final content with directory checksum as the primary identifier
        // This ensures that when file contents change, the plugin checksum changes too
        let final_content = format!(
            "STATIC_DIR_CHECKSUM:{}\n{}",
            directory_checksum,
            serde_json::to_string_pretty(&summary_json)?
        );

        Ok(final_content)
    }

    async fn validate(&self) -> Result<()> {
        // Check if we can determine home directory for path expansion
        if dirs::home_dir().is_none() {
            return Err(anyhow::anyhow!("Could not determine home directory"));
        }

        // No additional validation needed since config is injected
        Ok(())
    }

    fn get_target_path(&self) -> Option<String> {
        None
    }

    fn get_output_file(&self) -> Option<String> {
        None
    }

    fn creates_own_output_files(&self) -> bool {
        true // Static files plugin handles its own file operations
    }

    fn get_restore_target_dir(&self) -> Option<String> {
        // Static files plugin doesn't use standard config pattern,
        // so this returns None and restoration uses default target
        None
    }

    fn get_default_restore_target_dir(&self) -> Result<PathBuf> {
        self.get_default_restore_target_dir()
    }

    async fn restore(
        &self,
        snapshot_path: &std::path::Path,
        target_path: &std::path::Path,
        dry_run: bool,
    ) -> Result<Vec<PathBuf>> {
        use tracing::{info, warn};

        let mut restored_files = Vec::new();

        // Look for static directory in the snapshot
        let static_snapshot_dir = snapshot_path.join("static");
        if !static_snapshot_dir.exists() {
            return Ok(restored_files);
        }

        if dry_run {
            warn!(
                "DRY RUN: Would restore static files from {} to {}",
                static_snapshot_dir.display(),
                target_path.display()
            );
            warn!("DRY RUN: Static files would be restored to their original locations");

            // In dry run, just count what would be restored
            if let Ok(_entries) = tokio::fs::read_dir(&static_snapshot_dir).await {
                warn!("DRY RUN: Static directory found with files to restore");
                restored_files.push(target_path.to_path_buf());
            }
        } else {
            // Restore static files by copying them back to their original locations
            match self
                .core
                .restore_static_files(&static_snapshot_dir, target_path)
                .await
            {
                Ok(files) => {
                    restored_files.extend(files);
                    if !restored_files.is_empty() {
                        info!(
                            "Restored {} static files from snapshot",
                            restored_files.len()
                        );
                        info!("Note: Static files have been restored to their original locations");
                        info!(
                            "Review the restored files and ensure they are in the correct places"
                        );
                    }
                }
                Err(e) => {
                    warn!("Failed to restore static files: {}", e);
                }
            }
        }

        Ok(restored_files)
    }
}

// Static files plugins don't use the standard config mixins
// because they need Arc<Config> access instead of toml::Value
// They implement the Plugin trait methods directly instead of using ConfigMixin

impl<T: StaticFilesCore> FilesMixin for StaticFilesPlugin<T> {
    // Uses default implementation
}

impl<T: StaticFilesCore> CommandMixin for StaticFilesPlugin<T> {
    // Uses default implementation
}

#[cfg(test)]
pub mod tests;

use anyhow::{Context, Result};
use async_trait::async_trait;
use glob::Pattern;
use serde_json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::info;

use crate::config::Config;
use crate::core::checksum::calculate_directory_checksum;
use crate::core::plugin::Plugin;
use crate::symbols::*;

/// Plugin for copying static files to snapshots based on configuration
pub struct StaticFilesPlugin {
    config: Option<Arc<Config>>,
    snapshot_dir: Option<PathBuf>,
}

impl StaticFilesPlugin {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            config: None,
            snapshot_dir: None,
        }
    }

    pub fn with_config(config: Arc<Config>) -> Self {
        Self {
            config: Some(config),
            snapshot_dir: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_config_path<P: AsRef<Path>>(_config_path: P) -> Self {
        // This method is kept for backward compatibility with tests
        // In practice, it creates a minimal config for testing
        Self {
            config: None,
            snapshot_dir: None,
        }
    }

    /// Reads the static files configuration from the main config
    async fn read_config(&self) -> Result<Vec<PathBuf>> {
        let config = match &self.config {
            Some(config) => config,
            None => {
                // No config provided, return empty list
                return Ok(Vec::new());
            }
        };

        let static_config = match config.get_static_files() {
            Some(static_config) => static_config,
            None => {
                // No static files configuration section
                return Ok(Vec::new());
            }
        };

        let file_list = match &static_config.files {
            Some(files) => files,
            None => {
                // No files specified in configuration
                return Ok(Vec::new());
            }
        };

        let mut file_paths = Vec::new();

        for file_path_str in file_list {
            // Skip empty paths
            if file_path_str.trim().is_empty() {
                continue;
            }

            // Expand path variables
            let expanded_path = self.expand_path(file_path_str.trim())?;
            file_paths.push(expanded_path);
        }

        Ok(file_paths)
    }

    /// Check if a path should be ignored based on ignore patterns
    fn should_ignore(&self, path: &Path, ignore_patterns: &[String]) -> bool {
        let path_str = path.to_string_lossy();

        for pattern_str in ignore_patterns {
            if let Ok(pattern) = Pattern::new(pattern_str) {
                // Check if the full path matches
                if pattern.matches(&path_str) {
                    return true;
                }

                // Also check just the file/directory name
                if let Some(file_name) = path.file_name() {
                    if pattern.matches(&file_name.to_string_lossy()) {
                        return true;
                    }
                }

                // Check each component of the path
                for component in path.components() {
                    if pattern.matches(&component.as_os_str().to_string_lossy()) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Get ignore patterns from configuration
    fn get_ignore_patterns(&self) -> Vec<String> {
        if let Some(config) = &self.config {
            if let Some(static_config) = config.get_static_files() {
                if let Some(ignore_patterns) = &static_config.ignore {
                    return ignore_patterns.clone();
                }
            }
        }
        Vec::new()
    }

    /// Expands path variables like ~, $HOME, etc.
    fn expand_path(&self, path: &str) -> Result<PathBuf> {
        let expanded = if path.starts_with('~') {
            let home = dirs::home_dir().context("Could not determine home directory")?;
            home.join(&path[2..]) // Skip "~/"
        } else if path.contains('$') {
            // Simple environment variable expansion
            let mut expanded = path.to_string();
            if let Ok(home) = std::env::var("HOME") {
                expanded = expanded.replace("$HOME", &home);
            }
            PathBuf::from(expanded)
        } else {
            PathBuf::from(path)
        };

        Ok(expanded)
    }

    /// Copies files to static folder and returns a JSON summary
    async fn copy_files(&self, file_paths: Vec<PathBuf>, static_dir: &Path) -> Result<String> {
        let mut copied_files = Vec::new();
        let mut failed_files = Vec::new();
        let mut ignored_files = Vec::new();

        // Get ignore patterns from config
        let ignore_patterns = self.get_ignore_patterns();

        // Create static directory if it doesn't exist
        tokio::fs::create_dir_all(static_dir)
            .await
            .context("Failed to create static directory")?;

        for file_path in file_paths {
            // Check if this path should be ignored
            if self.should_ignore(&file_path, &ignore_patterns) {
                info!(
                    "{} Ignoring static item: {} (matches ignore pattern)",
                    ACTION_BLOCK,
                    file_path.display()
                );
                ignored_files.push(file_path.display().to_string());
                continue;
            }

            match self
                .copy_single_file(&file_path, static_dir, &ignore_patterns)
                .await
            {
                Ok(dest_path) => {
                    let item_type = if file_path.is_dir() {
                        "directory"
                    } else {
                        "file"
                    };
                    info!(
                        "{} Copied static {}: {} -> {}",
                        CONTENT_FILE,
                        item_type,
                        file_path.display(),
                        dest_path.display()
                    );
                    copied_files.push(file_path.display().to_string());
                }
                Err(e) => {
                    let error_msg = format!("{}: {}", file_path.display(), e);
                    info!(
                        "{} Failed to copy static item: {}",
                        INDICATOR_ERROR, error_msg
                    );
                    failed_files.push(error_msg);
                }
            }
        }

        // Create summary
        let summary = serde_json::json!({
            "summary": {
                "total_files": copied_files.len() + failed_files.len() + ignored_files.len(),
                "copied": copied_files.len(),
                "failed": failed_files.len(),
                "ignored": ignored_files.len(),
                "copied_files": copied_files,
                "failed_files": failed_files,
                "ignored_files": ignored_files,
                "static_directory": static_dir.display().to_string()
            }
        });

        Ok(serde_json::to_string_pretty(&summary)?)
    }

    /// Copies a single file or directory to the static directory, preserving directory structure
    async fn copy_single_file(
        &self,
        file_path: &Path,
        static_dir: &Path,
        ignore_patterns: &[String],
    ) -> Result<PathBuf> {
        if !file_path.exists() {
            return Err(anyhow::anyhow!("Path does not exist"));
        }

        // Create a destination path that preserves directory structure but skips $HOME
        let dest_path = if file_path.is_absolute() {
            // Check if path is in user's home directory
            if let Some(home_dir) = dirs::home_dir() {
                if let Ok(relative_to_home) = file_path.strip_prefix(&home_dir) {
                    // Path is in home directory, use path relative to home
                    static_dir.join("home").join(relative_to_home)
                } else {
                    // Path is outside home directory, use full path but remove leading slash
                    let relative_path = file_path.strip_prefix("/").unwrap_or(file_path);
                    static_dir.join(relative_path)
                }
            } else {
                // Can't determine home directory, use full path
                let relative_path = file_path.strip_prefix("/").unwrap_or(file_path);
                static_dir.join(relative_path)
            }
        } else {
            // For relative paths, just join with static dir
            static_dir.join(file_path)
        };

        if file_path.is_dir() {
            // Copy entire directory recursively
            self.copy_directory_recursive(file_path, &dest_path, ignore_patterns)
                .await?;
        } else {
            // Create parent directories if they don't exist
            if let Some(parent) = dest_path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .context("Failed to create parent directories")?;
            }

            // Copy the file
            tokio::fs::copy(file_path, &dest_path)
                .await
                .context("Failed to copy file")?;
        }

        Ok(dest_path)
    }

    /// Recursively copies a directory and all its contents
    fn copy_directory_recursive<'a>(
        &'a self,
        src_dir: &'a Path,
        dest_dir: &'a Path,
        ignore_patterns: &'a [String],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            // Create the destination directory
            tokio::fs::create_dir_all(dest_dir)
                .await
                .context("Failed to create destination directory")?;

            let mut entries = tokio::fs::read_dir(src_dir)
                .await
                .context("Failed to read source directory")?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .context("Failed to read directory entry")?
            {
                let src_path = entry.path();

                // Check if this item should be ignored
                if self.should_ignore(&src_path, ignore_patterns) {
                    info!(
                        "{} Ignoring static item: {} (matches ignore pattern)",
                        ACTION_BLOCK,
                        src_path.display()
                    );
                    continue;
                }

                let file_name = src_path
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?;
                let dest_path = dest_dir.join(file_name);

                if src_path.is_dir() {
                    // Recursively copy subdirectory
                    self.copy_directory_recursive(&src_path, &dest_path, ignore_patterns)
                        .await?;
                } else {
                    // Copy file
                    tokio::fs::copy(&src_path, &dest_path)
                        .await
                        .context(format!("Failed to copy file: {}", src_path.display()))?;
                }
            }

            Ok(())
        })
    }
}

#[async_trait]
impl Plugin for StaticFilesPlugin {
    fn name(&self) -> &str {
        "static"
    }

    fn filename(&self) -> &str {
        "static.json"
    }

    fn description(&self) -> &str {
        "Copies arbitrary static files and directories based on configuration"
    }

    /// Override output path to place static.json in .snapshot directory
    fn output_path(&self, base_path: &Path) -> PathBuf {
        base_path.join(".snapshot").join(self.filename())
    }

    async fn execute(&self) -> Result<String> {
        let file_paths = match self.read_config().await {
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

        let summary = self.copy_files(file_paths, &static_dir).await?;

        // Calculate checksum of the static directory contents for better change detection
        let directory_checksum = if static_dir.exists() {
            calculate_directory_checksum(&static_dir)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_static_plugin_name() {
        let plugin = StaticFilesPlugin::new();
        assert_eq!(plugin.name(), "static");
        assert_eq!(plugin.filename(), "static.json");
    }

    #[tokio::test]
    async fn test_expand_path() {
        let plugin = StaticFilesPlugin::new();

        // Test absolute path
        let abs_path = plugin.expand_path("/usr/local/bin/test").unwrap();
        assert_eq!(abs_path, PathBuf::from("/usr/local/bin/test"));

        // Test home expansion
        if let Ok(home) = std::env::var("HOME") {
            let home_path = plugin.expand_path("~/test").unwrap();
            assert_eq!(home_path, PathBuf::from(home).join("test"));
        }
    }

    #[tokio::test]
    async fn test_plugin_with_empty_config() {
        // Test with no config
        let plugin = StaticFilesPlugin::new();
        let result = plugin.execute().await.unwrap();

        // Should return empty result when no config exists
        assert!(result.contains("No files configured"));
    }

    #[tokio::test]
    async fn test_plugin_with_main_config() {
        use crate::config::{Config, StaticFilesConfig};
        use std::sync::Arc;
        use tempfile::TempDir;

        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let static_dir = temp_dir.path().join("static");

        // Set environment variable for snapshot directory
        std::env::set_var("DOTSNAPSHOT_SNAPSHOT_DIR", temp_dir.path());

        // Create a test config with static files
        let config = Config {
            output_dir: None,
            include_plugins: None,
            logging: None,
            hooks: None,
            global: None,
            static_files: Some(StaticFilesConfig {
                files: Some(vec!["/etc/hosts".to_string()]),
                ignore: None,
            }),
            plugins: None,
        };

        let plugin = StaticFilesPlugin::with_config(Arc::new(config));
        let result = plugin.execute().await.unwrap();

        // Should attempt to process the config file
        assert!(result.contains("/etc/hosts") || result.contains("summary"));

        // Check that static directory was created
        assert!(static_dir.exists());

        // Clean up environment variable
        std::env::remove_var("DOTSNAPSHOT_SNAPSHOT_DIR");
    }

    #[tokio::test]
    async fn test_ignore_patterns() {
        let plugin = StaticFilesPlugin::new();
        let ignore_patterns = vec![
            "*.key".to_string(),
            "*_rsa".to_string(),
            ".git".to_string(),
            "node_modules".to_string(),
        ];

        // Test file patterns
        assert!(plugin.should_ignore(&PathBuf::from("/path/to/private.key"), &ignore_patterns));
        assert!(plugin.should_ignore(&PathBuf::from("/path/to/id_rsa"), &ignore_patterns));
        assert!(!plugin.should_ignore(&PathBuf::from("/path/to/public.pub"), &ignore_patterns));

        // Test directory patterns
        assert!(plugin.should_ignore(&PathBuf::from("/project/.git"), &ignore_patterns));
        assert!(plugin.should_ignore(&PathBuf::from("/project/node_modules"), &ignore_patterns));
        assert!(!plugin.should_ignore(&PathBuf::from("/project/src"), &ignore_patterns));

        // Test nested paths
        assert!(plugin.should_ignore(&PathBuf::from("/project/.git/config"), &ignore_patterns));
        assert!(plugin.should_ignore(&PathBuf::from("/deep/path/to/secret.key"), &ignore_patterns));
    }
}

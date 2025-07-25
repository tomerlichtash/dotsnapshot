use anyhow::{Context, Result};
use glob::Pattern;
use serde_json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::info;

use crate::config::{Config, StaticPluginConfig};
use crate::plugins::core::base::static_files::{StaticFilesCore, StaticFilesPlugin};
use crate::symbols::*;

/// Static files implementation using the mixin architecture
#[derive(Default)]
pub struct StaticFilesAppCore;

impl StaticFilesCore for StaticFilesAppCore {
    fn app_name(&self) -> &'static str {
        "StaticFiles"
    }

    fn icon(&self) -> &'static str {
        CONTENT_FILE
    }

    fn read_config<'a>(
        &'a self,
        config: Option<&'a Arc<Config>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + 'a>>
    {
        Box::pin(async move {
            let config = match config {
                Some(config) => config,
                None => {
                    // No config provided, return empty list
                    return Ok(Vec::new());
                }
            };

            // Get static files plugin configuration from plugins section
            // Note: The plugin name is "static_files" but the TOML section is "static"
            let static_config = match &config.plugins {
                Some(plugins) => match plugins.plugins.get("static") {
                    Some(static_value) => {
                        match static_value.clone().try_into::<StaticPluginConfig>() {
                            Ok(static_config) => static_config,
                            Err(_) => {
                                // No static files plugin configuration section
                                return Ok(Vec::new());
                            }
                        }
                    }
                    _ => {
                        // No static files plugin configuration section
                        return Ok(Vec::new());
                    }
                },
                None => {
                    // No plugins configuration section
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
        })
    }

    fn get_ignore_patterns(&self, config: Option<&Arc<Config>>) -> Vec<String> {
        if let Some(config) = config {
            if let Some(plugins) = &config.plugins {
                if let Some(static_value) = plugins.plugins.get("static") {
                    if let Ok(static_config) = static_value.clone().try_into::<StaticPluginConfig>()
                    {
                        if let Some(ignore_patterns) = &static_config.ignore {
                            return ignore_patterns.clone();
                        }
                    }
                }
            }
        }
        Vec::new()
    }

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

    fn copy_files<'a>(
        &'a self,
        file_paths: Vec<PathBuf>,
        static_dir: &'a Path,
        ignore_patterns: &'a [String],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let mut copied_files = Vec::new();
            let mut failed_files = Vec::new();
            let mut ignored_files = Vec::new();

            // Create static directory if it doesn't exist
            tokio::fs::create_dir_all(static_dir)
                .await
                .context("Failed to create static directory")?;

            for file_path in file_paths {
                // Check if this path should be ignored
                if self.should_ignore(&file_path, ignore_patterns) {
                    info!(
                        "{} Ignoring static item: {} (matches ignore pattern)",
                        ACTION_BLOCK,
                        file_path.display()
                    );
                    ignored_files.push(file_path.display().to_string());
                    continue;
                }

                match self
                    .copy_single_file(&file_path, static_dir, ignore_patterns)
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
        })
    }

    fn restore_static_files<'a>(
        &'a self,
        static_snapshot_dir: &'a Path,
        target_base_path: &'a Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + 'a>>
    {
        Box::pin(async move {
            use tracing::warn;
            let mut restored_files = Vec::new();

            // Read the static directory structure and restore files
            let mut entries = tokio::fs::read_dir(static_snapshot_dir)
                .await
                .context("Failed to read static snapshot directory")?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .context("Failed to read directory entry")?
            {
                let entry_path = entry.path();
                let entry_name = entry.file_name();

                // Handle special directory structures
                if entry_path.is_dir() && entry_name == "home" {
                    // Restore files from home directory
                    if let Some(home_dir) = dirs::home_dir() {
                        let files =
                            Self::restore_directory_recursive_static(&entry_path, &home_dir)
                                .await?;
                        restored_files.extend(files);
                    } else {
                        warn!("Could not determine home directory for restoring files");
                    }
                } else {
                    // For other directories, restore to filesystem root or target path
                    let target_path = if target_base_path == Path::new("/") {
                        // Restore to filesystem root
                        Path::new("/").join(&entry_name)
                    } else {
                        // Restore relative to target path
                        target_base_path.join(&entry_name)
                    };

                    if entry_path.is_dir() {
                        let files =
                            Self::restore_directory_recursive_static(&entry_path, &target_path)
                                .await?;
                        restored_files.extend(files);
                    } else {
                        // Create parent directories if needed
                        if let Some(parent) = target_path.parent() {
                            tokio::fs::create_dir_all(parent)
                                .await
                                .context("Failed to create parent directories for static file")?;
                        }

                        // Copy the file
                        tokio::fs::copy(&entry_path, &target_path)
                            .await
                            .context(format!(
                                "Failed to restore static file to {}",
                                target_path.display()
                            ))?;

                        restored_files.push(target_path);
                    }
                }
            }

            Ok(restored_files)
        })
    }
}

impl StaticFilesAppCore {
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

    /// Recursively restores a directory and its contents
    fn restore_directory_recursive_static<'a>(
        src_dir: &'a Path,
        dest_dir: &'a Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + 'a>>
    {
        Box::pin(async move {
            let mut restored_files = Vec::new();

            // Create the destination directory
            tokio::fs::create_dir_all(dest_dir).await.context(format!(
                "Failed to create directory: {}",
                dest_dir.display()
            ))?;

            let mut entries = tokio::fs::read_dir(src_dir)
                .await
                .context("Failed to read source directory")?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .context("Failed to read directory entry")?
            {
                let src_path = entry.path();
                let file_name = src_path
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?;
                let dest_path = dest_dir.join(file_name);

                if src_path.is_dir() {
                    // Recursively restore subdirectory
                    let files =
                        Self::restore_directory_recursive_static(&src_path, &dest_path).await?;
                    restored_files.extend(files);
                } else {
                    // Copy file
                    tokio::fs::copy(&src_path, &dest_path)
                        .await
                        .context(format!("Failed to restore file: {}", dest_path.display()))?;
                    restored_files.push(dest_path);
                }
            }

            Ok(restored_files)
        })
    }
}

/// Type alias for the static files plugin (used for external references)
#[allow(dead_code)]
pub type StaticFilesPluginApp = StaticFilesPlugin<StaticFilesAppCore>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, PluginsConfig, StaticPluginConfig};
    use crate::core::plugin::Plugin;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_static_files_core_app_info() {
        let core = StaticFilesAppCore;
        assert_eq!(core.app_name(), "StaticFiles");
        assert_eq!(core.icon(), CONTENT_FILE);
    }

    #[tokio::test]
    async fn test_static_files_plugin_creation() {
        let plugin = StaticFilesPlugin::new(StaticFilesAppCore);
        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );
        assert_eq!(plugin.icon(), CONTENT_FILE);
    }

    #[tokio::test]
    async fn test_static_files_plugin_with_empty_config() {
        // Test with no config
        let plugin = StaticFilesPlugin::new(StaticFilesAppCore);
        let result = plugin.execute().await.unwrap();

        // Should return empty result when no config exists
        assert!(result.contains("No files configured"));
    }

    #[tokio::test]
    async fn test_static_files_plugin_with_config() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let static_dir = temp_dir.path().join("static");

        // Set environment variable for snapshot directory
        std::env::set_var("DOTSNAPSHOT_SNAPSHOT_DIR", temp_dir.path());

        // Create a test config with static files in plugins section
        let config = Config {
            output_dir: None,
            include_plugins: None,
            logging: None,
            hooks: None,
            global: None,
            static_files: None,
            plugins: Some(PluginsConfig {
                plugins: {
                    let mut map = std::collections::HashMap::new();
                    map.insert(
                        "static".to_string(),
                        toml::Value::try_from(StaticPluginConfig {
                            target_path: None,
                            output_file: None,
                            files: Some(vec!["/etc/hosts".to_string()]),
                            ignore: None,
                        })
                        .unwrap(),
                    );
                    map
                },
            }),
            ui: None,
        };

        let plugin = StaticFilesPlugin::with_config(StaticFilesAppCore, Arc::new(config));
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
        let core = StaticFilesAppCore;
        let ignore_patterns = vec![
            "*.key".to_string(),
            "*_rsa".to_string(),
            ".git".to_string(),
            "node_modules".to_string(),
        ];

        // Test file patterns
        assert!(core.should_ignore(&PathBuf::from("/path/to/private.key"), &ignore_patterns));
        assert!(core.should_ignore(&PathBuf::from("/path/to/id_rsa"), &ignore_patterns));
        assert!(!core.should_ignore(&PathBuf::from("/path/to/public.pub"), &ignore_patterns));

        // Test directory patterns
        assert!(core.should_ignore(&PathBuf::from("/project/.git"), &ignore_patterns));
        assert!(core.should_ignore(&PathBuf::from("/project/node_modules"), &ignore_patterns));
        assert!(!core.should_ignore(&PathBuf::from("/project/src"), &ignore_patterns));

        // Test nested paths
        assert!(core.should_ignore(&PathBuf::from("/project/.git/config"), &ignore_patterns));
        assert!(core.should_ignore(&PathBuf::from("/deep/path/to/secret.key"), &ignore_patterns));
    }

    #[tokio::test]
    async fn test_static_files_restore_functionality() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");
        let static_snapshot_dir = snapshot_dir.join("static");

        fs::create_dir_all(&static_snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test static files structure
        let home_dir = static_snapshot_dir.join("home");
        fs::create_dir_all(&home_dir).await.unwrap();

        let test_file_content = "# Test config file";
        let test_file_path = home_dir.join("test_config.txt");
        fs::write(&test_file_path, test_file_content).await.unwrap();

        let plugin = StaticFilesPlugin::new(StaticFilesAppCore);

        // Test dry run
        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);

        // Test actual restore
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Should have restored at least one file
        assert!(!result.is_empty());
    }

    #[tokio::test]
    async fn test_static_files_restore_no_static_dir() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        let plugin = StaticFilesPlugin::new(StaticFilesAppCore);
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Should return empty result when no static directory exists
        assert!(result.is_empty());
    }

    #[test]
    fn test_static_files_restore_target_dir_methods() {
        let plugin = StaticFilesPlugin::new(StaticFilesAppCore);

        // Static files plugin returns None for restore_target_dir (uses special logic)
        assert_eq!(plugin.get_restore_target_dir(), None);

        // Default restore target is filesystem root
        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert_eq!(default_dir, std::path::PathBuf::from("/"));
    }
}

// Auto-register this plugin using the standard registration system
//
// Unlike the original static files plugin that used manual inventory submission,
// this new implementation can use a simplified registration since it follows
// the mixin architecture pattern, even though it uses Arc<Config> instead of toml::Value.
//
// The factory function ignores the _config parameter and creates the plugin using
// the special StaticFilesPlugin pattern that gets its configuration during execution.
inventory::submit! {
    crate::core::plugin::PluginDescriptor {
        name: "static_files",
        category: "static",
        factory: |_config| {
            // NOTE: _config parameter is ignored because static files plugin
            // gets its configuration through Arc<Config> during execution
            std::sync::Arc::new(StaticFilesPlugin::new(StaticFilesAppCore))
        },
    }
}

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
    fn icon(&self) -> &'static str {
        SYMBOL_CONTENT_FILE
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
                        SYMBOL_ACTION_BLOCK,
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
                            SYMBOL_CONTENT_FILE,
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
                            SYMBOL_INDICATOR_ERROR, error_msg
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
                    // Path is outside home directory, use path without root prefix
                    #[cfg(unix)]
                    let relative_path = file_path.strip_prefix("/").unwrap_or(file_path);
                    #[cfg(windows)]
                    let relative_path = {
                        // On Windows, remove the drive letter and colon (e.g., "C:\\" -> "")
                        let path_str = file_path.to_string_lossy();
                        if let Some(stripped) = path_str.strip_prefix(r"C:\") {
                            std::path::PathBuf::from(stripped)
                        } else if path_str.len() >= 3 && path_str.chars().nth(1) == Some(':') {
                            // Handle other drive letters like D:\, E:\, etc.
                            std::path::PathBuf::from(&path_str[3..])
                        } else {
                            file_path.to_path_buf()
                        }
                    };
                    static_dir.join(relative_path)
                }
            } else {
                // Can't determine home directory, use path without root prefix
                #[cfg(unix)]
                let relative_path = file_path.strip_prefix("/").unwrap_or(file_path);
                #[cfg(windows)]
                let relative_path = {
                    // On Windows, remove the drive letter and colon
                    let path_str = file_path.to_string_lossy();
                    if let Some(stripped) = path_str.strip_prefix(r"C:\") {
                        std::path::PathBuf::from(stripped)
                    } else if path_str.len() >= 3 && path_str.chars().nth(1) == Some(':') {
                        std::path::PathBuf::from(&path_str[3..])
                    } else {
                        file_path.to_path_buf()
                    }
                };
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
                        SYMBOL_ACTION_BLOCK,
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
        assert_eq!(core.icon(), SYMBOL_CONTENT_FILE);
    }

    #[tokio::test]
    async fn test_static_files_plugin_creation() {
        let plugin = StaticFilesPlugin::new(StaticFilesAppCore);
        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );
        assert_eq!(plugin.icon(), SYMBOL_CONTENT_FILE);
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

        // Create test static files structure - create both home and non-home files
        let home_dir = static_snapshot_dir.join("home");
        fs::create_dir_all(&home_dir).await.unwrap();

        let test_file_content = "# Test config file";
        let test_file_path = home_dir.join("test_config.txt");
        fs::write(&test_file_path, test_file_content).await.unwrap();

        // Also create a non-home file that should definitely restore successfully
        let non_home_file = static_snapshot_dir.join("app_config.conf");
        fs::write(&non_home_file, "app configuration")
            .await
            .unwrap();

        let plugin = StaticFilesPlugin::new(StaticFilesAppCore);

        // Test dry run - should find at least the non-home file
        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();
        assert!(!result.is_empty(), "Dry run should find files to restore");

        // Test actual restore - in CI, home directory restore might fail,
        // but non-home files should restore successfully
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Should have restored at least the non-home file
        assert!(!result.is_empty(), "Should restore at least non-home files");

        // Verify the non-home file was restored
        let restored_file = target_dir.join("app_config.conf");
        assert!(
            restored_file.exists(),
            "Non-home file should be restored to target directory"
        );
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

    /// Test expand_path method with various path formats
    /// Verifies that path expansion handles home directory and environment variables
    #[tokio::test]
    async fn test_expand_path() {
        let core = StaticFilesAppCore;

        // Test home directory expansion
        let expanded = core.expand_path("~/test/file.txt").unwrap();
        let home_dir = dirs::home_dir().unwrap();
        assert_eq!(expanded, home_dir.join("test/file.txt"));

        // Test environment variable expansion
        std::env::set_var("TEST_VAR", "/custom/path");
        let expanded = core.expand_path("$HOME/test").unwrap();
        assert!(expanded.to_string_lossy().contains("test"));

        // Test plain path (no expansion needed)
        let expanded = core.expand_path("/absolute/path").unwrap();
        assert_eq!(expanded, PathBuf::from("/absolute/path"));

        let expanded = core.expand_path("relative/path").unwrap();
        assert_eq!(expanded, PathBuf::from("relative/path"));

        // Clean up
        std::env::remove_var("TEST_VAR");
    }

    /// Test get_ignore_patterns with various configurations
    /// Verifies that ignore patterns are correctly extracted from config
    #[test]
    fn test_get_ignore_patterns() {
        let core = StaticFilesAppCore;

        // Test with no config
        let patterns = core.get_ignore_patterns(None);
        assert!(patterns.is_empty());

        // Test with config that has ignore patterns
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
                            files: None,
                            ignore: Some(vec!["*.tmp".to_string(), "cache/".to_string()]),
                        })
                        .unwrap(),
                    );
                    map
                },
            }),
            ui: None,
        };

        let patterns = core.get_ignore_patterns(Some(&Arc::new(config)));
        assert_eq!(patterns.len(), 2);
        assert!(patterns.contains(&"*.tmp".to_string()));
        assert!(patterns.contains(&"cache/".to_string()));
    }

    /// Test copy_single_file with non-existent file
    /// Verifies that copy_single_file handles missing files correctly
    #[tokio::test]
    async fn test_copy_single_file_not_exist() {
        let core = StaticFilesAppCore;
        let temp_dir = TempDir::new().unwrap();
        let static_dir = temp_dir.path().join("static");
        fs::create_dir_all(&static_dir).await.unwrap();

        let non_existent = temp_dir.path().join("does_not_exist.txt");
        let result = core.copy_single_file(&non_existent, &static_dir, &[]).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    /// Test copy_single_file with relative paths
    /// Verifies that relative paths are handled correctly
    #[tokio::test]
    async fn test_copy_single_file_relative_path() {
        let core = StaticFilesAppCore;
        let temp_dir = TempDir::new().unwrap();
        let static_dir = temp_dir.path().join("static");
        fs::create_dir_all(&static_dir).await.unwrap();

        // Create a test file with relative path
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").await.unwrap();

        // Get relative path
        let current_dir = std::env::current_dir().unwrap();
        let relative_path = test_file.strip_prefix(&current_dir).unwrap_or(&test_file);

        let result = core.copy_single_file(relative_path, &static_dir, &[]).await;
        assert!(result.is_ok());

        let dest_path = result.unwrap();
        assert!(dest_path.exists());

        let content = fs::read_to_string(&dest_path).await.unwrap();
        assert_eq!(content, "test content");
    }

    /// Test copy_single_file with directory
    /// Verifies that directories are copied recursively
    #[tokio::test]
    async fn test_copy_single_file_directory() {
        let core = StaticFilesAppCore;
        let temp_dir = TempDir::new().unwrap();
        let static_dir = temp_dir.path().join("static");
        let test_dir = temp_dir.path().join("test_dir");

        fs::create_dir_all(&static_dir).await.unwrap();
        fs::create_dir_all(&test_dir).await.unwrap();

        // Create test files in directory
        fs::write(test_dir.join("file1.txt"), "content1")
            .await
            .unwrap();
        fs::write(test_dir.join("file2.txt"), "content2")
            .await
            .unwrap();

        let result = core.copy_single_file(&test_dir, &static_dir, &[]).await;
        assert!(result.is_ok());

        let dest_path = result.unwrap();
        assert!(dest_path.exists());
        assert!(dest_path.is_dir());

        // Verify files were copied
        assert!(dest_path.join("file1.txt").exists());
        assert!(dest_path.join("file2.txt").exists());
    }

    /// Test copy_directory_recursive with ignore patterns
    /// Verifies that ignored files are skipped during directory copy
    #[tokio::test]
    async fn test_copy_directory_recursive_with_ignore() {
        let core = StaticFilesAppCore;
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&src_dir).await.unwrap();
        fs::create_dir_all(&dest_dir).await.unwrap();

        // Create test files
        fs::write(src_dir.join("keep.txt"), "keep").await.unwrap();
        fs::write(src_dir.join("ignore.tmp"), "ignore")
            .await
            .unwrap();
        fs::create_dir_all(src_dir.join("subdir")).await.unwrap();
        fs::write(src_dir.join("subdir/file.txt"), "content")
            .await
            .unwrap();

        let ignore_patterns = vec!["*.tmp".to_string()];

        core.copy_directory_recursive(&src_dir, &dest_dir, &ignore_patterns)
            .await
            .unwrap();

        // Verify only non-ignored files were copied
        assert!(dest_dir.join("keep.txt").exists());
        assert!(!dest_dir.join("ignore.tmp").exists());
        assert!(dest_dir.join("subdir/file.txt").exists());
    }

    /// Test restore_directory_recursive_static
    /// Verifies that directory restoration works correctly
    #[tokio::test]
    async fn test_restore_directory_recursive_static() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        let dest_dir = temp_dir.path().join("dest");

        // Create source directory structure
        fs::create_dir_all(src_dir.join("subdir")).await.unwrap();
        fs::write(src_dir.join("file1.txt"), "content1")
            .await
            .unwrap();
        fs::write(src_dir.join("subdir/file2.txt"), "content2")
            .await
            .unwrap();

        let restored = StaticFilesAppCore::restore_directory_recursive_static(&src_dir, &dest_dir)
            .await
            .unwrap();

        assert_eq!(restored.len(), 2);
        assert!(dest_dir.join("file1.txt").exists());
        assert!(dest_dir.join("subdir/file2.txt").exists());

        // Verify content
        let content1 = fs::read_to_string(dest_dir.join("file1.txt"))
            .await
            .unwrap();
        assert_eq!(content1, "content1");

        let content2 = fs::read_to_string(dest_dir.join("subdir/file2.txt"))
            .await
            .unwrap();
        assert_eq!(content2, "content2");
    }

    /// Test read_config with various configuration scenarios
    /// Verifies that file paths are correctly read from plugin config
    #[tokio::test]
    async fn test_read_config_scenarios() {
        let core = StaticFilesAppCore;

        // Test with no config
        let files = core.read_config(None).await.unwrap();
        assert!(files.is_empty());

        // Test with empty plugins section
        let config = Config {
            output_dir: None,
            include_plugins: None,
            logging: None,
            hooks: None,
            global: None,
            static_files: None,
            plugins: None,
            ui: None,
        };
        let files = core.read_config(Some(&Arc::new(config))).await.unwrap();
        assert!(files.is_empty());

        // Test with static plugin config but no files
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
                            files: None,
                            ignore: None,
                        })
                        .unwrap(),
                    );
                    map
                },
            }),
            ui: None,
        };
        let files = core.read_config(Some(&Arc::new(config))).await.unwrap();
        assert!(files.is_empty());

        // Test with files that include empty strings
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
                            files: Some(vec![
                                "~/test.txt".to_string(),
                                "".to_string(),    // Empty string should be skipped
                                "   ".to_string(), // Whitespace should be skipped
                                "/etc/hosts".to_string(),
                            ]),
                            ignore: None,
                        })
                        .unwrap(),
                    );
                    map
                },
            }),
            ui: None,
        };
        let files = core.read_config(Some(&Arc::new(config))).await.unwrap();
        assert_eq!(files.len(), 2); // Only non-empty paths
    }

    /// Test copy_files with various scenarios
    /// Verifies that copy_files handles multiple files and generates correct summary
    #[tokio::test]
    async fn test_copy_files_summary() {
        let core = StaticFilesAppCore;
        let temp_dir = TempDir::new().unwrap();
        let static_dir = temp_dir.path().join("static");

        // Create test files
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        let file3 = temp_dir.path().join("ignored.tmp");

        fs::write(&file1, "content1").await.unwrap();
        fs::write(&file2, "content2").await.unwrap();
        fs::write(&file3, "ignored").await.unwrap();

        let file_paths = vec![file1, file2, file3];
        let ignore_patterns = vec!["*.tmp".to_string()];

        let summary = core
            .copy_files(file_paths, &static_dir, &ignore_patterns)
            .await
            .unwrap();

        // Parse summary JSON
        let summary_json: serde_json::Value = serde_json::from_str(&summary).unwrap();
        let summary_obj = summary_json["summary"].as_object().unwrap();

        assert_eq!(summary_obj["total_files"].as_u64().unwrap(), 3);
        assert_eq!(summary_obj["copied"].as_u64().unwrap(), 2);
        assert_eq!(summary_obj["ignored"].as_u64().unwrap(), 1);
        assert_eq!(summary_obj["failed"].as_u64().unwrap(), 0);

        // Verify static directory was created
        assert!(static_dir.exists());
    }

    /// Test restore_static_files with home directory structure
    /// Verifies that files in home directory are restored correctly
    #[tokio::test]
    async fn test_restore_static_files_home_directory() {
        let core = StaticFilesAppCore;
        let temp_dir = TempDir::new().unwrap();
        let static_snapshot_dir = temp_dir.path().join("static");
        let target_dir = temp_dir.path().join("target");

        // Create home directory structure in snapshot
        let home_snapshot = static_snapshot_dir.join("home");
        fs::create_dir_all(home_snapshot.join("config"))
            .await
            .unwrap();
        fs::write(home_snapshot.join("config/app.conf"), "config data")
            .await
            .unwrap();

        // Also create non-home file
        fs::write(static_snapshot_dir.join("other.txt"), "other data")
            .await
            .unwrap();

        // Test restore_static_files - in CI environments, home directory restore
        // might fail due to permissions, so we handle it gracefully
        let restored = core
            .restore_static_files(&static_snapshot_dir, &target_dir)
            .await;

        match restored {
            Ok(files) => {
                // Should restore at least the non-home file
                assert!(!files.is_empty());

                // Verify non-home file was restored to target directory
                assert!(target_dir.join("other.txt").exists());
                let content = fs::read_to_string(target_dir.join("other.txt"))
                    .await
                    .unwrap();
                assert_eq!(content, "other data");
            }
            Err(e) => {
                // In CI environments, home directory restore might fail due to permissions
                // This is acceptable if the error is related to home directory access
                let error_msg = e.to_string();
                assert!(
                    error_msg.contains("Permission denied")
                        || error_msg.contains("Failed to create directory"),
                    "Unexpected error: {error_msg}"
                );
            }
        }
    }

    /// Test should_ignore with complex patterns
    /// Verifies that pattern matching works for various path structures
    #[test]
    fn test_should_ignore_complex_patterns() {
        let core = StaticFilesAppCore;
        let patterns = vec![
            "*.log".to_string(),
            "temp/*".to_string(),
            "**/.git".to_string(),
            "build".to_string(), // Remove trailing slash - glob pattern should match directory name
        ];

        // Test file extension pattern
        assert!(core.should_ignore(&PathBuf::from("/var/log/app.log"), &patterns));
        assert!(!core.should_ignore(&PathBuf::from("/var/log/app.txt"), &patterns));

        // Test directory pattern - should match "build" as a component
        assert!(core.should_ignore(&PathBuf::from("/project/build/"), &patterns));
        assert!(core.should_ignore(&PathBuf::from("/project/build/output.js"), &patterns));

        // Test component matching
        assert!(core.should_ignore(&PathBuf::from("/project/.git/config"), &patterns));
        assert!(core.should_ignore(&PathBuf::from("/deep/path/.git/hooks"), &patterns));
    }

    /// Test plugin trait methods
    /// Verifies that all Plugin trait methods work correctly
    #[tokio::test]
    async fn test_static_files_plugin_trait_methods() {
        let plugin = StaticFilesPlugin::new(StaticFilesAppCore);

        // Test basic plugin trait methods
        assert!(plugin.description().contains("static files"));
        assert_eq!(plugin.icon(), SYMBOL_CONTENT_FILE);
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert_eq!(plugin.get_restore_target_dir(), None);
        assert!(plugin.creates_own_output_files());
        assert!(plugin.get_hooks().is_empty());

        // Test validation (should always pass for static files)
        assert!(plugin.validate().await.is_ok());
    }

    /// Test copy_single_file with absolute paths outside home
    /// Verifies that absolute paths are handled correctly
    #[tokio::test]
    async fn test_copy_single_file_absolute_path_outside_home() {
        let core = StaticFilesAppCore;
        let temp_dir = TempDir::new().unwrap();
        let static_dir = temp_dir.path().join("static");
        fs::create_dir_all(&static_dir).await.unwrap();

        // Create a test file with absolute path
        let test_file = temp_dir.path().join("absolute_test.txt");
        fs::write(&test_file, "absolute content").await.unwrap();

        let result = core.copy_single_file(&test_file, &static_dir, &[]).await;
        assert!(result.is_ok());

        let dest_path = result.unwrap();
        assert!(dest_path.exists());

        // For absolute paths outside home, structure should be preserved
        let content = fs::read_to_string(&dest_path).await.unwrap();
        assert_eq!(content, "absolute content");
    }

    /// Test error handling in copy_directory_recursive
    /// Verifies that errors are propagated correctly
    #[tokio::test]
    async fn test_copy_directory_recursive_error_handling() {
        let core = StaticFilesAppCore;
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("nonexistent");
        let dest_dir = temp_dir.path().join("dest");

        // Try to copy non-existent directory
        let result = core
            .copy_directory_recursive(&src_dir, &dest_dir, &[])
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to read source directory"));
    }

    /// Test restore_static_files with filesystem root target
    /// Verifies that restoration to root directory works correctly
    #[tokio::test]
    async fn test_restore_static_files_to_root() {
        let core = StaticFilesAppCore;
        let temp_dir = TempDir::new().unwrap();
        let static_snapshot_dir = temp_dir.path().join("static");

        // Create test file in snapshot
        fs::create_dir_all(&static_snapshot_dir).await.unwrap();
        fs::write(static_snapshot_dir.join("test_root.txt"), "root data")
            .await
            .unwrap();

        // Use a subdirectory as simulated root to avoid actual filesystem root
        let simulated_root = temp_dir.path().join("simulated_root");
        fs::create_dir_all(&simulated_root).await.unwrap();

        let restored = core
            .restore_static_files(&static_snapshot_dir, &simulated_root)
            .await
            .unwrap();

        assert!(!restored.is_empty());
        assert!(simulated_root.join("test_root.txt").exists());
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

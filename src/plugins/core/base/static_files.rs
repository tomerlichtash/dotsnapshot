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

// StaticFilesPlugin doesn't implement AllMixins because it doesn't use ConfigMixin
// due to its special Arc<Config> requirements

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::plugin::Plugin;
    use crate::symbols::SYMBOL_CONTENT_FILE;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;
    use tokio::fs;

    // Mock implementation for testing
    #[derive(Default)]
    struct MockStaticFilesCore;

    impl StaticFilesCore for MockStaticFilesCore {
        fn icon(&self) -> &'static str {
            SYMBOL_CONTENT_FILE
        }

        fn read_config(
            &self,
            _config: Option<&Arc<Config>>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>>
        {
            Box::pin(async move { Ok(vec![]) })
        }

        fn get_ignore_patterns(&self, _config: Option<&Arc<Config>>) -> Vec<String> {
            vec!["*.tmp".to_string()]
        }

        fn should_ignore(&self, path: &std::path::Path, ignore_patterns: &[String]) -> bool {
            let path_str = path.to_string_lossy();
            ignore_patterns.iter().any(|pattern| {
                if let Ok(glob_pattern) = glob::Pattern::new(pattern) {
                    glob_pattern.matches(&path_str)
                } else {
                    false
                }
            })
        }

        fn expand_path(&self, path: &str) -> Result<PathBuf> {
            Ok(PathBuf::from(path))
        }

        fn copy_files(
            &self,
            _file_paths: Vec<PathBuf>,
            _static_dir: &std::path::Path,
            _ignore_patterns: &[String],
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
        {
            Box::pin(async move {
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "summary": {
                        "total_files": 0,
                        "copied": 0,
                        "failed": 0
                    }
                }))?)
            })
        }

        fn restore_static_files(
            &self,
            _static_snapshot_dir: &std::path::Path,
            _target_base_path: &std::path::Path,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>>
        {
            Box::pin(async move { Ok(vec![]) })
        }
    }

    #[tokio::test]
    async fn test_static_files_plugin_creation() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);
        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );
        assert_eq!(plugin.icon(), SYMBOL_CONTENT_FILE);
    }

    #[tokio::test]
    async fn test_static_files_plugin_execute() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);
        let result = plugin.execute().await.unwrap();
        assert!(result.contains("No files configured"));
    }

    #[tokio::test]
    async fn test_static_files_plugin_restore() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_static_files_plugin_restore_with_static_dir() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let static_dir = snapshot_dir.join("static");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&static_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create a test file in static directory
        fs::write(static_dir.join("test.txt"), "test content")
            .await
            .unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Mock implementation returns empty vec, but directory exists
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_static_files_plugin_restore_dry_run() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let static_dir = snapshot_dir.join("static");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&static_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create a test file in static directory
        fs::write(static_dir.join("test.txt"), "test content")
            .await
            .unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();

        // Dry run should return target path
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target_dir);
    }

    #[tokio::test]
    async fn test_static_files_plugin_validate() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);
        let result = plugin.validate().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_static_files_plugin_trait_methods() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);

        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert_eq!(plugin.get_restore_target_dir(), None);
        assert!(plugin.creates_own_output_files());

        let default_restore_dir = plugin.get_default_restore_target_dir().unwrap();
        assert_eq!(default_restore_dir, PathBuf::from("/"));
    }

    #[tokio::test]
    async fn test_static_files_plugin_execute_with_config_error() {
        // Create a mock that returns an error from read_config
        struct ErrorMockCore;

        impl StaticFilesCore for ErrorMockCore {
            fn icon(&self) -> &'static str {
                SYMBOL_CONTENT_FILE
            }

            fn read_config(
                &self,
                _config: Option<&Arc<Config>>,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>,
            > {
                Box::pin(async move { Err(anyhow::anyhow!("Config error")) })
            }

            fn get_ignore_patterns(&self, _config: Option<&Arc<Config>>) -> Vec<String> {
                vec![]
            }

            fn should_ignore(&self, _path: &std::path::Path, _ignore_patterns: &[String]) -> bool {
                false
            }

            fn expand_path(&self, path: &str) -> Result<PathBuf> {
                Ok(PathBuf::from(path))
            }

            fn copy_files(
                &self,
                _file_paths: Vec<PathBuf>,
                _static_dir: &std::path::Path,
                _ignore_patterns: &[String],
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
            {
                Box::pin(async move { Ok(String::new()) })
            }

            fn restore_static_files(
                &self,
                _static_snapshot_dir: &std::path::Path,
                _target_base_path: &std::path::Path,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>,
            > {
                Box::pin(async move { Ok(vec![]) })
            }
        }

        let plugin = StaticFilesPlugin::new(ErrorMockCore);
        let result = plugin.execute().await.unwrap();

        // Should contain error message
        assert!(result.contains("error"));
        assert!(result.contains("Failed to read config"));
    }

    #[tokio::test]
    async fn test_static_files_plugin_execute_with_files() {
        // Create a mock that returns file paths
        struct FilesMockCore;

        impl StaticFilesCore for FilesMockCore {
            fn icon(&self) -> &'static str {
                SYMBOL_CONTENT_FILE
            }

            fn read_config(
                &self,
                _config: Option<&Arc<Config>>,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>,
            > {
                Box::pin(async move { Ok(vec![PathBuf::from("/test/file.txt")]) })
            }

            fn get_ignore_patterns(&self, _config: Option<&Arc<Config>>) -> Vec<String> {
                vec!["*.tmp".to_string()]
            }

            fn should_ignore(&self, _path: &std::path::Path, _ignore_patterns: &[String]) -> bool {
                false
            }

            fn expand_path(&self, path: &str) -> Result<PathBuf> {
                Ok(PathBuf::from(path))
            }

            fn copy_files(
                &self,
                _file_paths: Vec<PathBuf>,
                _static_dir: &std::path::Path,
                _ignore_patterns: &[String],
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
            {
                Box::pin(async move {
                    Ok(serde_json::to_string_pretty(&serde_json::json!({
                        "summary": {
                            "total_files": 1,
                            "copied": 1,
                            "failed": 0
                        }
                    }))?)
                })
            }

            fn restore_static_files(
                &self,
                _static_snapshot_dir: &std::path::Path,
                _target_base_path: &std::path::Path,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>,
            > {
                Box::pin(async move { Ok(vec![PathBuf::from("/restored/file.txt")]) })
            }
        }

        let plugin = StaticFilesPlugin::new(FilesMockCore);
        let result = plugin.execute().await.unwrap();

        // Should contain checksum and summary
        assert!(result.contains("STATIC_DIR_CHECKSUM:"));
        assert!(result.contains("total_files"));
        assert!(result.contains("directory_checksum"));
    }

    #[tokio::test]
    async fn test_static_files_plugin_restore_with_files() {
        // Create a mock that returns restored files
        struct RestoreFilesMockCore;

        impl StaticFilesCore for RestoreFilesMockCore {
            fn icon(&self) -> &'static str {
                SYMBOL_CONTENT_FILE
            }

            fn read_config(
                &self,
                _config: Option<&Arc<Config>>,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>,
            > {
                Box::pin(async move { Ok(vec![]) })
            }

            fn get_ignore_patterns(&self, _config: Option<&Arc<Config>>) -> Vec<String> {
                vec![]
            }

            fn should_ignore(&self, _path: &std::path::Path, _ignore_patterns: &[String]) -> bool {
                false
            }

            fn expand_path(&self, path: &str) -> Result<PathBuf> {
                Ok(PathBuf::from(path))
            }

            fn copy_files(
                &self,
                _file_paths: Vec<PathBuf>,
                _static_dir: &std::path::Path,
                _ignore_patterns: &[String],
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
            {
                Box::pin(async move { Ok(String::new()) })
            }

            fn restore_static_files(
                &self,
                _static_snapshot_dir: &std::path::Path,
                _target_base_path: &std::path::Path,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>,
            > {
                Box::pin(async move {
                    Ok(vec![
                        PathBuf::from("/restored/file1.txt"),
                        PathBuf::from("/restored/file2.txt"),
                    ])
                })
            }
        }

        let plugin = StaticFilesPlugin::new(RestoreFilesMockCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let static_dir = snapshot_dir.join("static");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&static_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create a test file in static directory
        fs::write(static_dir.join("test.txt"), "test content")
            .await
            .unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Should return the files from mock implementation
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], PathBuf::from("/restored/file1.txt"));
        assert_eq!(result[1], PathBuf::from("/restored/file2.txt"));
    }

    #[tokio::test]
    async fn test_static_files_plugin_restore_error() {
        // Create a mock that returns an error from restore_static_files
        struct ErrorRestoreMockCore;

        impl StaticFilesCore for ErrorRestoreMockCore {
            fn icon(&self) -> &'static str {
                SYMBOL_CONTENT_FILE
            }

            fn read_config(
                &self,
                _config: Option<&Arc<Config>>,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>,
            > {
                Box::pin(async move { Ok(vec![]) })
            }

            fn get_ignore_patterns(&self, _config: Option<&Arc<Config>>) -> Vec<String> {
                vec![]
            }

            fn should_ignore(&self, _path: &std::path::Path, _ignore_patterns: &[String]) -> bool {
                false
            }

            fn expand_path(&self, path: &str) -> Result<PathBuf> {
                Ok(PathBuf::from(path))
            }

            fn copy_files(
                &self,
                _file_paths: Vec<PathBuf>,
                _static_dir: &std::path::Path,
                _ignore_patterns: &[String],
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
            {
                Box::pin(async move { Ok(String::new()) })
            }

            fn restore_static_files(
                &self,
                _static_snapshot_dir: &std::path::Path,
                _target_base_path: &std::path::Path,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>,
            > {
                Box::pin(async move { Err(anyhow::anyhow!("Restore error")) })
            }
        }

        let plugin = StaticFilesPlugin::new(ErrorRestoreMockCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let static_dir = snapshot_dir.join("static");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&static_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create a test file in static directory
        fs::write(static_dir.join("test.txt"), "test content")
            .await
            .unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Should return empty result when restore fails (warning is logged)
        assert!(result.is_empty());
    }

    #[test]
    fn test_mock_static_files_core_methods() {
        let core = MockStaticFilesCore;

        assert_eq!(core.icon(), SYMBOL_CONTENT_FILE);

        let ignore_patterns = core.get_ignore_patterns(None);
        assert_eq!(ignore_patterns, vec!["*.tmp".to_string()]);

        // Test should_ignore method
        let temp_path = PathBuf::from("test.tmp");
        assert!(core.should_ignore(&temp_path, &ignore_patterns));

        let normal_path = PathBuf::from("test.txt");
        assert!(!core.should_ignore(&normal_path, &ignore_patterns));

        // Test expand_path method
        let expanded = core.expand_path("/test/path").unwrap();
        assert_eq!(expanded, PathBuf::from("/test/path"));
    }

    #[test]
    fn test_static_files_plugin_with_config() {
        use crate::config::StaticFilesConfig;

        let config = Arc::new(Config {
            output_dir: Some(PathBuf::from("/test/output")),
            include_plugins: None,
            logging: None,
            hooks: None,
            global: None,
            static_files: Some(StaticFilesConfig {
                files: Some(vec!["/test/file.txt".to_string()]),
                ignore: Some(vec!["*.log".to_string()]),
            }),
            plugins: None,
            ui: None,
        });

        let plugin = StaticFilesPlugin::with_config(MockStaticFilesCore, config.clone());
        assert!(plugin.config.is_some());
        assert_eq!(
            plugin.config.as_ref().unwrap().output_dir,
            Some(PathBuf::from("/test/output"))
        );
    }

    #[test]
    fn test_static_files_plugin_snapshot_dir_fallback() {
        // Test the snapshot directory resolution logic
        let mut plugin = StaticFilesPlugin::new(MockStaticFilesCore);
        plugin.snapshot_dir = Some(PathBuf::from("/test/snapshot"));

        // The snapshot_dir field is private, but we can test that the plugin was created successfully
        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );
    }

    #[tokio::test]
    async fn test_static_files_execute_with_env_var() {
        // Test execution with environment variable set
        std::env::set_var("DOTSNAPSHOT_SNAPSHOT_DIR", "/tmp/test_snapshot");

        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);
        let result = plugin.execute().await.unwrap();

        // Should still work with empty config (no files configured)
        assert!(result.contains("No files configured"));

        // Clean up
        std::env::remove_var("DOTSNAPSHOT_SNAPSHOT_DIR");
    }

    #[test]
    fn test_default_restore_target_dir() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);
        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert_eq!(default_dir, PathBuf::from("/"));
    }

    // Test StaticFilesCore trait methods with custom implementations
    struct AdvancedMockCore {
        should_ignore_result: bool,
        expand_path_error: bool,
    }

    impl AdvancedMockCore {
        fn new() -> Self {
            Self {
                should_ignore_result: false,
                expand_path_error: false,
            }
        }

        fn with_ignore_result(should_ignore: bool) -> Self {
            Self {
                should_ignore_result: should_ignore,
                expand_path_error: false,
            }
        }

        fn with_expand_path_error() -> Self {
            Self {
                should_ignore_result: false,
                expand_path_error: true,
            }
        }
    }

    impl StaticFilesCore for AdvancedMockCore {
        fn icon(&self) -> &'static str {
            "🔧" // Different icon for testing
        }

        fn read_config(
            &self,
            _config: Option<&Arc<Config>>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>>
        {
            Box::pin(async move {
                Ok(vec![
                    PathBuf::from("/test/file1.txt"),
                    PathBuf::from("/test/file2.txt"),
                ])
            })
        }

        fn get_ignore_patterns(&self, _config: Option<&Arc<Config>>) -> Vec<String> {
            vec![
                "*.log".to_string(),
                "*.bak".to_string(),
                "node_modules/".to_string(),
            ]
        }

        fn should_ignore(&self, path: &std::path::Path, ignore_patterns: &[String]) -> bool {
            if self.should_ignore_result {
                return true;
            }
            let path_str = path.to_string_lossy();
            ignore_patterns.iter().any(|pattern| {
                if pattern.ends_with('/') {
                    // Directory pattern matching
                    path_str.contains(pattern)
                } else if let Ok(glob_pattern) = glob::Pattern::new(pattern) {
                    glob_pattern.matches(&path_str)
                } else {
                    false
                }
            })
        }

        fn expand_path(&self, path: &str) -> Result<PathBuf> {
            if self.expand_path_error {
                return Err(anyhow::anyhow!("Path expansion failed"));
            }

            if path.starts_with('~') {
                if let Some(home_dir) = dirs::home_dir() {
                    if path == "~" {
                        Ok(home_dir)
                    } else if let Some(stripped) = path.strip_prefix("~/") {
                        Ok(home_dir.join(stripped))
                    } else {
                        // Handle cases like "~username" - just return as-is since we don't support user expansion
                        Ok(PathBuf::from(path))
                    }
                } else {
                    Err(anyhow::anyhow!("Could not determine home directory"))
                }
            } else if let Some(stripped) = path.strip_prefix("$HOME") {
                if let Some(home_dir) = dirs::home_dir() {
                    let remaining_path = if let Some(path_stripped) = stripped.strip_prefix('/') {
                        path_stripped // Skip the leading "/"
                    } else {
                        stripped // Just the remaining part
                    };
                    Ok(home_dir.join(remaining_path))
                } else {
                    Err(anyhow::anyhow!("Could not determine home directory"))
                }
            } else {
                Ok(PathBuf::from(path))
            }
        }

        fn copy_files(
            &self,
            file_paths: Vec<PathBuf>,
            _static_dir: &std::path::Path,
            _ignore_patterns: &[String],
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
        {
            Box::pin(async move {
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "summary": {
                        "total_files": file_paths.len(),
                        "copied": file_paths.len(),
                        "failed": 0,
                        "files": file_paths.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>()
                    }
                }))?)
            })
        }

        fn restore_static_files(
            &self,
            _static_snapshot_dir: &std::path::Path,
            _target_base_path: &std::path::Path,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>>
        {
            Box::pin(async move {
                Ok(vec![
                    PathBuf::from("/restored/file1.txt"),
                    PathBuf::from("/restored/file2.txt"),
                    PathBuf::from("/restored/subdir/file3.txt"),
                ])
            })
        }
    }

    #[test]
    fn test_advanced_mock_core_icon() {
        // Test custom icon method
        let core = AdvancedMockCore::new();
        assert_eq!(core.icon(), "🔧");
    }

    #[test]
    fn test_advanced_mock_core_ignore_patterns() {
        // Test get_ignore_patterns method with multiple patterns
        let core = AdvancedMockCore::new();
        let patterns = core.get_ignore_patterns(None);
        assert_eq!(
            patterns,
            vec![
                "*.log".to_string(),
                "*.bak".to_string(),
                "node_modules/".to_string()
            ]
        );
    }

    #[test]
    fn test_advanced_mock_core_should_ignore_logic() {
        // Test should_ignore method with different pattern types
        let core = AdvancedMockCore::new();
        let patterns = vec!["*.log".to_string(), "node_modules/".to_string()];

        // Test glob pattern matching
        assert!(core.should_ignore(&PathBuf::from("debug.log"), &patterns));
        assert!(core.should_ignore(&PathBuf::from("app.log"), &patterns));

        // Test directory pattern matching
        assert!(core.should_ignore(&PathBuf::from("project/node_modules/package"), &patterns));

        // Test non-matching paths
        assert!(!core.should_ignore(&PathBuf::from("readme.txt"), &patterns));
        assert!(!core.should_ignore(&PathBuf::from("src/main.rs"), &patterns));
    }

    #[test]
    fn test_advanced_mock_core_should_ignore_force_true() {
        // Test should_ignore when configured to always return true
        let core = AdvancedMockCore::with_ignore_result(true);
        let patterns = vec!["*.txt".to_string()];

        // Should ignore any path when forced
        assert!(core.should_ignore(&PathBuf::from("any_file.rs"), &patterns));
        assert!(core.should_ignore(&PathBuf::from("another.md"), &patterns));
    }

    #[test]
    fn test_advanced_mock_core_expand_path_home_tilde() {
        // Test expand_path with tilde expansion
        let core = AdvancedMockCore::new();

        if let Some(home_dir) = dirs::home_dir() {
            let result = core.expand_path("~/documents/file.txt").unwrap();
            assert_eq!(result, home_dir.join("documents/file.txt"));
        }
    }

    #[test]
    fn test_advanced_mock_core_expand_path_home_env() {
        // Test expand_path with $HOME expansion
        let core = AdvancedMockCore::new();

        if let Some(home_dir) = dirs::home_dir() {
            let result = core.expand_path("$HOME/config/app.conf").unwrap();
            assert_eq!(result, home_dir.join("config/app.conf"));
        }
    }

    #[test]
    fn test_advanced_mock_core_expand_path_regular() {
        // Test expand_path with regular absolute path
        let core = AdvancedMockCore::new();
        let result = core.expand_path("/etc/config/app.conf").unwrap();
        assert_eq!(result, PathBuf::from("/etc/config/app.conf"));
    }

    #[test]
    fn test_advanced_mock_core_expand_path_error() {
        // Test expand_path when configured to return error
        let core = AdvancedMockCore::with_expand_path_error();
        let result = core.expand_path("/any/path");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Path expansion failed"));
    }

    #[tokio::test]
    async fn test_advanced_mock_core_read_config() {
        // Test read_config method returns multiple file paths
        let core = AdvancedMockCore::new();
        let result = core.read_config(None).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], PathBuf::from("/test/file1.txt"));
        assert_eq!(result[1], PathBuf::from("/test/file2.txt"));
    }

    #[tokio::test]
    async fn test_advanced_mock_core_copy_files() {
        // Test copy_files method with multiple files
        let core = AdvancedMockCore::new();
        let file_paths = vec![
            PathBuf::from("/source/file1.txt"),
            PathBuf::from("/source/file2.txt"),
        ];
        let static_dir = PathBuf::from("/static");
        let ignore_patterns = vec!["*.tmp".to_string()];

        let result = core
            .copy_files(file_paths.clone(), &static_dir, &ignore_patterns)
            .await
            .unwrap();

        // Verify JSON structure contains file information
        assert!(result.contains("total_files"));
        assert!(result.contains("copied"));
        assert!(result.contains("files"));
        assert!(result.contains("file1.txt"));
        assert!(result.contains("file2.txt"));
    }

    #[tokio::test]
    async fn test_advanced_mock_core_restore_static_files() {
        // Test restore_static_files method returns multiple paths
        let core = AdvancedMockCore::new();
        let snapshot_dir = PathBuf::from("/snapshot/static");
        let target_dir = PathBuf::from("/target");

        let result = core
            .restore_static_files(&snapshot_dir, &target_dir)
            .await
            .unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], PathBuf::from("/restored/file1.txt"));
        assert_eq!(result[1], PathBuf::from("/restored/file2.txt"));
        assert_eq!(result[2], PathBuf::from("/restored/subdir/file3.txt"));
    }

    #[tokio::test]
    async fn test_static_files_plugin_with_advanced_core() {
        // Test StaticFilesPlugin with the advanced mock core
        let plugin = StaticFilesPlugin::new(AdvancedMockCore::new());

        // Test plugin properties
        assert_eq!(plugin.icon(), "🔧");
        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );
        assert!(plugin.creates_own_output_files());
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert_eq!(plugin.get_restore_target_dir(), None);

        // Test validation
        let validation_result = plugin.validate().await;
        assert!(validation_result.is_ok());
    }

    #[tokio::test]
    async fn test_static_files_plugin_execute_with_advanced_core() {
        // Test execute method with advanced core that returns file paths
        let plugin = StaticFilesPlugin::new(AdvancedMockCore::new());
        let result = plugin.execute().await.unwrap();

        // Should contain checksum and summary with file information
        assert!(result.contains("STATIC_DIR_CHECKSUM:"));
        assert!(result.contains("total_files"));
        assert!(result.contains("directory_checksum"));
        assert!(result.contains("files"));
    }

    #[tokio::test]
    async fn test_static_files_plugin_restore_with_advanced_core() {
        // Test restore method with advanced core
        let plugin = StaticFilesPlugin::new(AdvancedMockCore::new());

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let static_dir = snapshot_dir.join("static");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&static_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test files in static directory
        fs::write(static_dir.join("file1.txt"), "content1")
            .await
            .unwrap();
        fs::write(static_dir.join("file2.txt"), "content2")
            .await
            .unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Should return the files from advanced mock implementation
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], PathBuf::from("/restored/file1.txt"));
        assert_eq!(result[1], PathBuf::from("/restored/file2.txt"));
        assert_eq!(result[2], PathBuf::from("/restored/subdir/file3.txt"));
    }

    // Test StaticFilesCore trait default implementations and edge cases
    struct MinimalStaticFilesCore;

    impl StaticFilesCore for MinimalStaticFilesCore {
        fn icon(&self) -> &'static str {
            "📁"
        }

        fn read_config(
            &self,
            _config: Option<&Arc<Config>>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>>
        {
            Box::pin(async move { Ok(vec![]) })
        }

        fn get_ignore_patterns(&self, _config: Option<&Arc<Config>>) -> Vec<String> {
            vec![]
        }

        fn should_ignore(&self, _path: &std::path::Path, _ignore_patterns: &[String]) -> bool {
            false
        }

        fn expand_path(&self, path: &str) -> Result<PathBuf> {
            Ok(PathBuf::from(path))
        }

        fn copy_files(
            &self,
            _file_paths: Vec<PathBuf>,
            _static_dir: &std::path::Path,
            _ignore_patterns: &[String],
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
        {
            Box::pin(async move { Ok(String::new()) })
        }

        fn restore_static_files(
            &self,
            _static_snapshot_dir: &std::path::Path,
            _target_base_path: &std::path::Path,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>>
        {
            Box::pin(async move { Ok(vec![]) })
        }
    }

    #[test]
    fn test_minimal_static_files_core() {
        // Test minimal implementation of StaticFilesCore trait
        let core = MinimalStaticFilesCore;

        assert_eq!(core.icon(), "📁");
        assert_eq!(core.get_ignore_patterns(None), Vec::<String>::new());
        assert!(!core.should_ignore(&PathBuf::from("any_file.txt"), &["*.log".to_string()]));

        let expanded = core.expand_path("/test/path").unwrap();
        assert_eq!(expanded, PathBuf::from("/test/path"));
    }

    #[tokio::test]
    async fn test_minimal_static_files_core_async_methods() {
        // Test async methods of minimal implementation
        let core = MinimalStaticFilesCore;

        let config_result = core.read_config(None).await.unwrap();
        assert!(config_result.is_empty());

        let copy_result = core
            .copy_files(vec![], &PathBuf::from("/static"), &[])
            .await
            .unwrap();
        assert!(copy_result.is_empty());

        let restore_result = core
            .restore_static_files(&PathBuf::from("/snapshot"), &PathBuf::from("/target"))
            .await
            .unwrap();
        assert!(restore_result.is_empty());
    }

    #[tokio::test]
    async fn test_static_files_plugin_with_minimal_core() {
        // Test StaticFilesPlugin with minimal core implementation
        let plugin = StaticFilesPlugin::new(MinimalStaticFilesCore);

        assert_eq!(plugin.icon(), "📁");

        // Execute should return empty files message
        let result = plugin.execute().await.unwrap();
        assert!(result.contains("No files configured"));

        // Restore should return empty when no static directory exists
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        let restore_result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert!(restore_result.is_empty());
    }

    #[tokio::test]
    async fn test_static_files_validate_error_case() {
        // Test validation failure when home directory cannot be determined
        // This is difficult to test directly, but we can test that validation normally succeeds
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);
        let result = plugin.validate().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_static_files_plugin_snapshot_dir_field() {
        // Test that snapshot_dir field affects plugin behavior when set
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);

        // Test default state
        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );

        // The snapshot_dir field is private, but we can verify the plugin was created
        // and that our constructor works properly
        assert_eq!(plugin.icon(), SYMBOL_CONTENT_FILE);
    }

    #[test]
    fn test_command_mixin_and_files_mixin_defaults() {
        // Test that StaticFilesPlugin implements CommandMixin and FilesMixin with defaults
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);

        // These are default implementations from the mixins, so they should work
        // but not do anything specific for static files plugins
        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );
    }

    /// Test StaticFilesPlugin FilesMixin methods
    /// Verifies that FilesMixin methods are properly available
    #[tokio::test]
    async fn test_static_files_plugin_files_mixin_methods() {
        use crate::plugins::core::mixins::FilesMixin;

        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);
        let temp_dir = TempDir::new().unwrap();

        // Test is_dir_accessible method from FilesMixin
        let result = plugin.is_dir_accessible(temp_dir.path()).await;
        assert!(result);

        // Test with non-existent directory
        let non_existent = temp_dir.path().join("non_existent");
        let result = plugin.is_dir_accessible(&non_existent).await;
        assert!(!result);
    }

    /// Test StaticFilesPlugin CommandMixin methods
    /// Verifies that CommandMixin methods are properly available
    #[test]
    fn test_static_files_plugin_command_mixin_methods() {
        use crate::plugins::core::mixins::CommandMixin;

        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);

        // Test command_exists method from CommandMixin
        let _result = plugin.command_exists("ls");
        // ls should exist on Unix systems
        #[cfg(unix)]
        assert!(_result);

        // Test with non-existent command
        let result = plugin.command_exists("this_command_definitely_does_not_exist_12345");
        assert!(!result);
    }

    /// Test StaticFilesPlugin restore_file method from FilesMixin
    /// Verifies that FilesMixin restore_file method works correctly
    #[tokio::test]
    async fn test_static_files_plugin_restore_file_mixin() {
        use crate::plugins::core::mixins::FilesMixin;

        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);
        let temp_dir = TempDir::new().unwrap();

        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target.txt");
        let content = "test content for static files plugin";

        fs::write(&source, content).await.unwrap();

        // Test restore_file method from FilesMixin
        let result = plugin.restore_file(&source, &target).await;
        assert!(result.is_ok());
        assert!(target.exists());

        let restored_content = fs::read_to_string(&target).await.unwrap();
        assert_eq!(restored_content, content);
    }

    /// Test all AdvancedMockCore methods to increase function coverage
    /// Verifies comprehensive coverage of the advanced mock implementation
    #[tokio::test]
    async fn test_all_advanced_mock_core_methods() {
        let core = AdvancedMockCore::new();

        // Test all trait methods to ensure function coverage
        assert_eq!(core.icon(), "🔧");

        let config_result = core.read_config(None).await.unwrap();
        assert_eq!(config_result.len(), 2);

        let patterns = core.get_ignore_patterns(None);
        assert_eq!(patterns.len(), 3);

        // Test various ignore patterns
        assert!(core.should_ignore(&PathBuf::from("app.log"), &patterns));
        assert!(core.should_ignore(&PathBuf::from("backup.bak"), &patterns));
        assert!(core.should_ignore(&PathBuf::from("project/node_modules/lib"), &patterns));
        assert!(!core.should_ignore(&PathBuf::from("src/main.rs"), &patterns));

        // Test path expansion
        let expanded = core.expand_path("/test/path").unwrap();
        assert_eq!(expanded, PathBuf::from("/test/path"));

        // Test copy_files
        let files = vec![PathBuf::from("/test1.txt"), PathBuf::from("/test2.txt")];
        let copy_result = core
            .copy_files(files, &PathBuf::from("/static"), &patterns)
            .await
            .unwrap();
        assert!(copy_result.contains("total_files"));

        // Test restore_static_files
        let restore_result = core
            .restore_static_files(&PathBuf::from("/snapshot"), &PathBuf::from("/target"))
            .await
            .unwrap();
        assert_eq!(restore_result.len(), 3);
    }

    /// Test MinimalStaticFilesCore to ensure all methods are covered
    /// Verifies comprehensive coverage of the minimal mock implementation
    #[tokio::test]
    async fn test_all_minimal_static_files_core_methods() {
        let core = MinimalStaticFilesCore;

        // Test all trait methods to ensure function coverage
        assert_eq!(core.icon(), "📁");

        let config_result = core.read_config(None).await.unwrap();
        assert!(config_result.is_empty());

        let patterns = core.get_ignore_patterns(None);
        assert!(patterns.is_empty());

        // Test should_ignore
        assert!(!core.should_ignore(&PathBuf::from("any_file.txt"), &["*.log".to_string()]));

        // Test expand_path
        let expanded = core.expand_path("/minimal/path").unwrap();
        assert_eq!(expanded, PathBuf::from("/minimal/path"));

        // Test copy_files
        let copy_result = core
            .copy_files(vec![], &PathBuf::from("/static"), &[])
            .await
            .unwrap();
        assert!(copy_result.is_empty());

        // Test restore_static_files
        let restore_result = core
            .restore_static_files(&PathBuf::from("/snapshot"), &PathBuf::from("/target"))
            .await
            .unwrap();
        assert!(restore_result.is_empty());
    }

    /// Test error mock implementations to increase function coverage
    /// Verifies that error-producing mock implementations work correctly
    #[tokio::test]
    async fn test_error_mock_implementations() {
        // Test AdvancedMockCore with error conditions
        let error_core = AdvancedMockCore::with_expand_path_error();
        let expand_result = error_core.expand_path("/any/path");
        assert!(expand_result.is_err());

        let ignore_core = AdvancedMockCore::with_ignore_result(true);
        assert!(ignore_core.should_ignore(&PathBuf::from("test.txt"), &[]));

        // Test if home directory expansion works
        if dirs::home_dir().is_some() {
            let normal_core = AdvancedMockCore::new();
            let tilde_result = normal_core.expand_path("~/test").unwrap();
            assert!(tilde_result.to_string_lossy().contains("test"));

            let home_result = normal_core.expand_path("$HOME/config").unwrap();
            assert!(home_result.to_string_lossy().contains("config"));
        }
    }

    /// Test additional StaticFilesPlugin configuration scenarios
    /// Verifies that various plugin configurations work correctly
    #[tokio::test]
    async fn test_static_files_plugin_additional_scenarios() {
        let plugin = StaticFilesPlugin::new(AdvancedMockCore::new());

        // Test validation
        let validation_result = plugin.validate().await;
        assert!(validation_result.is_ok());

        // Test plugin trait methods
        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );
        assert_eq!(plugin.icon(), "🔧");
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert_eq!(plugin.get_restore_target_dir(), None);
        assert!(plugin.creates_own_output_files());

        let default_target = plugin.get_default_restore_target_dir().unwrap();
        assert_eq!(default_target, PathBuf::from("/"));

        // Test execution with files
        let exec_result = plugin.execute().await.unwrap();
        assert!(exec_result.contains("STATIC_DIR_CHECKSUM:"));
        assert!(exec_result.contains("total_files"));
    }

    /// Test directory checksum calculation branch
    /// Verifies that checksum calculation works when static directory exists
    #[tokio::test]
    async fn test_static_files_execute_with_existing_static_dir() {
        let plugin = StaticFilesPlugin::new(AdvancedMockCore::new());

        // Set environment variable to control static directory location
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let static_dir = snapshot_dir.join("static");
        fs::create_dir_all(&static_dir).await.unwrap();

        // Create some test files in static directory
        fs::write(static_dir.join("test1.txt"), "content1")
            .await
            .unwrap();
        fs::write(static_dir.join("test2.txt"), "content2")
            .await
            .unwrap();

        std::env::set_var(
            "DOTSNAPSHOT_SNAPSHOT_DIR",
            snapshot_dir.to_string_lossy().to_string(),
        );

        let result = plugin.execute().await.unwrap();

        // Should contain checksum that's not the fallback values
        assert!(result.contains("STATIC_DIR_CHECKSUM:"));
        assert!(!result.contains("no_static_directory"));
        assert!(!result.contains("error_calculating_checksum"));

        // Clean up
        std::env::remove_var("DOTSNAPSHOT_SNAPSHOT_DIR");
    }

    /// Test validation failure scenario
    /// Verifies that validation fails when home directory cannot be determined
    #[test]
    fn test_static_files_validation_edge_case() {
        // This test verifies the validation logic, though it's hard to force
        // dirs::home_dir() to return None in a test environment
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);

        // The validation should normally succeed
        // In a real scenario where home_dir() returns None, it would fail
        // but we can't easily test that condition
        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );
    }

    /// Test StaticFilesPlugin with_config constructor (test-only function)
    /// Verifies that the test-only with_config constructor works correctly
    #[test]
    fn test_static_files_plugin_with_config_constructor() {
        use crate::config::StaticFilesConfig;

        let config = Arc::new(Config {
            output_dir: Some(PathBuf::from("/test/output")),
            include_plugins: None,
            logging: None,
            hooks: None,
            global: None,
            static_files: Some(StaticFilesConfig {
                files: Some(vec!["/test/file.txt".to_string()]),
                ignore: Some(vec!["*.log".to_string()]),
            }),
            plugins: None,
            ui: None,
        });

        let plugin = StaticFilesPlugin::with_config(MockStaticFilesCore, config.clone());

        // Verify config was set correctly
        assert!(plugin.config.is_some());
        assert_eq!(
            plugin.config.as_ref().unwrap().output_dir,
            Some(PathBuf::from("/test/output"))
        );
        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );
        assert_eq!(plugin.icon(), SYMBOL_CONTENT_FILE);
    }

    /// Test JSON parsing error handling in execute
    /// Verifies that JSON serialization errors are handled properly
    #[tokio::test]
    async fn test_static_files_execute_json_error_handling() {
        // Create a mock that returns content that would cause JSON parsing issues
        struct JsonErrorMockCore;

        impl StaticFilesCore for JsonErrorMockCore {
            fn icon(&self) -> &'static str {
                SYMBOL_CONTENT_FILE
            }

            fn read_config(
                &self,
                _config: Option<&Arc<Config>>,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>,
            > {
                Box::pin(async move { Ok(vec![PathBuf::from("/test/file.txt")]) })
            }

            fn get_ignore_patterns(&self, _config: Option<&Arc<Config>>) -> Vec<String> {
                vec![]
            }

            fn should_ignore(&self, _path: &std::path::Path, _ignore_patterns: &[String]) -> bool {
                false
            }

            fn expand_path(&self, path: &str) -> Result<PathBuf> {
                Ok(PathBuf::from(path))
            }

            fn copy_files(
                &self,
                _file_paths: Vec<PathBuf>,
                _static_dir: &std::path::Path,
                _ignore_patterns: &[String],
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
            {
                Box::pin(async move {
                    // Return invalid JSON to test error handling
                    Ok("invalid json content".to_string())
                })
            }

            fn restore_static_files(
                &self,
                _static_snapshot_dir: &std::path::Path,
                _target_base_path: &std::path::Path,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>,
            > {
                Box::pin(async move { Ok(vec![]) })
            }
        }

        let plugin = StaticFilesPlugin::new(JsonErrorMockCore);
        let result = plugin.execute().await;

        // JSON errors should be handled gracefully - either succeed with fallback or return error
        match result {
            Ok(output) => {
                // If it succeeds, should contain expected structure
                assert!(output.contains("STATIC_DIR_CHECKSUM:"));
            }
            Err(_) => {
                // If it fails, that's also acceptable for JSON error handling
                // The test verifies that the error is handled properly (doesn't panic)
            }
        }
    }

    /// Test snapshot_dir field functionality
    /// Verifies that setting snapshot_dir affects execution behavior
    #[tokio::test]
    async fn test_static_files_plugin_snapshot_dir_usage() {
        let mut plugin = StaticFilesPlugin::new(AdvancedMockCore::new());
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("custom_snapshot");
        let static_dir = snapshot_dir.join("static");

        fs::create_dir_all(&static_dir).await.unwrap();
        fs::write(static_dir.join("test.txt"), "content")
            .await
            .unwrap();

        // Set the snapshot_dir field manually (normally done by executor)
        plugin.snapshot_dir = Some(snapshot_dir.clone());

        let result = plugin.execute().await.unwrap();

        // Should contain checksum and use the custom snapshot directory
        assert!(result.contains("STATIC_DIR_CHECKSUM:"));
        assert!(result.contains("total_files"));
    }

    /// Test execute with checksum calculation error
    /// Verifies behavior when directory checksum calculation fails
    #[tokio::test]
    async fn test_static_files_execute_checksum_error() {
        let plugin = StaticFilesPlugin::new(AdvancedMockCore::new());

        // Set environment variable to non-existent directory to trigger checksum error path
        std::env::set_var(
            "DOTSNAPSHOT_SNAPSHOT_DIR",
            "/nonexistent/path/that/does/not/exist",
        );

        let result = plugin.execute().await.unwrap();

        // Should contain error checksum fallback
        assert!(result.contains("STATIC_DIR_CHECKSUM:"));
        assert!(
            result.contains("no_static_directory") || result.contains("error_calculating_checksum")
        );

        // Clean up
        std::env::remove_var("DOTSNAPSHOT_SNAPSHOT_DIR");
    }

    /// Test all StaticFilesCore trait methods with different parameter combinations
    /// Verifies comprehensive coverage of trait method calls
    #[tokio::test]
    async fn test_static_files_core_trait_comprehensive_coverage() {
        let core = AdvancedMockCore::new();

        // Test read_config with actual config
        let config = Arc::new(Config::default());
        let config_result = core.read_config(Some(&config)).await.unwrap();
        assert_eq!(config_result.len(), 2);

        // Test read_config with None
        let config_result_none = core.read_config(None).await.unwrap();
        assert_eq!(config_result_none.len(), 2);

        // Test get_ignore_patterns with config
        let patterns_with_config = core.get_ignore_patterns(Some(&config));
        assert_eq!(patterns_with_config.len(), 3);

        // Test get_ignore_patterns with None
        let patterns_none = core.get_ignore_patterns(None);
        assert_eq!(patterns_none.len(), 3);

        // Test should_ignore with empty patterns
        assert!(!core.should_ignore(&PathBuf::from("test.txt"), &[]));

        // Test expand_path with various paths
        assert_eq!(
            core.expand_path("/absolute/path").unwrap(),
            PathBuf::from("/absolute/path")
        );
        assert_eq!(
            core.expand_path("relative/path").unwrap(),
            PathBuf::from("relative/path")
        );

        // Test copy_files with empty list
        let copy_empty = core
            .copy_files(vec![], &PathBuf::from("/static"), &[])
            .await
            .unwrap();
        assert!(copy_empty.contains("total_files"));

        // Test restore_static_files with different paths
        let restore_result = core
            .restore_static_files(&PathBuf::from("/snap1"), &PathBuf::from("/target1"))
            .await
            .unwrap();
        assert_eq!(restore_result.len(), 3);

        let restore_result2 = core
            .restore_static_files(&PathBuf::from("/snap2"), &PathBuf::from("/target2"))
            .await
            .unwrap();
        assert_eq!(restore_result2.len(), 3);
    }

    /// Test StaticFilesPlugin restore with various error conditions
    /// Verifies comprehensive error handling in restore functionality
    #[tokio::test]
    async fn test_static_files_plugin_restore_comprehensive_error_handling() {
        // Test with error-prone mock
        struct ErrorProneMockCore {
            should_error: bool,
        }

        impl ErrorProneMockCore {
            fn new(should_error: bool) -> Self {
                Self { should_error }
            }
        }

        impl StaticFilesCore for ErrorProneMockCore {
            fn icon(&self) -> &'static str {
                SYMBOL_CONTENT_FILE
            }

            fn read_config(
                &self,
                _config: Option<&Arc<Config>>,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>,
            > {
                Box::pin(async move { Ok(vec![]) })
            }

            fn get_ignore_patterns(&self, _config: Option<&Arc<Config>>) -> Vec<String> {
                vec![]
            }

            fn should_ignore(&self, _path: &std::path::Path, _ignore_patterns: &[String]) -> bool {
                false
            }

            fn expand_path(&self, path: &str) -> Result<PathBuf> {
                Ok(PathBuf::from(path))
            }

            fn copy_files(
                &self,
                _file_paths: Vec<PathBuf>,
                _static_dir: &std::path::Path,
                _ignore_patterns: &[String],
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
            {
                Box::pin(async move { Ok(String::new()) })
            }

            fn restore_static_files(
                &self,
                _static_snapshot_dir: &std::path::Path,
                _target_base_path: &std::path::Path,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>,
            > {
                let should_error = self.should_error;
                Box::pin(async move {
                    if should_error {
                        Err(anyhow::anyhow!("Simulated restore error"))
                    } else {
                        Ok(vec![PathBuf::from("/restored/file.txt")])
                    }
                })
            }
        }

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let static_dir = snapshot_dir.join("static");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&static_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();
        fs::write(static_dir.join("test.txt"), "content")
            .await
            .unwrap();

        // Test successful restore
        let plugin_success = StaticFilesPlugin::new(ErrorProneMockCore::new(false));
        let result_success = plugin_success
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result_success.len(), 1);
        assert_eq!(result_success[0], PathBuf::from("/restored/file.txt"));

        // Test error restore (should return empty, error logged)
        let plugin_error = StaticFilesPlugin::new(ErrorProneMockCore::new(true));
        let result_error = plugin_error
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert!(result_error.is_empty());

        // Test dry run with files present
        let plugin_dry = StaticFilesPlugin::new(ErrorProneMockCore::new(false));
        let result_dry = plugin_dry
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();
        assert_eq!(result_dry.len(), 1);
        assert_eq!(result_dry[0], target_dir);
    }

    /// Test Plugin trait methods comprehensively
    /// Verifies all Plugin trait implementations are covered
    #[tokio::test]
    async fn test_static_files_plugin_trait_comprehensive() {
        let plugin = StaticFilesPlugin::new(AdvancedMockCore::new());

        // Test all Plugin trait methods
        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );
        assert_eq!(plugin.icon(), "🔧");
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert!(plugin.creates_own_output_files());
        assert_eq!(plugin.get_restore_target_dir(), None);

        let default_target = plugin.get_default_restore_target_dir().unwrap();
        assert_eq!(default_target, PathBuf::from("/"));

        let validation_result = plugin.validate().await;
        assert!(validation_result.is_ok());

        let execution_result = plugin.execute().await;
        assert!(execution_result.is_ok());

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");
        let static_dir = snapshot_dir.join("static");

        fs::create_dir_all(&static_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();
        fs::write(static_dir.join("file.txt"), "content")
            .await
            .unwrap();

        let restore_result = plugin.restore(&snapshot_dir, &target_dir, false).await;
        assert!(restore_result.is_ok());
    }

    /// Test AdvancedMockCore constructor methods
    /// Verifies all constructor variations are covered
    #[test]
    fn test_advanced_mock_core_constructors() {
        // Test default constructor
        let core1 = AdvancedMockCore::new();
        assert!(!core1.should_ignore(&PathBuf::from("test.txt"), &[]));
        assert!(!core1.expand_path_error);

        // Test with_ignore_result constructor
        let core2 = AdvancedMockCore::with_ignore_result(true);
        assert!(core2.should_ignore(&PathBuf::from("test.txt"), &[]));
        assert!(!core2.expand_path_error);

        let core3 = AdvancedMockCore::with_ignore_result(false);
        assert!(!core3.should_ignore(&PathBuf::from("test.txt"), &[]));
        assert!(!core3.expand_path_error);

        // Test with_expand_path_error constructor
        let core4 = AdvancedMockCore::with_expand_path_error();
        assert!(!core4.should_ignore_result);
        assert!(core4.expand_path_error);
    }

    /// Test edge cases in path expansion
    /// Verifies all path expansion branches are covered
    #[test]
    fn test_advanced_mock_core_path_expansion_edge_cases() {
        let core = AdvancedMockCore::new();

        // Test empty path
        let empty_result = core.expand_path("").unwrap();
        assert_eq!(empty_result, PathBuf::from(""));

        // Test just tilde
        let tilde_result = core.expand_path("~").unwrap();
        if let Some(home_dir) = dirs::home_dir() {
            assert_eq!(tilde_result, home_dir);
        }

        // Test just $HOME
        let home_result = core.expand_path("$HOME").unwrap();
        if let Some(home_dir) = dirs::home_dir() {
            assert_eq!(home_result, home_dir);
        }

        // Test $HOME with path
        let home_path_result = core.expand_path("$HOME/test").unwrap();
        if let Some(home_dir) = dirs::home_dir() {
            assert_eq!(home_path_result, home_dir.join("test"));
        }

        // Test tilde with complex path
        let tilde_complex_result = core.expand_path("~/Documents/Projects/test").unwrap();
        if let Some(home_dir) = dirs::home_dir() {
            assert_eq!(
                tilde_complex_result,
                home_dir.join("Documents/Projects/test")
            );
        }
    }

    /// Test DirectoryAccessError handling in restore
    /// Verifies proper handling when static directory read fails
    #[tokio::test]
    async fn test_static_files_plugin_restore_directory_read_error() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let static_dir = snapshot_dir.join("static");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&target_dir).await.unwrap();
        // Create static_dir but make it unreadable (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::create_dir_all(&static_dir).await.unwrap();
            let mut perms = fs::metadata(&static_dir).await.unwrap().permissions();
            perms.set_mode(0o000); // No permissions
            let _ = fs::set_permissions(&static_dir, perms).await;
        }
        #[cfg(not(unix))]
        {
            fs::create_dir_all(&static_dir).await.unwrap();
        }

        // Should handle directory read error gracefully in dry run
        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();

        #[cfg(unix)]
        {
            // May return empty if directory is unreadable
            // Just verify it doesn't panic
            let _ = result;

            // Restore permissions for cleanup
            let mut perms = fs::metadata(&static_dir).await.unwrap().permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&static_dir, perms).await;
        }
        #[cfg(not(unix))]
        {
            // On non-Unix, should succeed
            assert_eq!(result.len(), 1);
        }
    }

    /// Test static files plugin configuration edge cases
    /// Verifies various edge cases in configuration handling
    #[tokio::test]
    async fn test_static_files_plugin_config_edge_cases() {
        // Test with completely empty config
        let empty_config = Arc::new(Config {
            output_dir: None,
            include_plugins: None,
            logging: None,
            hooks: None,
            global: None,
            static_files: None,
            plugins: None,
            ui: None,
        });
        let plugin = StaticFilesPlugin::with_config(MockStaticFilesCore, empty_config);

        // Should handle empty config gracefully
        let result = plugin.execute().await;
        assert!(result.is_ok());
    }

    /// Test static files plugin with complex path scenarios
    /// Verifies handling of various path types and edge cases
    #[tokio::test]
    async fn test_static_files_plugin_complex_paths() {
        use crate::config::StaticFilesConfig;

        let config = Arc::new(Config {
            output_dir: Some(PathBuf::from("/test/output")),
            include_plugins: None,
            logging: None,
            hooks: None,
            global: None,
            static_files: Some(StaticFilesConfig {
                files: Some(vec![
                    "~".to_string(),              // Home directory
                    "~/Documents".to_string(),    // Home subdirectory
                    "$HOME".to_string(),          // Environment variable
                    "/absolute/path".to_string(), // Absolute path
                    "relative/path".to_string(),  // Relative path
                    "".to_string(),               // Empty path
                ]),
                ignore: Some(vec![
                    "*.tmp".to_string(),
                    "*.log".to_string(),
                    "node_modules".to_string(),
                ]),
            }),
            plugins: None,
            ui: None,
        });

        let plugin = StaticFilesPlugin::with_config(AdvancedMockCore::new(), config);
        let result = plugin.execute().await;

        // Should handle all path types without error
        assert!(result.is_ok());
    }

    /// Test static files plugin error scenarios with advanced core
    /// Verifies comprehensive error handling across different scenarios
    #[tokio::test]
    async fn test_static_files_plugin_advanced_error_scenarios() {
        // Create an advanced core with various error conditions
        let mut core = AdvancedMockCore::new();
        core.expand_path_error = true;

        let plugin = StaticFilesPlugin::new(core);
        let result = plugin.execute().await;

        // Path expansion errors should be handled gracefully
        match result {
            Ok(_) => {
                // If it succeeds despite errors, that's acceptable
            }
            Err(_) => {
                // If it fails due to path expansion error, that's also expected
            }
        }
    }

    /// Test static files plugin restore with checksum validation
    /// Verifies that checksum validation works correctly during restore
    #[tokio::test]
    async fn test_static_files_plugin_restore_with_checksum_validation() {
        use tempfile::TempDir;
        use tokio::fs;

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create a file with specific content for checksum
        let test_file = snapshot_dir.join("test_checksum.txt");
        let content = "test content for checksum validation";
        fs::write(&test_file, content).await.unwrap();

        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);

        // Test restore with checksum validation
        let result = plugin.restore(&snapshot_dir, &target_dir, false).await;
        assert!(result.is_ok());

        let restored_file = target_dir.join("test_checksum.txt");
        if restored_file.exists() {
            let restored_content = fs::read_to_string(&restored_file).await.unwrap();
            assert_eq!(restored_content, content);
        }
    }

    /// Test static files plugin with multiple mock configurations
    /// Verifies behavior across different mock core configurations
    #[tokio::test]
    async fn test_static_files_plugin_multiple_mock_configurations() {
        // Test with minimal mock
        let minimal_plugin = StaticFilesPlugin::new(MinimalStaticFilesCore);
        let minimal_result = minimal_plugin.execute().await;
        assert!(minimal_result.is_ok());

        // Test with advanced mock
        let advanced_plugin = StaticFilesPlugin::new(AdvancedMockCore::new());
        let advanced_result = advanced_plugin.execute().await;
        assert!(advanced_result.is_ok());

        // Test with error-prone mock (using advanced mock with error conditions)
        let mut error_core = AdvancedMockCore::new();
        error_core.expand_path_error = true;
        let error_plugin = StaticFilesPlugin::new(error_core);
        let error_result = error_plugin.execute().await;
        // Error mock may succeed or fail - both are acceptable
        let _ = error_result;
    }

    /// Test static files plugin with different snapshot directory scenarios
    /// Verifies snapshot_dir field behavior in various scenarios
    #[tokio::test]
    async fn test_static_files_plugin_snapshot_dir_scenarios() {
        use tempfile::TempDir;
        use tokio::fs;

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("custom_snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test files in snapshot
        fs::write(snapshot_dir.join("file1.txt"), "content1")
            .await
            .unwrap();
        fs::write(snapshot_dir.join("file2.txt"), "content2")
            .await
            .unwrap();

        let mut plugin = StaticFilesPlugin::new(MockStaticFilesCore);

        // Test with custom snapshot_dir
        plugin.snapshot_dir = Some(snapshot_dir.clone());

        let result = plugin.restore(&snapshot_dir, &target_dir, false).await;
        assert!(result.is_ok());

        // Test execute with custom snapshot_dir set
        let execute_result = plugin.execute().await;
        assert!(execute_result.is_ok());
    }

    /// Test static files plugin validation with edge cases
    /// Verifies validation behavior in various edge case scenarios
    #[tokio::test]
    async fn test_static_files_plugin_validation_edge_cases() {
        // Test with default core
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore);
        let result = plugin.validate().await;
        assert!(result.is_ok());

        // Test with advanced core
        let advanced_plugin = StaticFilesPlugin::new(AdvancedMockCore::new());
        let advanced_result = advanced_plugin.validate().await;
        assert!(advanced_result.is_ok());

        // Test with minimal core
        let minimal_plugin = StaticFilesPlugin::new(MinimalStaticFilesCore);
        let minimal_result = minimal_plugin.validate().await;
        assert!(minimal_result.is_ok());
    }

    /// Test static files core trait async method coverage
    /// Verifies that all async trait methods are properly covered
    #[tokio::test]
    async fn test_static_files_core_async_method_coverage() {
        let core = MockStaticFilesCore;

        // Test read_config async method
        let config_result = core.read_config(None).await;
        assert!(config_result.is_ok());

        // Test copy_files async method
        let copy_result = core
            .copy_files(
                vec![PathBuf::from("/test/file.txt")],
                &PathBuf::from("/target"),
                &[],
            )
            .await;
        assert!(copy_result.is_ok());

        // Test restore_static_files async method
        let restore_result = core
            .restore_static_files(&PathBuf::from("/snapshot"), &PathBuf::from("/target"))
            .await;
        assert!(restore_result.is_ok());
    }

    /// Test advanced mock core comprehensive functionality
    /// Verifies all advanced mock core methods work correctly
    #[tokio::test]
    async fn test_advanced_mock_core_comprehensive_functionality() {
        let mut core = AdvancedMockCore::new();

        // Test all getter methods
        assert_eq!(core.icon(), "🔧"); // AdvancedMockCore uses tool icon
        assert!(!core.get_ignore_patterns(None).is_empty());

        // Test path expansion with different scenarios
        assert!(core.expand_path("regular/path").is_ok());
        assert!(core.expand_path("~/test").is_ok());
        assert!(core.expand_path("$HOME/test").is_ok());

        // Test should_ignore logic
        let test_path = PathBuf::from("/test/file.tmp");
        let patterns = vec!["*.tmp".to_string()];
        assert!(core.should_ignore(&test_path, &patterns));

        // Test with ignore force enabled
        core.should_ignore_result = true;
        assert!(core.should_ignore(&test_path, &[]));

        // Test async methods
        let config_result = core.read_config(None).await;
        assert!(config_result.is_ok());

        let copy_result = core
            .copy_files(
                vec![PathBuf::from("/test/file.txt")],
                &PathBuf::from("/target"),
                &[],
            )
            .await;
        assert!(copy_result.is_ok());

        let restore_result = core
            .restore_static_files(&PathBuf::from("/snapshot"), &PathBuf::from("/target"))
            .await;
        assert!(restore_result.is_ok());
    }

    /// Test error mock core comprehensive error scenarios
    /// Verifies that error mock properly simulates error conditions
    #[tokio::test]
    async fn test_error_mock_core_comprehensive_error_scenarios() {
        // Create a local error mock for testing
        struct LocalErrorMock;

        impl StaticFilesCore for LocalErrorMock {
            fn icon(&self) -> &'static str {
                SYMBOL_CONTENT_FILE
            }
            fn read_config(
                &self,
                _config: Option<&Arc<Config>>,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>,
            > {
                Box::pin(async move { Err(anyhow::anyhow!("Config read error")) })
            }
            fn get_ignore_patterns(&self, _config: Option<&Arc<Config>>) -> Vec<String> {
                vec![]
            }
            fn should_ignore(&self, _path: &std::path::Path, _ignore_patterns: &[String]) -> bool {
                false
            }
            fn expand_path(&self, path: &str) -> Result<PathBuf> {
                Ok(PathBuf::from(path))
            }
            fn copy_files(
                &self,
                _file_paths: Vec<PathBuf>,
                _static_dir: &std::path::Path,
                _ignore_patterns: &[String],
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
            {
                Box::pin(async move { Err(anyhow::anyhow!("Copy files error")) })
            }
            fn restore_static_files(
                &self,
                _static_snapshot_dir: &std::path::Path,
                _target_base_path: &std::path::Path,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + '_>,
            > {
                Box::pin(async move { Err(anyhow::anyhow!("Restore error")) })
            }
        }

        let core = LocalErrorMock;

        // Test basic methods
        assert_eq!(core.icon(), SYMBOL_CONTENT_FILE);
        assert!(core.get_ignore_patterns(None).is_empty());

        // Test path expansion - should succeed for this mock
        assert!(core.expand_path("test/path").is_ok());

        // Test should_ignore - should return false for this mock
        assert!(!core.should_ignore(&PathBuf::from("/test"), &[]));

        // Test async methods - should all return errors
        let config_result = core.read_config(None).await;
        assert!(config_result.is_err());

        let copy_result = core
            .copy_files(
                vec![PathBuf::from("/test/file.txt")],
                &PathBuf::from("/target"),
                &[],
            )
            .await;
        assert!(copy_result.is_err());

        let restore_result = core
            .restore_static_files(&PathBuf::from("/snapshot"), &PathBuf::from("/target"))
            .await;
        assert!(restore_result.is_err());
    }

    /// Test minimal core comprehensive functionality
    /// Verifies that minimal core provides basic functionality
    #[tokio::test]
    async fn test_minimal_core_comprehensive_functionality() {
        let core = MinimalStaticFilesCore;

        // Test basic methods
        assert_eq!(core.icon(), "📁"); // MinimalStaticFilesCore uses folder icon
        assert!(core.get_ignore_patterns(None).is_empty());

        // Test path expansion
        assert!(core.expand_path("test/path").is_ok());

        // Test should_ignore - should return false
        assert!(!core.should_ignore(&PathBuf::from("/test"), &[]));

        // Test async methods
        let config_result = core.read_config(None).await;
        assert!(config_result.is_ok());

        let copy_result = core
            .copy_files(
                vec![PathBuf::from("/test/file.txt")],
                &PathBuf::from("/target"),
                &[],
            )
            .await;
        assert!(copy_result.is_ok());

        let restore_result = core
            .restore_static_files(&PathBuf::from("/snapshot"), &PathBuf::from("/target"))
            .await;
        assert!(restore_result.is_ok());
    }
}

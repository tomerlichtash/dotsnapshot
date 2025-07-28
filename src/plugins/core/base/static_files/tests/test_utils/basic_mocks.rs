//! Basic mock implementations for StaticFilesCore trait testing

use crate::config::Config;
use crate::plugins::core::base::static_files::StaticFilesCore;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

/// Simple mock for basic testing scenarios
/// Provides minimal functionality with configurable behavior
#[derive(Debug, Clone)]
pub struct MockStaticFilesCore {
    pub files: Vec<PathBuf>,
    pub ignore_patterns: Vec<String>,
    pub should_error: bool,
    pub copy_files_error: bool,
    pub expand_path_error: bool,
    pub files_for_read_config: Option<Vec<PathBuf>>,
    pub path_expansion_map: HashMap<String, String>,
}

impl Default for MockStaticFilesCore {
    fn default() -> Self {
        Self::new()
    }
}

impl MockStaticFilesCore {
    pub fn new() -> Self {
        Self {
            files: vec![],
            ignore_patterns: vec![],
            should_error: false,
            copy_files_error: false,
            expand_path_error: false,
            files_for_read_config: None,
            path_expansion_map: HashMap::new(),
        }
    }

    pub fn with_files(mut self, files: Vec<PathBuf>) -> Self {
        self.files = files;
        self
    }

    pub fn with_copy_files_error(mut self) -> Self {
        self.copy_files_error = true;
        self
    }

    pub fn with_files_for_read_config(mut self, files: Vec<PathBuf>) -> Self {
        self.files_for_read_config = Some(files);
        self
    }
}

impl StaticFilesCore for MockStaticFilesCore {
    fn icon(&self) -> &'static str {
        "📄"
    }

    fn read_config<'a>(
        &'a self,
        _config: Option<&'a Arc<Config>>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + 'a>> {
        Box::pin(async move {
            if self.should_error {
                return Err(anyhow::anyhow!("Mock error reading config"));
            }
            Ok(self
                .files_for_read_config
                .clone()
                .unwrap_or_else(|| self.files.clone()))
        })
    }

    fn get_ignore_patterns(&self, _config: Option<&Arc<Config>>) -> Vec<String> {
        self.ignore_patterns.clone()
    }

    fn should_ignore(&self, path: &std::path::Path, ignore_patterns: &[String]) -> bool {
        let path_str = path.to_string_lossy();
        ignore_patterns.iter().any(|pattern| {
            if pattern.contains('*') {
                // Simple wildcard matching
                let pattern_parts: Vec<&str> = pattern.split('*').collect();
                if pattern_parts.len() == 2 {
                    path_str.starts_with(pattern_parts[0]) && path_str.ends_with(pattern_parts[1])
                } else {
                    false
                }
            } else {
                path_str.contains(pattern)
            }
        })
    }

    fn expand_path(&self, path: &str) -> Result<PathBuf> {
        if self.expand_path_error {
            return Err(anyhow::anyhow!("Mock error expanding path"));
        }

        if let Some(expanded) = self.path_expansion_map.get(path) {
            return Ok(PathBuf::from(expanded));
        }

        // Default expansion logic
        if path.starts_with("~/") {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
            Ok(PathBuf::from(path.replace("~", &home)))
        } else {
            Ok(PathBuf::from(path))
        }
    }

    fn copy_files<'a>(
        &'a self,
        file_paths: Vec<PathBuf>,
        static_dir: &'a std::path::Path,
        _ignore_patterns: &'a [String],
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            if self.copy_files_error {
                return Err(anyhow::anyhow!("Mock error copying files"));
            }

            // Return a JSON summary like the real implementation
            let summary = serde_json::json!({
                "summary": {
                    "total_files": file_paths.len(),
                    "copied": file_paths.len(),
                    "failed": 0,
                    "target_directory": static_dir.display().to_string()
                }
            });
            Ok(serde_json::to_string_pretty(&summary)?)
        })
    }

    fn restore_static_files<'a>(
        &'a self,
        _static_snapshot_dir: &'a std::path::Path,
        _target_base_path: &'a std::path::Path,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + 'a>> {
        Box::pin(async move {
            if self.should_error {
                return Err(anyhow::anyhow!("Mock error restoring files"));
            }
            Ok(self.files.clone())
        })
    }
}

/// Minimal mock for testing basic functionality
/// Provides empty responses for all operations
#[derive(Debug, Clone)]
pub struct MinimalStaticFilesCore;

impl StaticFilesCore for MinimalStaticFilesCore {
    fn icon(&self) -> &'static str {
        "📁"
    }

    fn read_config<'a>(
        &'a self,
        _config: Option<&'a Arc<Config>>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + 'a>> {
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

    fn copy_files<'a>(
        &'a self,
        file_paths: Vec<PathBuf>,
        static_dir: &'a std::path::Path,
        _ignore_patterns: &'a [String],
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let summary = serde_json::json!({
                "summary": {
                    "total_files": file_paths.len(),
                    "copied": 0,
                    "failed": 0,
                    "target_directory": static_dir.display().to_string()
                }
            });
            Ok(serde_json::to_string_pretty(&summary)?)
        })
    }

    fn restore_static_files<'a>(
        &'a self,
        _static_snapshot_dir: &'a std::path::Path,
        _target_base_path: &'a std::path::Path,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + 'a>> {
        Box::pin(async move { Ok(vec![]) })
    }
}

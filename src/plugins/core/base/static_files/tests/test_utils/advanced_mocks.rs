//! Advanced mock implementations for comprehensive testing scenarios

use crate::config::Config;
use crate::plugins::core::base::static_files::StaticFilesCore;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

/// Advanced mock with configurable behavior for comprehensive testing
/// Supports complex scenarios with custom responses and error conditions
#[derive(Debug, Clone)]
pub struct AdvancedMockCore {
    pub files: Vec<PathBuf>,
    pub ignore_patterns: Vec<String>,
    pub error_scenarios: HashMap<String, String>,
    pub path_expansions: HashMap<String, String>,
    pub restore_results: Vec<PathBuf>,
    pub custom_icon: Option<&'static str>,
}

impl Default for AdvancedMockCore {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvancedMockCore {
    pub fn new() -> Self {
        Self {
            files: vec![
                PathBuf::from("/test/file1.txt"),
                PathBuf::from("/test/file2.txt"),
            ],
            ignore_patterns: vec!["*.tmp".to_string(), "*.log".to_string()],
            error_scenarios: HashMap::new(),
            path_expansions: HashMap::new(),
            restore_results: vec![],
            custom_icon: None,
        }
    }

    pub fn with_files(mut self, files: Vec<PathBuf>) -> Self {
        self.files = files;
        self
    }

    pub fn with_ignore_patterns(mut self, patterns: Vec<String>) -> Self {
        self.ignore_patterns = patterns;
        self
    }

    pub fn with_error(mut self, operation: &str, error: &str) -> Self {
        self.error_scenarios
            .insert(operation.to_string(), error.to_string());
        self
    }

    pub fn with_restore_results(mut self, results: Vec<PathBuf>) -> Self {
        self.restore_results = results;
        self
    }

    pub fn with_ignore_result(mut self, always_ignore: bool) -> Self {
        if always_ignore {
            self.error_scenarios
                .insert("should_ignore".to_string(), "force_true".to_string());
        }
        self
    }
}

impl StaticFilesCore for AdvancedMockCore {
    fn icon(&self) -> &'static str {
        self.custom_icon.unwrap_or("🔧")
    }

    fn read_config<'a>(
        &'a self,
        _config: Option<&'a Arc<Config>>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(error) = self.error_scenarios.get("read_config") {
                return Err(anyhow::anyhow!("Advanced mock error: {}", error));
            }
            Ok(self.files.clone())
        })
    }

    fn get_ignore_patterns(&self, _config: Option<&Arc<Config>>) -> Vec<String> {
        self.ignore_patterns.clone()
    }

    fn should_ignore(&self, path: &std::path::Path, ignore_patterns: &[String]) -> bool {
        if self
            .error_scenarios
            .get("should_ignore")
            .map(|s| s.as_str())
            == Some("force_true")
        {
            return true;
        }

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
        if let Some(error) = self.error_scenarios.get("expand_path") {
            return Err(anyhow::anyhow!("Advanced mock error: {}", error));
        }

        if let Some(expanded) = self.path_expansions.get(path) {
            return Ok(PathBuf::from(expanded));
        }

        // Advanced expansion logic
        if path.starts_with("~/") {
            let home = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/home/user"))
                .to_string_lossy()
                .to_string();
            Ok(PathBuf::from(path.replace("~", &home)))
        } else if path.starts_with("$HOME/") {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
            Ok(PathBuf::from(path.replace("$HOME", &home)))
        } else {
            Ok(PathBuf::from(path))
        }
    }

    fn copy_files<'a>(
        &'a self,
        file_paths: Vec<PathBuf>,
        static_dir: &'a std::path::Path,
        ignore_patterns: &'a [String],
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(error) = self.error_scenarios.get("copy_files") {
                return Err(anyhow::anyhow!("Advanced mock error: {}", error));
            }

            // Filter files based on ignore patterns
            let mut copied = 0;
            let mut failed = 0;

            for file_path in &file_paths {
                if self.should_ignore(file_path, ignore_patterns) {
                    failed += 1;
                } else {
                    copied += 1;
                }
            }

            let summary = serde_json::json!({
                "summary": {
                    "total_files": file_paths.len(),
                    "copied": copied,
                    "failed": failed,
                    "target_directory": static_dir.display().to_string(),
                    "ignore_patterns": ignore_patterns,
                },
                "details": {
                    "processed_files": file_paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                    "ignored_patterns": ignore_patterns
                }
            });
            Ok(serde_json::to_string_pretty(&summary)?)
        })
    }

    fn restore_static_files<'a>(
        &'a self,
        static_snapshot_dir: &'a std::path::Path,
        target_base_path: &'a std::path::Path,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(error) = self.error_scenarios.get("restore_static_files") {
                return Err(anyhow::anyhow!("Advanced mock error: {}", error));
            }

            if !self.restore_results.is_empty() {
                return Ok(self.restore_results.clone());
            }

            // Return some realistic restore paths based on input paths
            let mut restored_files = vec![
                target_base_path.join("restored_file1.txt"),
                target_base_path.join("restored_file2.txt"),
            ];

            // Add snapshot directory to simulate realistic behavior
            if static_snapshot_dir.exists()
                || static_snapshot_dir.to_string_lossy().contains("snapshot")
            {
                restored_files.push(target_base_path.join("from_snapshot.txt"));
            }

            Ok(restored_files)
        })
    }
}

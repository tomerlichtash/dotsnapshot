//! Error-focused mock implementations for testing failure scenarios

use crate::config::Config;
use crate::plugins::core::base::static_files::StaticFilesCore;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

/// Mock that generates specific errors for testing error handling
/// Designed to fail operations in controlled ways
#[derive(Debug, Clone)]
pub struct ErrorMockCore {
    pub operation_errors: HashMap<String, String>,
}

impl Default for ErrorMockCore {
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorMockCore {
    pub fn new() -> Self {
        Self {
            operation_errors: HashMap::new(),
        }
    }

    pub fn with_read_config_error(mut self) -> Self {
        self.operation_errors.insert(
            "read_config".to_string(),
            "Failed to read configuration".to_string(),
        );
        self
    }
}

impl StaticFilesCore for ErrorMockCore {
    fn icon(&self) -> &'static str {
        "❌"
    }

    fn read_config<'a>(
        &'a self,
        _config: Option<&'a Arc<Config>>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(error) = self.operation_errors.get("read_config") {
                return Err(anyhow::anyhow!("ErrorMockCore: {}", error));
            }
            Ok(vec![])
        })
    }

    fn get_ignore_patterns(&self, _config: Option<&Arc<Config>>) -> Vec<String> {
        vec![]
    }

    fn should_ignore(&self, _path: &std::path::Path, _ignore_patterns: &[String]) -> bool {
        false
    }

    fn expand_path(&self, path: &str) -> Result<PathBuf> {
        if let Some(error) = self.operation_errors.get("expand_path") {
            return Err(anyhow::anyhow!("ErrorMockCore: {}", error));
        }
        Ok(PathBuf::from(path))
    }

    fn copy_files<'a>(
        &'a self,
        _file_paths: Vec<PathBuf>,
        _static_dir: &'a std::path::Path,
        _ignore_patterns: &'a [String],
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(error) = self.operation_errors.get("copy_files") {
                return Err(anyhow::anyhow!("ErrorMockCore: {}", error));
            }
            Ok("{}".to_string())
        })
    }

    fn restore_static_files<'a>(
        &'a self,
        _static_snapshot_dir: &'a std::path::Path,
        _target_base_path: &'a std::path::Path,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(error) = self.operation_errors.get("restore_static_files") {
                return Err(anyhow::anyhow!("ErrorMockCore: {}", error));
            }
            Ok(vec![])
        })
    }
}

/// Mock specifically for JSON error scenarios
/// Returns malformed JSON to test error handling
#[derive(Debug, Clone)]
pub struct JsonErrorMockCore;

impl StaticFilesCore for JsonErrorMockCore {
    fn icon(&self) -> &'static str {
        "📊"
    }

    fn read_config<'a>(
        &'a self,
        _config: Option<&'a Arc<Config>>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + 'a>> {
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

    fn copy_files<'a>(
        &'a self,
        _file_paths: Vec<PathBuf>,
        _static_dir: &'a std::path::Path,
        _ignore_patterns: &'a [String],
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            // Return malformed JSON to test error handling
            Ok("{ invalid json content that will cause parsing errors".to_string())
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

/// Mock for testing error-prone scenarios during restoration
/// Simulates various failure modes during file restoration
#[derive(Debug, Clone)]
pub struct ErrorProneMockCore {
    pub should_fail_restore: bool,
}

impl Default for ErrorProneMockCore {
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorProneMockCore {
    pub fn new() -> Self {
        Self {
            should_fail_restore: false,
        }
    }

    pub fn with_restore_failure(mut self) -> Self {
        self.should_fail_restore = true;
        self
    }
}

impl StaticFilesCore for ErrorProneMockCore {
    fn icon(&self) -> &'static str {
        "⚠️"
    }

    fn read_config<'a>(
        &'a self,
        _config: Option<&'a Arc<Config>>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>>> + Send + 'a>> {
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
                    "failed": file_paths.len(),
                    "target_directory": static_dir.display().to_string(),
                    "error": "Simulated copy failure"
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
            if self.should_fail_restore {
                return Err(anyhow::anyhow!(
                    "ErrorProneMockCore: Simulated restore failure"
                ));
            }
            Ok(vec![])
        })
    }
}

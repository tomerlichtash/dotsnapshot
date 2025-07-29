//! Test modules for snapshot executor functionality

use crate::core::plugin::Plugin;
use anyhow::Result;
use async_trait::async_trait;

pub mod hooks;
pub mod performance;
pub mod plugins;
pub mod progress;
pub mod snapshots;
pub mod test_utils;

// Test-only symbol for mock plugins
const SYMBOL_ACTION_TEST: &str = "🧪";

/// Mock plugin implementation for testing executor functionality
pub struct TestPlugin {
    content: String,
    should_fail: bool,
    validation_error: Option<String>,
    creates_own_files: bool,
}

impl TestPlugin {
    pub fn new(content: String) -> Self {
        Self {
            content,
            should_fail: false,
            validation_error: None,
            creates_own_files: false,
        }
    }

    pub fn with_validation_error(mut self, error: String) -> Self {
        self.validation_error = Some(error);
        self
    }

    pub fn with_execution_failure(mut self) -> Self {
        self.should_fail = true;
        self
    }

    pub fn with_custom_file_handling(mut self) -> Self {
        self.creates_own_files = true;
        self
    }
}

#[async_trait]
impl Plugin for TestPlugin {
    fn description(&self) -> &str {
        "Test plugin for executor tests"
    }

    fn icon(&self) -> &str {
        SYMBOL_ACTION_TEST
    }

    async fn execute(&self) -> Result<String> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Test plugin execution failure"));
        }
        Ok(self.content.clone())
    }

    async fn validate(&self) -> Result<()> {
        if let Some(ref error) = self.validation_error {
            return Err(anyhow::anyhow!(error.clone()));
        }
        Ok(())
    }

    fn get_target_path(&self) -> Option<String> {
        None
    }

    fn get_output_file(&self) -> Option<String> {
        None
    }

    fn creates_own_output_files(&self) -> bool {
        self.creates_own_files
    }
}

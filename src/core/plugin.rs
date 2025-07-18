use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Represents the result of a plugin execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResult {
    pub plugin_name: String,
    pub content: String,
    pub checksum: String,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Core trait that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Returns the unique name of the plugin
    fn name(&self) -> &str;

    /// Returns the filename that will be used for the snapshot
    fn filename(&self) -> &str;

    /// Returns a description of what the plugin does
    fn description(&self) -> &str;

    /// Executes the plugin and returns the content to be saved
    async fn execute(&self) -> Result<String>;

    /// Validates that the plugin can run (e.g., required binaries exist)
    async fn validate(&self) -> Result<()>;

    /// Returns the expected output file path for this plugin
    fn output_path(&self, base_path: &Path) -> PathBuf {
        base_path.join(self.filename())
    }
}

/// Plugin registry for managing available plugins
#[derive(Default)]
pub struct PluginRegistry {
    plugins: Vec<Arc<dyn Plugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Registers a new plugin
    pub fn register(&mut self, plugin: Arc<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    /// Returns all registered plugins
    pub fn plugins(&self) -> &[Arc<dyn Plugin>] {
        &self.plugins
    }

    /// Finds a plugin by name
    #[allow(dead_code)]
    pub fn find_plugin(&self, name: &str) -> Option<&Arc<dyn Plugin>> {
        self.plugins.iter().find(|p| p.name() == name)
    }

    /// Lists all available plugins with their descriptions
    pub fn list_plugins(&self) -> Vec<(String, String, String)> {
        self.plugins
            .iter()
            .map(|p| {
                (
                    p.name().to_string(),
                    p.filename().to_string(),
                    p.description().to_string(),
                )
            })
            .collect()
    }
}

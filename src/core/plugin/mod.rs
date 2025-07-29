use crate::config::Config;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[cfg(test)]
mod tests;

/// Represents the result of a plugin execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResult {
    pub plugin_name: String,
    pub content: String,
    pub checksum: String,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Plugin descriptor for auto-registration
#[derive(Debug, Clone)]
pub struct PluginDescriptor {
    pub name: &'static str,
    pub category: &'static str,
    pub factory: fn(Option<toml::Value>) -> Arc<dyn Plugin>,
}

// Inventory collection for auto-registering plugins
inventory::collect!(PluginDescriptor);

/// Core trait that all plugins must implement with self-discovery capabilities
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Returns a description of what the plugin does
    fn description(&self) -> &str;

    /// Returns the icon/emoji to display with this plugin
    fn icon(&self) -> &str;

    /// Executes the plugin and returns the content to be saved
    async fn execute(&self) -> Result<String>;

    /// Validates that the plugin can run (e.g., required binaries exist)
    async fn validate(&self) -> Result<()>;

    /// Get target path from plugin's own configuration
    fn get_target_path(&self) -> Option<String>;

    /// Get output file from plugin's own configuration
    fn get_output_file(&self) -> Option<String>;

    /// Get restore target directory from plugin's own configuration
    fn get_restore_target_dir(&self) -> Option<String> {
        None // Default: no custom restore target
    }

    /// Get plugin's default restore target directory (not from config)
    /// This is used when no custom target is specified
    fn get_default_restore_target_dir(&self) -> Result<std::path::PathBuf> {
        // Default: use home directory
        Ok(dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from(".")))
    }

    /// Get plugin hooks from plugin's own configuration
    fn get_hooks(&self) -> Vec<crate::core::hooks::HookAction> {
        Vec::new() // Default: no hooks
    }

    /// Indicates whether this plugin creates its own output files and should skip
    /// the standard executor output file creation
    ///
    /// Returns true for plugins like static_files that handle their own file operations
    /// Returns false (default) for standard plugins that return content to be saved
    fn creates_own_output_files(&self) -> bool {
        false // Default: executor should save the plugin's output
    }

    /// Restores configuration from a snapshot for this plugin
    ///
    /// This method allows plugins to implement custom restoration logic beyond
    /// simple file copying. For example, VSCode plugins might want to install
    /// extensions after restoring settings.
    ///
    /// # Arguments
    /// * `snapshot_path` - Path to the plugin's snapshot directory
    /// * `target_path` - Target directory where files should be restored
    /// * `dry_run` - If true, only simulate the restoration
    ///
    /// # Returns
    /// Vector of successfully restored file paths
    async fn restore(
        &self,
        _snapshot_path: &std::path::Path,
        _target_path: &std::path::Path,
        _dry_run: bool,
    ) -> Result<Vec<std::path::PathBuf>> {
        // Default implementation: no custom restore logic
        Ok(Vec::new())
    }
}

/// Plugin registry for managing available plugins
#[derive(Default)]
pub struct PluginRegistry {
    plugins: Vec<(String, Arc<dyn Plugin>)>, // (name, plugin)
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Returns all registered plugins with their names
    pub fn plugins(&self) -> &[(String, Arc<dyn Plugin>)] {
        &self.plugins
    }

    /// Derives the default filename for a plugin (always .txt extension)
    pub fn derive_plugin_filename(plugin_name: &str) -> String {
        format!("{plugin_name}.txt")
    }

    /// Finds a plugin by name
    pub fn find_plugin(&self, name: &str) -> Option<&Arc<dyn Plugin>> {
        self.plugins
            .iter()
            .find(|(plugin_name, _)| plugin_name == name)
            .map(|(_, plugin)| plugin)
    }

    /// Gets a plugin by name (alias for find_plugin)
    pub fn get_plugin(&self, name: &str) -> Option<&Arc<dyn Plugin>> {
        self.find_plugin(name)
    }

    /// Add a plugin directly (for testing only)
    #[cfg(test)]
    pub fn add_plugin(&mut self, name: String, plugin: Arc<dyn Plugin>) {
        self.plugins.push((name, plugin));
    }

    /// Gets the final output file for a plugin using plugin self-discovery
    pub fn get_plugin_output_file_from_plugin(plugin: &dyn Plugin, plugin_name: &str) -> String {
        // Static files plugin doesn't use a single output file - it copies files directly
        if matches!(plugin_name, "static_files") {
            return "N/A (copies files directly)".to_string();
        }

        // Use plugin's own configuration first
        if let Some(custom_output_file) = plugin.get_output_file() {
            return custom_output_file;
        }

        // Fall back to auto-derived filename
        Self::derive_plugin_filename(plugin_name)
    }

    /// Extract category from plugin name based on folder structure
    pub fn extract_category_from_plugin_name(plugin_name: &str, config: Option<&Config>) -> String {
        // Extract folder name from plugin name (e.g., "vscode_extensions" -> "vscode")
        let folder_name = if let Some(underscore_pos) = plugin_name.find('_') {
            &plugin_name[..underscore_pos]
        } else {
            plugin_name
        };

        // Check for user-defined category override
        if let Some(config) = config {
            if let Some(ui_config) = &config.ui {
                if let Some(custom_categories) = &ui_config.plugin_categories {
                    if let Some(custom_name) = custom_categories.get(folder_name) {
                        return custom_name.clone();
                    }
                }
            }
        }

        // Default transformation: folder_name to Title Case
        Self::folder_name_to_category(folder_name)
    }

    /// Convert folder name to title case (e.g., "static_files" -> "Static Files")
    pub fn folder_name_to_category(folder_name: &str) -> String {
        folder_name
            .split('_')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Lists all available plugins with detailed information including display names and icons
    pub fn list_plugins_detailed(
        &self,
        config: Option<&Config>,
    ) -> Vec<(String, String, String, String, String)> {
        self.plugins
            .iter()
            .map(|(name, plugin)| {
                let category = Self::extract_category_from_plugin_name(name, config);
                let output_file = Self::get_plugin_output_file_from_plugin(plugin.as_ref(), name);
                (
                    name.clone(),
                    output_file,
                    plugin.description().to_string(),
                    category,
                    plugin.icon().to_string(),
                )
            })
            .collect()
    }

    /// Auto-discover and register all plugins from inventory
    pub fn discover_plugins(config: Option<&Config>) -> Self {
        let mut registry = Self::new();

        for descriptor in inventory::iter::<PluginDescriptor> {
            let plugin_config = config
                .and_then(|c| c.get_raw_plugin_config(descriptor.name))
                .cloned();

            let plugin = (descriptor.factory)(plugin_config);
            registry.plugins.push((descriptor.name.to_string(), plugin));
        }

        registry
    }

    /// Register all plugins from descriptors with optional configuration filtering
    pub fn register_from_descriptors(
        &mut self,
        config: Option<&Config>,
        selected_plugins: &[&str],
    ) {
        for descriptor in inventory::iter::<PluginDescriptor> {
            // Check if this plugin should be included
            if !selected_plugins.is_empty()
                && !selected_plugins.contains(&"all")
                && !selected_plugins
                    .iter()
                    .any(|&sel| descriptor.name.contains(sel) || descriptor.category == sel)
            {
                continue;
            }

            let plugin_config = config
                .and_then(|c| c.get_raw_plugin_config(descriptor.name))
                .cloned();

            let plugin = (descriptor.factory)(plugin_config);
            self.plugins.push((descriptor.name.to_string(), plugin));
        }
    }
}

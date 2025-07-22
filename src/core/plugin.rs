use crate::config::Config;
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
    /// Returns the filename that will be used for the snapshot
    fn filename(&self) -> &str;

    /// Returns a description of what the plugin does
    fn description(&self) -> &str;

    /// Returns the icon/emoji to display with this plugin
    fn icon(&self) -> &str;

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
    plugins: Vec<(String, Arc<dyn Plugin>)>, // (name, plugin)
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Registers a new plugin with auto-derived name from type
    pub fn register<T: Plugin + 'static>(&mut self, plugin: Arc<T>) {
        let name = Self::derive_plugin_name_from_type::<T>();
        let plugin_dyn: Arc<dyn Plugin> = plugin;
        self.plugins.push((name, plugin_dyn));
    }

    /// Derives plugin name from type name based on folder structure
    fn derive_plugin_name_from_type<T: 'static>() -> String {
        let type_name = std::any::type_name::<T>();
        Self::convert_type_name_to_plugin_name(type_name)
    }

    /// Converts type name to plugin name format using folder structure
    fn convert_type_name_to_plugin_name(type_name: &str) -> String {
        // Extract the module path: dotsnapshot::plugins::vscode::settings::VSCodeSettingsPlugin
        // We want: vscode_settings

        let parts: Vec<&str> = type_name.split("::").collect();

        // Look for "plugins" in the path and extract folder + file name
        if let Some(plugins_index) = parts.iter().position(|&p| p == "plugins") {
            if plugins_index + 2 < parts.len() {
                // Get folder name (e.g., "vscode") and file name (e.g., "settings")
                let folder = parts[plugins_index + 1];
                let file = parts[plugins_index + 2];

                return format!("{folder}_{file}");
            }
        }

        // Fallback: use the old approach for backward compatibility
        let struct_name = parts.last().unwrap_or(&type_name);
        let name_without_plugin = struct_name.strip_suffix("Plugin").unwrap_or(struct_name);
        Self::simple_camel_to_snake(name_without_plugin)
    }

    /// Simple CamelCase to snake_case conversion for fallback cases
    fn simple_camel_to_snake(s: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = s.chars().collect();

        for (i, &ch) in chars.iter().enumerate() {
            if ch.is_uppercase() && i > 0 {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
        }

        result
    }

    /// Returns all registered plugins with their names
    pub fn plugins(&self) -> &[(String, Arc<dyn Plugin>)] {
        &self.plugins
    }

    /// Finds a plugin by name
    #[allow(dead_code)]
    pub fn find_plugin(&self, name: &str) -> Option<&Arc<dyn Plugin>> {
        self.plugins
            .iter()
            .find(|(plugin_name, _)| plugin_name == name)
            .map(|(_, plugin)| plugin)
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
                (
                    name.clone(),
                    plugin.filename().to_string(),
                    plugin.description().to_string(),
                    category,
                    plugin.icon().to_string(),
                )
            })
            .collect()
    }

    /// Extract category from plugin name based on folder structure
    fn extract_category_from_plugin_name(plugin_name: &str, config: Option<&Config>) -> String {
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
    fn folder_name_to_category(folder_name: &str) -> String {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_camel_to_snake() {
        // Basic CamelCase conversion for fallback cases
        assert_eq!(
            PluginRegistry::simple_camel_to_snake("SimpleCase"),
            "simple_case"
        );
        assert_eq!(
            PluginRegistry::simple_camel_to_snake("CamelCaseExample"),
            "camel_case_example"
        );
        assert_eq!(
            PluginRegistry::simple_camel_to_snake("Homebrew"),
            "homebrew"
        );
        assert_eq!(PluginRegistry::simple_camel_to_snake("Static"), "static");
    }

    #[test]
    fn test_convert_type_name_to_plugin_name() {
        // Full type paths with folder structure (primary approach)
        assert_eq!(
            PluginRegistry::convert_type_name_to_plugin_name(
                "dotsnapshot::plugins::vscode::settings::VSCodeSettingsPlugin"
            ),
            "vscode_settings"
        );
        assert_eq!(
            PluginRegistry::convert_type_name_to_plugin_name(
                "dotsnapshot::plugins::cursor::extensions::CursorExtensionsPlugin"
            ),
            "cursor_extensions"
        );
        assert_eq!(
            PluginRegistry::convert_type_name_to_plugin_name(
                "dotsnapshot::plugins::homebrew::brewfile::HomebrewBrewfilePlugin"
            ),
            "homebrew_brewfile"
        );
        assert_eq!(
            PluginRegistry::convert_type_name_to_plugin_name(
                "dotsnapshot::plugins::npm::global_packages::NpmGlobalPackagesPlugin"
            ),
            "npm_global_packages"
        );
        assert_eq!(
            PluginRegistry::convert_type_name_to_plugin_name(
                "dotsnapshot::plugins::static_files::plugin::StaticFilesPlugin"
            ),
            "static_files_plugin"
        );

        // Fallback for type names without full paths (backward compatibility)
        assert_eq!(
            PluginRegistry::convert_type_name_to_plugin_name("VSCodeSettingsPlugin"),
            "v_s_code_settings"
        );
        assert_eq!(
            PluginRegistry::convert_type_name_to_plugin_name("SimplePlugin"),
            "simple"
        );

        // Edge cases
        assert_eq!(
            PluginRegistry::convert_type_name_to_plugin_name("Plugin"),
            ""
        );
    }

    #[test]
    fn test_derive_plugin_name_from_type() {
        // This would test the actual type derivation, but we can't easily test std::any::type_name
        // in unit tests since it requires actual types. The integration test in --list covers this.
    }

    #[test]
    fn test_extract_category_from_plugin_name() {
        // Basic folder extraction
        assert_eq!(
            PluginRegistry::extract_category_from_plugin_name("vscode_settings", None),
            "Vscode"
        );
        assert_eq!(
            PluginRegistry::extract_category_from_plugin_name("cursor_extensions", None),
            "Cursor"
        );
        assert_eq!(
            PluginRegistry::extract_category_from_plugin_name("homebrew_brewfile", None),
            "Homebrew"
        );
        assert_eq!(
            PluginRegistry::extract_category_from_plugin_name("npm_global_packages", None),
            "Npm"
        );
        assert_eq!(
            PluginRegistry::extract_category_from_plugin_name("static_files", None),
            "Static"
        );

        // Single word plugins
        assert_eq!(
            PluginRegistry::extract_category_from_plugin_name("static", None),
            "Static"
        );

        // With config overrides
        use crate::config::{Config, UiConfig};
        use std::collections::HashMap;

        let mut categories = HashMap::new();
        categories.insert("vscode".to_string(), "VSCode".to_string());
        categories.insert("npm".to_string(), "NPM".to_string());

        let config = Config {
            output_dir: None,
            include_plugins: None,
            logging: None,
            hooks: None,
            global: None,
            static_files: None,
            plugins: None,
            ui: Some(UiConfig {
                plugin_categories: Some(categories),
            }),
        };

        assert_eq!(
            PluginRegistry::extract_category_from_plugin_name("vscode_settings", Some(&config)),
            "VSCode"
        );
        assert_eq!(
            PluginRegistry::extract_category_from_plugin_name("npm_config", Some(&config)),
            "NPM"
        );
        assert_eq!(
            PluginRegistry::extract_category_from_plugin_name("cursor_settings", Some(&config)),
            "Cursor"
        );
    }

    #[test]
    fn test_folder_name_to_category() {
        assert_eq!(PluginRegistry::folder_name_to_category("vscode"), "Vscode");
        assert_eq!(
            PluginRegistry::folder_name_to_category("static_files"),
            "Static Files"
        );
        assert_eq!(PluginRegistry::folder_name_to_category("npm"), "Npm");
        assert_eq!(
            PluginRegistry::folder_name_to_category("homebrew"),
            "Homebrew"
        );
        assert_eq!(PluginRegistry::folder_name_to_category("cursor"), "Cursor");
    }
}

use crate::config::Config;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
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

    /// Registers a new plugin with auto-derived name from type
    ///
    /// **Deprecated**: Use auto-registration system instead
    #[deprecated(
        since = "1.2.0",
        note = "Use auto-registration system with register_plugin! macro instead. This method will be removed in version 2.0.0."
    )]
    #[allow(dead_code)]
    #[allow(deprecated)]
    pub fn register<T: Plugin + 'static>(&mut self, plugin: Arc<T>) {
        let name = Self::derive_plugin_name_from_type::<T>();
        let plugin_dyn: Arc<dyn Plugin> = plugin;
        self.plugins.push((name, plugin_dyn));
    }

    /// Derives plugin name from type name based on folder structure
    ///
    /// **Deprecated**: Use auto-registration system instead
    #[deprecated(
        since = "1.2.0",
        note = "Use auto-registration system with register_plugin! macro instead. This method will be removed in version 2.0.0."
    )]
    #[allow(dead_code)]
    #[allow(deprecated)]
    fn derive_plugin_name_from_type<T: 'static>() -> String {
        let type_name = std::any::type_name::<T>();
        Self::convert_type_name_to_plugin_name(type_name)
    }

    /// Converts type name to plugin name format using folder structure
    ///
    /// **Deprecated**: Use auto-registration system instead
    #[deprecated(
        since = "1.2.0",
        note = "Use auto-registration system with register_plugin! macro instead. This method will be removed in version 2.0.0."
    )]
    #[allow(dead_code)]
    #[allow(deprecated)]
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
    ///
    /// **Deprecated**: Use auto-registration system instead
    #[deprecated(
        since = "1.2.0",
        note = "Use auto-registration system with register_plugin! macro instead. This method will be removed in version 2.0.0."
    )]
    #[allow(dead_code)]
    #[allow(deprecated)]
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

    /// Derives the default filename for a plugin (always .txt extension)
    pub fn derive_plugin_filename(plugin_name: &str) -> String {
        format!("{plugin_name}.txt")
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

    /// Finds a plugin by name
    #[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(deprecated)]
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
    #[allow(deprecated)]
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
                "dotsnapshot::plugins::static::files::StaticFilesPlugin"
            ),
            "static_files"
        );

        // Fallback for type names without full paths (backward compatibility)
        assert_eq!(
            PluginRegistry::convert_type_name_to_plugin_name("TestVscodeSettingsPlugin"),
            "test_vscode_settings"
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
    #[allow(deprecated)]
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

    #[test]
    fn test_derive_plugin_filename() {
        // Test auto-derived filenames always use .txt extension
        assert_eq!(
            PluginRegistry::derive_plugin_filename("vscode_settings"),
            "vscode_settings.txt"
        );
        assert_eq!(
            PluginRegistry::derive_plugin_filename("homebrew_brewfile"),
            "homebrew_brewfile.txt"
        );
        assert_eq!(
            PluginRegistry::derive_plugin_filename("npm_global_packages"),
            "npm_global_packages.txt"
        );
        assert_eq!(
            PluginRegistry::derive_plugin_filename("test_plugin"),
            "test_plugin.txt"
        );
    }

    #[test]
    fn test_get_plugin_output_file_from_plugin() {
        use crate::plugins::core::base::settings::SettingsPlugin;
        use crate::plugins::core::base::static_files::StaticFilesPlugin;
        use crate::plugins::r#static::files::StaticFilesAppCore;
        use crate::plugins::vscode::settings::VSCodeCore;

        // Test plugin that doesn't specify a custom output file - should use auto-derived
        let vscode_plugin = SettingsPlugin::new(VSCodeCore);
        assert_eq!(
            PluginRegistry::get_plugin_output_file_from_plugin(&vscode_plugin, "vscode_settings"),
            "vscode_settings.txt"
        );

        // Test static files plugin special handling
        let static_plugin = StaticFilesPlugin::new(StaticFilesAppCore);
        assert_eq!(
            PluginRegistry::get_plugin_output_file_from_plugin(&static_plugin, "static_files"),
            "N/A (copies files directly)"
        );

        // Test that the method would work with any plugin that returns None from get_output_file()
        // This verifies the fallback behavior to auto-derived filenames
        struct MockPlugin;
        #[async_trait::async_trait]
        impl crate::core::plugin::Plugin for MockPlugin {
            fn description(&self) -> &str {
                "Mock plugin"
            }
            fn icon(&self) -> &str {
                "🔧"
            }
            async fn execute(&self) -> anyhow::Result<String> {
                Ok("test".to_string())
            }
            async fn validate(&self) -> anyhow::Result<()> {
                Ok(())
            }
            fn get_target_path(&self) -> Option<String> {
                None
            }
            fn get_output_file(&self) -> Option<String> {
                None
            }
        }

        let mock_plugin = MockPlugin;
        assert_eq!(
            PluginRegistry::get_plugin_output_file_from_plugin(&mock_plugin, "mock_plugin"),
            "mock_plugin.txt"
        );

        // Test plugin that specifies a custom output file
        struct CustomOutputPlugin;
        #[async_trait::async_trait]
        impl crate::core::plugin::Plugin for CustomOutputPlugin {
            fn description(&self) -> &str {
                "Custom output plugin"
            }
            fn icon(&self) -> &str {
                "📝"
            }
            async fn execute(&self) -> anyhow::Result<String> {
                Ok("test".to_string())
            }
            async fn validate(&self) -> anyhow::Result<()> {
                Ok(())
            }
            fn get_target_path(&self) -> Option<String> {
                None
            }
            fn get_output_file(&self) -> Option<String> {
                Some("custom-output.json".to_string())
            }
        }

        let custom_plugin = CustomOutputPlugin;
        assert_eq!(
            PluginRegistry::get_plugin_output_file_from_plugin(&custom_plugin, "custom_plugin"),
            "custom-output.json"
        );
    }
}

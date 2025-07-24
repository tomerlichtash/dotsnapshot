use anyhow::{Context, Result};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tracing::warn;

use crate::core::config_schema::{ConfigSchema, ValidationHelpers};
use crate::core::hooks::HookAction;
use crate::core::plugin::Plugin;
use crate::symbols::*;

/// Plugin for capturing Cursor keybindings
pub struct CursorKeybindingsPlugin {
    config: Option<toml::Value>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct CursorKeybindingsConfig {
    #[schemars(description = "Custom directory path within the snapshot for this plugin's output")]
    target_path: Option<String>,

    #[schemars(
        description = "Custom filename for the keybindings output (default: keybindings.json)"
    )]
    output_file: Option<String>,

    #[schemars(description = "Plugin-specific hooks configuration")]
    hooks: Option<PluginHooks>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct PluginHooks {
    #[serde(rename = "pre-plugin", default)]
    #[schemars(description = "Hooks to run before plugin execution")]
    pre_plugin: Vec<HookAction>,

    #[serde(rename = "post-plugin", default)]
    #[schemars(description = "Hooks to run after plugin execution")]
    post_plugin: Vec<HookAction>,
}

impl ConfigSchema for CursorKeybindingsConfig {
    fn schema_name() -> &'static str {
        "CursorKeybindingsConfig"
    }

    fn validate(&self) -> Result<()> {
        // Validate output file extension if specified
        if let Some(output_file) = &self.output_file {
            // Keybindings are typically JSON files
            ValidationHelpers::validate_file_extension(output_file, &["json", "jsonc"])?;
        }

        Ok(())
    }
}

impl CursorKeybindingsPlugin {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn with_config(config: toml::Value) -> Self {
        // Validate configuration using schema validation
        match CursorKeybindingsConfig::from_toml_value(&config) {
            Ok(_) => {
                // Configuration is valid
                Self {
                    config: Some(config),
                }
            }
            Err(e) => {
                // Use shared error formatting
                let error_msg = ValidationHelpers::format_validation_error(
                    "Cursor Keybindings plugin",
                    "cursor_keybindings",
                    "target_path (string), output_file (string), hooks (object)",
                    "target_path = \"cursor\", output_file = \"keybindings.json\"",
                    &e,
                );

                warn!("{error_msg}");

                // Still create plugin to avoid breaking the application
                Self {
                    config: Some(config),
                }
            }
        }
    }

    fn get_config(&self) -> Option<CursorKeybindingsConfig> {
        self.config
            .as_ref()
            .and_then(|c| CursorKeybindingsConfig::from_toml_value(c).ok())
    }

    /// Gets the Cursor settings directory based on OS
    fn get_cursor_settings_dir(&self) -> Result<PathBuf> {
        let home_dir = dirs::home_dir().context("Could not determine home directory")?;

        let settings_dir = if cfg!(target_os = "macos") {
            home_dir.join("Library/Application Support/Cursor/User")
        } else if cfg!(target_os = "windows") {
            home_dir.join("AppData/Roaming/Cursor/User")
        } else {
            // Linux and other Unix-like systems
            home_dir.join(".config/Cursor/User")
        };

        Ok(settings_dir)
    }

    /// Reads Cursor keybindings.json file
    async fn get_keybindings(&self) -> Result<String> {
        let keybindings_path = self.get_cursor_settings_dir()?.join("keybindings.json");

        if !keybindings_path.exists() {
            return Ok("[]".to_string());
        }

        let content = fs::read_to_string(&keybindings_path)
            .await
            .context("Failed to read Cursor keybindings.json")?;

        Ok(content)
    }
}

#[async_trait]
impl Plugin for CursorKeybindingsPlugin {
    fn description(&self) -> &str {
        "Captures Cursor editor custom keybindings configuration"
    }

    fn icon(&self) -> &str {
        TOOL_EDITOR
    }

    async fn execute(&self) -> Result<String> {
        self.get_keybindings().await
    }

    async fn validate(&self) -> Result<()> {
        // Check if settings directory exists
        let settings_dir = self.get_cursor_settings_dir()?;
        if !settings_dir.exists() {
            return Err(anyhow::anyhow!(
                "Cursor settings directory not found: {}",
                settings_dir.display()
            ));
        }

        Ok(())
    }

    fn get_target_path(&self) -> Option<String> {
        self.get_config()?.target_path
    }

    fn get_output_file(&self) -> Option<String> {
        self.get_config()?.output_file
    }

    fn get_hooks(&self) -> Vec<HookAction> {
        self.get_config()
            .and_then(|c| c.hooks)
            .map(|h| {
                let mut hooks = h.pre_plugin;
                hooks.extend(h.post_plugin);
                hooks
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cursor_keybindings_plugin_description() {
        let plugin = CursorKeybindingsPlugin::new();
        assert_eq!(
            plugin.description(),
            "Captures Cursor editor custom keybindings configuration"
        );
    }

    #[tokio::test]
    async fn test_cursor_keybindings_dir() {
        let plugin = CursorKeybindingsPlugin::new();
        let settings_dir = plugin.get_cursor_settings_dir().unwrap();

        // Just check that we get a valid path
        assert!(settings_dir.is_absolute());
    }

    #[tokio::test]
    async fn test_cursor_keybindings_plugin_config() {
        // Test with no config
        let plugin = CursorKeybindingsPlugin::new();
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert!(plugin.get_hooks().is_empty());

        // Test with config
        let config_toml = r#"
            target_path = "cursor"
            output_file = "keybindings.json"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = CursorKeybindingsPlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_target_path(),
            Some("cursor".to_string())
        );
        assert_eq!(
            plugin_with_config.get_output_file(),
            Some("keybindings.json".to_string())
        );
        assert!(plugin_with_config.get_hooks().is_empty());
    }
}

// Auto-register this plugin
crate::register_plugin!(CursorKeybindingsPlugin, "cursor_keybindings", "cursor");

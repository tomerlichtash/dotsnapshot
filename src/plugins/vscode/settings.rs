use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;

use crate::core::hooks::HookAction;
use crate::core::plugin::Plugin;
use crate::symbols::*;

/// Plugin for capturing VSCode settings
pub struct VSCodeSettingsPlugin {
    config: Option<toml::Value>,
}

#[derive(serde::Deserialize)]
struct VSCodeSettingsConfig {
    target_path: Option<String>,
    output_file: Option<String>,
    hooks: Option<PluginHooks>,
}

#[derive(serde::Deserialize)]
struct PluginHooks {
    #[serde(rename = "pre-plugin", default)]
    pre_plugin: Vec<HookAction>,
    #[serde(rename = "post-plugin", default)]
    post_plugin: Vec<HookAction>,
}

impl VSCodeSettingsPlugin {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn with_config(config: toml::Value) -> Self {
        Self {
            config: Some(config),
        }
    }

    fn get_config(&self) -> Option<VSCodeSettingsConfig> {
        self.config.as_ref().and_then(|c| c.clone().try_into().ok())
    }

    /// Gets the VSCode settings directory based on OS
    fn get_vscode_settings_dir(&self) -> Result<PathBuf> {
        let home_dir = dirs::home_dir().context("Could not determine home directory")?;

        let settings_dir = if cfg!(target_os = "macos") {
            home_dir.join("Library/Application Support/Code/User")
        } else if cfg!(target_os = "windows") {
            home_dir.join("AppData/Roaming/Code/User")
        } else {
            // Linux and other Unix-like systems
            home_dir.join(".config/Code/User")
        };

        Ok(settings_dir)
    }

    /// Reads VSCode settings.json file
    async fn get_settings(&self) -> Result<String> {
        let settings_path = self.get_vscode_settings_dir()?.join("settings.json");

        if !settings_path.exists() {
            return Ok("{}".to_string());
        }

        let content = fs::read_to_string(&settings_path)
            .await
            .context("Failed to read VSCode settings.json")?;

        Ok(content)
    }
}

#[async_trait]
impl Plugin for VSCodeSettingsPlugin {
    fn description(&self) -> &str {
        "Captures VSCode user settings configuration"
    }

    fn icon(&self) -> &str {
        TOOL_COMPUTER
    }

    async fn execute(&self) -> Result<String> {
        self.get_settings().await
    }

    async fn validate(&self) -> Result<()> {
        // Check if settings directory exists
        let settings_dir = self.get_vscode_settings_dir()?;
        if !settings_dir.exists() {
            return Err(anyhow::anyhow!(
                "VSCode settings directory not found: {}",
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
    async fn test_vscode_settings_plugin_description() {
        let plugin = VSCodeSettingsPlugin::new();
        assert_eq!(
            plugin.description(),
            "Captures VSCode user settings configuration"
        );
    }

    #[tokio::test]
    async fn test_vscode_settings_dir() {
        let plugin = VSCodeSettingsPlugin::new();
        let settings_dir = plugin.get_vscode_settings_dir().unwrap();

        // Just check that we get a valid path
        assert!(settings_dir.is_absolute());
    }

    #[tokio::test]
    async fn test_vscode_settings_plugin_config() {
        // Test with no config
        let plugin = VSCodeSettingsPlugin::new();
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert!(plugin.get_hooks().is_empty());

        // Test with config
        let config_toml = r#"
            target_path = "vscode"
            output_file = "settings.json"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = VSCodeSettingsPlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_target_path(),
            Some("vscode".to_string())
        );
        assert_eq!(
            plugin_with_config.get_output_file(),
            Some("settings.json".to_string())
        );
        assert!(plugin_with_config.get_hooks().is_empty());
    }
}

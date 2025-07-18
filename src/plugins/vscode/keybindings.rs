use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;

use crate::core::plugin::Plugin;

/// Plugin for capturing VSCode keybindings
pub struct VSCodeKeybindingsPlugin;

impl VSCodeKeybindingsPlugin {
    pub fn new() -> Self {
        Self
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

    /// Reads VSCode keybindings.json file
    async fn get_keybindings(&self) -> Result<String> {
        let keybindings_path = self.get_vscode_settings_dir()?.join("keybindings.json");

        if !keybindings_path.exists() {
            return Ok("[]".to_string());
        }

        let content = fs::read_to_string(&keybindings_path)
            .await
            .context("Failed to read VSCode keybindings.json")?;

        Ok(content)
    }
}

#[async_trait]
impl Plugin for VSCodeKeybindingsPlugin {
    fn name(&self) -> &str {
        "vscode_keybindings"
    }

    fn filename(&self) -> &str {
        "vscode_keybindings.json"
    }

    fn description(&self) -> &str {
        "Captures VSCode custom keybindings configuration"
    }

    async fn execute(&self) -> Result<String> {
        self.get_keybindings().await
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vscode_keybindings_plugin_name() {
        let plugin = VSCodeKeybindingsPlugin::new();
        assert_eq!(plugin.name(), "vscode_keybindings");
        assert_eq!(plugin.filename(), "vscode_keybindings.json");
    }

    #[tokio::test]
    async fn test_vscode_keybindings_dir() {
        let plugin = VSCodeKeybindingsPlugin::new();
        let settings_dir = plugin.get_vscode_settings_dir().unwrap();

        // Just check that we get a valid path
        assert!(settings_dir.is_absolute());
    }
}

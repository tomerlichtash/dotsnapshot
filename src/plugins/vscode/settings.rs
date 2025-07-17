use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;

use crate::core::plugin::Plugin;

/// Plugin for capturing VSCode settings
pub struct VSCodeSettingsPlugin;

impl VSCodeSettingsPlugin {
    pub fn new() -> Self {
        Self
    }
    
    /// Gets the VSCode settings directory based on OS
    fn get_vscode_settings_dir(&self) -> Result<PathBuf> {
        let home_dir = dirs::home_dir()
            .context("Could not determine home directory")?;
        
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
        
        let content = fs::read_to_string(&settings_path).await
            .context("Failed to read VSCode settings.json")?;
        
        Ok(content)
    }
}

#[async_trait]
impl Plugin for VSCodeSettingsPlugin {
    fn name(&self) -> &str {
        "vscode_settings"
    }
    
    fn filename(&self) -> &str {
        "vscode_settings.json"
    }
    
    fn description(&self) -> &str {
        "Captures VSCode user settings configuration"
    }
    
    async fn execute(&self) -> Result<String> {
        self.get_settings().await
    }
    
    async fn validate(&self) -> Result<()> {
        // Check if settings directory exists
        let settings_dir = self.get_vscode_settings_dir()?;
        if !settings_dir.exists() {
            return Err(anyhow::anyhow!("VSCode settings directory not found: {}", settings_dir.display()));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vscode_settings_plugin_name() {
        let plugin = VSCodeSettingsPlugin::new();
        assert_eq!(plugin.name(), "vscode_settings");
        assert_eq!(plugin.filename(), "vscode_settings.json");
    }

    #[tokio::test]
    async fn test_vscode_settings_dir() {
        let plugin = VSCodeSettingsPlugin::new();
        let settings_dir = plugin.get_vscode_settings_dir().unwrap();
        
        // Just check that we get a valid path
        assert!(settings_dir.is_absolute());
    }
}
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;

use crate::core::plugin::Plugin;

/// Plugin for capturing Cursor keybindings
pub struct CursorKeybindingsPlugin;

impl CursorKeybindingsPlugin {
    pub fn new() -> Self {
        Self
    }
    
    /// Gets the Cursor settings directory based on OS
    fn get_cursor_settings_dir(&self) -> Result<PathBuf> {
        let home_dir = dirs::home_dir()
            .context("Could not determine home directory")?;
        
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
        
        let content = fs::read_to_string(&keybindings_path).await
            .context("Failed to read Cursor keybindings.json")?;
        
        Ok(content)
    }
}

#[async_trait]
impl Plugin for CursorKeybindingsPlugin {
    fn name(&self) -> &str {
        "cursor_keybindings"
    }
    
    fn filename(&self) -> &str {
        "cursor_keybindings.json"
    }
    
    fn description(&self) -> &str {
        "Captures Cursor editor custom keybindings configuration"
    }
    
    async fn execute(&self) -> Result<String> {
        self.get_keybindings().await
    }
    
    async fn validate(&self) -> Result<()> {
        // Check if settings directory exists
        let settings_dir = self.get_cursor_settings_dir()?;
        if !settings_dir.exists() {
            return Err(anyhow::anyhow!("Cursor settings directory not found: {}", settings_dir.display()));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cursor_keybindings_plugin_name() {
        let plugin = CursorKeybindingsPlugin::new();
        assert_eq!(plugin.name(), "cursor_keybindings");
        assert_eq!(plugin.filename(), "cursor_keybindings.json");
    }

    #[tokio::test]
    async fn test_cursor_keybindings_dir() {
        let plugin = CursorKeybindingsPlugin::new();
        let settings_dir = plugin.get_cursor_settings_dir().unwrap();
        
        // Just check that we get a valid path
        assert!(settings_dir.is_absolute());
    }
}
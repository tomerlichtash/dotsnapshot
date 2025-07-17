use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Command;
use which::which;

use crate::core::plugin::Plugin;

/// Plugin for generating Homebrew Brewfile
pub struct HomebrewBrewfilePlugin;

impl HomebrewBrewfilePlugin {
    pub fn new() -> Self {
        Self
    }
    
    /// Executes brew bundle dump to generate Brewfile content
    async fn generate_brewfile(&self) -> Result<String> {
        use std::env;
        
        // Create a temporary directory for the Brewfile
        let temp_dir = env::temp_dir();
        let temp_dir_clone = temp_dir.clone();
        
        // Run brew bundle dump to create the Brewfile
        let output = tokio::task::spawn_blocking(move || {
            Command::new("brew")
                .args(&["bundle", "dump", "--force"])
                .current_dir(&temp_dir_clone)
                .output()
        })
        .await??;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("brew bundle dump failed: {}", stderr));
        }
        
        // brew bundle dump creates "Brewfile" in current directory by default
        let default_brewfile_path = temp_dir.join("Brewfile");
        
        // Read the generated Brewfile
        let brewfile_content = tokio::fs::read_to_string(&default_brewfile_path).await
            .context("Failed to read generated Brewfile")?;
        
        // Clean up the temporary file
        let _ = tokio::fs::remove_file(&default_brewfile_path).await;
        
        Ok(brewfile_content)
    }
}

#[async_trait]
impl Plugin for HomebrewBrewfilePlugin {
    fn name(&self) -> &str {
        "homebrew_brewfile"
    }
    
    fn filename(&self) -> &str {
        "Brewfile"
    }
    
    fn description(&self) -> &str {
        "Generates a Homebrew Brewfile with all installed packages"
    }
    
    async fn execute(&self) -> Result<String> {
        // Generate clean Brewfile content that can be used for installation
        match self.generate_brewfile().await {
            Ok(brewfile) => {
                if !brewfile.is_empty() {
                    Ok(brewfile)
                } else {
                    Ok("# No Brewfile content generated\n".to_string())
                }
            }
            Err(e) => {
                Ok(format!("# Error generating Brewfile: {}\n", e))
            }
        }
    }
    
    async fn validate(&self) -> Result<()> {
        // Check if brew command exists
        which("brew").context("brew command not found. Please install Homebrew.")?;
        
        // Check if brew bundle is available
        let output = tokio::task::spawn_blocking(|| {
            Command::new("brew")
                .args(&["bundle", "--help"])
                .output()
        })
        .await??;
        
        if !output.status.success() {
            return Err(anyhow::anyhow!("brew bundle command not available"));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_homebrew_brewfile_plugin_name() {
        let plugin = HomebrewBrewfilePlugin::new();
        assert_eq!(plugin.name(), "homebrew_brewfile");
        assert_eq!(plugin.filename(), "Brewfile");
    }

    #[tokio::test]
    async fn test_homebrew_brewfile_plugin_validation() {
        let plugin = HomebrewBrewfilePlugin::new();
        
        // This test will only pass if homebrew is installed
        // In CI environments, this might fail
        if which("brew").is_ok() {
            assert!(plugin.validate().await.is_ok());
        }
    }
}
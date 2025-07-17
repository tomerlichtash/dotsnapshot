use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Command;
use which::which;

use crate::core::plugin::Plugin;

/// Plugin for capturing NPM configuration
pub struct NpmConfigPlugin;

impl NpmConfigPlugin {
    pub fn new() -> Self {
        Self
    }
    
    /// Gets NPM configuration
    async fn get_npm_config(&self) -> Result<String> {
        let output = tokio::task::spawn_blocking(|| {
            Command::new("npm")
                .args(&["config", "list"])
                .output()
        })
        .await??;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("npm config list failed: {}", stderr));
        }
        
        let config = String::from_utf8(output.stdout)
            .context("Failed to parse npm config list output as UTF-8")?;
        
        // Filter out sensitive information
        let filtered_config = config
            .lines()
            .filter(|line| {
                !line.contains("password") &&
                !line.contains("token") &&
                !line.contains("auth") &&
                !line.contains("key")
            })
            .collect::<Vec<_>>()
            .join("\n");
        
        Ok(filtered_config)
    }
}

#[async_trait]
impl Plugin for NpmConfigPlugin {
    fn name(&self) -> &str {
        "npm_config"
    }
    
    fn filename(&self) -> &str {
        "npm_config.txt"
    }
    
    fn description(&self) -> &str {
        "Captures NPM configuration settings"
    }
    
    async fn execute(&self) -> Result<String> {
        self.get_npm_config().await
    }
    
    async fn validate(&self) -> Result<()> {
        // Check if npm command exists
        which("npm").context("npm command not found. Please install Node.js and NPM.")?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_npm_config_plugin_name() {
        let plugin = NpmConfigPlugin::new();
        assert_eq!(plugin.name(), "npm_config");
        assert_eq!(plugin.filename(), "npm_config.txt");
    }

    #[tokio::test]
    async fn test_npm_config_plugin_validation() {
        let plugin = NpmConfigPlugin::new();
        
        // This test will only pass if npm is installed
        if which("npm").is_ok() {
            assert!(plugin.validate().await.is_ok());
        }
    }
}
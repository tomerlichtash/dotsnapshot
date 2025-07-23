use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Command;
use which::which;

use crate::core::hooks::HookAction;
use crate::core::plugin::Plugin;
use crate::symbols::*;

/// Plugin for generating Homebrew Brewfile
pub struct HomebrewBrewfilePlugin {
    config: Option<toml::Value>,
}

#[derive(serde::Deserialize)]
struct HomebrewBrewfileConfig {
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

impl HomebrewBrewfilePlugin {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn with_config(config: toml::Value) -> Self {
        Self {
            config: Some(config),
        }
    }

    fn get_config(&self) -> Option<HomebrewBrewfileConfig> {
        self.config.as_ref().and_then(|c| c.clone().try_into().ok())
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
                .args(["bundle", "dump", "--force"])
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
        let brewfile_content = tokio::fs::read_to_string(&default_brewfile_path)
            .await
            .context("Failed to read generated Brewfile")?;

        // Clean up the temporary file
        let _ = tokio::fs::remove_file(&default_brewfile_path).await;

        Ok(brewfile_content)
    }
}

#[async_trait]
impl Plugin for HomebrewBrewfilePlugin {
    fn description(&self) -> &str {
        "Generates a Homebrew Brewfile with all installed packages"
    }

    fn icon(&self) -> &str {
        TOOL_PACKAGE_MANAGER
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
            Err(e) => Ok(format!("# Error generating Brewfile: {e}\n")),
        }
    }

    async fn validate(&self) -> Result<()> {
        // Check if brew command exists
        which("brew").context("brew command not found. Please install Homebrew.")?;

        // Check if brew bundle is available
        let output = tokio::task::spawn_blocking(|| {
            Command::new("brew").args(["bundle", "--help"]).output()
        })
        .await??;

        if !output.status.success() {
            return Err(anyhow::anyhow!("brew bundle command not available"));
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
    async fn test_homebrew_brewfile_plugin_description() {
        let plugin = HomebrewBrewfilePlugin::new();
        assert_eq!(
            plugin.description(),
            "Generates a Homebrew Brewfile with all installed packages"
        );
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

    #[tokio::test]
    async fn test_homebrew_brewfile_plugin_config() {
        // Test with no config
        let plugin = HomebrewBrewfilePlugin::new();
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert!(plugin.get_hooks().is_empty());

        // Test with config
        let config_toml = r#"
            target_path = "homebrew"
            output_file = "Brewfile"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = HomebrewBrewfilePlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_target_path(),
            Some("homebrew".to_string())
        );
        assert_eq!(
            plugin_with_config.get_output_file(),
            Some("Brewfile".to_string())
        );
        assert!(plugin_with_config.get_hooks().is_empty());
    }
}

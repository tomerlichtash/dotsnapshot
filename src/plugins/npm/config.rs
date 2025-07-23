use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Command;
use which::which;

use crate::core::hooks::HookAction;
use crate::core::plugin::Plugin;
use crate::symbols::*;

/// Plugin for capturing NPM configuration
pub struct NpmConfigPlugin {
    config: Option<toml::Value>,
}

#[derive(serde::Deserialize)]
struct NpmConfigConfig {
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

impl NpmConfigPlugin {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn with_config(config: toml::Value) -> Self {
        Self {
            config: Some(config),
        }
    }

    fn get_config(&self) -> Option<NpmConfigConfig> {
        self.config.as_ref().and_then(|c| c.clone().try_into().ok())
    }

    /// Gets NPM configuration
    async fn get_npm_config(&self) -> Result<String> {
        let output =
            tokio::task::spawn_blocking(|| Command::new("npm").args(["config", "list"]).output())
                .await??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("npm config list failed: {stderr}"));
        }

        let config = String::from_utf8(output.stdout)
            .context("Failed to parse npm config list output as UTF-8")?;

        // Filter out sensitive information
        let filtered_config = config
            .lines()
            .filter(|line| {
                !line.contains("password")
                    && !line.contains("token")
                    && !line.contains("auth")
                    && !line.contains("key")
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(filtered_config)
    }
}

#[async_trait]
impl Plugin for NpmConfigPlugin {
    // Uses default "txt" extension

    fn description(&self) -> &str {
        "Captures NPM configuration settings"
    }

    fn icon(&self) -> &str {
        CONTENT_PACKAGE
    }

    async fn execute(&self) -> Result<String> {
        self.get_npm_config().await
    }

    async fn validate(&self) -> Result<()> {
        // Check if npm command exists
        which("npm").context("npm command not found. Please install Node.js and NPM.")?;

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
    async fn test_npm_config_plugin_description() {
        let plugin = NpmConfigPlugin::new();
        assert_eq!(plugin.description(), "Captures NPM configuration settings");
    }

    #[tokio::test]
    async fn test_npm_config_plugin_validation() {
        let plugin = NpmConfigPlugin::new();

        // This test will only pass if npm is installed
        if which("npm").is_ok() {
            assert!(plugin.validate().await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_npm_config_plugin_config() {
        // Test with no config
        let plugin = NpmConfigPlugin::new();
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert!(plugin.get_hooks().is_empty());

        // Test with config
        let config_toml = r#"
            target_path = "npm"
            output_file = "npmrc"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = NpmConfigPlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_target_path(),
            Some("npm".to_string())
        );
        assert_eq!(
            plugin_with_config.get_output_file(),
            Some("npmrc".to_string())
        );
        assert!(plugin_with_config.get_hooks().is_empty());
    }
}

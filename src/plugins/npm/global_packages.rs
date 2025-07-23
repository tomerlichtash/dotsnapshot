use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Command;
use which::which;

use crate::core::hooks::HookAction;
use crate::core::plugin::Plugin;
use crate::symbols::*;

/// Plugin for capturing NPM global packages
pub struct NpmGlobalPackagesPlugin {
    config: Option<toml::Value>,
}

#[derive(serde::Deserialize)]
struct NpmGlobalPackagesConfig {
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

impl NpmGlobalPackagesPlugin {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn with_config(config: toml::Value) -> Self {
        Self {
            config: Some(config),
        }
    }

    fn get_config(&self) -> Option<NpmGlobalPackagesConfig> {
        self.config.as_ref().and_then(|c| c.clone().try_into().ok())
    }

    /// Gets list of globally installed NPM packages
    async fn get_global_packages(&self) -> Result<String> {
        let output = tokio::task::spawn_blocking(|| {
            Command::new("npm")
                .args(["list", "-g", "--depth=0"])
                .output()
        })
        .await??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("npm list -g failed: {stderr}"));
        }

        let packages =
            String::from_utf8(output.stdout).context("Failed to parse npm list output as UTF-8")?;

        Ok(packages)
    }
}

#[async_trait]
impl Plugin for NpmGlobalPackagesPlugin {
    fn description(&self) -> &str {
        "Lists globally installed NPM packages with versions"
    }

    fn icon(&self) -> &str {
        CONTENT_PACKAGE
    }

    async fn execute(&self) -> Result<String> {
        self.get_global_packages().await
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
    async fn test_npm_global_packages_plugin_description() {
        let plugin = NpmGlobalPackagesPlugin::new();
        assert_eq!(
            plugin.description(),
            "Lists globally installed NPM packages with versions"
        );
    }

    #[tokio::test]
    async fn test_npm_global_packages_plugin_validation() {
        let plugin = NpmGlobalPackagesPlugin::new();

        // This test will only pass if npm is installed
        if which("npm").is_ok() {
            assert!(plugin.validate().await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_npm_global_packages_plugin_config() {
        // Test with no config
        let plugin = NpmGlobalPackagesPlugin::new();
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert!(plugin.get_hooks().is_empty());

        // Test with config
        let config_toml = r#"
            target_path = "npm"
            output_file = "global-packages.txt"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = NpmGlobalPackagesPlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_target_path(),
            Some("npm".to_string())
        );
        assert_eq!(
            plugin_with_config.get_output_file(),
            Some("global-packages.txt".to_string())
        );
        assert!(plugin_with_config.get_hooks().is_empty());
    }
}

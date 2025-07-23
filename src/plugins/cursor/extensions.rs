use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Command;
use which::which;

use crate::core::hooks::HookAction;
use crate::core::plugin::Plugin;
use crate::symbols::*;

/// Plugin for capturing Cursor extensions
pub struct CursorExtensionsPlugin {
    config: Option<toml::Value>,
}

#[derive(serde::Deserialize)]
struct CursorExtensionsConfig {
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

impl CursorExtensionsPlugin {
    // Allow new_without_default because plugins intentionally use new() instead of Default
    // to maintain consistent plugin instantiation patterns across the codebase
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn with_config(config: toml::Value) -> Self {
        Self {
            config: Some(config),
        }
    }

    fn get_config(&self) -> Option<CursorExtensionsConfig> {
        self.config.as_ref().and_then(|c| c.clone().try_into().ok())
    }

    /// Gets list of installed Cursor extensions
    async fn get_extensions(&self) -> Result<String> {
        let output = tokio::task::spawn_blocking(|| {
            Command::new("cursor")
                .args(["--list-extensions", "--show-versions"])
                .output()
        })
        .await??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("cursor --list-extensions failed: {stderr}"));
        }

        let extensions = String::from_utf8(output.stdout)
            .context("Failed to parse cursor --list-extensions output as UTF-8")?;

        Ok(extensions)
    }
}

#[async_trait]
impl Plugin for CursorExtensionsPlugin {
    fn description(&self) -> &str {
        "Lists installed Cursor editor extensions with versions"
    }

    fn icon(&self) -> &str {
        TOOL_EDITOR
    }

    async fn execute(&self) -> Result<String> {
        self.get_extensions().await
    }

    async fn validate(&self) -> Result<()> {
        // Check if cursor command exists
        which("cursor").context("cursor command not found. Please install Cursor CLI.")?;

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
    async fn test_cursor_extensions_plugin_description() {
        let plugin = CursorExtensionsPlugin::new();
        assert_eq!(
            plugin.description(),
            "Lists installed Cursor editor extensions with versions"
        );
    }

    #[tokio::test]
    async fn test_cursor_extensions_plugin_validation() {
        let plugin = CursorExtensionsPlugin::new();

        // This test will only pass if Cursor CLI is installed
        if which("cursor").is_ok() {
            assert!(plugin.validate().await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_cursor_extensions_plugin_config() {
        // Test with no config
        let plugin = CursorExtensionsPlugin::new();
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert!(plugin.get_hooks().is_empty());

        // Test with config
        let config_toml = r#"
            target_path = "cursor"
            output_file = "extensions.txt"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = CursorExtensionsPlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_target_path(),
            Some("cursor".to_string())
        );
        assert_eq!(
            plugin_with_config.get_output_file(),
            Some("extensions.txt".to_string())
        );
        assert!(plugin_with_config.get_hooks().is_empty());
    }
}

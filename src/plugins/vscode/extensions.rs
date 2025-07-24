use anyhow::{Context, Result};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::process::Command;
use tracing::warn;
use which::which;

use crate::core::config_schema::{ConfigSchema, ValidationHelpers};
use crate::core::hooks::HookAction;
use crate::core::plugin::Plugin;
use crate::symbols::*;

/// Plugin for capturing VSCode extensions
pub struct VSCodeExtensionsPlugin {
    config: Option<toml::Value>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct VSCodeExtensionsConfig {
    #[schemars(description = "Custom directory path within the snapshot for this plugin's output")]
    target_path: Option<String>,

    #[schemars(
        description = "Custom filename for the extensions output (default: extensions.txt)"
    )]
    output_file: Option<String>,

    #[schemars(description = "Plugin-specific hooks configuration")]
    hooks: Option<PluginHooks>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct PluginHooks {
    #[serde(rename = "pre-plugin", default)]
    #[schemars(description = "Hooks to run before plugin execution")]
    pre_plugin: Vec<HookAction>,

    #[serde(rename = "post-plugin", default)]
    #[schemars(description = "Hooks to run after plugin execution")]
    post_plugin: Vec<HookAction>,
}

impl ConfigSchema for VSCodeExtensionsConfig {
    fn schema_name() -> &'static str {
        "VSCodeExtensionsConfig"
    }

    fn validate(&self) -> Result<()> {
        // Validate output file extension if specified
        if let Some(output_file) = &self.output_file {
            // Extensions list is typically a text file
            ValidationHelpers::validate_file_extension(output_file, &["txt", "log", "list"])?;
        }

        // Validate that code command exists (warning only, not error)
        if ValidationHelpers::validate_command_exists("code").is_err() {
            warn!("code command not found - VSCode functionality may not work");
        }

        Ok(())
    }
}

impl VSCodeExtensionsPlugin {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn with_config(config: toml::Value) -> Self {
        // Validate configuration using schema validation
        match VSCodeExtensionsConfig::from_toml_value(&config) {
            Ok(_) => {
                // Configuration is valid
                Self {
                    config: Some(config),
                }
            }
            Err(e) => {
                // Use shared error formatting
                let error_msg = ValidationHelpers::format_validation_error(
                    "VSCode Extensions plugin",
                    "vscode_extensions",
                    "target_path (string), output_file (string), hooks (object)",
                    "target_path = \"vscode\", output_file = \"extensions.txt\"",
                    &e,
                );

                warn!("{error_msg}");

                // Still create plugin to avoid breaking the application
                Self {
                    config: Some(config),
                }
            }
        }
    }

    fn get_config(&self) -> Option<VSCodeExtensionsConfig> {
        self.config
            .as_ref()
            .and_then(|c| VSCodeExtensionsConfig::from_toml_value(c).ok())
    }

    /// Gets list of installed VSCode extensions
    async fn get_extensions(&self) -> Result<String> {
        let output = tokio::task::spawn_blocking(|| {
            Command::new("code")
                .args(["--list-extensions", "--show-versions"])
                .output()
        })
        .await??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("code --list-extensions failed: {stderr}"));
        }

        let extensions = String::from_utf8(output.stdout)
            .context("Failed to parse code --list-extensions output as UTF-8")?;

        Ok(extensions)
    }
}

#[async_trait]
impl Plugin for VSCodeExtensionsPlugin {
    fn description(&self) -> &str {
        "Lists installed VSCode extensions with versions"
    }

    fn icon(&self) -> &str {
        TOOL_COMPUTER
    }

    async fn execute(&self) -> Result<String> {
        self.get_extensions().await
    }

    async fn validate(&self) -> Result<()> {
        // Check if code command exists
        which("code").context("code command not found. Please install VSCode CLI.")?;

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
    async fn test_vscode_extensions_plugin_description() {
        let plugin = VSCodeExtensionsPlugin::new();
        assert_eq!(
            plugin.description(),
            "Lists installed VSCode extensions with versions"
        );
    }

    #[tokio::test]
    async fn test_vscode_extensions_plugin_validation() {
        let plugin = VSCodeExtensionsPlugin::new();

        // This test will only pass if VSCode CLI is installed
        if which("code").is_ok() {
            assert!(plugin.validate().await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_vscode_extensions_plugin_config() {
        // Test with no config
        let plugin = VSCodeExtensionsPlugin::new();
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert!(plugin.get_hooks().is_empty());

        // Test with config
        let config_toml = r#"
            target_path = "vscode"
            output_file = "extensions.txt"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = VSCodeExtensionsPlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_target_path(),
            Some("vscode".to_string())
        );
        assert_eq!(
            plugin_with_config.get_output_file(),
            Some("extensions.txt".to_string())
        );
        assert!(plugin_with_config.get_hooks().is_empty());
    }
}

// Auto-register this plugin
crate::register_plugin!(VSCodeExtensionsPlugin, "vscode_extensions", "vscode");

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

    #[schemars(
        description = "Custom target directory for restoration (default: current directory for extensions list)"
    )]
    restore_target_dir: Option<String>,

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

    fn get_restore_target_dir(&self) -> Option<String> {
        self.get_config()?.restore_target_dir
    }

    fn get_default_restore_target_dir(&self) -> Result<std::path::PathBuf> {
        // VSCode extensions list is typically saved to the current directory
        Ok(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")))
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

    async fn restore(
        &self,
        snapshot_path: &std::path::Path,
        target_path: &std::path::Path,
        dry_run: bool,
    ) -> Result<Vec<std::path::PathBuf>> {
        use tokio::fs;
        use tracing::{info, warn};

        let mut restored_files = Vec::new();

        // Find extensions file in the snapshot
        let extensions_filename = self
            .get_output_file()
            .unwrap_or_else(|| "extensions.txt".to_string());
        let mut source_extensions = snapshot_path.join(&extensions_filename);

        if !source_extensions.exists() {
            // Try alternative common names
            let alternative_names = ["extensions.txt", "vscode_extensions.txt", "extensions.list"];
            let mut found = false;

            for name in &alternative_names {
                let alt_path = snapshot_path.join(name);
                if alt_path.exists() {
                    source_extensions = alt_path;
                    info!(
                        "Found VSCode extensions file at alternative path: {}",
                        source_extensions.display()
                    );
                    found = true;
                    break;
                }
            }

            if !found {
                return Ok(restored_files); // No extensions file found
            }
        }

        let target_extensions_file = target_path.join("vscode_extensions.txt");

        if dry_run {
            warn!(
                "DRY RUN: Would restore VSCode extensions list to {}",
                target_extensions_file.display()
            );
            warn!("DRY RUN: Review the extension list and install manually with 'code --install-extension <extension-id>'");
            restored_files.push(target_extensions_file);
        } else {
            // Create target directory if it doesn't exist
            if let Some(parent) = target_extensions_file.parent() {
                fs::create_dir_all(parent)
                    .await
                    .context("Failed to create target directory for VSCode extensions file")?;
            }

            // Copy extensions file to target location
            fs::copy(&source_extensions, &target_extensions_file)
                .await
                .with_context(|| {
                    format!(
                        "Failed to restore VSCode extensions from {}",
                        source_extensions.display()
                    )
                })?;

            info!(
                "Restored VSCode extensions list to {}",
                target_extensions_file.display()
            );
            info!("Note: This is a reference list. To install extensions, you'll need to:");
            info!("  1. Review the extension list in the restored file");
            info!(
                "  2. Install extensions manually with 'code --install-extension <extension-id>'"
            );
            info!("  3. Or create an automation script based on the extension list");

            restored_files.push(target_extensions_file);
        }

        Ok(restored_files)
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

    #[tokio::test]
    async fn test_vscode_extensions_restore_functionality() {
        use tempfile::TempDir;
        use tokio::fs;

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        let test_content = "ms-python.python@2023.2.0\nbradlc.vscode-tailwindcss@0.8.6";
        let extensions_path = snapshot_dir.join("extensions.txt");
        fs::write(&extensions_path, test_content).await.unwrap();

        let plugin = VSCodeExtensionsPlugin::new();
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert!(target_dir.join("vscode_extensions.txt").exists());

        let restored_content = fs::read_to_string(target_dir.join("vscode_extensions.txt"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_content);
    }

    #[test]
    fn test_vscode_extensions_restore_target_dir_methods() {
        let plugin = VSCodeExtensionsPlugin::new();

        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert!(default_dir.is_absolute() || default_dir == std::path::PathBuf::from("."));

        assert_eq!(plugin.get_restore_target_dir(), None);
    }
}

// Auto-register this plugin
crate::register_plugin!(VSCodeExtensionsPlugin, "vscode_extensions", "vscode");

use anyhow::{Context, Result};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tracing::warn;

use crate::core::config_schema::{ConfigSchema, ValidationHelpers};
use crate::core::hooks::HookAction;
use crate::core::plugin::Plugin;
use crate::symbols::*;

/// Plugin for capturing VSCode keybindings
pub struct VSCodeKeybindingsPlugin {
    config: Option<toml::Value>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct VSCodeKeybindingsConfig {
    #[schemars(description = "Custom directory path within the snapshot for this plugin's output")]
    target_path: Option<String>,

    #[schemars(
        description = "Custom filename for the keybindings output (default: keybindings.json)"
    )]
    output_file: Option<String>,

    #[schemars(
        description = "Custom target directory for restoration (default: VSCode settings directory)"
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

impl ConfigSchema for VSCodeKeybindingsConfig {
    fn schema_name() -> &'static str {
        "VSCodeKeybindingsConfig"
    }

    fn validate(&self) -> Result<()> {
        // Validate output file extension if specified
        if let Some(output_file) = &self.output_file {
            // Keybindings are typically JSON files
            ValidationHelpers::validate_file_extension(output_file, &["json", "jsonc"])?;
        }

        Ok(())
    }
}

impl VSCodeKeybindingsPlugin {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn with_config(config: toml::Value) -> Self {
        // Validate configuration using schema validation
        match VSCodeKeybindingsConfig::from_toml_value(&config) {
            Ok(_) => {
                // Configuration is valid
                Self {
                    config: Some(config),
                }
            }
            Err(e) => {
                // Use shared error formatting
                let error_msg = ValidationHelpers::format_validation_error(
                    "VSCode Keybindings plugin",
                    "vscode_keybindings",
                    "target_path (string), output_file (string), hooks (object)",
                    "target_path = \"vscode\", output_file = \"keybindings.json\"",
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

    fn get_config(&self) -> Option<VSCodeKeybindingsConfig> {
        self.config
            .as_ref()
            .and_then(|c| VSCodeKeybindingsConfig::from_toml_value(c).ok())
    }

    /// Gets the VSCode settings directory based on OS
    fn get_vscode_settings_dir(&self) -> Result<PathBuf> {
        let home_dir = dirs::home_dir().context("Could not determine home directory")?;

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

    /// Reads VSCode keybindings.json file
    async fn get_keybindings(&self) -> Result<String> {
        let keybindings_path = self.get_vscode_settings_dir()?.join("keybindings.json");

        if !keybindings_path.exists() {
            return Ok("[]".to_string());
        }

        let content = fs::read_to_string(&keybindings_path)
            .await
            .context("Failed to read VSCode keybindings.json")?;

        Ok(content)
    }
}

#[async_trait]
impl Plugin for VSCodeKeybindingsPlugin {
    fn description(&self) -> &str {
        "Captures VSCode custom keybindings configuration"
    }

    fn icon(&self) -> &str {
        TOOL_COMPUTER
    }

    async fn execute(&self) -> Result<String> {
        self.get_keybindings().await
    }

    async fn validate(&self) -> Result<()> {
        // Check if settings directory exists
        let settings_dir = self.get_vscode_settings_dir()?;
        if !settings_dir.exists() {
            return Err(anyhow::anyhow!(
                "VSCode settings directory not found: {}",
                settings_dir.display()
            ));
        }

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
        self.get_vscode_settings_dir()
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
        use tracing::{info, warn};

        let mut restored_files = Vec::new();

        // Find keybindings.json in the snapshot
        let keybindings_file = snapshot_path.join("keybindings.json");
        if !keybindings_file.exists() {
            return Ok(restored_files);
        }

        // Use the target directory provided by RestoreManager
        let target_keybindings_file = target_path.join("keybindings.json");

        if dry_run {
            warn!(
                "DRY RUN: Would restore VSCode keybindings to {}",
                target_keybindings_file.display()
            );
            restored_files.push(target_keybindings_file);
        } else {
            // Create VSCode settings directory if it doesn't exist
            if let Some(parent) = target_keybindings_file.parent() {
                fs::create_dir_all(parent)
                    .await
                    .context("Failed to create VSCode settings directory")?;
            }

            // Copy keybindings file
            fs::copy(&keybindings_file, &target_keybindings_file)
                .await
                .with_context(|| {
                    format!(
                        "Failed to restore VSCode keybindings from {}",
                        keybindings_file.display()
                    )
                })?;

            info!(
                "Restored VSCode keybindings to {}",
                target_keybindings_file.display()
            );
            restored_files.push(target_keybindings_file);
        }

        Ok(restored_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vscode_keybindings_plugin_description() {
        let plugin = VSCodeKeybindingsPlugin::new();
        assert_eq!(
            plugin.description(),
            "Captures VSCode custom keybindings configuration"
        );
    }

    #[tokio::test]
    async fn test_vscode_keybindings_dir() {
        let plugin = VSCodeKeybindingsPlugin::new();
        let settings_dir = plugin.get_vscode_settings_dir().unwrap();

        // Just check that we get a valid path
        assert!(settings_dir.is_absolute());
    }

    #[tokio::test]
    async fn test_vscode_keybindings_plugin_config() {
        // Test with no config
        let plugin = VSCodeKeybindingsPlugin::new();
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert!(plugin.get_hooks().is_empty());

        // Test with config
        let config_toml = r#"
            target_path = "vscode"
            output_file = "keybindings.json"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = VSCodeKeybindingsPlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_target_path(),
            Some("vscode".to_string())
        );
        assert_eq!(
            plugin_with_config.get_output_file(),
            Some("keybindings.json".to_string())
        );
        assert!(plugin_with_config.get_hooks().is_empty());
    }

    #[tokio::test]
    async fn test_vscode_keybindings_restore_functionality() {
        use tempfile::TempDir;
        use tokio::fs;

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        let test_content =
            r#"[{"key": "ctrl+k ctrl+s", "command": "workbench.action.openKeyboardShortcuts"}]"#;
        let keybindings_path = snapshot_dir.join("keybindings.json");
        fs::write(&keybindings_path, test_content).await.unwrap();

        let plugin = VSCodeKeybindingsPlugin::new();
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert!(target_dir.join("keybindings.json").exists());

        let restored_content = fs::read_to_string(target_dir.join("keybindings.json"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_content);
    }

    #[test]
    fn test_vscode_keybindings_restore_target_dir_methods() {
        let plugin = VSCodeKeybindingsPlugin::new();

        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert!(default_dir.is_absolute());

        assert_eq!(plugin.get_restore_target_dir(), None);
    }
}

// Auto-register this plugin
crate::register_plugin!(VSCodeKeybindingsPlugin, "vscode_keybindings", "vscode");

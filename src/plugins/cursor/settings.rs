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

/// Plugin for capturing Cursor settings
pub struct CursorSettingsPlugin {
    config: Option<toml::Value>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct CursorSettingsConfig {
    #[schemars(description = "Custom directory path within the snapshot for this plugin's output")]
    target_path: Option<String>,

    #[schemars(description = "Custom filename for the settings output (default: settings.json)")]
    output_file: Option<String>,

    #[schemars(
        description = "Custom target directory for restoration (default: Cursor settings directory)"
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

impl ConfigSchema for CursorSettingsConfig {
    fn schema_name() -> &'static str {
        "CursorSettingsConfig"
    }

    fn validate(&self) -> Result<()> {
        // Validate output file extension if specified
        if let Some(output_file) = &self.output_file {
            // Settings are typically JSON files
            ValidationHelpers::validate_file_extension(output_file, &["json", "jsonc"])?;
        }

        Ok(())
    }
}

impl CursorSettingsPlugin {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn with_config(config: toml::Value) -> Self {
        // Validate configuration using schema validation
        match CursorSettingsConfig::from_toml_value(&config) {
            Ok(_) => {
                // Configuration is valid
                Self {
                    config: Some(config),
                }
            }
            Err(e) => {
                // Use shared error formatting
                let error_msg = ValidationHelpers::format_validation_error(
                    "Cursor Settings plugin",
                    "cursor_settings",
                    "target_path (string), output_file (string), hooks (object)",
                    "target_path = \"cursor\", output_file = \"settings.json\"",
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

    fn get_config(&self) -> Option<CursorSettingsConfig> {
        self.config
            .as_ref()
            .and_then(|c| CursorSettingsConfig::from_toml_value(c).ok())
    }

    /// Gets the Cursor settings directory based on OS
    fn get_cursor_settings_dir(&self) -> Result<PathBuf> {
        let home_dir = dirs::home_dir().context("Could not determine home directory")?;

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

    /// Reads Cursor settings.json file
    async fn get_settings(&self) -> Result<String> {
        let settings_path = self.get_cursor_settings_dir()?.join("settings.json");

        if !settings_path.exists() {
            return Ok("{}".to_string());
        }

        let content = fs::read_to_string(&settings_path)
            .await
            .context("Failed to read Cursor settings.json")?;

        Ok(content)
    }
}

#[async_trait]
impl Plugin for CursorSettingsPlugin {
    fn description(&self) -> &str {
        "Captures Cursor editor user settings configuration"
    }

    fn icon(&self) -> &str {
        TOOL_EDITOR
    }

    async fn execute(&self) -> Result<String> {
        self.get_settings().await
    }

    async fn validate(&self) -> Result<()> {
        // Check if settings directory exists
        let settings_dir = self.get_cursor_settings_dir()?;
        if !settings_dir.exists() {
            return Err(anyhow::anyhow!(
                "Cursor settings directory not found: {}",
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
        self.get_cursor_settings_dir()
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

        // Find settings.json in the snapshot
        let settings_file = snapshot_path.join("settings.json");
        if !settings_file.exists() {
            return Ok(restored_files);
        }

        // Use the target directory provided by RestoreManager
        // (RestoreManager handles CLI override > plugin config > default precedence)
        let target_settings_file = target_path.join("settings.json");

        if dry_run {
            warn!(
                "DRY RUN: Would restore Cursor settings to {}",
                target_settings_file.display()
            );
            restored_files.push(target_settings_file);
        } else {
            // Create Cursor settings directory if it doesn't exist
            if let Some(parent) = target_settings_file.parent() {
                fs::create_dir_all(parent)
                    .await
                    .context("Failed to create Cursor settings directory")?;
            }

            // Copy settings file
            fs::copy(&settings_file, &target_settings_file)
                .await
                .with_context(|| {
                    format!(
                        "Failed to restore Cursor settings from {}",
                        settings_file.display()
                    )
                })?;

            info!(
                "Restored Cursor settings to {}",
                target_settings_file.display()
            );
            restored_files.push(target_settings_file);
        }

        Ok(restored_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cursor_settings_plugin_description() {
        let plugin = CursorSettingsPlugin::new();
        assert_eq!(
            plugin.description(),
            "Captures Cursor editor user settings configuration"
        );
    }

    #[tokio::test]
    async fn test_cursor_settings_dir() {
        let plugin = CursorSettingsPlugin::new();
        let settings_dir = plugin.get_cursor_settings_dir().unwrap();

        // Just check that we get a valid path
        assert!(settings_dir.is_absolute());
    }

    #[tokio::test]
    async fn test_cursor_settings_plugin_config() {
        // Test with no config
        let plugin = CursorSettingsPlugin::new();
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert!(plugin.get_hooks().is_empty());

        // Test with config
        let config_toml = r#"
            target_path = "cursor"
            output_file = "settings.json"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = CursorSettingsPlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_target_path(),
            Some("cursor".to_string())
        );
        assert_eq!(
            plugin_with_config.get_output_file(),
            Some("settings.json".to_string())
        );
        assert!(plugin_with_config.get_hooks().is_empty());
    }

    #[tokio::test]
    async fn test_cursor_settings_restore_functionality() {
        use tempfile::TempDir;
        use tokio::fs;

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test settings.json
        let test_settings_content = r#"{
    "editor.fontSize": 14,
    "editor.theme": "dark",
    "workbench.colorTheme": "Default Dark+"
}"#;
        let settings_path = snapshot_dir.join("settings.json");
        fs::write(&settings_path, test_settings_content)
            .await
            .unwrap();

        let plugin = CursorSettingsPlugin::new();

        // Test dry run
        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target_dir.join("settings.json"));
        assert!(!target_dir.join("settings.json").exists());

        // Test actual restore
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert!(target_dir.join("settings.json").exists());

        let restored_content = fs::read_to_string(target_dir.join("settings.json"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_settings_content);
    }

    #[tokio::test]
    async fn test_cursor_settings_restore_no_file() {
        use tempfile::TempDir;
        use tokio::fs;

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        let plugin = CursorSettingsPlugin::new();
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        assert!(result.is_empty());
    }

    #[test]
    fn test_cursor_settings_restore_target_dir_methods() {
        let plugin = CursorSettingsPlugin::new();

        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert!(default_dir.is_absolute());

        assert_eq!(plugin.get_restore_target_dir(), None);

        let config_toml = r#"
            target_path = "cursor"
            output_file = "settings.json"
            restore_target_dir = "/custom/cursor/path"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = CursorSettingsPlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_restore_target_dir(),
            Some("/custom/cursor/path".to_string())
        );
    }
}

// Auto-register this plugin
crate::register_plugin!(CursorSettingsPlugin, "cursor_settings", "cursor");

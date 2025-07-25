use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::fs;

use crate::plugins::core::base::settings::{SettingsCore, SettingsPlugin};
use crate::plugins::core::mixins::CommandMixin;
use crate::symbols::*;

/// VSCode-specific settings implementation using the mixin architecture
#[derive(Default)]
pub struct VSCodeCore;

impl SettingsCore for VSCodeCore {
    fn app_name(&self) -> &'static str {
        "VSCode"
    }

    fn settings_file_name(&self) -> &'static str {
        "settings.json"
    }

    fn get_settings_dir(&self) -> Result<PathBuf> {
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

    fn read_settings(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>> {
        Box::pin(async move {
            let settings_dir = self.get_settings_dir()?;
            let settings_path = settings_dir.join("settings.json");

            if !settings_path.exists() {
                return Ok("{}".to_string());
            }

            let content = fs::read_to_string(&settings_path)
                .await
                .context("Failed to read VSCode settings.json")?;

            Ok(content)
        })
    }

    fn icon(&self) -> &'static str {
        TOOL_COMPUTER
    }

    fn allowed_extensions(&self) -> &'static [&'static str] {
        &["json", "jsonc"]
    }
}

impl CommandMixin for VSCodeCore {
    // Uses default implementation - no custom command behavior needed
}

/// Type alias for the VSCode settings plugin
pub type VSCodeSettingsPlugin = SettingsPlugin<VSCodeCore>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::plugin::Plugin;
    use crate::plugins::core::mixins::ConfigMixin;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_vscode_core_app_info() {
        let core = VSCodeCore;
        assert_eq!(core.app_name(), "VSCode");
        assert_eq!(core.settings_file_name(), "settings.json");
        assert_eq!(core.icon(), TOOL_COMPUTER);
        assert_eq!(core.allowed_extensions(), &["json", "jsonc"]);
    }

    #[tokio::test]
    async fn test_vscode_settings_plugin_creation() {
        let plugin = SettingsPlugin::new(VSCodeCore);
        assert_eq!(
            plugin.description(),
            "Captures application settings configuration"
        );
        assert_eq!(plugin.icon(), TOOL_COMPUTER);
    }

    #[tokio::test]
    async fn test_vscode_settings_plugin_with_config() {
        let config_toml = r#"
            target_path = "vscode"
            output_file = "settings.json"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin = SettingsPlugin::with_config(VSCodeCore, config);

        assert_eq!(
            ConfigMixin::get_target_path(&plugin),
            Some("vscode".to_string())
        );
        assert_eq!(
            ConfigMixin::get_output_file(&plugin),
            Some("settings.json".to_string())
        );
    }

    #[tokio::test]
    async fn test_vscode_settings_plugin_restore() {
        let plugin = SettingsPlugin::new(VSCodeCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test settings file
        let test_settings = r#"{"theme": "dark", "fontSize": 14}"#;
        let settings_path = snapshot_dir.join("settings.json");
        fs::write(&settings_path, test_settings).await.unwrap();

        // Test dry run
        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
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
        assert_eq!(restored_content, test_settings);
    }

    #[tokio::test]
    async fn test_vscode_settings_validation() {
        let plugin = SettingsPlugin::new(VSCodeCore);

        // Note: This test will only pass if VSCode is actually installed
        // In CI/CD, this might fail, but that's expected behavior
        let validation_result = plugin.validate().await;

        // The validation should either succeed (VSCode installed) or fail with directory not found
        if let Err(e) = validation_result {
            assert!(e
                .to_string()
                .contains("VSCode settings directory not found"));
        }
    }
}

// Auto-register this plugin using the VSCodeCore implementation
crate::register_mixin_plugin!(
    VSCodeSettingsPlugin,
    VSCodeCore,
    "vscode_settings",
    "vscode"
);

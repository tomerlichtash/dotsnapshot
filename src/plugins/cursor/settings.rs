use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::fs;

use crate::plugins::core::base::settings::{SettingsCore, SettingsPlugin};
use crate::plugins::core::mixins::CommandMixin;
use crate::symbols::*;

/// Cursor-specific settings implementation using the mixin architecture
#[derive(Default)]
pub struct CursorCore;

impl SettingsCore for CursorCore {
    fn app_name(&self) -> &'static str {
        "Cursor"
    }

    fn settings_file_name(&self) -> &'static str {
        "settings.json"
    }

    fn get_settings_dir(&self) -> Result<PathBuf> {
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
                .context("Failed to read Cursor settings.json")?;

            Ok(content)
        })
    }

    fn icon(&self) -> &'static str {
        TOOL_EDITOR
    }

    fn allowed_extensions(&self) -> &'static [&'static str] {
        &["json", "jsonc"]
    }
}

impl CommandMixin for CursorCore {
    // Uses default implementation - no custom command behavior needed
}

/// Type alias for the new Cursor settings plugin
pub type CursorSettingsPluginNew = SettingsPlugin<CursorCore>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::plugin::Plugin;
    use crate::plugins::core::mixins::ConfigMixin;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_cursor_core_app_info() {
        let core = CursorCore;
        assert_eq!(core.app_name(), "Cursor");
        assert_eq!(core.settings_file_name(), "settings.json");
        assert_eq!(core.icon(), TOOL_EDITOR);
        assert_eq!(core.allowed_extensions(), &["json", "jsonc"]);
    }

    #[tokio::test]
    async fn test_cursor_settings_plugin_creation() {
        let plugin = SettingsPlugin::new(CursorCore);
        assert_eq!(
            plugin.description(),
            "Captures application settings configuration"
        );
        assert_eq!(plugin.icon(), TOOL_EDITOR);
    }

    #[tokio::test]
    async fn test_cursor_settings_plugin_with_config() {
        let config_toml = r#"
            target_path = "cursor"
            output_file = "settings.json"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin = SettingsPlugin::with_config(CursorCore, config);

        assert_eq!(
            ConfigMixin::get_target_path(&plugin),
            Some("cursor".to_string())
        );
        assert_eq!(
            ConfigMixin::get_output_file(&plugin),
            Some("settings.json".to_string())
        );
    }

    #[tokio::test]
    async fn test_cursor_settings_plugin_restore() {
        let plugin = SettingsPlugin::new(CursorCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test settings file
        let test_settings = r#"{
    "editor.fontSize": 14,
    "editor.theme": "dark",
    "workbench.colorTheme": "Default Dark+"
}"#;
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
    async fn test_cursor_settings_validation() {
        let plugin = SettingsPlugin::new(CursorCore);

        // Note: This test will only pass if Cursor is actually installed
        // In CI/CD, this might fail, but that's expected behavior
        let validation_result = plugin.validate().await;

        // The validation should either succeed (Cursor installed) or fail with directory not found
        if let Err(e) = validation_result {
            assert!(e
                .to_string()
                .contains("Cursor settings directory not found"));
        }
    }

    #[tokio::test]
    async fn test_cursor_settings_restore_no_file() {
        let plugin = SettingsPlugin::new(CursorCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_cursor_settings_restore_target_dir_methods() {
        let plugin = SettingsPlugin::new(CursorCore);

        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert!(default_dir.is_absolute());

        assert_eq!(ConfigMixin::get_restore_target_dir(&plugin), None);

        let config_toml = r#"
            target_path = "cursor"
            output_file = "settings.json"
            restore_target_dir = "/custom/cursor/path"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = SettingsPlugin::with_config(CursorCore, config);

        assert_eq!(
            ConfigMixin::get_restore_target_dir(&plugin_with_config),
            Some("/custom/cursor/path".to_string())
        );
    }
}

// Auto-register this plugin using the CursorCore implementation
crate::register_mixin_plugin!(
    CursorSettingsPluginNew,
    CursorCore,
    "cursor_settings",
    "cursor"
);

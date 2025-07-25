use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::fs;

use crate::plugins::core::base::keybindings::{KeybindingsCore, KeybindingsPlugin};
use crate::symbols::*;

/// VSCode-specific keybindings implementation using the mixin architecture
#[derive(Default)]
pub struct VSCodeKeybindingsCore;

impl KeybindingsCore for VSCodeKeybindingsCore {
    fn app_name(&self) -> &'static str {
        "VSCode"
    }

    fn keybindings_file_name(&self) -> &'static str {
        "keybindings.json"
    }

    fn get_keybindings_dir(&self) -> Result<PathBuf> {
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

    fn read_keybindings(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>> {
        Box::pin(async move {
            let keybindings_dir = self.get_keybindings_dir()?;
            let keybindings_path = keybindings_dir.join("keybindings.json");

            if !keybindings_path.exists() {
                return Ok("[]".to_string());
            }

            let content = fs::read_to_string(&keybindings_path)
                .await
                .context("Failed to read VSCode keybindings.json")?;

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

/// Type alias for the VSCode keybindings plugin
pub type VSCodeKeybindingsPlugin = KeybindingsPlugin<VSCodeKeybindingsCore>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::plugin::Plugin;
    use crate::plugins::core::mixins::ConfigMixin;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_vscode_keybindings_core_app_info() {
        let core = VSCodeKeybindingsCore;
        assert_eq!(core.app_name(), "VSCode");
        assert_eq!(core.keybindings_file_name(), "keybindings.json");
        assert_eq!(core.icon(), TOOL_COMPUTER);
        assert_eq!(core.allowed_extensions(), &["json", "jsonc"]);
    }

    #[tokio::test]
    async fn test_vscode_keybindings_plugin_creation() {
        let plugin = KeybindingsPlugin::new(VSCodeKeybindingsCore);
        assert_eq!(
            plugin.description(),
            "Captures application keybindings configuration"
        );
        assert_eq!(plugin.icon(), TOOL_COMPUTER);
    }

    #[tokio::test]
    async fn test_vscode_keybindings_plugin_with_config() {
        let config_toml = r#"
            target_path = "vscode"
            output_file = "keybindings.json"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin = KeybindingsPlugin::with_config(VSCodeKeybindingsCore, config);

        assert_eq!(
            ConfigMixin::get_target_path(&plugin),
            Some("vscode".to_string())
        );
        assert_eq!(
            ConfigMixin::get_output_file(&plugin),
            Some("keybindings.json".to_string())
        );
    }

    #[tokio::test]
    async fn test_vscode_keybindings_plugin_restore() {
        let plugin = KeybindingsPlugin::new(VSCodeKeybindingsCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test keybindings file
        let test_keybindings = r#"[
    {
        "key": "ctrl+shift+p",
        "command": "workbench.action.showCommands"
    },
    {
        "key": "ctrl+`",
        "command": "workbench.action.terminal.toggleTerminal"
    }
]"#;
        let keybindings_path = snapshot_dir.join("keybindings.json");
        fs::write(&keybindings_path, test_keybindings)
            .await
            .unwrap();

        // Test dry run
        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert!(!target_dir.join("keybindings.json").exists());

        // Test actual restore
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert!(target_dir.join("keybindings.json").exists());

        let restored_content = fs::read_to_string(target_dir.join("keybindings.json"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_keybindings);
    }

    #[tokio::test]
    async fn test_vscode_keybindings_validation() {
        let plugin = KeybindingsPlugin::new(VSCodeKeybindingsCore);

        // Note: This test will only pass if VSCode is actually installed
        // In CI/CD, this might fail, but that's expected behavior
        let validation_result = plugin.validate().await;

        // The validation should either succeed (VSCode installed) or fail with directory not found
        if let Err(e) = validation_result {
            assert!(e
                .to_string()
                .contains("VSCode keybindings directory not found"));
        }
    }

    #[tokio::test]
    async fn test_vscode_keybindings_restore_no_file() {
        let plugin = KeybindingsPlugin::new(VSCodeKeybindingsCore);

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
    async fn test_vscode_keybindings_restore_target_dir_methods() {
        let plugin = KeybindingsPlugin::new(VSCodeKeybindingsCore);

        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert!(default_dir.is_absolute());

        assert_eq!(ConfigMixin::get_restore_target_dir(&plugin), None);

        let config_toml = r#"
            target_path = "vscode"
            output_file = "keybindings.json"
            restore_target_dir = "/custom/vscode/path"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = KeybindingsPlugin::with_config(VSCodeKeybindingsCore, config);

        assert_eq!(
            ConfigMixin::get_restore_target_dir(&plugin_with_config),
            Some("/custom/vscode/path".to_string())
        );
    }
}

// Auto-register this plugin using the VSCodeKeybindingsCore implementation
crate::register_mixin_plugin!(
    VSCodeKeybindingsPlugin,
    VSCodeKeybindingsCore,
    "vscode_keybindings",
    "vscode"
);

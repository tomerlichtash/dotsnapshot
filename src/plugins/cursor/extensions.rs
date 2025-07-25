use anyhow::Result;
use std::path::PathBuf;

use crate::plugins::core::base::extensions::{ExtensionsCore, ExtensionsPlugin};
use crate::plugins::core::mixins::CommandMixin;
use crate::symbols::*;

/// Cursor-specific extensions implementation using the mixin architecture
#[derive(Default)]
pub struct CursorExtensionsCore;

impl ExtensionsCore for CursorExtensionsCore {
    fn app_name(&self) -> &'static str {
        "Cursor"
    }

    fn extensions_command(&self) -> &'static str {
        "cursor"
    }

    fn list_extensions_args(&self) -> &'static [&'static str] {
        &["--list-extensions", "--show-versions"]
    }

    fn icon(&self) -> &'static str {
        TOOL_EDITOR
    }

    fn extensions_file_name(&self) -> String {
        "extensions.txt".to_string()
    }

    fn restore_file_name(&self) -> String {
        "cursor_extensions.txt".to_string()
    }

    fn allowed_extensions(&self) -> &'static [&'static str] {
        &["txt", "log", "list"]
    }

    fn get_default_restore_dir(&self) -> Result<PathBuf> {
        // Cursor extensions list is typically saved to the current directory
        Ok(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }
}

impl CommandMixin for CursorExtensionsCore {
    // Uses default implementation with the extensions_command
}

/// Type alias for the new Cursor extensions plugin
pub type CursorExtensionsPluginNew = ExtensionsPlugin<CursorExtensionsCore>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::plugin::Plugin;
    use crate::plugins::core::mixins::ConfigMixin;
    use tempfile::TempDir;
    use tokio::fs;
    use which::which;

    #[tokio::test]
    async fn test_cursor_extensions_core_app_info() {
        let core = CursorExtensionsCore;
        assert_eq!(core.app_name(), "Cursor");
        assert_eq!(core.extensions_command(), "cursor");
        assert_eq!(
            core.list_extensions_args(),
            &["--list-extensions", "--show-versions"]
        );
        assert_eq!(core.icon(), TOOL_EDITOR);
        assert_eq!(core.extensions_file_name(), "extensions.txt");
        assert_eq!(core.restore_file_name(), "cursor_extensions.txt");
        assert_eq!(core.allowed_extensions(), &["txt", "log", "list"]);
    }

    #[tokio::test]
    async fn test_cursor_extensions_plugin_new_creation() {
        let plugin = ExtensionsPlugin::new(CursorExtensionsCore);
        assert_eq!(
            plugin.description(),
            "Lists installed extensions for application"
        );
        assert_eq!(plugin.icon(), TOOL_EDITOR);
    }

    #[tokio::test]
    async fn test_cursor_extensions_plugin_new_validation() {
        let plugin = ExtensionsPlugin::new(CursorExtensionsCore);

        // This test will only pass if Cursor CLI is installed
        if which("cursor").is_ok() {
            assert!(plugin.validate().await.is_ok());
        } else {
            // Should fail with command not found
            assert!(plugin.validate().await.is_err());
        }
    }

    #[tokio::test]
    async fn test_cursor_extensions_plugin_new_with_config() {
        let config_toml = r#"
            target_path = "cursor"
            output_file = "extensions.txt"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin = ExtensionsPlugin::with_config(CursorExtensionsCore, config);

        assert_eq!(
            ConfigMixin::get_target_path(&plugin),
            Some("cursor".to_string())
        );
        assert_eq!(
            ConfigMixin::get_output_file(&plugin),
            Some("extensions.txt".to_string())
        );
    }

    #[tokio::test]
    async fn test_cursor_extensions_plugin_new_restore() {
        let plugin = ExtensionsPlugin::new(CursorExtensionsCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test extensions file
        let test_extensions = "ms-python.python@2023.1.0\nms-vscode.vscode-typescript-next@5.0.0";
        let extensions_path = snapshot_dir.join("extensions.txt");
        fs::write(&extensions_path, test_extensions).await.unwrap();

        // Test restore
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert!(target_dir.join("cursor_extensions.txt").exists());

        let restored_content = fs::read_to_string(target_dir.join("cursor_extensions.txt"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_extensions);
    }

    #[test]
    fn test_cursor_extensions_restore_target_dir_methods() {
        let plugin = ExtensionsPlugin::new(CursorExtensionsCore);

        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert!(default_dir.is_absolute() || default_dir == std::path::PathBuf::from("."));

        assert_eq!(ConfigMixin::get_restore_target_dir(&plugin), None);
    }
}

// Auto-register this plugin using the CursorExtensionsCore implementation
crate::register_mixin_plugin!(
    CursorExtensionsPluginNew,
    CursorExtensionsCore,
    "cursor_extensions",
    "cursor"
);

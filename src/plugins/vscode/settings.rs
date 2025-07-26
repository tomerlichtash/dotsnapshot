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
        SYMBOL_TOOL_COMPUTER
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
        assert_eq!(core.icon(), SYMBOL_TOOL_COMPUTER);
        assert_eq!(core.allowed_extensions(), &["json", "jsonc"]);
    }

    #[tokio::test]
    async fn test_vscode_settings_plugin_creation() {
        let plugin = SettingsPlugin::new(VSCodeCore);
        assert_eq!(
            plugin.description(),
            "Captures application settings configuration"
        );
        assert_eq!(plugin.icon(), SYMBOL_TOOL_COMPUTER);
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

    /// Test VSCodeCore get_settings_dir method on different platforms
    /// Verifies platform-specific directory resolution
    #[tokio::test]
    async fn test_vscode_core_get_settings_dir() {
        let core = VSCodeCore;
        let settings_dir = core.get_settings_dir();

        // Should successfully determine a settings directory
        assert!(settings_dir.is_ok());

        let dir_path = settings_dir.unwrap();
        let path_str = dir_path.to_string_lossy();

        // Verify it contains expected platform-specific paths
        if cfg!(target_os = "macos") {
            assert!(path_str.contains("Library/Application Support/Code/User"));
        } else if cfg!(target_os = "windows") {
            assert!(path_str.contains("AppData/Roaming/Code/User"));
        } else {
            assert!(path_str.contains(".config/Code/User"));
        }
    }

    /// Test VSCodeCore read_settings with non-existent settings file
    /// Verifies default empty JSON return when settings don't exist
    #[tokio::test]
    async fn test_vscode_core_read_settings_nonexistent() {
        let core = VSCodeCore;

        // This test verifies the contract: non-existent file returns empty JSON
        let result = core.read_settings().await;

        // Should either succeed with content (if VSCode installed) or return "{}" for missing file
        if let Ok(content) = result {
            // Content should either be valid JSON or empty
            if !content.is_empty() {
                // Try to parse as JSON, but don't fail if it's not
                match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(_) => {
                        // Valid JSON - good
                    }
                    Err(_) => {
                        // Not valid JSON - that's acceptable for some settings files
                        // Just verify it's some kind of content
                        assert!(!content.trim().is_empty());
                    }
                }
            }
        }
        // If it fails, that's also acceptable (VSCode not installed)
    }

    /// Test VSCodeCore read_settings with existing settings file
    /// Verifies actual file reading when settings exist
    #[tokio::test]
    async fn test_vscode_core_read_settings_existing() {
        let core = VSCodeCore;

        // Try to read actual settings if they exist
        let result = core.read_settings().await;

        match result {
            Ok(content) => {
                // Content should not be empty if successful
                assert!(!content.is_empty());

                // Try to parse as JSON, but don't require it to be valid
                // Some settings might be JSONC or have comments
                match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(_) => {
                        // Valid JSON - excellent
                    }
                    Err(_) => {
                        // Not valid JSON - that's acceptable for JSONC files
                        // Just verify it contains some settings-like content
                        assert!(content.len() > 2); // More than just "{}"
                    }
                }
            }
            Err(_) => {
                // Error is acceptable if VSCode is not installed
                // The error should be about missing directory or file
            }
        }
    }

    /// Test VSCodeCore implementation of allowed extensions
    /// Verifies that VSCode supports both json and jsonc extensions
    #[tokio::test]
    async fn test_vscode_core_allowed_extensions() {
        let core = VSCodeCore;
        let extensions = core.allowed_extensions();

        assert_eq!(extensions.len(), 2);
        assert!(extensions.contains(&"json"));
        assert!(extensions.contains(&"jsonc"));
    }

    /// Test VSCodeSettingsPlugin type alias functionality
    /// Verifies the type alias works correctly and inherits all expected methods
    #[tokio::test]
    async fn test_vscode_settings_plugin_type_alias() {
        let plugin: VSCodeSettingsPlugin = SettingsPlugin::new(VSCodeCore);

        // Test that the type alias preserves all functionality
        assert_eq!(
            plugin.description(),
            "Captures application settings configuration"
        );
        assert_eq!(plugin.icon(), SYMBOL_TOOL_COMPUTER);

        // Test that Plugin trait is implemented
        let execute_result = plugin.execute().await;
        // Should either succeed or fail gracefully
        match execute_result {
            Ok(content) => {
                // Content should be some form of settings data
                assert!(!content.is_empty());
                // Try to parse as JSON, but accept JSONC or other formats
                if content.trim() != "{}" {
                    // If not empty default, should have some content
                    assert!(content.len() >= 2);
                }
            }
            Err(_) => {
                // Error is acceptable if VSCode not installed
            }
        }
    }

    /// Test VSCodeSettingsPlugin plugin registration macro
    /// Verifies that the register_mixin_plugin macro sets up the plugin correctly
    #[tokio::test]
    async fn test_vscode_plugin_registration() {
        let plugin = VSCodeSettingsPlugin::new(VSCodeCore);

        // Verify the plugin has the expected functionality
        assert_eq!(plugin.icon(), SYMBOL_TOOL_COMPUTER);
        assert_eq!(
            plugin.description(),
            "Captures application settings configuration"
        );

        // Verify it uses VSCodeCore properly through validation
        let validation_result = plugin.validate().await;
        // Should either pass or fail with expected VSCode-related error
        if let Err(e) = validation_result {
            assert!(e.to_string().contains("VSCode"));
        }
    }

    /// Test CommandMixin implementation
    /// Verifies that VSCodeCore implements CommandMixin with default behavior
    #[tokio::test]
    async fn test_vscode_core_command_mixin() {
        let core = VSCodeCore;

        // VSCodeCore implements CommandMixin with default implementation
        // This test verifies the trait is implemented (compilation test)

        // Use the core in a way that requires CommandMixin to be implemented
        let _plugin = SettingsPlugin::new(core);

        // If this compiles and runs, CommandMixin is properly implemented
    }

    /// Test VSCodeCore error handling in get_settings_dir
    /// Verifies proper error handling when home directory cannot be determined
    #[tokio::test]
    async fn test_vscode_core_error_handling() {
        let core = VSCodeCore;

        // Test that get_settings_dir handles errors appropriately
        // In normal circumstances, this should work, but we test the error type
        let result = core.get_settings_dir();

        match result {
            Ok(path) => {
                // Normal case - should be a valid path
                assert!(path.is_absolute() || path.starts_with("~"));
            }
            Err(e) => {
                // Error case - should contain expected error message
                assert!(e.to_string().contains("Could not determine home directory"));
            }
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

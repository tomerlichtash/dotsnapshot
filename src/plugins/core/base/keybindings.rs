use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;

use crate::core::plugin::Plugin;
use crate::plugins::core::mixins::{
    CommandMixin, ConfigMixin, FilesMixin, StandardConfig, StandardConfigMixin,
};

/// Core trait for keybindings-specific functionality
pub trait KeybindingsCore: Send + Sync {
    /// Get the application name for this keybindings implementation
    fn app_name(&self) -> &'static str;

    /// Get the keybindings file name (typically "keybindings.json")
    fn keybindings_file_name(&self) -> &'static str;

    /// Get the directory where keybindings are stored
    fn get_keybindings_dir(&self) -> Result<PathBuf>;

    /// Read the keybindings file content
    fn read_keybindings(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>;

    /// Get the icon for this keybindings implementation
    fn icon(&self) -> &'static str;
}

/// Generic keybindings plugin that uses mixins for common functionality
pub struct KeybindingsPlugin<T: KeybindingsCore> {
    core: T,
    config: StandardConfig,
}

impl<T: KeybindingsCore> KeybindingsPlugin<T> {
    /// Create a new keybindings plugin with the given core implementation
    pub fn new(core: T) -> Self {
        Self {
            core,
            config: StandardConfig::default(),
        }
    }

    /// Create a new keybindings plugin with configuration
    pub fn with_config(core: T, config: toml::Value) -> Self {
        let parsed_config = config
            .try_into()
            .unwrap_or_else(|_| StandardConfig::default());

        Self {
            core,
            config: parsed_config,
        }
    }
}

#[async_trait]
impl<T: KeybindingsCore> Plugin for KeybindingsPlugin<T> {
    fn description(&self) -> &str {
        "Captures application keybindings configuration"
    }

    fn icon(&self) -> &str {
        self.core.icon()
    }

    async fn execute(&self) -> Result<String> {
        self.core.read_keybindings().await
    }

    async fn validate(&self) -> Result<()> {
        // Check if keybindings directory exists
        let keybindings_dir = self.core.get_keybindings_dir()?;
        if !keybindings_dir.exists() {
            return Err(anyhow::anyhow!(
                "{} keybindings directory not found: {}",
                self.core.app_name(),
                keybindings_dir.display()
            ));
        }
        Ok(())
    }

    fn get_target_path(&self) -> Option<String> {
        ConfigMixin::get_target_path(self)
    }

    fn get_output_file(&self) -> Option<String> {
        ConfigMixin::get_output_file(self)
    }

    async fn restore(
        &self,
        snapshot_dir: &std::path::Path,
        target_dir: &std::path::Path,
        dry_run: bool,
    ) -> Result<Vec<PathBuf>> {
        // Use FilesMixin for file operations
        let keybindings_file = self.core.keybindings_file_name();
        let source_file = snapshot_dir.join(keybindings_file);

        if !source_file.exists() {
            return Ok(vec![]);
        }

        let target_file = target_dir.join(keybindings_file);

        if dry_run {
            tracing::warn!(
                "DRY RUN: Would restore {} keybindings to {}",
                self.core.app_name(),
                target_file.display()
            );
            return Ok(vec![target_dir.to_path_buf()]);
        }

        // Ensure target directory exists
        if let Some(parent) = target_file.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create target directory: {}", e))?;
        }

        // Copy the keybindings file
        tokio::fs::copy(&source_file, &target_file)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to copy keybindings file: {}", e))?;

        tracing::info!(
            "Restored {} keybindings to {}",
            self.core.app_name(),
            target_file.display()
        );

        Ok(vec![target_dir.to_path_buf()])
    }
}

impl<T: KeybindingsCore> ConfigMixin for KeybindingsPlugin<T> {
    type Config = StandardConfig;

    fn config(&self) -> Option<&Self::Config> {
        Some(&self.config)
    }

    fn get_target_path(&self) -> Option<String> {
        self.get_standard_target_path()
    }

    fn get_output_file(&self) -> Option<String> {
        self.get_standard_output_file()
    }

    fn get_restore_target_dir(&self) -> Option<String> {
        self.get_standard_restore_target_dir()
    }
}

impl<T: KeybindingsCore> StandardConfigMixin for KeybindingsPlugin<T> {}

impl<T: KeybindingsCore> FilesMixin for KeybindingsPlugin<T> {
    // Uses default implementation
}

impl<T: KeybindingsCore> CommandMixin for KeybindingsPlugin<T> {
    // Uses default implementation
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::plugin::Plugin;
    use crate::plugins::core::mixins::ConfigMixin;
    use crate::symbols::SYMBOL_TOOL_COMPUTER;
    use tempfile::TempDir;
    use tokio::fs;

    // Mock implementation for testing
    #[derive(Default)]
    struct MockKeybindingsCore;

    impl KeybindingsCore for MockKeybindingsCore {
        fn app_name(&self) -> &'static str {
            "TestApp"
        }

        fn keybindings_file_name(&self) -> &'static str {
            "keybindings.json"
        }

        fn get_keybindings_dir(&self) -> Result<PathBuf> {
            Ok(std::env::temp_dir().join("test_keybindings"))
        }

        fn read_keybindings(
            &self,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
        {
            Box::pin(async move { Ok("[]".to_string()) })
        }

        fn icon(&self) -> &'static str {
            crate::symbols::SYMBOL_TOOL_COMPUTER
        }
    }

    #[tokio::test]
    async fn test_keybindings_plugin_creation() {
        let plugin = KeybindingsPlugin::new(MockKeybindingsCore);
        assert_eq!(
            plugin.description(),
            "Captures application keybindings configuration"
        );
        assert_eq!(plugin.icon(), SYMBOL_TOOL_COMPUTER);
    }

    #[tokio::test]
    async fn test_keybindings_plugin_execute() {
        let plugin = KeybindingsPlugin::new(MockKeybindingsCore);
        let result = plugin.execute().await.unwrap();
        assert_eq!(result, "[]");
    }

    #[tokio::test]
    async fn test_keybindings_plugin_with_config() {
        let config_toml = r#"
            target_path = "test"
            output_file = "test_keybindings.json"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin = KeybindingsPlugin::with_config(MockKeybindingsCore, config);

        assert_eq!(
            ConfigMixin::get_target_path(&plugin),
            Some("test".to_string())
        );
        assert_eq!(
            ConfigMixin::get_output_file(&plugin),
            Some("test_keybindings.json".to_string())
        );
    }

    #[tokio::test]
    async fn test_keybindings_plugin_restore() {
        let plugin = KeybindingsPlugin::new(MockKeybindingsCore);

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
    }
]"#;
        let keybindings_path = snapshot_dir.join("keybindings.json");
        fs::write(&keybindings_path, test_keybindings)
            .await
            .unwrap();

        // Test restore
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

    /// Test keybindings plugin restore with dry run
    /// Verifies that dry run mode doesn't actually copy files
    #[tokio::test]
    async fn test_keybindings_plugin_restore_dry_run() {
        let plugin = KeybindingsPlugin::new(MockKeybindingsCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test keybindings file
        let keybindings_path = snapshot_dir.join("keybindings.json");
        fs::write(&keybindings_path, "test content").await.unwrap();

        // Test dry run restore
        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        // File should not exist after dry run
        assert!(!target_dir.join("keybindings.json").exists());
    }

    /// Test keybindings plugin restore with no source file
    /// Verifies that restore returns empty vector when source doesn't exist
    #[tokio::test]
    async fn test_keybindings_plugin_restore_no_file() {
        let plugin = KeybindingsPlugin::new(MockKeybindingsCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // No keybindings file exists
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 0);
    }

    /// Test keybindings plugin validation success
    /// Verifies that validation passes when keybindings directory exists
    #[tokio::test]
    async fn test_keybindings_plugin_validation() {
        let plugin = KeybindingsPlugin::new(MockKeybindingsCore);

        // Create the keybindings directory
        let keybindings_dir = std::env::temp_dir().join("test_keybindings");
        fs::create_dir_all(&keybindings_dir).await.unwrap();

        let result = plugin.validate().await;
        assert!(result.is_ok());

        // Clean up
        if (fs::remove_dir_all(&keybindings_dir).await).is_err() {
            // If we can't remove it, that's ok - might be a permission issue
        }
    }

    /// Test keybindings plugin validation failure
    /// Verifies that validation fails when keybindings directory doesn't exist
    #[tokio::test]
    async fn test_keybindings_plugin_validation_failure() {
        let plugin = KeybindingsPlugin::new(MockKeybindingsCore);

        // Ensure keybindings directory doesn't exist
        let keybindings_dir = std::env::temp_dir().join("test_keybindings");
        if keybindings_dir.exists() && (fs::remove_dir_all(&keybindings_dir).await).is_err() {
            // If we can't remove it, that's ok - might be a permission issue
            // Just continue with the test
        }

        let result = plugin.validate().await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("directory not found"));
    }

    /// Test keybindings plugin with invalid config
    /// Verifies that invalid config falls back to defaults
    #[tokio::test]
    async fn test_keybindings_plugin_with_invalid_config() {
        let invalid_config = toml::Value::String("invalid".to_string());
        let plugin = KeybindingsPlugin::with_config(MockKeybindingsCore, invalid_config);

        // Should fall back to defaults
        assert_eq!(ConfigMixin::get_target_path(&plugin), None);
        assert_eq!(ConfigMixin::get_output_file(&plugin), None);
    }

    /// Test keybindings plugin restore with nested target directory
    /// Verifies that parent directories are created during restore
    #[tokio::test]
    async fn test_keybindings_plugin_restore_nested_target() {
        let plugin = KeybindingsPlugin::new(MockKeybindingsCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("deep").join("nested").join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        // Don't create target_dir - should be created by restore

        // Create test keybindings file
        let keybindings_path = snapshot_dir.join("keybindings.json");
        fs::write(&keybindings_path, "test content").await.unwrap();

        // Test restore
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert!(target_dir.join("keybindings.json").exists());
    }

    /// Test keybindings plugin mixin implementations
    /// Verifies that all mixin traits are properly implemented
    #[test]
    fn test_keybindings_plugin_mixins() {
        let plugin = KeybindingsPlugin::new(MockKeybindingsCore);

        // Test ConfigMixin methods
        assert!(plugin.config().is_some());
        assert_eq!(ConfigMixin::get_target_path(&plugin), None);
        assert_eq!(ConfigMixin::get_output_file(&plugin), None);
        assert_eq!(ConfigMixin::get_restore_target_dir(&plugin), None);
    }

    /// Test keybindings plugin with complex config
    /// Verifies that complex configuration options are handled correctly
    #[tokio::test]
    async fn test_keybindings_plugin_with_complex_config() {
        let config_toml = r#"
            target_path = "custom/path"
            output_file = "custom_keybindings.json"
            restore_target_dir = "~/custom/restore"
            [hooks]
            pre_snapshot = ["echo before"]
            post_snapshot = ["echo after"]
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin = KeybindingsPlugin::with_config(MockKeybindingsCore, config);

        assert_eq!(
            ConfigMixin::get_target_path(&plugin),
            Some("custom/path".to_string())
        );
        assert_eq!(
            ConfigMixin::get_output_file(&plugin),
            Some("custom_keybindings.json".to_string())
        );
        assert_eq!(
            ConfigMixin::get_restore_target_dir(&plugin),
            Some("~/custom/restore".to_string())
        );
    }

    /// Test keybindings core trait methods
    /// Verifies that all KeybindingsCore methods work correctly
    #[tokio::test]
    async fn test_keybindings_core_methods() {
        let core = MockKeybindingsCore;

        assert_eq!(core.app_name(), "TestApp");
        assert_eq!(core.keybindings_file_name(), "keybindings.json");
        assert_eq!(core.icon(), SYMBOL_TOOL_COMPUTER);

        let keybindings_dir = core.get_keybindings_dir().unwrap();
        assert!(keybindings_dir.ends_with("test_keybindings"));

        let content = core.read_keybindings().await.unwrap();
        assert_eq!(content, "[]");
    }

    /// Test keybindings plugin restore error handling
    /// Verifies that file operation errors are properly handled
    #[tokio::test]
    async fn test_keybindings_plugin_restore_error_handling() {
        let plugin = KeybindingsPlugin::new(MockKeybindingsCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();

        // Create a regular file where we expect a directory
        fs::write(&target_dir, "not a directory").await.unwrap();

        // Create test keybindings file
        let keybindings_path = snapshot_dir.join("keybindings.json");
        fs::write(&keybindings_path, "test content").await.unwrap();

        // Test restore should handle directory creation error
        let result = plugin.restore(&snapshot_dir, &target_dir, false).await;

        // Should get an error because target_dir is a file, not a directory
        assert!(result.is_err());
    }
}

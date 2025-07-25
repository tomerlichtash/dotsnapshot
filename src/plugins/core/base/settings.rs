use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::warn;

use crate::core::plugin::Plugin;
use crate::plugins::core::mixins::{ConfigMixin, FilesMixin, StandardConfig, StandardConfigMixin};

/// Core trait that defines application-specific settings behavior
pub trait SettingsCore: Send + Sync {
    /// The name of the application (e.g., "VSCode", "Cursor")
    fn app_name(&self) -> &'static str;

    /// The default filename for settings (e.g., "settings.json")
    fn settings_file_name(&self) -> &'static str;

    /// Get the application's settings directory (sync version for path resolution)
    fn get_settings_dir(&self) -> Result<PathBuf>;

    /// Read the current settings from the application  
    fn read_settings(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>;

    /// Get the icon for this settings plugin
    fn icon(&self) -> &'static str {
        "⚙️" // Default settings icon
    }

    /// Get custom file extensions allowed for settings files
    fn allowed_extensions(&self) -> &'static [&'static str] {
        &["json", "jsonc"] // Default: JSON files
    }
}

/// Generic settings plugin that can be used for any application
pub struct SettingsPlugin<T: SettingsCore> {
    config: Option<StandardConfig>,
    core: T,
}

impl<T: SettingsCore> SettingsPlugin<T> {
    /// Create a new settings plugin without configuration
    pub fn new(core: T) -> Self {
        Self { config: None, core }
    }

    /// Create a new settings plugin with configuration
    pub fn with_config(core: T, config: toml::Value) -> Self {
        let (parsed_config, is_valid) = Self::with_config_validation(
            config,
            &format!("{} Settings plugin", core.app_name()),
            &format!("{}_settings", core.app_name().to_lowercase()),
            "target_path (string), output_file (string), restore_target_dir (string), hooks (object)",
            &format!(
                "target_path = \"{}\", output_file = \"{}\"",
                core.app_name().to_lowercase(),
                core.settings_file_name()
            ),
        );

        // Additional validation for settings-specific fields
        if is_valid {
            if let Some(output_file) = &parsed_config.output_file {
                let extensions = core.allowed_extensions();
                if let Err(e) =
                    crate::core::config_schema::ValidationHelpers::validate_file_extension(
                        output_file,
                        extensions,
                    )
                {
                    warn!(
                        "Invalid output file extension for {} settings: {}",
                        core.app_name(),
                        e
                    );
                }
            }
        }

        Self {
            config: Some(parsed_config),
            core,
        }
    }
}

// Implement mixins for the settings plugin
impl<T: SettingsCore> ConfigMixin for SettingsPlugin<T> {
    type Config = StandardConfig;

    fn config(&self) -> Option<&Self::Config> {
        self.config.as_ref()
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

impl<T: SettingsCore> StandardConfigMixin for SettingsPlugin<T> {}

// Implement CommandMixin for the settings plugin (basic implementation)
impl<T: SettingsCore> crate::plugins::core::mixins::CommandMixin for SettingsPlugin<T> {}

// Implement FilesMixin for the settings plugin
impl<T: SettingsCore> crate::plugins::core::mixins::FilesMixin for SettingsPlugin<T> {}

#[async_trait]
impl<T: SettingsCore + Send + Sync> Plugin for SettingsPlugin<T> {
    fn description(&self) -> &str {
        // We can't return a dynamic string from a trait method that expects &str,
        // so we'll use a generic description. Individual plugins can override this.
        "Captures application settings configuration"
    }

    fn icon(&self) -> &str {
        self.core.icon()
    }

    async fn execute(&self) -> Result<String> {
        self.core.read_settings().await
    }

    async fn validate(&self) -> Result<()> {
        let settings_dir = self.core.get_settings_dir()?;
        if !self.is_dir_accessible(&settings_dir).await {
            return Err(anyhow::anyhow!(
                "{} settings directory not found: {}",
                self.core.app_name(),
                settings_dir.display()
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

    fn get_restore_target_dir(&self) -> Option<String> {
        ConfigMixin::get_restore_target_dir(self)
    }

    fn get_default_restore_target_dir(&self) -> Result<PathBuf> {
        // Use the application's settings directory as the default restore target
        self.core.get_settings_dir()
    }

    fn get_hooks(&self) -> Vec<crate::core::hooks::HookAction> {
        // Settings plugins don't have hooks by default
        Vec::new()
    }

    async fn restore(
        &self,
        snapshot_path: &std::path::Path,
        target_path: &std::path::Path,
        dry_run: bool,
    ) -> Result<Vec<PathBuf>> {
        use tokio::fs;

        let mut restored_files = Vec::new();

        // Find settings file in the snapshot
        let settings_file = snapshot_path.join(self.core.settings_file_name());
        if !settings_file.exists() {
            return Ok(restored_files);
        }

        // Use the target directory provided by RestoreManager
        let target_settings_file = target_path.join(self.core.settings_file_name());

        if dry_run {
            warn!(
                "DRY RUN: Would restore {} settings to {}",
                self.core.app_name(),
                target_settings_file.display()
            );
            restored_files.push(target_settings_file);
        } else {
            // Create settings directory if it doesn't exist
            if let Some(parent) = target_settings_file.parent() {
                fs::create_dir_all(parent).await.with_context(|| {
                    format!(
                        "Failed to create {} settings directory",
                        self.core.app_name()
                    )
                })?;
            }

            // Copy settings file
            self.restore_file(&settings_file, &target_settings_file)
                .await?;
            restored_files.push(target_settings_file);
        }

        Ok(restored_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    struct MockSettingsCore;

    impl SettingsCore for MockSettingsCore {
        fn app_name(&self) -> &'static str {
            "TestApp"
        }

        fn settings_file_name(&self) -> &'static str {
            "settings.json"
        }

        fn get_settings_dir(&self) -> Result<PathBuf> {
            Ok(PathBuf::from("/test/settings"))
        }

        fn read_settings(
            &self,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
        {
            Box::pin(async { Ok(r#"{"theme": "dark", "fontSize": 14}"#.to_string()) })
        }
    }

    // Implement CommandMixin for MockSettingsCore (needed for compilation)
    impl crate::plugins::core::mixins::CommandMixin for MockSettingsCore {}

    #[tokio::test]
    async fn test_settings_plugin_creation() {
        let core = MockSettingsCore;
        let plugin = SettingsPlugin::new(core);

        assert_eq!(plugin.core.app_name(), "TestApp");
        assert_eq!(plugin.core.settings_file_name(), "settings.json");
        assert_eq!(
            plugin.description(),
            "Captures application settings configuration"
        );
    }

    #[tokio::test]
    async fn test_settings_plugin_execute() {
        let core = MockSettingsCore;
        let plugin = SettingsPlugin::new(core);

        let result = plugin.execute().await.unwrap();
        assert_eq!(result, r#"{"theme": "dark", "fontSize": 14}"#);
    }

    #[tokio::test]
    async fn test_settings_plugin_with_config() {
        let core = MockSettingsCore;
        let config_toml = r#"
            target_path = "testapp"
            output_file = "settings.json"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin = SettingsPlugin::with_config(core, config);

        assert_eq!(
            ConfigMixin::get_target_path(&plugin),
            Some("testapp".to_string())
        );
        assert_eq!(
            ConfigMixin::get_output_file(&plugin),
            Some("settings.json".to_string())
        );
    }

    #[tokio::test]
    async fn test_settings_plugin_restore() {
        let core = MockSettingsCore;
        let plugin = SettingsPlugin::new(core);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test settings file
        let test_settings = r#"{"theme": "light"}"#;
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
}

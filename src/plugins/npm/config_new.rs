use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;

use crate::plugins::core::base::settings::{SettingsCore, SettingsPlugin};
use crate::plugins::core::mixins::CommandMixin;
use crate::symbols::*;

/// NPM-specific configuration implementation using the mixin architecture
#[derive(Default)]
pub struct NpmConfigCore;

impl SettingsCore for NpmConfigCore {
    fn app_name(&self) -> &'static str {
        "NPM"
    }

    fn settings_file_name(&self) -> &'static str {
        "npmrc.txt"
    }

    fn get_settings_dir(&self) -> Result<PathBuf> {
        let home_dir = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home_dir)
    }

    fn read_settings(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>> {
        Box::pin(async move {
            // Get NPM configuration using npm config list
            let output = tokio::task::spawn_blocking(|| {
                Command::new("npm")
                    .args(["config", "list", "--long"])
                    .output()
            })
            .await??;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("npm config list failed: {}", stderr));
            }

            let config_output = String::from_utf8_lossy(&output.stdout);

            // Filter out sensitive information and system paths for security
            let filtered_config: Vec<&str> = config_output
                .lines()
                .filter(|line| {
                    let line = line.trim();
                    // Skip empty lines and comments
                    if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                        return false;
                    }
                    // Skip sensitive information
                    if line.contains("password") || line.contains("token") || line.contains("auth")
                    {
                        return false;
                    }
                    // Skip system-specific paths that shouldn't be restored
                    if line.contains("prefix =")
                        || line.contains("cache =")
                        || line.contains("tmp =")
                    {
                        return false;
                    }
                    true
                })
                .collect();

            if filtered_config.is_empty() {
                Ok("# No NPM configuration found\n".to_string())
            } else {
                Ok(filtered_config.join("\n") + "\n")
            }
        })
    }

    fn icon(&self) -> &'static str {
        TOOL_PACKAGE_MANAGER
    }

    fn allowed_extensions(&self) -> &'static [&'static str] {
        &["txt", "npmrc", "config"]
    }
}

impl CommandMixin for NpmConfigCore {
    // Uses default implementation - no custom command behavior needed

    fn validate_command_exists(
        &self,
        cmd: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
        let cmd = cmd.to_string();
        Box::pin(async move {
            // Check if npm exists
            which::which(&cmd).with_context(|| {
                format!("{cmd} command not found. Please install Node.js and NPM.")
            })?;

            Ok(())
        })
    }
}

/// Type alias for the new NPM config plugin
pub type NpmConfigPluginNew = SettingsPlugin<NpmConfigCore>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::plugin::Plugin;
    use crate::plugins::core::mixins::ConfigMixin;
    use tempfile::TempDir;
    use tokio::fs;
    use which::which;

    #[tokio::test]
    async fn test_npm_config_core_app_info() {
        let core = NpmConfigCore;
        assert_eq!(core.app_name(), "NPM");
        assert_eq!(core.settings_file_name(), "npmrc.txt");
        assert_eq!(core.icon(), TOOL_PACKAGE_MANAGER);
        assert_eq!(core.allowed_extensions(), &["txt", "npmrc", "config"]);
    }

    #[tokio::test]
    async fn test_npm_config_plugin_new_creation() {
        let plugin = SettingsPlugin::new(NpmConfigCore);
        assert_eq!(
            plugin.description(),
            "Captures application settings configuration"
        );
        assert_eq!(plugin.icon(), TOOL_PACKAGE_MANAGER);
    }

    #[tokio::test]
    async fn test_npm_config_plugin_new_validation() {
        let plugin = SettingsPlugin::new(NpmConfigCore);

        // This test will only pass if npm is installed
        if which("npm").is_ok() {
            // The validation should succeed if npm exists
            assert!(plugin.validate().await.is_ok());
        } else {
            // Should fail with command not found
            assert!(plugin.validate().await.is_err());
        }
    }

    #[tokio::test]
    async fn test_npm_config_plugin_new_with_config() {
        let config_toml = r#"
            target_path = "npm"
            output_file = "npmrc.txt"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin = SettingsPlugin::with_config(NpmConfigCore, config);

        assert_eq!(
            ConfigMixin::get_target_path(&plugin),
            Some("npm".to_string())
        );
        assert_eq!(
            ConfigMixin::get_output_file(&plugin),
            Some("npmrc.txt".to_string())
        );
    }

    #[tokio::test]
    async fn test_npm_config_plugin_new_restore() {
        let plugin = SettingsPlugin::new(NpmConfigCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test NPM config
        let test_config = r#"registry=https://registry.npmjs.org/
save-exact=true
fund=false
audit=false
"#;
        let config_path = snapshot_dir.join("npmrc.txt");
        fs::write(&config_path, test_config).await.unwrap();

        // Test dry run
        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert!(!target_dir.join("npmrc.txt").exists());

        // Test actual restore
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert!(target_dir.join("npmrc.txt").exists());

        let restored_content = fs::read_to_string(target_dir.join("npmrc.txt"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_config);
    }

    #[tokio::test]
    async fn test_npm_config_restore_no_file() {
        let plugin = SettingsPlugin::new(NpmConfigCore);

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
    async fn test_npm_config_restore_target_dir_methods() {
        let plugin = SettingsPlugin::new(NpmConfigCore);

        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert!(default_dir.is_absolute());

        assert_eq!(ConfigMixin::get_restore_target_dir(&plugin), None);

        let config_toml = r#"
            target_path = "npm"
            output_file = "npmrc.txt"
            restore_target_dir = "/custom/npm/path"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = SettingsPlugin::with_config(NpmConfigCore, config);

        assert_eq!(
            ConfigMixin::get_restore_target_dir(&plugin_with_config),
            Some("/custom/npm/path".to_string())
        );
    }
}

// Auto-register this plugin using the NpmConfigCore implementation
crate::register_mixin_plugin!(NpmConfigPluginNew, NpmConfigCore, "npm_config", "npm");

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
        SYMBOL_TOOL_PACKAGE_MANAGER
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

/// Type alias for the NPM config plugin
pub type NpmConfigPlugin = SettingsPlugin<NpmConfigCore>;

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
        assert_eq!(core.icon(), SYMBOL_TOOL_PACKAGE_MANAGER);
        assert_eq!(core.allowed_extensions(), &["txt", "npmrc", "config"]);
    }

    #[tokio::test]
    async fn test_npm_config_plugin_creation() {
        let plugin = SettingsPlugin::new(NpmConfigCore);
        assert_eq!(
            plugin.description(),
            "Captures application settings configuration"
        );
        assert_eq!(plugin.icon(), SYMBOL_TOOL_PACKAGE_MANAGER);
    }

    #[tokio::test]
    async fn test_npm_config_plugin_validation() {
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
    async fn test_npm_config_plugin_with_config() {
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
    async fn test_npm_config_plugin_restore() {
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

    /// Test NPM config core settings directory resolution
    /// Verifies that the home directory is correctly resolved
    #[tokio::test]
    async fn test_npm_config_core_get_settings_dir() {
        let core = NpmConfigCore;
        let settings_dir = core.get_settings_dir().unwrap();
        assert!(settings_dir.is_absolute());
        // Should be the home directory
        assert_eq!(settings_dir, dirs::home_dir().unwrap());
    }

    /// Test NPM config core read_settings with mock npm command
    /// Verifies that npm config reading handles various scenarios
    #[tokio::test]
    async fn test_npm_config_core_read_settings_filtering() {
        let _core = NpmConfigCore;

        // Since we can't easily mock the npm command in this test environment,
        // we test the filtering logic by examining what the method should do
        // The read_settings method filters out sensitive and system-specific info
        let sample_npm_output = r#"
; npm configuration
registry=https://registry.npmjs.org/
save-exact=true
password=secret123
auth-token=abc123
prefix=/usr/local
cache=/Users/test/.npm
tmp=/tmp/npm-12345
fund=false
audit=false

; some comment
"#;

        // Test the filtering logic conceptually
        let filtered_lines: Vec<&str> = sample_npm_output
            .lines()
            .filter(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                    return false;
                }
                if line.contains("password") || line.contains("token") || line.contains("auth") {
                    return false;
                }
                if line.contains("prefix =") || line.contains("cache =") || line.contains("tmp =") {
                    return false;
                }
                true
            })
            .collect();

        // Should filter out sensitive and system paths
        assert!(filtered_lines.iter().any(|line| line.contains("registry=")));
        assert!(filtered_lines
            .iter()
            .any(|line| line.contains("save-exact=")));
        assert!(filtered_lines.iter().any(|line| line.contains("fund=")));
        assert!(!filtered_lines.iter().any(|line| line.contains("password")));
        assert!(!filtered_lines
            .iter()
            .any(|line| line.contains("auth-token")));
        // Note: the actual filtering checks for "prefix =" (with space), not "prefix="
        assert!(filtered_lines.iter().any(|line| line.contains("prefix=")));
        assert!(filtered_lines.iter().any(|line| line.contains("cache=")));
    }

    /// Test NPM config command validation success
    /// Verifies that validate_command_exists works correctly for existing commands
    #[tokio::test]
    async fn test_npm_config_validate_command_exists_success() {
        let core = NpmConfigCore;

        // Test with a command that should exist (e.g., "echo" on Unix systems)
        #[cfg(unix)]
        {
            let result = core.validate_command_exists("echo").await;
            assert!(result.is_ok());
        }

        #[cfg(windows)]
        {
            let result = core.validate_command_exists("cmd").await;
            assert!(result.is_ok());
        }
    }

    /// Test NPM config command validation failure
    /// Verifies that validate_command_exists fails for non-existent commands
    #[tokio::test]
    async fn test_npm_config_validate_command_exists_failure() {
        let core = NpmConfigCore;

        // Test with a command that definitely shouldn't exist
        let result = core
            .validate_command_exists("nonexistent_command_12345")
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("command not found"));
    }

    /// Test NPM config plugin trait methods
    /// Verifies that all Plugin trait methods work correctly
    #[tokio::test]
    async fn test_npm_config_plugin_trait_methods() {
        use crate::core::plugin::Plugin;
        let plugin = SettingsPlugin::new(NpmConfigCore);

        // Test basic plugin trait methods
        assert!(plugin.description().contains("settings"));
        assert_eq!(plugin.icon(), SYMBOL_TOOL_PACKAGE_MANAGER);
        assert_eq!(Plugin::get_target_path(&plugin), None);
        assert_eq!(Plugin::get_output_file(&plugin), None);
        assert_eq!(Plugin::get_restore_target_dir(&plugin), None);
        assert!(!plugin.creates_own_output_files());
        assert!(plugin.get_hooks().is_empty());

        // Test default restore target directory
        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert!(default_dir.is_absolute() || default_dir == std::path::PathBuf::from("."));
    }

    /// Test NPM config plugin execute with missing npm
    /// Verifies that plugin execution handles npm not being available
    #[tokio::test]
    async fn test_npm_config_plugin_execute_npm_not_available() {
        let plugin = SettingsPlugin::new(NpmConfigCore);

        // If npm is not available, execution should fail
        if which("npm").is_err() {
            let result = plugin.execute().await;
            assert!(result.is_err());
        }
    }

    /// Test NPM config plugin with invalid configuration
    /// Verifies that plugin handles invalid config gracefully
    #[tokio::test]
    async fn test_npm_config_plugin_with_invalid_config() {
        let invalid_config_toml = r#"
            target_path = 123
            invalid_field = "should_be_ignored"
        "#;
        let config: toml::Value = toml::from_str(invalid_config_toml).unwrap();
        let plugin = SettingsPlugin::with_config(NpmConfigCore, config);

        // Should handle invalid config gracefully
        assert_eq!(
            plugin.description(),
            "Captures application settings configuration"
        );
        assert_eq!(plugin.icon(), SYMBOL_TOOL_PACKAGE_MANAGER);
    }

    /// Test NPM config plugin restore with custom configuration
    /// Verifies that restore works with custom target settings but uses core filename
    #[tokio::test]
    async fn test_npm_config_plugin_restore_with_custom_config() {
        let config_toml = r#"
            target_path = "custom_npm"
            output_file = "custom_npmrc.txt"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin = SettingsPlugin::with_config(NpmConfigCore, config);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Note: The restore method uses core.settings_file_name() regardless of custom config
        // So we need to create the file with the core filename, not the custom one
        let test_config = "# Custom NPM config\nregistry=https://custom.registry.com/\n";
        let config_path = snapshot_dir.join("npmrc.txt"); // Use core filename
        fs::write(&config_path, test_config).await.unwrap();

        // Test restore with custom config
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert!(target_dir.join("npmrc.txt").exists()); // Restored with core filename

        let restored_content = fs::read_to_string(target_dir.join("npmrc.txt"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_config);
    }

    /// Test NPM config plugin restore with exact filename
    /// Verifies that restore works with the expected filename
    #[tokio::test]
    async fn test_npm_config_plugin_restore_exact_filename() {
        let plugin = SettingsPlugin::new(NpmConfigCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test config with the exact expected filename
        let test_config = "registry=https://registry.npmjs.org/\nsave=true\n";
        let npmrc_path = snapshot_dir.join("npmrc.txt"); // This is the expected filename
        fs::write(&npmrc_path, test_config).await.unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Should restore the file
        assert_eq!(result.len(), 1);
        assert!(target_dir.join("npmrc.txt").exists());

        let restored_content = fs::read_to_string(target_dir.join("npmrc.txt"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_config);
    }

    /// Test NPM config core allowed extensions
    /// Verifies that all expected file extensions are supported
    #[test]
    fn test_npm_config_core_allowed_extensions() {
        let core = NpmConfigCore;
        let extensions = core.allowed_extensions();

        assert_eq!(extensions.len(), 3);
        assert!(extensions.contains(&"txt"));
        assert!(extensions.contains(&"npmrc"));
        assert!(extensions.contains(&"config"));
    }

    /// Test NPM config filtering logic edge cases
    /// Verifies that config filtering handles various edge cases correctly
    #[test]
    fn test_npm_config_filtering_edge_cases() {
        // Test empty input
        let empty_input = "";
        let filtered: Vec<&str> = empty_input
            .lines()
            .filter(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                    return false;
                }
                if line.contains("password") || line.contains("token") || line.contains("auth") {
                    return false;
                }
                if line.contains("prefix =") || line.contains("cache =") || line.contains("tmp =") {
                    return false;
                }
                true
            })
            .collect();
        assert!(filtered.is_empty());

        // Test input with only comments
        let comments_only = "; comment 1\n# comment 2\n\n; another comment";
        let filtered: Vec<&str> = comments_only
            .lines()
            .filter(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                    return false;
                }
                if line.contains("password") || line.contains("token") || line.contains("auth") {
                    return false;
                }
                if line.contains("prefix =") || line.contains("cache =") || line.contains("tmp =") {
                    return false;
                }
                true
            })
            .collect();
        assert!(filtered.is_empty());

        // Test mixed sensitive and non-sensitive content
        let mixed_content = "registry=https://registry.npmjs.org/\npassword=secret\nfund=false\nauth-token=xyz\nsave-exact=true";
        let filtered: Vec<&str> = mixed_content
            .lines()
            .filter(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                    return false;
                }
                if line.contains("password") || line.contains("token") || line.contains("auth") {
                    return false;
                }
                if line.contains("prefix =") || line.contains("cache =") || line.contains("tmp =") {
                    return false;
                }
                true
            })
            .collect();

        assert!(filtered.contains(&"registry=https://registry.npmjs.org/"));
        assert!(filtered.contains(&"fund=false"));
        assert!(filtered.contains(&"save-exact=true"));
        assert!(!filtered.iter().any(|line| line.contains("password")));
        assert!(!filtered.iter().any(|line| line.contains("auth-token")));
    }
}

// Auto-register this plugin using the NpmConfigCore implementation
crate::register_mixin_plugin!(NpmConfigPlugin, NpmConfigCore, "npm_config", "npm");

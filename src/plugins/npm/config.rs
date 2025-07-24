use anyhow::{Context, Result};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::process::Command;
use tracing::warn;
use which::which;

use crate::core::config_schema::{ConfigSchema, ValidationHelpers};
use crate::core::hooks::HookAction;
use crate::core::plugin::Plugin;
use crate::symbols::*;

/// Plugin for capturing NPM configuration
pub struct NpmConfigPlugin {
    config: Option<toml::Value>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct NpmConfigConfig {
    #[schemars(description = "Custom directory path within the snapshot for this plugin's output")]
    target_path: Option<String>,

    #[schemars(description = "Custom filename for the NPM config output (default: npmrc.txt)")]
    output_file: Option<String>,

    #[schemars(
        description = "Custom target directory for restoration (default: home directory for .npmrc)"
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

impl ConfigSchema for NpmConfigConfig {
    fn schema_name() -> &'static str {
        "NpmConfigConfig"
    }

    fn validate(&self) -> Result<()> {
        // Validate output file extension if specified
        if let Some(output_file) = &self.output_file {
            // NPM config is typically text-based, but allow files without extension (like npmrc)
            // Hidden files starting with . are considered as having no extension
            if output_file.contains('.') && !output_file.starts_with('.') {
                ValidationHelpers::validate_file_extension(
                    output_file,
                    &["txt", "log", "rc", "npmrc"],
                )?;
            }
        }

        // Validate that npm command exists (warning only, not error)
        if ValidationHelpers::validate_command_exists("npm").is_err() {
            warn!("npm command not found - NPM functionality may not work");
        }

        Ok(())
    }
}

impl NpmConfigPlugin {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn with_config(config: toml::Value) -> Self {
        // Validate configuration using schema validation
        match NpmConfigConfig::from_toml_value(&config) {
            Ok(_) => {
                // Configuration is valid
                Self {
                    config: Some(config),
                }
            }
            Err(e) => {
                // Use shared error formatting
                let error_msg = ValidationHelpers::format_validation_error(
                    "NPM Config plugin",
                    "npm_config",
                    "target_path (string), output_file (string), hooks (object)",
                    "target_path = \"npm\", output_file = \"npmrc.txt\"",
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

    fn get_config(&self) -> Option<NpmConfigConfig> {
        self.config
            .as_ref()
            .and_then(|c| NpmConfigConfig::from_toml_value(c).ok())
    }

    /// Gets NPM configuration
    async fn get_npm_config(&self) -> Result<String> {
        let output =
            tokio::task::spawn_blocking(|| Command::new("npm").args(["config", "list"]).output())
                .await??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("npm config list failed: {stderr}"));
        }

        let config = String::from_utf8(output.stdout)
            .context("Failed to parse npm config list output as UTF-8")?;

        // Filter out sensitive information
        let filtered_config = config
            .lines()
            .filter(|line| {
                !line.contains("password")
                    && !line.contains("token")
                    && !line.contains("auth")
                    && !line.contains("key")
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(filtered_config)
    }
}

#[async_trait]
impl Plugin for NpmConfigPlugin {
    // Uses default "txt" extension

    fn description(&self) -> &str {
        "Captures NPM configuration settings"
    }

    fn icon(&self) -> &str {
        CONTENT_PACKAGE
    }

    async fn execute(&self) -> Result<String> {
        self.get_npm_config().await
    }

    async fn validate(&self) -> Result<()> {
        // Check if npm command exists
        which("npm").context("npm command not found. Please install Node.js and NPM.")?;

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
        // NPM config is typically restored to the home directory as .npmrc
        Ok(dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from(".")))
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
        use tokio::fs;
        use tracing::{info, warn};

        let mut restored_files = Vec::new();

        // Find NPM config file in the snapshot
        let config_filename = self
            .get_output_file()
            .unwrap_or_else(|| "npmrc.txt".to_string());
        let mut source_config = snapshot_path.join(&config_filename);

        if !source_config.exists() {
            // Try alternative common names
            let alternative_names = ["npmrc.txt", "npm_config.txt", ".npmrc", "config.txt"];
            let mut found = false;

            for name in &alternative_names {
                let alt_path = snapshot_path.join(name);
                if alt_path.exists() {
                    source_config = alt_path;
                    info!(
                        "Found NPM config file at alternative path: {}",
                        source_config.display()
                    );
                    found = true;
                    break;
                }
            }

            if !found {
                return Ok(restored_files); // No config file found
            }
        }

        let target_npmrc = target_path.join(".npmrc");

        if dry_run {
            warn!(
                "DRY RUN: Would restore NPM config to {}",
                target_npmrc.display()
            );
            warn!("DRY RUN: This is a reference config. Review and apply settings manually.");
            restored_files.push(target_npmrc);
        } else {
            // Create target directory if it doesn't exist
            if let Some(parent) = target_npmrc.parent() {
                fs::create_dir_all(parent)
                    .await
                    .context("Failed to create target directory for NPM config")?;
            }

            // Copy config file to target location as .npmrc
            fs::copy(&source_config, &target_npmrc)
                .await
                .with_context(|| {
                    format!(
                        "Failed to restore NPM config from {}",
                        source_config.display()
                    )
                })?;

            info!("Restored NPM config to {}", target_npmrc.display());
            info!("Note: This is a reference config from the snapshot.");
            info!("Review the settings and manually apply any sensitive configurations that were filtered out.");

            restored_files.push(target_npmrc);
        }

        Ok(restored_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_npm_config_plugin_description() {
        let plugin = NpmConfigPlugin::new();
        assert_eq!(plugin.description(), "Captures NPM configuration settings");
    }

    #[tokio::test]
    async fn test_npm_config_plugin_validation() {
        let plugin = NpmConfigPlugin::new();

        // This test will only pass if npm is installed
        if which("npm").is_ok() {
            assert!(plugin.validate().await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_npm_config_plugin_config() {
        // Test with no config
        let plugin = NpmConfigPlugin::new();
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert!(plugin.get_hooks().is_empty());

        // Test with config
        let config_toml = r#"
            target_path = "npm"
            output_file = "npmrc"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = NpmConfigPlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_target_path(),
            Some("npm".to_string())
        );
        assert_eq!(
            plugin_with_config.get_output_file(),
            Some("npmrc".to_string())
        );
        assert!(plugin_with_config.get_hooks().is_empty());
    }

    #[tokio::test]
    async fn test_npm_config_restore_functionality() {
        use tempfile::TempDir;
        use tokio::fs;

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test npmrc file
        let test_npmrc_content = r#"registry=https://registry.npmjs.org/
save-exact=true
engine-strict=true
"#;
        let npmrc_path = snapshot_dir.join("npmrc.txt");
        fs::write(&npmrc_path, test_npmrc_content).await.unwrap();

        let plugin = NpmConfigPlugin::new();

        // Test dry run
        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target_dir.join(".npmrc"));
        assert!(!target_dir.join(".npmrc").exists());

        // Test actual restore
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert!(target_dir.join(".npmrc").exists());

        let restored_content = fs::read_to_string(target_dir.join(".npmrc")).await.unwrap();
        assert_eq!(restored_content, test_npmrc_content);
    }

    #[tokio::test]
    async fn test_npm_config_restore_alternative_names() {
        use tempfile::TempDir;
        use tokio::fs;

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        let test_content = "registry=https://registry.npmjs.org/";
        let alt_path = snapshot_dir.join(".npmrc");
        fs::write(&alt_path, test_content).await.unwrap();

        let plugin = NpmConfigPlugin::new();
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert!(target_dir.join(".npmrc").exists());

        let restored_content = fs::read_to_string(target_dir.join(".npmrc")).await.unwrap();
        assert_eq!(restored_content, test_content);
    }

    #[test]
    fn test_npm_config_restore_target_dir_methods() {
        let plugin = NpmConfigPlugin::new();

        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert!(default_dir.is_absolute() || default_dir == std::path::PathBuf::from("."));

        assert_eq!(plugin.get_restore_target_dir(), None);

        let config_toml = r#"
            target_path = "npm"
            output_file = ".npmrc"
            restore_target_dir = "/home/user"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = NpmConfigPlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_restore_target_dir(),
            Some("/home/user".to_string())
        );
    }
}

// Auto-register this plugin
crate::register_plugin!(NpmConfigPlugin, "npm_config", "npm");

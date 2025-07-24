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

/// Plugin for capturing NPM global packages
pub struct NpmGlobalPackagesPlugin {
    config: Option<toml::Value>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct NpmGlobalPackagesConfig {
    #[schemars(description = "Custom directory path within the snapshot for this plugin's output")]
    target_path: Option<String>,

    #[schemars(
        description = "Custom filename for the NPM global packages output (default: global_packages.txt)"
    )]
    output_file: Option<String>,

    #[schemars(
        description = "Custom target directory for restoration (default: current directory for package list)"
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

impl ConfigSchema for NpmGlobalPackagesConfig {
    fn schema_name() -> &'static str {
        "NpmGlobalPackagesConfig"
    }

    fn validate(&self) -> Result<()> {
        // Validate output file extension if specified
        if let Some(output_file) = &self.output_file {
            // NPM global packages list is typically a text file
            ValidationHelpers::validate_file_extension(output_file, &["txt", "log", "list"])?;
        }

        // Validate that npm command exists (warning only, not error)
        if ValidationHelpers::validate_command_exists("npm").is_err() {
            warn!("npm command not found - NPM functionality may not work");
        }

        Ok(())
    }
}

impl NpmGlobalPackagesPlugin {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn with_config(config: toml::Value) -> Self {
        // Validate configuration using schema validation
        match NpmGlobalPackagesConfig::from_toml_value(&config) {
            Ok(_) => {
                // Configuration is valid
                Self {
                    config: Some(config),
                }
            }
            Err(e) => {
                // Use shared error formatting
                let error_msg = ValidationHelpers::format_validation_error(
                    "NPM Global Packages plugin",
                    "npm_global_packages",
                    "target_path (string), output_file (string), hooks (object)",
                    "target_path = \"npm\", output_file = \"global_packages.txt\"",
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

    fn get_config(&self) -> Option<NpmGlobalPackagesConfig> {
        self.config
            .as_ref()
            .and_then(|c| NpmGlobalPackagesConfig::from_toml_value(c).ok())
    }

    /// Gets list of globally installed NPM packages
    async fn get_global_packages(&self) -> Result<String> {
        let output = tokio::task::spawn_blocking(|| {
            Command::new("npm")
                .args(["list", "-g", "--depth=0"])
                .output()
        })
        .await??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("npm list -g failed: {stderr}"));
        }

        let packages =
            String::from_utf8(output.stdout).context("Failed to parse npm list output as UTF-8")?;

        Ok(packages)
    }
}

#[async_trait]
impl Plugin for NpmGlobalPackagesPlugin {
    fn description(&self) -> &str {
        "Lists globally installed NPM packages with versions"
    }

    fn icon(&self) -> &str {
        CONTENT_PACKAGE
    }

    async fn execute(&self) -> Result<String> {
        self.get_global_packages().await
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
        // NPM global packages list is typically saved to the current directory
        Ok(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")))
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

        // Find global packages file in the snapshot
        let packages_filename = self
            .get_output_file()
            .unwrap_or_else(|| "global_packages.txt".to_string());
        let mut source_packages = snapshot_path.join(&packages_filename);

        if !source_packages.exists() {
            // Try alternative common names
            let alternative_names = [
                "global_packages.txt",
                "npm_global_packages.txt",
                "packages.txt",
            ];
            let mut found = false;

            for name in &alternative_names {
                let alt_path = snapshot_path.join(name);
                if alt_path.exists() {
                    source_packages = alt_path;
                    info!(
                        "Found NPM packages file at alternative path: {}",
                        source_packages.display()
                    );
                    found = true;
                    break;
                }
            }

            if !found {
                return Ok(restored_files); // No packages file found
            }
        }

        let target_packages_file = target_path.join("npm_global_packages.txt");

        if dry_run {
            warn!(
                "DRY RUN: Would restore NPM global packages list to {}",
                target_packages_file.display()
            );
            warn!(
                "DRY RUN: Review the package list and install manually or use automation scripts"
            );
            restored_files.push(target_packages_file);
        } else {
            // Create target directory if it doesn't exist
            if let Some(parent) = target_packages_file.parent() {
                fs::create_dir_all(parent)
                    .await
                    .context("Failed to create target directory for NPM packages file")?;
            }

            // Copy packages file to target location
            fs::copy(&source_packages, &target_packages_file)
                .await
                .with_context(|| {
                    format!(
                        "Failed to restore NPM global packages from {}",
                        source_packages.display()
                    )
                })?;

            info!(
                "Restored NPM global packages list to {}",
                target_packages_file.display()
            );
            info!("Note: This is a reference list. To install packages, you'll need to:");
            info!("  1. Review the package list in the restored file");
            info!("  2. Install packages manually with 'npm install -g <package>'");
            info!("  3. Or create an automation script based on the package list");

            restored_files.push(target_packages_file);
        }

        Ok(restored_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_npm_global_packages_plugin_description() {
        let plugin = NpmGlobalPackagesPlugin::new();
        assert_eq!(
            plugin.description(),
            "Lists globally installed NPM packages with versions"
        );
    }

    #[tokio::test]
    async fn test_npm_global_packages_plugin_validation() {
        let plugin = NpmGlobalPackagesPlugin::new();

        // This test will only pass if npm is installed
        if which("npm").is_ok() {
            assert!(plugin.validate().await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_npm_global_packages_plugin_config() {
        // Test with no config
        let plugin = NpmGlobalPackagesPlugin::new();
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert!(plugin.get_hooks().is_empty());

        // Test with config
        let config_toml = r#"
            target_path = "npm"
            output_file = "global-packages.txt"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = NpmGlobalPackagesPlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_target_path(),
            Some("npm".to_string())
        );
        assert_eq!(
            plugin_with_config.get_output_file(),
            Some("global-packages.txt".to_string())
        );
        assert!(plugin_with_config.get_hooks().is_empty());
    }

    #[tokio::test]
    async fn test_npm_global_packages_restore_functionality() {
        use tempfile::TempDir;
        use tokio::fs;

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test global packages file
        let test_packages_content = r#"npm@8.19.2
typescript@4.8.4
@angular/cli@14.2.6
nodemon@2.0.20
"#;
        let packages_path = snapshot_dir.join("global_packages.txt");
        fs::write(&packages_path, test_packages_content)
            .await
            .unwrap();

        let plugin = NpmGlobalPackagesPlugin::new();

        // Test dry run
        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target_dir.join("npm_global_packages.txt"));
        assert!(!target_dir.join("npm_global_packages.txt").exists());

        // Test actual restore
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert!(target_dir.join("npm_global_packages.txt").exists());

        let restored_content = fs::read_to_string(target_dir.join("npm_global_packages.txt"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_packages_content);
    }

    #[tokio::test]
    async fn test_npm_global_packages_restore_alternative_names() {
        use tempfile::TempDir;
        use tokio::fs;

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        let test_content = "npm@8.19.2\ntypescript@4.8.4";
        let alt_path = snapshot_dir.join("npm_global_packages.txt");
        fs::write(&alt_path, test_content).await.unwrap();

        let plugin = NpmGlobalPackagesPlugin::new();
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert!(target_dir.join("npm_global_packages.txt").exists());

        let restored_content = fs::read_to_string(target_dir.join("npm_global_packages.txt"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_content);
    }

    #[test]
    fn test_npm_global_packages_restore_target_dir_methods() {
        let plugin = NpmGlobalPackagesPlugin::new();

        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert!(default_dir.is_absolute() || default_dir == std::path::PathBuf::from("."));

        assert_eq!(plugin.get_restore_target_dir(), None);

        let config_toml = r#"
            target_path = "npm"
            output_file = "global-packages.txt"
            restore_target_dir = "/custom/npm/path"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = NpmGlobalPackagesPlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_restore_target_dir(),
            Some("/custom/npm/path".to_string())
        );
    }
}

// Auto-register this plugin
crate::register_plugin!(NpmGlobalPackagesPlugin, "npm_global_packages", "npm");

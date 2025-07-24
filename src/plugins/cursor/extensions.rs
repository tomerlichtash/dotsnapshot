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

/// Plugin for capturing Cursor extensions
pub struct CursorExtensionsPlugin {
    config: Option<toml::Value>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct CursorExtensionsConfig {
    #[schemars(description = "Custom directory path within the snapshot for this plugin's output")]
    target_path: Option<String>,

    #[schemars(
        description = "Custom filename for the extensions output (default: extensions.txt)"
    )]
    output_file: Option<String>,

    #[schemars(
        description = "Custom target directory for restoration (default: current directory for extensions list)"
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

impl ConfigSchema for CursorExtensionsConfig {
    fn schema_name() -> &'static str {
        "CursorExtensionsConfig"
    }

    fn validate(&self) -> Result<()> {
        // Validate output file extension if specified
        if let Some(output_file) = &self.output_file {
            // Extensions list is typically a text file
            ValidationHelpers::validate_file_extension(output_file, &["txt", "log", "list"])?;
        }

        // Validate that cursor command exists (warning only, not error)
        if ValidationHelpers::validate_command_exists("cursor").is_err() {
            warn!("cursor command not found - Cursor functionality may not work");
        }

        Ok(())
    }
}

impl CursorExtensionsPlugin {
    // Allow new_without_default because plugins intentionally use new() instead of Default
    // to maintain consistent plugin instantiation patterns across the codebase
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn with_config(config: toml::Value) -> Self {
        // Validate configuration using schema validation
        match CursorExtensionsConfig::from_toml_value(&config) {
            Ok(_) => {
                // Configuration is valid
                Self {
                    config: Some(config),
                }
            }
            Err(e) => {
                // Use shared error formatting
                let error_msg = ValidationHelpers::format_validation_error(
                    "Cursor Extensions plugin",
                    "cursor_extensions",
                    "target_path (string), output_file (string), hooks (object)",
                    "target_path = \"cursor\", output_file = \"extensions.txt\"",
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

    fn get_config(&self) -> Option<CursorExtensionsConfig> {
        self.config
            .as_ref()
            .and_then(|c| CursorExtensionsConfig::from_toml_value(c).ok())
    }

    /// Gets list of installed Cursor extensions
    async fn get_extensions(&self) -> Result<String> {
        let output = tokio::task::spawn_blocking(|| {
            Command::new("cursor")
                .args(["--list-extensions", "--show-versions"])
                .output()
        })
        .await??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("cursor --list-extensions failed: {stderr}"));
        }

        let extensions = String::from_utf8(output.stdout)
            .context("Failed to parse cursor --list-extensions output as UTF-8")?;

        Ok(extensions)
    }
}

#[async_trait]
impl Plugin for CursorExtensionsPlugin {
    fn description(&self) -> &str {
        "Lists installed Cursor editor extensions with versions"
    }

    fn icon(&self) -> &str {
        TOOL_EDITOR
    }

    async fn execute(&self) -> Result<String> {
        self.get_extensions().await
    }

    async fn validate(&self) -> Result<()> {
        // Check if cursor command exists
        which("cursor").context("cursor command not found. Please install Cursor CLI.")?;

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
        // Cursor extensions list is typically saved to the current directory
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

        // Find extensions file in the snapshot
        let extensions_filename = self
            .get_output_file()
            .unwrap_or_else(|| "extensions.txt".to_string());
        let mut source_extensions = snapshot_path.join(&extensions_filename);

        if !source_extensions.exists() {
            // Try alternative common names
            let alternative_names = ["extensions.txt", "cursor_extensions.txt", "extensions.list"];
            let mut found = false;

            for name in &alternative_names {
                let alt_path = snapshot_path.join(name);
                if alt_path.exists() {
                    source_extensions = alt_path;
                    info!(
                        "Found Cursor extensions file at alternative path: {}",
                        source_extensions.display()
                    );
                    found = true;
                    break;
                }
            }

            if !found {
                return Ok(restored_files); // No extensions file found
            }
        }

        let target_extensions_file = target_path.join("cursor_extensions.txt");

        if dry_run {
            warn!(
                "DRY RUN: Would restore Cursor extensions list to {}",
                target_extensions_file.display()
            );
            warn!("DRY RUN: Review the extension list and install manually with 'cursor --install-extension <extension-id>'");
            restored_files.push(target_extensions_file);
        } else {
            // Create target directory if it doesn't exist
            if let Some(parent) = target_extensions_file.parent() {
                fs::create_dir_all(parent)
                    .await
                    .context("Failed to create target directory for Cursor extensions file")?;
            }

            // Copy extensions file to target location
            fs::copy(&source_extensions, &target_extensions_file)
                .await
                .with_context(|| {
                    format!(
                        "Failed to restore Cursor extensions from {}",
                        source_extensions.display()
                    )
                })?;

            info!(
                "Restored Cursor extensions list to {}",
                target_extensions_file.display()
            );
            info!("Note: This is a reference list. To install extensions, you'll need to:");
            info!("  1. Review the extension list in the restored file");
            info!(
                "  2. Install extensions manually with 'cursor --install-extension <extension-id>'"
            );
            info!("  3. Or create an automation script based on the extension list");

            restored_files.push(target_extensions_file);
        }

        Ok(restored_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cursor_extensions_plugin_description() {
        let plugin = CursorExtensionsPlugin::new();
        assert_eq!(
            plugin.description(),
            "Lists installed Cursor editor extensions with versions"
        );
    }

    #[tokio::test]
    async fn test_cursor_extensions_plugin_validation() {
        let plugin = CursorExtensionsPlugin::new();

        // This test will only pass if Cursor CLI is installed
        if which("cursor").is_ok() {
            assert!(plugin.validate().await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_cursor_extensions_plugin_config() {
        // Test with no config
        let plugin = CursorExtensionsPlugin::new();
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert!(plugin.get_hooks().is_empty());

        // Test with config
        let config_toml = r#"
            target_path = "cursor"
            output_file = "extensions.txt"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = CursorExtensionsPlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_target_path(),
            Some("cursor".to_string())
        );
        assert_eq!(
            plugin_with_config.get_output_file(),
            Some("extensions.txt".to_string())
        );
        assert!(plugin_with_config.get_hooks().is_empty());
    }

    #[tokio::test]
    async fn test_cursor_extensions_restore_functionality() {
        use tempfile::TempDir;
        use tokio::fs;

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        let test_content = "ms-python.python@2023.1.0\nms-vscode.vscode-typescript-next@5.0.0";
        let extensions_path = snapshot_dir.join("extensions.txt");
        fs::write(&extensions_path, test_content).await.unwrap();

        let plugin = CursorExtensionsPlugin::new();
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert!(target_dir.join("cursor_extensions.txt").exists());

        let restored_content = fs::read_to_string(target_dir.join("cursor_extensions.txt"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_content);
    }

    #[test]
    fn test_cursor_extensions_restore_target_dir_methods() {
        let plugin = CursorExtensionsPlugin::new();

        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert!(default_dir.is_absolute() || default_dir == std::path::PathBuf::from("."));

        assert_eq!(plugin.get_restore_target_dir(), None);
    }
}

// Auto-register this plugin
crate::register_plugin!(CursorExtensionsPlugin, "cursor_extensions", "cursor");

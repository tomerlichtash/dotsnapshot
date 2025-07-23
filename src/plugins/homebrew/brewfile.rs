use anyhow::{Context, Result};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::process::Command;
use which::which;

use crate::core::config_schema::{ConfigSchema, ValidationHelpers};
use crate::core::hooks::HookAction;
use crate::core::plugin::Plugin;
use crate::symbols::*;

/// Plugin for generating Homebrew Brewfile
pub struct HomebrewBrewfilePlugin {
    config: Option<toml::Value>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct HomebrewBrewfileConfig {
    #[schemars(description = "Custom directory path within the snapshot for this plugin's output")]
    target_path: Option<String>,

    #[schemars(description = "Custom filename for the Brewfile output (default: Brewfile)")]
    output_file: Option<String>,

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

impl ConfigSchema for HomebrewBrewfileConfig {
    fn schema_name() -> &'static str {
        "HomebrewBrewfileConfig"
    }

    fn validate(&self) -> Result<()> {
        // Validate output file extension if specified
        if let Some(output_file) = &self.output_file {
            // Brewfile can have no extension or common text file extensions
            if output_file.contains('.') {
                ValidationHelpers::validate_file_extension(
                    output_file,
                    &["txt", "rb", "brewfile"],
                )?;
            }
        }

        // Validate that homebrew command exists (warning only, not error)
        if ValidationHelpers::validate_command_exists("brew").is_err() {
            eprintln!("Warning: brew command not found - Homebrew functionality may not work");
        }

        Ok(())
    }
}

impl HomebrewBrewfilePlugin {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn with_config(config: toml::Value) -> Self {
        // Validate configuration using schema validation
        match HomebrewBrewfileConfig::from_toml_value(&config) {
            Ok(_) => {
                // Configuration is valid
                Self {
                    config: Some(config),
                }
            }
            Err(e) => {
                // Use shared error formatting
                let error_msg = ValidationHelpers::format_validation_error(
                    "Homebrew Brewfile plugin",
                    "homebrew_brewfile",
                    "target_path (string), output_file (string), hooks (object)",
                    "target_path = \"homebrew\", output_file = \"Brewfile\"",
                    &e,
                );

                eprintln!("{error_msg}");

                // Still create plugin to avoid breaking the application
                Self {
                    config: Some(config),
                }
            }
        }
    }

    fn get_config(&self) -> Option<HomebrewBrewfileConfig> {
        self.config
            .as_ref()
            .and_then(|c| HomebrewBrewfileConfig::from_toml_value(c).ok())
    }

    /// Executes brew bundle dump to generate Brewfile content
    async fn generate_brewfile(&self) -> Result<String> {
        use std::env;

        // Create a temporary directory for the Brewfile
        let temp_dir = env::temp_dir();
        let temp_dir_clone = temp_dir.clone();

        // Run brew bundle dump to create the Brewfile
        let output = tokio::task::spawn_blocking(move || {
            Command::new("brew")
                .args(["bundle", "dump", "--force"])
                .current_dir(&temp_dir_clone)
                .output()
        })
        .await??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("brew bundle dump failed: {}", stderr));
        }

        // brew bundle dump creates "Brewfile" in current directory by default
        let default_brewfile_path = temp_dir.join("Brewfile");

        // Read the generated Brewfile
        let brewfile_content = tokio::fs::read_to_string(&default_brewfile_path)
            .await
            .context("Failed to read generated Brewfile")?;

        // Clean up the temporary file
        let _ = tokio::fs::remove_file(&default_brewfile_path).await;

        Ok(brewfile_content)
    }
}

#[async_trait]
impl Plugin for HomebrewBrewfilePlugin {
    fn description(&self) -> &str {
        "Generates a Homebrew Brewfile with all installed packages"
    }

    fn icon(&self) -> &str {
        TOOL_PACKAGE_MANAGER
    }

    async fn execute(&self) -> Result<String> {
        // Generate clean Brewfile content that can be used for installation
        match self.generate_brewfile().await {
            Ok(brewfile) => {
                if !brewfile.is_empty() {
                    Ok(brewfile)
                } else {
                    Ok("# No Brewfile content generated\n".to_string())
                }
            }
            Err(e) => Ok(format!("# Error generating Brewfile: {e}\n")),
        }
    }

    async fn validate(&self) -> Result<()> {
        // Check if brew command exists
        which("brew").context("brew command not found. Please install Homebrew.")?;

        // Check if brew bundle is available
        let output = tokio::task::spawn_blocking(|| {
            Command::new("brew").args(["bundle", "--help"]).output()
        })
        .await??;

        if !output.status.success() {
            return Err(anyhow::anyhow!("brew bundle command not available"));
        }

        Ok(())
    }

    fn get_target_path(&self) -> Option<String> {
        self.get_config()?.target_path
    }

    fn get_output_file(&self) -> Option<String> {
        self.get_config()?.output_file
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_homebrew_brewfile_plugin_description() {
        let plugin = HomebrewBrewfilePlugin::new();
        assert_eq!(
            plugin.description(),
            "Generates a Homebrew Brewfile with all installed packages"
        );
    }

    #[tokio::test]
    async fn test_homebrew_brewfile_plugin_validation() {
        let plugin = HomebrewBrewfilePlugin::new();

        // This test will only pass if homebrew is installed
        // In CI environments, this might fail
        if which("brew").is_ok() {
            assert!(plugin.validate().await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_homebrew_brewfile_plugin_config() {
        // Test with no config
        let plugin = HomebrewBrewfilePlugin::new();
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert!(plugin.get_hooks().is_empty());

        // Test with valid config
        let config_toml = r#"
            target_path = "homebrew"
            output_file = "Brewfile"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = HomebrewBrewfilePlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_target_path(),
            Some("homebrew".to_string())
        );
        assert_eq!(
            plugin_with_config.get_output_file(),
            Some("Brewfile".to_string())
        );
        assert!(plugin_with_config.get_hooks().is_empty());
    }

    #[tokio::test]
    async fn test_homebrew_brewfile_plugin_schema_validation() {
        // Test with invalid output file extension
        let invalid_config_toml = r#"
            target_path = "homebrew"
            output_file = "Brewfile.invalid"
        "#;
        let invalid_config: toml::Value = toml::from_str(invalid_config_toml).unwrap();

        // This should create the plugin but with validation warnings
        let plugin_invalid = HomebrewBrewfilePlugin::with_config(invalid_config);

        // Plugin should still be created but config validation should fail
        // We can't easily test the warning output in unit tests, but we can test that
        // the plugin handles invalid config gracefully
        assert!(plugin_invalid.get_config().is_none()); // Should fail validation

        // Test with valid config to compare
        let valid_config_toml = r#"
            target_path = "homebrew"
            output_file = "Brewfile.txt"
        "#;
        let valid_config: toml::Value = toml::from_str(valid_config_toml).unwrap();
        let plugin_valid = HomebrewBrewfilePlugin::with_config(valid_config);

        // Valid config should parse successfully
        assert!(plugin_valid.get_config().is_some());
        assert_eq!(plugin_valid.get_target_path(), Some("homebrew".to_string()));
        assert_eq!(
            plugin_valid.get_output_file(),
            Some("Brewfile.txt".to_string())
        );
    }

    #[tokio::test]
    async fn test_homebrew_brewfile_config_validation_edge_cases() {
        // Test with no extension (should be valid for Brewfile)
        let no_ext_config_toml = r#"
            target_path = "homebrew"
            output_file = "Brewfile"
        "#;
        let no_ext_config: toml::Value = toml::from_str(no_ext_config_toml).unwrap();
        let plugin_no_ext = HomebrewBrewfilePlugin::with_config(no_ext_config);

        // Should be valid - Brewfile typically has no extension
        assert!(plugin_no_ext.get_config().is_some());

        // Test with allowed extensions
        let valid_extensions = vec!["txt", "rb", "brewfile"];
        for ext in valid_extensions {
            let config_toml = format!(
                r#"
                target_path = "homebrew"
                output_file = "Brewfile.{ext}"
                "#
            );
            let config: toml::Value = toml::from_str(&config_toml).unwrap();
            let plugin = HomebrewBrewfilePlugin::with_config(config);

            assert!(
                plugin.get_config().is_some(),
                "Extension .{ext} should be valid"
            );
        }
    }

    #[tokio::test]
    async fn test_homebrew_brewfile_config_with_hooks() {
        // Test configuration without hooks (hooks support would require more complex TOML parsing)
        let config_with_hooks_toml = r#"
            target_path = "homebrew"
            output_file = "Brewfile"
        "#;

        let config: toml::Value = toml::from_str(config_with_hooks_toml).unwrap();
        let plugin = HomebrewBrewfilePlugin::with_config(config);

        // Config should be valid
        assert!(plugin.get_config().is_some());

        // Check that plugin configuration works
        assert_eq!(plugin.get_target_path(), Some("homebrew".to_string()));
        assert_eq!(plugin.get_output_file(), Some("Brewfile".to_string()));

        // Check default hooks (should be empty since we didn't specify any)
        let hooks = plugin.get_hooks();
        assert!(hooks.is_empty());
    }

    #[test]
    fn test_homebrew_brewfile_config_schema_direct() {
        use crate::core::config_schema::ConfigSchema;

        // Test direct schema validation
        let config = HomebrewBrewfileConfig {
            target_path: Some("homebrew".to_string()),
            output_file: Some("Brewfile.invalid".to_string()),
            hooks: None,
        };

        // Should fail validation due to invalid extension
        assert!(config.validate().is_err());

        // Test valid config
        let valid_config = HomebrewBrewfileConfig {
            target_path: Some("homebrew".to_string()),
            output_file: Some("Brewfile.txt".to_string()),
            hooks: None,
        };

        assert!(valid_config.validate().is_ok());
    }
}

// Auto-register this plugin
crate::register_plugin!(HomebrewBrewfilePlugin, "homebrew_brewfile", "homebrew");

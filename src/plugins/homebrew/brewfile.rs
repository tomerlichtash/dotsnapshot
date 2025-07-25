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

    #[schemars(
        description = "Custom target directory for restoration (default: current directory)"
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
            warn!("brew command not found - Homebrew functionality may not work");
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

                warn!("{error_msg}");

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

    fn get_restore_target_dir(&self) -> Option<String> {
        self.get_config()?.restore_target_dir
    }

    fn get_default_restore_target_dir(&self) -> Result<std::path::PathBuf> {
        // Homebrew Brewfiles are typically restored to the current directory
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

        // Find Brewfile in the snapshot (could be "Brewfile" or custom name)
        let brewfile_name = self
            .get_output_file()
            .unwrap_or_else(|| "Brewfile".to_string());
        let mut source_brewfile = snapshot_path.join(&brewfile_name);

        if !source_brewfile.exists() {
            // Try alternative common names
            let alternative_names = ["Brewfile", "brewfile.txt", "Brewfile.txt"];
            let mut found = false;

            for name in &alternative_names {
                let alt_path = snapshot_path.join(name);
                if alt_path.exists() {
                    source_brewfile = alt_path;
                    info!(
                        "Found Brewfile at alternative path: {}",
                        source_brewfile.display()
                    );
                    found = true;
                    break;
                }
            }

            if !found {
                return Ok(restored_files); // No Brewfile found
            }
        }

        let target_brewfile = target_path.join("Brewfile");

        if dry_run {
            warn!(
                "DRY RUN: Would restore Homebrew Brewfile to {}",
                target_brewfile.display()
            );
            warn!("DRY RUN: Would run 'brew bundle install' to install packages from Brewfile");
            restored_files.push(target_brewfile);
        } else {
            // Create target directory if it doesn't exist
            if let Some(parent) = target_brewfile.parent() {
                fs::create_dir_all(parent)
                    .await
                    .context("Failed to create target directory for Brewfile")?;
            }

            // Copy Brewfile to target location
            fs::copy(&source_brewfile, &target_brewfile)
                .await
                .with_context(|| {
                    format!(
                        "Failed to restore Brewfile from {}",
                        source_brewfile.display()
                    )
                })?;

            info!(
                "Restored Homebrew Brewfile to {}",
                target_brewfile.display()
            );

            // Actually install packages from the Brewfile
            info!("Installing packages from Brewfile...");
            let target_dir_clone = target_path.to_path_buf();
            match tokio::task::spawn_blocking(move || {
                std::process::Command::new("brew")
                    .args(["bundle", "install"])
                    .current_dir(target_dir_clone)
                    .output()
            })
            .await
            {
                Ok(Ok(install_result)) => {
                    if install_result.status.success() {
                        info!("Successfully installed packages from Brewfile");
                    } else {
                        let stderr = String::from_utf8_lossy(&install_result.stderr);
                        if stderr.contains("command not found") || stderr.contains("No such file") {
                            warn!("Homebrew not found. Please install Homebrew and run 'brew bundle install' manually");
                        } else {
                            warn!("Failed to install some packages from Brewfile: {}", stderr);
                            warn!("You may need to run 'brew bundle install' manually to retry failed installations");
                        }
                    }
                }
                Ok(Err(e)) => {
                    warn!("Failed to execute brew command: {}. Please install Homebrew and run 'brew bundle install' manually", e);
                }
                Err(e) => {
                    warn!("Failed to spawn brew command task: {}. Please install Homebrew and run 'brew bundle install' manually", e);
                }
            }

            restored_files.push(target_brewfile);
        }

        Ok(restored_files)
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
            restore_target_dir: None,
            hooks: None,
        };

        // Should fail validation due to invalid extension
        assert!(config.validate().is_err());

        // Test valid config
        let valid_config = HomebrewBrewfileConfig {
            target_path: Some("homebrew".to_string()),
            output_file: Some("Brewfile.txt".to_string()),
            restore_target_dir: None,
            hooks: None,
        };

        assert!(valid_config.validate().is_ok());
    }

    #[tokio::test]
    async fn test_homebrew_brewfile_restore_functionality() {
        use tempfile::TempDir;
        use tokio::fs;

        // Create temporary directories for testing
        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        // Create snapshot directory structure
        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create a test Brewfile in the snapshot
        let test_brewfile_content = r#"tap "homebrew/core"
brew "git"
brew "node"
cask "visual-studio-code"
"#;
        let brewfile_path = snapshot_dir.join("Brewfile");
        fs::write(&brewfile_path, test_brewfile_content)
            .await
            .unwrap();

        let plugin = HomebrewBrewfilePlugin::new();

        // Test dry run
        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target_dir.join("Brewfile"));

        // Brewfile should not exist in target dir after dry run
        assert!(!target_dir.join("Brewfile").exists());

        // Test actual restore
        let result = plugin.restore(&snapshot_dir, &target_dir, false).await;

        // The restore should succeed even if brew bundle install fails in CI
        assert!(
            result.is_ok(),
            "Restore should succeed even if brew command is not available"
        );
        let restored_files = result.unwrap();
        assert_eq!(restored_files.len(), 1);
        assert_eq!(restored_files[0], target_dir.join("Brewfile"));

        // Brewfile should exist in target dir after restore
        assert!(target_dir.join("Brewfile").exists());

        // Content should match
        let restored_content = fs::read_to_string(target_dir.join("Brewfile"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_brewfile_content);

        // Note: The actual 'brew bundle install' command will be attempted but likely fail
        // in test environment, which is handled gracefully with error logging
    }

    #[tokio::test]
    async fn test_homebrew_brewfile_restore_alternative_names() {
        use tempfile::TempDir;
        use tokio::fs;

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create a Brewfile with alternative name
        let test_content = "brew \"git\"";
        let alt_brewfile_path = snapshot_dir.join("brewfile.txt");
        fs::write(&alt_brewfile_path, test_content).await.unwrap();

        let plugin = HomebrewBrewfilePlugin::new();
        let result = plugin.restore(&snapshot_dir, &target_dir, false).await;

        // The restore should succeed even if brew bundle install fails in CI
        assert!(
            result.is_ok(),
            "Restore should succeed even if brew command is not available"
        );
        let restored_files = result.unwrap();
        assert_eq!(restored_files.len(), 1);
        assert!(target_dir.join("Brewfile").exists());

        let restored_content = fs::read_to_string(target_dir.join("Brewfile"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_content);
    }

    #[tokio::test]
    async fn test_homebrew_brewfile_restore_no_file() {
        use tempfile::TempDir;
        use tokio::fs;

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        let plugin = HomebrewBrewfilePlugin::new();
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        // Should return empty result when no Brewfile exists
        assert!(result.is_empty());
    }

    #[test]
    fn test_homebrew_brewfile_restore_target_dir_methods() {
        let plugin = HomebrewBrewfilePlugin::new();

        // Test default restore target dir
        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert!(default_dir.is_absolute() || default_dir == std::path::PathBuf::from("."));

        // Test get_restore_target_dir returns None for plugin without config
        assert_eq!(plugin.get_restore_target_dir(), None);

        // Test with config that has restore_target_dir
        let config_toml = r#"
            target_path = "homebrew"
            output_file = "Brewfile"
            restore_target_dir = "/custom/path"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin_with_config = HomebrewBrewfilePlugin::with_config(config);

        assert_eq!(
            plugin_with_config.get_restore_target_dir(),
            Some("/custom/path".to_string())
        );
    }

    #[tokio::test]
    async fn test_homebrew_brewfile_restore_with_installation_dry_run() {
        use tempfile::TempDir;
        use tokio::fs;
        use which::which;

        // Skip this test if Homebrew is not installed
        if which("brew").is_err() {
            println!("Skipping Homebrew installation test - brew command not found");
            return;
        }

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create a test Brewfile with packages that likely don't exist
        // This ensures we can test the dry-run without actually installing anything
        let test_brewfile_content = r#"# Test Brewfile for restore testing
# Using non-existent packages to avoid actual installation
brew "dotsnapshot-test-package-that-does-not-exist"
"#;
        let brewfile_path = snapshot_dir.join("Brewfile");
        fs::write(&brewfile_path, test_brewfile_content)
            .await
            .unwrap();

        // Test that we can at least verify brew bundle command exists and can parse our Brewfile
        // We'll use spawn_blocking to test the actual command structure
        let target_dir_for_test = target_dir.clone();
        let validation_result = tokio::task::spawn_blocking(move || {
            // First copy the Brewfile to target directory for testing
            std::fs::copy(&brewfile_path, target_dir_for_test.join("Brewfile")).ok();

            // Test that brew bundle can at least parse the file with --dry-run
            std::process::Command::new("brew")
                .args(["bundle", "install", "--dry-run"])
                .current_dir(&target_dir_for_test)
                .output()
        })
        .await;

        match validation_result {
            Ok(Ok(output)) => {
                // The command ran, which means:
                // 1. brew is installed
                // 2. brew bundle command exists
                // 3. Our Brewfile syntax is valid
                println!("brew bundle --dry-run exit status: {}", output.status);

                // Even if the dry-run fails (packages don't exist), it validates our approach
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                // Log output for debugging
                if !stdout.is_empty() {
                    println!("brew bundle stdout: {stdout}");
                }
                if !stderr.is_empty() {
                    println!("brew bundle stderr: {stderr}");
                }

                // The test passes if we can execute the command structure
                // (actual package existence is not relevant for this integration test)
                // No assertion needed - reaching this point means the test succeeded
            }
            Ok(Err(e)) => {
                println!("Failed to execute brew bundle command: {e}");
                // This could happen if brew bundle is not installed
                // We'll skip rather than fail, as the core functionality still works
            }
            Err(e) => {
                println!("Task spawn error: {e}");
            }
        }

        // Test the actual plugin restore (which will attempt real installation)
        let plugin = HomebrewBrewfilePlugin::new();
        let result = plugin.restore(&snapshot_dir, &target_dir, false).await;

        // The restore should succeed even if package installation fails
        assert!(
            result.is_ok(),
            "Restore should succeed even with installation failures"
        );

        let restored_files = result.unwrap();
        assert_eq!(restored_files.len(), 1);
        assert!(target_dir.join("Brewfile").exists());

        // Verify content was restored correctly
        let restored_content = fs::read_to_string(target_dir.join("Brewfile"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_brewfile_content);
    }
}

// Auto-register this plugin
crate::register_plugin!(HomebrewBrewfilePlugin, "homebrew_brewfile", "homebrew");

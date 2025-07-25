use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn};

use crate::plugins::core::base::package::{PackageCore, PackagePlugin};
use crate::plugins::core::mixins::CommandMixin;
use crate::symbols::*;

/// Homebrew-specific package manager implementation using the mixin architecture
#[derive(Default)]
pub struct HomebrewCore;

impl PackageCore for HomebrewCore {
    fn package_manager_name(&self) -> &'static str {
        "Homebrew"
    }

    fn package_command(&self) -> &'static str {
        "brew"
    }

    fn get_package_config(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>> {
        Box::pin(async move {
            // Generate Brewfile using brew bundle dump
            let temp_dir = std::env::temp_dir();
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

            // Read the generated Brewfile
            let brewfile_path = temp_dir.join("Brewfile");
            let brewfile_content = tokio::fs::read_to_string(&brewfile_path)
                .await
                .context("Failed to read generated Brewfile")?;

            // Clean up the temporary file
            let _ = tokio::fs::remove_file(&brewfile_path).await;

            if brewfile_content.is_empty() {
                Ok("# No Brewfile content generated\n".to_string())
            } else {
                Ok(brewfile_content)
            }
        })
    }

    fn restore_packages(
        &self,
        config_content: &str,
        target_dir: &Path,
        dry_run: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
        let config_content = config_content.to_string();
        let target_dir = target_dir.to_path_buf();

        Box::pin(async move {
            // Write Brewfile to target directory
            let brewfile_path = target_dir.join("Brewfile");

            if dry_run {
                warn!(
                    "DRY RUN: Would restore Homebrew Brewfile to {}",
                    brewfile_path.display()
                );
                warn!("DRY RUN: Would run 'brew bundle install' to install packages from Brewfile");
                return Ok(());
            }

            // Ensure target directory exists
            if let Some(parent) = brewfile_path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .context("Failed to create target directory for Brewfile")?;
            }

            // Write the Brewfile content
            tokio::fs::write(&brewfile_path, &config_content)
                .await
                .context("Failed to write Brewfile")?;

            info!("Restored Homebrew Brewfile to {}", brewfile_path.display());

            // Actually install packages from the Brewfile
            info!("Installing packages from Brewfile...");
            let target_dir_clone = target_dir.clone();
            match tokio::task::spawn_blocking(move || {
                Command::new("brew")
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

            Ok(())
        })
    }

    fn icon(&self) -> &'static str {
        SYMBOL_TOOL_PACKAGE_MANAGER
    }

    fn config_file_name(&self) -> String {
        "Brewfile".to_string()
    }

    fn allowed_extensions(&self) -> &'static [&'static str] {
        &["txt", "rb", "brewfile"] // Brewfile can have no extension or these
    }

    fn get_default_restore_dir(&self) -> Result<PathBuf> {
        // Homebrew Brewfiles are typically restored to the current directory
        Ok(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }
}

impl CommandMixin for HomebrewCore {
    // Uses default implementation with the package_command

    fn validate_command_exists(
        &self,
        cmd: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
        let cmd = cmd.to_string();
        Box::pin(async move {
            // First check if brew exists
            which::which(&cmd)
                .with_context(|| format!("{cmd} command not found. Please install Homebrew."))?;

            // Also check if brew bundle is available
            let output = tokio::task::spawn_blocking(|| {
                Command::new("brew").args(["bundle", "--help"]).output()
            })
            .await??;

            if !output.status.success() {
                return Err(anyhow::anyhow!("brew bundle command not available"));
            }

            Ok(())
        })
    }
}

/// Type alias for the Homebrew plugin
pub type HomebrewBrewfilePlugin = PackagePlugin<HomebrewCore>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::plugin::Plugin;
    use crate::plugins::core::mixins::ConfigMixin;
    use tempfile::TempDir;
    use tokio::fs;
    use which::which;

    #[tokio::test]
    async fn test_homebrew_core_app_info() {
        let core = HomebrewCore;
        assert_eq!(core.package_manager_name(), "Homebrew");
        assert_eq!(core.package_command(), "brew");
        assert_eq!(core.icon(), SYMBOL_TOOL_PACKAGE_MANAGER);
        assert_eq!(core.config_file_name(), "Brewfile");
        assert_eq!(core.allowed_extensions(), &["txt", "rb", "brewfile"]);
    }

    #[tokio::test]
    async fn test_homebrew_plugin_creation() {
        let plugin = PackagePlugin::new(HomebrewCore);
        assert_eq!(
            plugin.description(),
            "Manages package manager configuration and state"
        );
        assert_eq!(plugin.icon(), SYMBOL_TOOL_PACKAGE_MANAGER);
    }

    #[tokio::test]
    async fn test_homebrew_plugin_validation() {
        let plugin = PackagePlugin::new(HomebrewCore);

        // This test will only pass if Homebrew is installed
        if which("brew").is_ok() {
            // Check if brew bundle is also available
            let result = plugin.validate().await;
            if result.is_err() {
                // brew might exist but brew bundle might not be available
                assert!(result.unwrap_err().to_string().contains("brew bundle"));
            }
        } else {
            // Should fail with command not found
            assert!(plugin.validate().await.is_err());
        }
    }

    #[tokio::test]
    async fn test_homebrew_plugin_with_config() {
        let config_toml = r#"
            target_path = "homebrew"
            output_file = "Brewfile"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin = PackagePlugin::with_config(HomebrewCore, config);

        assert_eq!(
            ConfigMixin::get_target_path(&plugin),
            Some("homebrew".to_string())
        );
        assert_eq!(
            ConfigMixin::get_output_file(&plugin),
            Some("Brewfile".to_string())
        );
    }

    #[tokio::test]
    async fn test_homebrew_plugin_restore() {
        let plugin = PackagePlugin::new(HomebrewCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test Brewfile
        let test_brewfile = r#"# Brewfile
tap "homebrew/bundle"
brew "git"
brew "curl"
cask "firefox"
"#;
        let brewfile_path = snapshot_dir.join("Brewfile");
        fs::write(&brewfile_path, test_brewfile).await.unwrap();

        // Test dry run
        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target_dir);

        // Test actual restore (without actually running brew bundle install due to complexity)
        // This tests the file copy but skips the package installation
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target_dir);
    }

    #[test]
    fn test_homebrew_restore_target_dir_methods() {
        let plugin = PackagePlugin::new(HomebrewCore);

        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert!(default_dir.is_absolute() || default_dir == PathBuf::from("."));

        assert_eq!(ConfigMixin::get_restore_target_dir(&plugin), None);
    }
}

// Auto-register this plugin using the HomebrewCore implementation
crate::register_mixin_plugin!(
    HomebrewBrewfilePlugin,
    HomebrewCore,
    "homebrew_brewfile",
    "homebrew"
);

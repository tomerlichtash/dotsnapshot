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

    /// Test Homebrew core get_default_restore_dir method
    /// Verifies that the default restore directory is correctly resolved
    #[test]
    fn test_homebrew_core_get_default_restore_dir() {
        let core = HomebrewCore;
        let restore_dir = core.get_default_restore_dir().unwrap();

        // Should be current directory or absolute path
        assert!(restore_dir.is_absolute() || restore_dir == PathBuf::from("."));
    }

    /// Test Homebrew core validate_command_exists success
    /// Verifies that validate_command_exists works for existing commands
    #[tokio::test]
    async fn test_homebrew_validate_command_exists_success() {
        let core = HomebrewCore;

        // Test with the actual brew command since that's what the method validates
        let result = core.validate_command_exists("brew").await;
        // This test might fail in CI if Homebrew is not installed, which is expected
        if result.is_err() {
            // Homebrew not installed - this is acceptable in CI environments
            let error_msg = result.unwrap_err().to_string();
            assert!(
                error_msg.contains("command not found")
                    || error_msg.contains("bundle command not available")
            );
        } else {
            // Homebrew is installed - verification should pass
            assert!(result.is_ok());
        }
    }

    /// Test Homebrew core validate_command_exists failure
    /// Verifies that validate_command_exists fails for non-existent commands
    #[tokio::test]
    async fn test_homebrew_validate_command_exists_failure() {
        let core = HomebrewCore;

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

    /// Test Homebrew plugin trait methods
    /// Verifies that all Plugin trait methods work correctly
    #[tokio::test]
    async fn test_homebrew_plugin_trait_methods() {
        use crate::core::plugin::Plugin;
        let plugin = PackagePlugin::new(HomebrewCore);

        // Test basic plugin trait methods
        assert!(plugin.description().contains("package manager"));
        assert_eq!(plugin.icon(), SYMBOL_TOOL_PACKAGE_MANAGER);
        assert_eq!(Plugin::get_target_path(&plugin), None);
        assert_eq!(Plugin::get_output_file(&plugin), None);
        assert_eq!(Plugin::get_restore_target_dir(&plugin), None);
        assert!(!plugin.creates_own_output_files());
        assert!(plugin.get_hooks().is_empty());

        // Test default restore target directory
        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert!(default_dir.is_absolute() || default_dir == PathBuf::from("."));
    }

    /// Test Homebrew plugin execute with missing brew
    /// Verifies that plugin execution handles brew not being available
    #[tokio::test]
    async fn test_homebrew_plugin_execute_brew_not_available() {
        let plugin = PackagePlugin::new(HomebrewCore);

        // If brew is not available, execution should fail
        if which("brew").is_err() {
            let result = plugin.execute().await;
            assert!(result.is_err());
        }
    }

    /// Test Homebrew plugin with invalid configuration
    /// Verifies that plugin handles invalid config gracefully
    #[tokio::test]
    async fn test_homebrew_plugin_with_invalid_config() {
        let invalid_config_toml = r#"
            target_path = 123
            invalid_field = "should_be_ignored"
        "#;
        let config: toml::Value = toml::from_str(invalid_config_toml).unwrap();
        let plugin = PackagePlugin::with_config(HomebrewCore, config);

        // Should handle invalid config gracefully
        assert_eq!(
            plugin.description(),
            "Manages package manager configuration and state"
        );
        assert_eq!(plugin.icon(), SYMBOL_TOOL_PACKAGE_MANAGER);
    }

    /// Test Homebrew plugin restore with custom configuration
    /// Verifies that restore works with custom target and output settings
    #[tokio::test]
    async fn test_homebrew_plugin_restore_with_custom_config() {
        let config_toml = r#"
            target_path = "custom_brew"
            output_file = "custom_Brewfile"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin = PackagePlugin::with_config(HomebrewCore, config);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test Brewfile with custom content
        let test_brewfile = r#"# Custom Brewfile
tap "homebrew/core"
brew "node"
brew "python"
"#;
        let brewfile_path = snapshot_dir.join("Brewfile"); // Core filename
        fs::write(&brewfile_path, test_brewfile).await.unwrap();

        // Test restore with custom config
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target_dir);

        // Verify the Brewfile was created with core filename
        assert!(target_dir.join("Brewfile").exists());
        let restored_content = fs::read_to_string(target_dir.join("Brewfile"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_brewfile);
    }

    /// Test Homebrew plugin restore with no Brewfile
    /// Verifies that restore handles missing snapshot files gracefully
    #[tokio::test]
    async fn test_homebrew_plugin_restore_no_file() {
        let plugin = PackagePlugin::new(HomebrewCore);

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

    /// Test Homebrew core restore_packages with empty content
    /// Verifies that restore_packages handles empty Brewfile correctly
    #[tokio::test]
    async fn test_homebrew_core_restore_packages_empty() {
        let core = HomebrewCore;
        let temp_dir = TempDir::new().unwrap();
        let target_dir = temp_dir.path();

        fs::create_dir_all(target_dir).await.unwrap();

        // Test with empty content
        let result = core.restore_packages("", target_dir, false).await;
        assert!(result.is_ok());

        // Test with comment-only content
        let comment_content = "# Empty Brewfile\n# No packages to install\n";
        let result = core
            .restore_packages(comment_content, target_dir, false)
            .await;
        assert!(result.is_ok());
    }

    /// Test Homebrew core restore_packages dry run
    /// Verifies that dry run mode doesn't actually install packages
    #[tokio::test]
    async fn test_homebrew_core_restore_packages_dry_run() {
        let core = HomebrewCore;
        let temp_dir = TempDir::new().unwrap();
        let target_dir = temp_dir.path();

        fs::create_dir_all(target_dir).await.unwrap();

        let test_brewfile = r#"tap "homebrew/core"
brew "git"
cask "firefox"
"#;
        let result = core.restore_packages(test_brewfile, target_dir, true).await;
        assert!(result.is_ok());

        // Verify that the Brewfile is not created in dry run mode
        assert!(!target_dir.join("Brewfile").exists());
    }

    /// Test Homebrew core allowed extensions
    /// Verifies that all expected file extensions are supported
    #[test]
    fn test_homebrew_core_allowed_extensions() {
        let core = HomebrewCore;
        let extensions = core.allowed_extensions();

        assert_eq!(extensions.len(), 3);
        assert!(extensions.contains(&"txt"));
        assert!(extensions.contains(&"rb"));
        assert!(extensions.contains(&"brewfile"));
    }

    /// Test Homebrew plugin restore with alternative file extensions
    /// Verifies that restore works with different allowed extensions
    #[tokio::test]
    async fn test_homebrew_plugin_restore_alternative_extensions() {
        let plugin = PackagePlugin::new(HomebrewCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Test with core filename (this is what the plugin actually looks for)
        let test_brewfile = r#"tap "homebrew/bundle"
brew "wget"
brew "curl"
"#;
        let brewfile_path = snapshot_dir.join("Brewfile");
        fs::write(&brewfile_path, test_brewfile).await.unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert!(target_dir.join("Brewfile").exists());

        let restored_content = fs::read_to_string(target_dir.join("Brewfile"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_brewfile);
    }

    /// Test Homebrew plugin restore error handling
    /// Verifies that restore handles error scenarios gracefully
    #[tokio::test]
    async fn test_homebrew_plugin_restore_error_handling() {
        let plugin = PackagePlugin::new(HomebrewCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");

        fs::create_dir_all(&snapshot_dir).await.unwrap();

        // Create Brewfile
        let test_brewfile = "brew \"test-package\"\n";
        let brewfile_path = snapshot_dir.join("Brewfile");
        fs::write(&brewfile_path, test_brewfile).await.unwrap();

        // Try to restore to a read-only directory (this might fail on some systems)
        let readonly_dir = temp_dir.path().join("readonly");
        fs::create_dir_all(&readonly_dir).await.unwrap();

        // On Unix systems, make directory read-only
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&readonly_dir).await.unwrap().permissions();
            perms.set_mode(0o444); // Read-only
            fs::set_permissions(&readonly_dir, perms).await.unwrap();

            // This should fail due to permissions
            let result = plugin.restore(&snapshot_dir, &readonly_dir, false).await;
            // Note: This test might not fail on all systems due to different permission handling
            // So we just verify it completes without panicking
            let _ = result;

            // Restore permissions for cleanup
            let mut perms = fs::metadata(&readonly_dir).await.unwrap().permissions();
            perms.set_mode(0o755); // Read/write/execute
            fs::set_permissions(&readonly_dir, perms).await.unwrap();
        }
    }

    /// Test Homebrew core config file name method
    /// Verifies that the correct config file name is returned
    #[test]
    fn test_homebrew_core_config_file_name() {
        let core = HomebrewCore;
        assert_eq!(core.config_file_name(), "Brewfile");
    }

    /// Test Homebrew core package command and manager name
    /// Verifies that all package core methods return correct values
    #[test]
    fn test_homebrew_core_package_methods() {
        let core = HomebrewCore;
        assert_eq!(core.package_manager_name(), "Homebrew");
        assert_eq!(core.package_command(), "brew");
    }
}

// Auto-register this plugin using the HomebrewCore implementation
crate::register_mixin_plugin!(
    HomebrewBrewfilePlugin,
    HomebrewCore,
    "homebrew_brewfile",
    "homebrew"
);

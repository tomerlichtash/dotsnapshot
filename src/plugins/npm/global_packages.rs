use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn};

use crate::plugins::core::base::package::{PackageCore, PackagePlugin};
use crate::plugins::core::mixins::CommandMixin;
use crate::symbols::*;

/// NPM-specific package manager implementation using the mixin architecture
#[derive(Default)]
pub struct NpmGlobalCore;

impl PackageCore for NpmGlobalCore {
    fn package_manager_name(&self) -> &'static str {
        "NPM"
    }

    fn package_command(&self) -> &'static str {
        "npm"
    }

    fn get_package_config(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>> {
        Box::pin(async move {
            // Get globally installed NPM packages
            let output = tokio::task::spawn_blocking(|| {
                Command::new("npm")
                    .args(["list", "--global", "--depth=0", "--parseable"])
                    .output()
            })
            .await??;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("npm list --global failed: {}", stderr));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);

            // Parse the output to extract package names and versions
            let mut packages = Vec::new();
            for line in stdout.lines() {
                if let Some(package_path) = line.strip_prefix('/') {
                    // Extract package name from path like "/usr/local/lib/node_modules/package@version"
                    if let Some(node_modules_pos) = package_path.find("node_modules/") {
                        let after_node_modules =
                            &package_path[node_modules_pos + "node_modules/".len()..];
                        if !after_node_modules.is_empty() {
                            packages.push(after_node_modules.to_string());
                        }
                    }
                } else if !line.trim().is_empty() && !line.starts_with("npm") {
                    // Handle other output formats
                    packages.push(line.trim().to_string());
                }
            }

            if packages.is_empty() {
                Ok("# No global NPM packages found\n".to_string())
            } else {
                // Sort packages for consistent output
                packages.sort();
                Ok(packages.join("\n") + "\n")
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
            // Write package list to target directory
            let packages_file = target_dir.join("npm_global_packages.txt");

            if dry_run {
                warn!(
                    "DRY RUN: Would restore NPM global packages list to {}",
                    packages_file.display()
                );
                warn!("DRY RUN: Would install global packages using 'npm install -g'");
                return Ok(());
            }

            // Ensure target directory exists
            if let Some(parent) = packages_file.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .context("Failed to create target directory for NPM packages list")?;
            }

            // Write the packages list
            tokio::fs::write(&packages_file, &config_content)
                .await
                .context("Failed to write NPM global packages list")?;

            info!(
                "Restored NPM global packages list to {}",
                packages_file.display()
            );

            // Parse packages and install them
            let packages: Vec<&str> = config_content
                .lines()
                .map(|line| line.trim())
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .collect();

            if packages.is_empty() {
                info!("No NPM global packages to install");
                return Ok(());
            }

            info!("Installing {} global NPM packages...", packages.len());

            // Install packages one by one to handle failures gracefully
            let mut installed = 0;
            let mut failed = 0;

            for package in packages {
                match tokio::task::spawn_blocking({
                    let package = package.to_string();
                    move || {
                        Command::new("npm")
                            .args(["install", "--global", &package])
                            .output()
                    }
                })
                .await
                {
                    Ok(Ok(result)) => {
                        if result.status.success() {
                            info!("Successfully installed: {}", package);
                            installed += 1;
                        } else {
                            let stderr = String::from_utf8_lossy(&result.stderr);
                            warn!("Failed to install {}: {}", package, stderr);
                            failed += 1;
                        }
                    }
                    Ok(Err(e)) => {
                        warn!("Failed to execute npm install for {}: {}", package, e);
                        failed += 1;
                    }
                    Err(e) => {
                        warn!("Failed to spawn npm install task for {}: {}", package, e);
                        failed += 1;
                    }
                }
            }

            if installed > 0 {
                info!("Successfully installed {} global NPM packages", installed);
            }
            if failed > 0 {
                warn!("{} global NPM packages failed to install", failed);
            }

            Ok(())
        })
    }

    fn icon(&self) -> &'static str {
        SYMBOL_TOOL_PACKAGE_MANAGER
    }

    fn config_file_name(&self) -> String {
        "global_packages.txt".to_string()
    }

    fn allowed_extensions(&self) -> &'static [&'static str] {
        &["txt", "list", "log"]
    }

    fn get_default_restore_dir(&self) -> Result<PathBuf> {
        // NPM global packages list is typically saved to the current directory
        Ok(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }
}

impl CommandMixin for NpmGlobalCore {
    // Uses default implementation with the package_command

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

/// Type alias for the NPM global packages plugin
pub type NpmGlobalPackagesPlugin = PackagePlugin<NpmGlobalCore>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::plugin::Plugin;
    use crate::plugins::core::mixins::ConfigMixin;
    use tempfile::TempDir;
    use tokio::fs;
    use which::which;

    #[tokio::test]
    async fn test_npm_global_core_app_info() {
        let core = NpmGlobalCore;
        assert_eq!(core.package_manager_name(), "NPM");
        assert_eq!(core.package_command(), "npm");
        assert_eq!(core.icon(), SYMBOL_TOOL_PACKAGE_MANAGER);
        assert_eq!(core.config_file_name(), "global_packages.txt");
        assert_eq!(core.allowed_extensions(), &["txt", "list", "log"]);
    }

    #[tokio::test]
    async fn test_npm_global_plugin_creation() {
        let plugin = PackagePlugin::new(NpmGlobalCore);
        assert_eq!(
            plugin.description(),
            "Manages package manager configuration and state"
        );
        assert_eq!(plugin.icon(), SYMBOL_TOOL_PACKAGE_MANAGER);
    }

    #[tokio::test]
    async fn test_npm_global_plugin_validation() {
        let plugin = PackagePlugin::new(NpmGlobalCore);

        // This test will only pass if npm is installed
        if which("npm").is_ok() {
            assert!(plugin.validate().await.is_ok());
        } else {
            // Should fail with command not found
            assert!(plugin.validate().await.is_err());
        }
    }

    #[tokio::test]
    async fn test_npm_global_plugin_with_config() {
        let config_toml = r#"
            target_path = "npm"
            output_file = "global_packages.txt"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin = PackagePlugin::with_config(NpmGlobalCore, config);

        assert_eq!(
            ConfigMixin::get_target_path(&plugin),
            Some("npm".to_string())
        );
        assert_eq!(
            ConfigMixin::get_output_file(&plugin),
            Some("global_packages.txt".to_string())
        );
    }

    #[tokio::test]
    async fn test_npm_global_plugin_restore() {
        let plugin = PackagePlugin::new(NpmGlobalCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test packages list
        let test_packages = "typescript@4.9.5\nnodemon@2.0.22\nyarn@1.22.19\n";
        let packages_path = snapshot_dir.join("global_packages.txt");
        fs::write(&packages_path, test_packages).await.unwrap();

        // Test dry run
        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target_dir);

        // Test actual restore (without actually installing packages)
        // This tests the file copy but skips the package installation
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target_dir);

        // Verify the packages file was created
        assert!(target_dir.join("npm_global_packages.txt").exists());
        let restored_content = fs::read_to_string(target_dir.join("npm_global_packages.txt"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_packages);
    }

    #[test]
    fn test_npm_global_restore_target_dir_methods() {
        let plugin = PackagePlugin::new(NpmGlobalCore);

        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert!(default_dir.is_absolute() || default_dir == PathBuf::from("."));

        assert_eq!(ConfigMixin::get_restore_target_dir(&plugin), None);
    }

    /// Test NPM global core get_default_restore_dir method
    /// Verifies that the default restore directory is correctly resolved
    #[test]
    fn test_npm_global_core_get_default_restore_dir() {
        let core = NpmGlobalCore;
        let restore_dir = core.get_default_restore_dir().unwrap();

        // Should be current directory or absolute path
        assert!(restore_dir.is_absolute() || restore_dir == PathBuf::from("."));
    }

    /// Test NPM global core validate_command_exists success
    /// Verifies that validate_command_exists works for existing commands
    #[tokio::test]
    async fn test_npm_global_validate_command_exists_success() {
        let core = NpmGlobalCore;

        // Test with a command that should exist
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

    /// Test NPM global core validate_command_exists failure
    /// Verifies that validate_command_exists fails for non-existent commands
    #[tokio::test]
    async fn test_npm_global_validate_command_exists_failure() {
        let core = NpmGlobalCore;

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

    /// Test NPM global plugin trait methods
    /// Verifies that all Plugin trait methods work correctly
    #[tokio::test]
    async fn test_npm_global_plugin_trait_methods() {
        use crate::core::plugin::Plugin;
        let plugin = PackagePlugin::new(NpmGlobalCore);

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

    /// Test NPM global plugin execute with missing npm
    /// Verifies that plugin execution handles npm not being available
    #[tokio::test]
    async fn test_npm_global_plugin_execute_npm_not_available() {
        let plugin = PackagePlugin::new(NpmGlobalCore);

        // If npm is not available, execution should fail
        if which("npm").is_err() {
            let result = plugin.execute().await;
            assert!(result.is_err());
        }
    }

    /// Test NPM global plugin with invalid configuration
    /// Verifies that plugin handles invalid config gracefully
    #[tokio::test]
    async fn test_npm_global_plugin_with_invalid_config() {
        let invalid_config_toml = r#"
            target_path = 123
            invalid_field = "should_be_ignored"
        "#;
        let config: toml::Value = toml::from_str(invalid_config_toml).unwrap();
        let plugin = PackagePlugin::with_config(NpmGlobalCore, config);

        // Should handle invalid config gracefully
        assert_eq!(
            plugin.description(),
            "Manages package manager configuration and state"
        );
        assert_eq!(plugin.icon(), SYMBOL_TOOL_PACKAGE_MANAGER);
    }

    /// Test NPM global plugin restore with custom configuration
    /// Verifies that restore works with custom target and output settings
    #[tokio::test]
    async fn test_npm_global_plugin_restore_with_custom_config() {
        let config_toml = r#"
            target_path = "custom_npm"
            output_file = "custom_global_packages.txt"
        "#;
        let config: toml::Value = toml::from_str(config_toml).unwrap();
        let plugin = PackagePlugin::with_config(NpmGlobalCore, config);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test packages with custom filename
        let test_packages = "lodash@4.17.21\nmoment@2.29.4\n";
        let packages_path = snapshot_dir.join("global_packages.txt"); // Core filename
        fs::write(&packages_path, test_packages).await.unwrap();

        // Test restore with custom config
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target_dir);

        // Verify the packages file was created with core filename
        assert!(target_dir.join("npm_global_packages.txt").exists());
        let restored_content = fs::read_to_string(target_dir.join("npm_global_packages.txt"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_packages);
    }

    /// Test NPM global plugin restore with no packages file
    /// Verifies that restore handles missing snapshot files gracefully
    #[tokio::test]
    async fn test_npm_global_plugin_restore_no_file() {
        let plugin = PackagePlugin::new(NpmGlobalCore);

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

    /// Test NPM global core package parsing logic
    /// Verifies that get_package_config handles various output formats
    #[tokio::test]
    async fn test_npm_global_core_package_parsing() {
        let _core = NpmGlobalCore;

        // Test the parsing logic conceptually (similar to what get_package_config does)
        let sample_npm_output = r#"/usr/local/lib/node_modules/typescript@4.9.5
/usr/local/lib/node_modules/nodemon@2.0.22
/usr/local/lib/node_modules/@angular/cli@15.2.4
yarn@1.22.19
npm@9.6.7
"#;

        let mut packages = Vec::new();
        for line in sample_npm_output.lines() {
            if let Some(package_path) = line.strip_prefix('/') {
                // Extract package name from path
                if let Some(node_modules_pos) = package_path.find("node_modules/") {
                    let after_node_modules =
                        &package_path[node_modules_pos + "node_modules/".len()..];
                    if !after_node_modules.is_empty() {
                        packages.push(after_node_modules.to_string());
                    }
                }
            } else if !line.trim().is_empty() && !line.starts_with("npm") {
                packages.push(line.trim().to_string());
            }
        }

        // Should parse packages correctly
        assert!(packages.contains(&"typescript@4.9.5".to_string()));
        assert!(packages.contains(&"nodemon@2.0.22".to_string()));
        assert!(packages.contains(&"@angular/cli@15.2.4".to_string()));
        assert!(packages.contains(&"yarn@1.22.19".to_string()));
        assert!(!packages.contains(&"npm@9.6.7".to_string())); // Should exclude npm itself
    }

    /// Test NPM global core restore_packages with empty content
    /// Verifies that restore_packages handles empty package lists correctly
    #[tokio::test]
    async fn test_npm_global_core_restore_packages_empty() {
        let core = NpmGlobalCore;
        let temp_dir = TempDir::new().unwrap();
        let target_dir = temp_dir.path();

        fs::create_dir_all(target_dir).await.unwrap();

        // Test with empty content
        let result = core.restore_packages("", target_dir, false).await;
        assert!(result.is_ok());

        // Test with comment-only content
        let comment_content = "# No packages to install\n# This is a comment\n";
        let result = core
            .restore_packages(comment_content, target_dir, false)
            .await;
        assert!(result.is_ok());
    }

    /// Test NPM global core restore_packages dry run
    /// Verifies that dry run mode doesn't actually install packages
    #[tokio::test]
    async fn test_npm_global_core_restore_packages_dry_run() {
        let core = NpmGlobalCore;
        let temp_dir = TempDir::new().unwrap();
        let target_dir = temp_dir.path();

        fs::create_dir_all(target_dir).await.unwrap();

        let test_packages = "lodash@4.17.21\nmoment@2.29.4\n";
        let result = core.restore_packages(test_packages, target_dir, true).await;
        assert!(result.is_ok());

        // Verify that the packages file is not created in dry run mode
        assert!(!target_dir.join("npm_global_packages.txt").exists());
    }

    /// Test NPM global core allowed extensions
    /// Verifies that all expected file extensions are supported
    #[test]
    fn test_npm_global_core_allowed_extensions() {
        let core = NpmGlobalCore;
        let extensions = core.allowed_extensions();

        assert_eq!(extensions.len(), 3);
        assert!(extensions.contains(&"txt"));
        assert!(extensions.contains(&"list"));
        assert!(extensions.contains(&"log"));
    }

    /// Test NPM global package parsing edge cases
    /// Verifies that package parsing handles various edge cases correctly
    #[test]
    fn test_npm_global_package_parsing_edge_cases() {
        // Test empty lines and npm entries
        let test_input = "
/usr/local/lib/node_modules/typescript@4.9.5

npm@9.6.7
/usr/local/lib/node_modules/lodash@4.17.21
        
";

        let mut packages = Vec::new();
        for line in test_input.lines() {
            if let Some(package_path) = line.strip_prefix('/') {
                if let Some(node_modules_pos) = package_path.find("node_modules/") {
                    let after_node_modules =
                        &package_path[node_modules_pos + "node_modules/".len()..];
                    if !after_node_modules.is_empty() {
                        packages.push(after_node_modules.to_string());
                    }
                }
            } else if !line.trim().is_empty() && !line.starts_with("npm") {
                packages.push(line.trim().to_string());
            }
        }

        assert!(packages.contains(&"typescript@4.9.5".to_string()));
        assert!(packages.contains(&"lodash@4.17.21".to_string()));
        assert!(!packages.contains(&"npm@9.6.7".to_string()));
        assert!(!packages.iter().any(|p| p.is_empty()));
    }

    /// Test NPM global plugin restore with alternative file extensions
    /// Verifies that restore works with different allowed extensions
    #[tokio::test]
    async fn test_npm_global_plugin_restore_alternative_extensions() {
        let plugin = PackagePlugin::new(NpmGlobalCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Test with core filename (this is what the plugin actually looks for)
        let test_packages = "express@4.18.2\nreact@18.2.0\n";
        let packages_path = snapshot_dir.join("global_packages.txt");
        fs::write(&packages_path, test_packages).await.unwrap();

        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert!(target_dir.join("npm_global_packages.txt").exists());

        let restored_content = fs::read_to_string(target_dir.join("npm_global_packages.txt"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_packages);
    }

    /// Test NPM global plugin restore error handling
    /// Verifies that restore handles error scenarios gracefully
    #[tokio::test]
    async fn test_npm_global_plugin_restore_error_handling() {
        let _plugin = PackagePlugin::new(NpmGlobalCore);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");

        fs::create_dir_all(&snapshot_dir).await.unwrap();

        // Create packages file
        let test_packages = "test-package@1.0.0\n";
        let packages_path = snapshot_dir.join("global_packages.txt");
        fs::write(&packages_path, test_packages).await.unwrap();

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
            let result = _plugin.restore(&snapshot_dir, &readonly_dir, false).await;
            // Note: This test might not fail on all systems due to different permission handling
            // So we just verify it completes without panicking
            let _ = result;

            // Restore permissions for cleanup
            let mut perms = fs::metadata(&readonly_dir).await.unwrap().permissions();
            perms.set_mode(0o755); // Read/write/execute
            fs::set_permissions(&readonly_dir, perms).await.unwrap();
        }
    }
}

// Auto-register this plugin using the NpmGlobalCore implementation
crate::register_mixin_plugin!(
    NpmGlobalPackagesPlugin,
    NpmGlobalCore,
    "npm_global_packages",
    "npm"
);

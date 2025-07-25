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
}

// Auto-register this plugin using the NpmGlobalCore implementation
crate::register_mixin_plugin!(
    NpmGlobalPackagesPlugin,
    NpmGlobalCore,
    "npm_global_packages",
    "npm"
);

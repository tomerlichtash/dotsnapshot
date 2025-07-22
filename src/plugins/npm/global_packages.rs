use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Command;
use which::which;

use crate::core::plugin::Plugin;
use crate::symbols::*;

/// Plugin for capturing NPM global packages
pub struct NpmGlobalPackagesPlugin;

impl NpmGlobalPackagesPlugin {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self
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
    fn filename(&self) -> &str {
        "npm_global_packages.txt"
    }

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_npm_global_packages_plugin_name() {
        let plugin = NpmGlobalPackagesPlugin::new();
        assert_eq!(plugin.filename(), "npm_global_packages.txt");
    }

    #[tokio::test]
    async fn test_npm_global_packages_plugin_validation() {
        let plugin = NpmGlobalPackagesPlugin::new();

        // This test will only pass if npm is installed
        if which("npm").is_ok() {
            assert!(plugin.validate().await.is_ok());
        }
    }
}

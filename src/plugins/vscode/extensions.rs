use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Command;
use which::which;

use crate::core::plugin::Plugin;
use crate::symbols::*;

/// Plugin for capturing VSCode extensions
pub struct VSCodeExtensionsPlugin;

impl VSCodeExtensionsPlugin {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self
    }

    /// Gets list of installed VSCode extensions
    async fn get_extensions(&self) -> Result<String> {
        let output = tokio::task::spawn_blocking(|| {
            Command::new("code")
                .args(["--list-extensions", "--show-versions"])
                .output()
        })
        .await??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("code --list-extensions failed: {stderr}"));
        }

        let extensions = String::from_utf8(output.stdout)
            .context("Failed to parse code --list-extensions output as UTF-8")?;

        Ok(extensions)
    }
}

#[async_trait]
impl Plugin for VSCodeExtensionsPlugin {
    fn filename(&self) -> &str {
        "vscode_extensions.txt"
    }

    fn description(&self) -> &str {
        "Lists installed VSCode extensions with versions"
    }

    fn icon(&self) -> &str {
        TOOL_COMPUTER
    }

    async fn execute(&self) -> Result<String> {
        self.get_extensions().await
    }

    async fn validate(&self) -> Result<()> {
        // Check if code command exists
        which("code").context("code command not found. Please install VSCode CLI.")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vscode_extensions_plugin_name() {
        let plugin = VSCodeExtensionsPlugin::new();
        assert_eq!(plugin.filename(), "vscode_extensions.txt");
    }

    #[tokio::test]
    async fn test_vscode_extensions_plugin_validation() {
        let plugin = VSCodeExtensionsPlugin::new();

        // This test will only pass if VSCode CLI is installed
        if which("code").is_ok() {
            assert!(plugin.validate().await.is_ok());
        }
    }
}

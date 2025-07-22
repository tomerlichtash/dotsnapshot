use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Command;
use which::which;

use crate::core::plugin::Plugin;

/// Plugin for capturing Cursor extensions
pub struct CursorExtensionsPlugin;

impl CursorExtensionsPlugin {
    // Allow new_without_default because plugins intentionally use new() instead of Default
    // to maintain consistent plugin instantiation patterns across the codebase
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self
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
    fn name(&self) -> &str {
        "cursor_extensions"
    }

    fn filename(&self) -> &str {
        "cursor_extensions.txt"
    }

    fn description(&self) -> &str {
        "Lists installed Cursor editor extensions with versions"
    }

    fn display_name(&self) -> &str {
        "Cursor"
    }

    fn icon(&self) -> &str {
        "✏️"
    }

    async fn execute(&self) -> Result<String> {
        self.get_extensions().await
    }

    async fn validate(&self) -> Result<()> {
        // Check if cursor command exists
        which("cursor").context("cursor command not found. Please install Cursor CLI.")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cursor_extensions_plugin_name() {
        let plugin = CursorExtensionsPlugin::new();
        assert_eq!(plugin.name(), "cursor_extensions");
        assert_eq!(plugin.filename(), "cursor_extensions.txt");
    }

    #[tokio::test]
    async fn test_cursor_extensions_plugin_validation() {
        let plugin = CursorExtensionsPlugin::new();

        // This test will only pass if Cursor CLI is installed
        if which("cursor").is_ok() {
            assert!(plugin.validate().await.is_ok());
        }
    }
}

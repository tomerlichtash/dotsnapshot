use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{info, warn};

use crate::core::plugin::Plugin;
use crate::plugins::core::mixins::{
    CommandMixin, ConfigMixin, FilesMixin, StandardConfig, StandardConfigMixin,
};

/// Core trait that defines application-specific extensions behavior
pub trait ExtensionsCore: Send + Sync + CommandMixin {
    /// The name of the application (e.g., "VSCode", "Cursor")
    fn app_name(&self) -> &'static str;

    /// The command used to list extensions (e.g., "code", "cursor")
    fn extensions_command(&self) -> &'static str;

    /// Arguments for listing extensions (e.g., ["--list-extensions"])
    fn list_extensions_args(&self) -> &'static [&'static str] {
        &["--list-extensions"]
    }

    /// Get the list of installed extensions
    fn get_extensions(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>> {
        // Default implementation uses the command mixin
        let cmd = self.extensions_command();
        let args = self.list_extensions_args();
        Box::pin(async move {
            let output = self.execute_command(cmd, args).await?;
            Ok(output)
        })
    }

    /// Get the icon for this extensions plugin
    fn icon(&self) -> &'static str {
        "📦" // Default extensions icon
    }

    /// Get the default filename for the extensions list
    fn extensions_file_name(&self) -> String {
        "extensions.txt".to_string()
    }

    /// Get the default restore filename for the extensions list
    fn restore_file_name(&self) -> String {
        format!("{}_extensions.txt", self.app_name().to_lowercase())
    }

    /// Get custom file extensions allowed for extension list files
    fn allowed_extensions(&self) -> &'static [&'static str] {
        &["txt", "list"] // Default: text files
    }

    /// Get the default restore target directory
    fn get_default_restore_dir(&self) -> Result<PathBuf> {
        // Extensions lists are typically saved to the current directory
        Ok(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }
}

/// Generic extensions plugin that can be used for any application
pub struct ExtensionsPlugin<T: ExtensionsCore + CommandMixin> {
    config: Option<StandardConfig>,
    core: T,
}

impl<T: ExtensionsCore + CommandMixin> ExtensionsPlugin<T> {
    /// Create a new extensions plugin without configuration
    pub fn new(core: T) -> Self {
        Self { config: None, core }
    }

    /// Create a new extensions plugin with configuration
    pub fn with_config(core: T, config: toml::Value) -> Self {
        let (parsed_config, is_valid) = Self::with_config_validation(
            config,
            &format!("{} Extensions plugin", core.app_name()),
            &format!("{}_extensions", core.app_name().to_lowercase()),
            "target_path (string), output_file (string), restore_target_dir (string), hooks (object)",
            &format!(
                "target_path = \"{}\", output_file = \"{}\"",
                core.app_name().to_lowercase(),
                core.extensions_file_name()
            ),
        );

        // Additional validation for extensions-specific fields
        if is_valid {
            if let Some(output_file) = &parsed_config.output_file {
                let extensions = core.allowed_extensions();
                if let Err(e) =
                    crate::core::config_schema::ValidationHelpers::validate_file_extension(
                        output_file,
                        extensions,
                    )
                {
                    warn!(
                        "Invalid output file extension for {} extensions: {}",
                        core.app_name(),
                        e
                    );
                }
            }
        }

        Self {
            config: Some(parsed_config),
            core,
        }
    }
}

// Implement mixins for the extensions plugin
impl<T: ExtensionsCore + CommandMixin> ConfigMixin for ExtensionsPlugin<T> {
    type Config = StandardConfig;

    fn config(&self) -> Option<&Self::Config> {
        self.config.as_ref()
    }

    fn get_target_path(&self) -> Option<String> {
        self.get_standard_target_path()
    }

    fn get_output_file(&self) -> Option<String> {
        self.get_standard_output_file()
    }

    fn get_restore_target_dir(&self) -> Option<String> {
        self.get_standard_restore_target_dir()
    }
}

impl<T: ExtensionsCore + CommandMixin> StandardConfigMixin for ExtensionsPlugin<T> {}

// Delegate CommandMixin to the core
impl<T: ExtensionsCore + CommandMixin> CommandMixin for ExtensionsPlugin<T> {
    fn execute_command(
        &self,
        cmd: &str,
        args: &[&str],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>> {
        self.core.execute_command(cmd, args)
    }

    fn validate_command_exists(
        &self,
        cmd: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
        self.core.validate_command_exists(cmd)
    }

    fn command_exists(&self, cmd: &str) -> bool {
        self.core.command_exists(cmd)
    }
}

// Implement FilesMixin for the extensions plugin
impl<T: ExtensionsCore + CommandMixin> crate::plugins::core::mixins::FilesMixin
    for ExtensionsPlugin<T>
{
}

// The plugin trait implementation gets all the mixin functionality automatically

#[async_trait]
impl<T: ExtensionsCore + CommandMixin + Send + Sync> Plugin for ExtensionsPlugin<T> {
    fn description(&self) -> &str {
        "Lists installed extensions for application"
    }

    fn icon(&self) -> &str {
        self.core.icon()
    }

    async fn execute(&self) -> Result<String> {
        self.core.get_extensions().await
    }

    async fn validate(&self) -> Result<()> {
        // Check if the extensions command exists
        self.validate_command_exists(self.core.extensions_command())
            .await?;
        Ok(())
    }

    fn get_target_path(&self) -> Option<String> {
        ConfigMixin::get_target_path(self)
    }

    fn get_output_file(&self) -> Option<String> {
        ConfigMixin::get_output_file(self)
    }

    fn get_restore_target_dir(&self) -> Option<String> {
        ConfigMixin::get_restore_target_dir(self)
    }

    fn get_default_restore_target_dir(&self) -> Result<PathBuf> {
        self.core.get_default_restore_dir()
    }

    fn get_hooks(&self) -> Vec<crate::core::hooks::HookAction> {
        // Extensions plugins don't have hooks by default
        Vec::new()
    }

    async fn restore(
        &self,
        snapshot_path: &std::path::Path,
        target_path: &std::path::Path,
        dry_run: bool,
    ) -> Result<Vec<PathBuf>> {
        use tokio::fs;

        let mut restored_files = Vec::new();

        // Find extensions file in the snapshot
        let extensions_filename =
            ConfigMixin::get_output_file(self).unwrap_or_else(|| self.core.extensions_file_name());
        let mut source_extensions = snapshot_path.join(&extensions_filename);

        if !source_extensions.exists() {
            // Try alternative common names
            let alternative_names = [
                &self.core.extensions_file_name(),
                &format!("{}_extensions.txt", self.core.app_name().to_lowercase()),
                "extensions.list",
            ];
            let mut found = false;

            for name in &alternative_names {
                let alt_path = snapshot_path.join(name);
                if alt_path.exists() {
                    source_extensions = alt_path;
                    info!(
                        "Found {} extensions file at alternative path: {}",
                        self.core.app_name(),
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

        let target_extensions_file = target_path.join(self.core.restore_file_name());

        if dry_run {
            warn!(
                "DRY RUN: Would restore {} extensions list to {}",
                self.core.app_name(),
                target_extensions_file.display()
            );
            warn!(
                "DRY RUN: Review the extension list and install manually with '{} --install-extension <extension-id>'",
                self.core.extensions_command()
            );
            restored_files.push(target_extensions_file);
        } else {
            // Create target directory if it doesn't exist
            if let Some(parent) = target_extensions_file.parent() {
                fs::create_dir_all(parent).await.with_context(|| {
                    format!(
                        "Failed to create target directory for {} extensions file",
                        self.core.app_name()
                    )
                })?;
            }

            // Copy extensions file to target location
            self.restore_file(&source_extensions, &target_extensions_file)
                .await?;

            info!(
                "Restored {} extensions list to {}",
                self.core.app_name(),
                target_extensions_file.display()
            );
            info!("Note: This is a reference list. To install extensions, you'll need to:");
            info!("  1. Review the extension list in the restored file");
            info!(
                "  2. Install extensions manually with '{} --install-extension <extension-id>'",
                self.core.extensions_command()
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
    use tempfile::TempDir;
    use tokio::fs;

    struct MockExtensionsCore;

    impl ExtensionsCore for MockExtensionsCore {
        fn app_name(&self) -> &'static str {
            "TestApp"
        }

        fn extensions_command(&self) -> &'static str {
            "testapp"
        }

        fn get_extensions(
            &self,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
        {
            Box::pin(async { Ok("extension1@1.0.0\nextension2@2.0.0".to_string()) })
        }
    }

    impl CommandMixin for MockExtensionsCore {
        fn execute_command(
            &self,
            _cmd: &str,
            _args: &[&str],
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
        {
            Box::pin(async { Ok("extension1@1.0.0\nextension2@2.0.0".to_string()) })
        }

        fn validate_command_exists(
            &self,
            _cmd: &str,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
            Box::pin(async { Ok(()) })
        }

        fn command_exists(&self, _cmd: &str) -> bool {
            true
        }
    }

    #[tokio::test]
    async fn test_extensions_plugin_creation() {
        let core = MockExtensionsCore;
        let plugin = ExtensionsPlugin::new(core);

        assert_eq!(plugin.core.app_name(), "TestApp");
        assert_eq!(plugin.core.extensions_command(), "testapp");
        assert_eq!(
            plugin.description(),
            "Lists installed extensions for application"
        );
    }

    #[tokio::test]
    async fn test_extensions_plugin_execute() {
        let core = MockExtensionsCore;
        let plugin = ExtensionsPlugin::new(core);

        let result = plugin.execute().await.unwrap();
        assert_eq!(result, "extension1@1.0.0\nextension2@2.0.0");
    }

    #[tokio::test]
    async fn test_extensions_plugin_restore() {
        let core = MockExtensionsCore;
        let plugin = ExtensionsPlugin::new(core);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test extensions file
        let test_extensions = "extension1@1.0.0\nextension2@2.0.0";
        let extensions_path = snapshot_dir.join("extensions.txt");
        fs::write(&extensions_path, test_extensions).await.unwrap();

        // Test restore
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert!(target_dir.join("testapp_extensions.txt").exists());

        let restored_content = fs::read_to_string(target_dir.join("testapp_extensions.txt"))
            .await
            .unwrap();
        assert_eq!(restored_content, test_extensions);
    }
}

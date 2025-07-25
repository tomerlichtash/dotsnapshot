use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{info, warn};

use crate::core::plugin::Plugin;
use crate::plugins::core::mixins::{
    AllMixins, CommandMixin, ConfigMixin, StandardConfig, StandardConfigMixin,
};

/// Core trait that defines package manager-specific behavior
#[allow(dead_code)]
pub trait PackageCore: Send + Sync {
    /// The name of the package manager (e.g., "Homebrew", "NPM")
    fn package_manager_name(&self) -> &'static str;

    /// The command used for the package manager (e.g., "brew", "npm")
    fn package_command(&self) -> &'static str;

    /// Get the package manager configuration/state
    fn get_package_config(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>;

    /// Restore packages from configuration
    fn restore_packages(
        &self,
        config_content: &str,
        target_dir: &std::path::Path,
        dry_run: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>>;

    /// Get the icon for this package manager plugin
    fn icon(&self) -> &'static str {
        "📦" // Default package icon
    }

    /// Get the default filename for the package configuration
    fn config_file_name(&self) -> String;

    /// Get custom file extensions allowed for config files
    fn allowed_extensions(&self) -> &'static [&'static str] {
        &["txt"] // Default: text files
    }

    /// Get the default restore target directory
    fn get_default_restore_dir(&self) -> Result<PathBuf> {
        // Package configs are typically restored to the current directory
        Ok(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    /// Validate that the package manager is available
    fn validate_package_manager(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>>
    where
        Self: CommandMixin,
    {
        let cmd = self.package_command();
        Box::pin(async move { self.validate_command_exists(cmd).await })
    }
}

/// Generic package plugin that can be used for any package manager
#[allow(dead_code)]
pub struct PackagePlugin<T: PackageCore + CommandMixin> {
    config: Option<StandardConfig>,
    core: T,
}

impl<T: PackageCore + CommandMixin> PackagePlugin<T> {
    /// Create a new package plugin without configuration
    #[allow(dead_code)]
    pub fn new(core: T) -> Self {
        Self { config: None, core }
    }

    /// Create a new package plugin with configuration
    #[allow(dead_code)]
    pub fn with_config(core: T, config: toml::Value) -> Self {
        let (parsed_config, is_valid) = Self::with_config_validation(
            config,
            &format!("{} plugin", core.package_manager_name()),
            &format!("{}_config", core.package_manager_name().to_lowercase()),
            "target_path (string), output_file (string), restore_target_dir (string), hooks (object)",
            &format!(
                "target_path = \"{}\", output_file = \"{}\"",
                core.package_manager_name().to_lowercase(),
                core.config_file_name()
            ),
        );

        // Additional validation for package-specific fields
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
                        "Invalid output file extension for {} config: {}",
                        core.package_manager_name(),
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

    /// Get access to the core implementation
    #[allow(dead_code)]
    pub fn core(&self) -> &T {
        &self.core
    }
}

// Implement mixins for the package plugin
impl<T: PackageCore + CommandMixin> ConfigMixin for PackagePlugin<T> {
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

impl<T: PackageCore + CommandMixin> StandardConfigMixin for PackagePlugin<T> {}

// Delegate CommandMixin to the core
impl<T: PackageCore + CommandMixin> CommandMixin for PackagePlugin<T> {
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

// Implement HooksMixin for the package plugin
impl<T: PackageCore + CommandMixin> crate::plugins::core::mixins::HooksMixin for PackagePlugin<T> {
    fn get_hooks(&self) -> Vec<crate::core::hooks::HookAction> {
        self.get_standard_hooks()
    }
}

// Implement FilesMixin for the package plugin
impl<T: PackageCore + CommandMixin> crate::plugins::core::mixins::FilesMixin for PackagePlugin<T> {}

// The plugin trait implementation gets all the mixin functionality automatically
impl<T: PackageCore + CommandMixin> AllMixins for PackagePlugin<T> {}

#[async_trait]
impl<T: PackageCore + CommandMixin + Send + Sync> Plugin for PackagePlugin<T> {
    fn description(&self) -> &str {
        "Manages package manager configuration and state"
    }

    fn icon(&self) -> &str {
        self.core.icon()
    }

    async fn execute(&self) -> Result<String> {
        self.core.get_package_config().await
    }

    async fn validate(&self) -> Result<()> {
        // Check if the package manager command exists
        self.core.validate_package_manager().await?;
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
        self.get_standard_hooks()
    }

    async fn restore(
        &self,
        snapshot_path: &std::path::Path,
        target_path: &std::path::Path,
        dry_run: bool,
    ) -> Result<Vec<PathBuf>> {
        use tokio::fs;

        let mut restored_files = Vec::new();

        // Find config file in the snapshot
        let config_filename =
            ConfigMixin::get_output_file(self).unwrap_or_else(|| self.core.config_file_name());
        let mut source_config = snapshot_path.join(&config_filename);

        if !source_config.exists() {
            // Try alternative common names
            let alternative_names = [
                &self.core.config_file_name(),
                &format!("{}.txt", self.core.package_manager_name().to_lowercase()),
            ];
            let mut found = false;

            for name in &alternative_names {
                let alt_path = snapshot_path.join(name);
                if alt_path.exists() {
                    source_config = alt_path;
                    info!(
                        "Found {} config file at alternative path: {}",
                        self.core.package_manager_name(),
                        source_config.display()
                    );
                    found = true;
                    break;
                }
            }

            if !found {
                return Ok(restored_files); // No config file found
            }
        }

        // Read the config content
        let config_content = fs::read_to_string(&source_config).await?;

        if dry_run {
            warn!(
                "DRY RUN: Would restore {} configuration to {}",
                self.core.package_manager_name(),
                target_path.display()
            );
            self.core
                .restore_packages(&config_content, target_path, true)
                .await?;
            // For dry run, just indicate we would restore to the target path
            restored_files.push(target_path.to_path_buf());
        } else {
            // Actually restore the packages
            self.core
                .restore_packages(&config_content, target_path, false)
                .await?;

            info!(
                "Restored {} configuration to {}",
                self.core.package_manager_name(),
                target_path.display()
            );

            restored_files.push(target_path.to_path_buf());
        }

        Ok(restored_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    struct MockPackageCore;

    impl PackageCore for MockPackageCore {
        fn package_manager_name(&self) -> &'static str {
            "TestPkg"
        }

        fn package_command(&self) -> &'static str {
            "testpkg"
        }

        fn config_file_name(&self) -> String {
            "testpkg.txt".to_string()
        }

        fn get_package_config(
            &self,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
        {
            Box::pin(async { Ok("package1==1.0.0\npackage2==2.0.0".to_string()) })
        }

        fn restore_packages(
            &self,
            _config_content: &str,
            _target_dir: &std::path::Path,
            _dry_run: bool,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
            Box::pin(async { Ok(()) })
        }
    }

    impl CommandMixin for MockPackageCore {
        fn execute_command(
            &self,
            _cmd: &str,
            _args: &[&str],
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
        {
            Box::pin(async { Ok("command output".to_string()) })
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
    async fn test_package_plugin_creation() {
        let core = MockPackageCore;
        let plugin = PackagePlugin::new(core);

        assert_eq!(plugin.core.package_manager_name(), "TestPkg");
        assert_eq!(plugin.core.package_command(), "testpkg");
        assert_eq!(
            plugin.description(),
            "Manages package manager configuration and state"
        );
    }

    #[tokio::test]
    async fn test_package_plugin_execute() {
        let core = MockPackageCore;
        let plugin = PackagePlugin::new(core);

        let result = plugin.execute().await.unwrap();
        assert_eq!(result, "package1==1.0.0\npackage2==2.0.0");
    }

    #[tokio::test]
    async fn test_package_plugin_restore() {
        let core = MockPackageCore;
        let plugin = PackagePlugin::new(core);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test config file
        let test_config = "package1==1.0.0\npackage2==2.0.0";
        let config_path = snapshot_dir.join("testpkg.txt");
        fs::write(&config_path, test_config).await.unwrap();

        // Test restore
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target_dir);
    }
}

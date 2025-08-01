use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{info, warn};

use crate::core::plugin::Plugin;
use crate::plugins::core::mixins::{
    CommandMixin, ConfigMixin, StandardConfig, StandardConfigMixin,
};

/// Core trait that defines package manager-specific behavior
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
pub struct PackagePlugin<T: PackageCore + CommandMixin> {
    config: Option<StandardConfig>,
    core: T,
}

impl<T: PackageCore + CommandMixin> PackagePlugin<T> {
    /// Create a new package plugin without configuration
    pub fn new(core: T) -> Self {
        Self { config: None, core }
    }

    /// Create a new package plugin with configuration
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

// Implement FilesMixin for the package plugin
impl<T: PackageCore + CommandMixin> crate::plugins::core::mixins::FilesMixin for PackagePlugin<T> {}

// The plugin trait implementation gets all the mixin functionality automatically

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
        // Package plugins don't have hooks by default
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

    #[tokio::test]
    async fn test_package_plugin_restore_dry_run() {
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

        // Test dry run restore
        let result = plugin
            .restore(&snapshot_dir, &target_dir, true)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target_dir);
    }

    #[tokio::test]
    async fn test_package_plugin_restore_alternative_names() {
        let core = MockPackageCore;
        let plugin = PackagePlugin::new(core);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test config file with alternative name
        let test_config = "package1==1.0.0\npackage2==2.0.0";
        let config_path = snapshot_dir.join("testpkg.txt");
        fs::write(&config_path, test_config).await.unwrap();

        // Test restore with alternative filename
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target_dir);
    }

    #[tokio::test]
    async fn test_package_plugin_restore_no_file() {
        let core = MockPackageCore;
        let plugin = PackagePlugin::new(core);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // No config file in snapshot
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 0); // No files restored
    }

    #[tokio::test]
    async fn test_package_plugin_validate() {
        let core = MockPackageCore;
        let plugin = PackagePlugin::new(core);

        let result = plugin.validate().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_package_core_defaults() {
        let core = MockPackageCore;

        assert_eq!(core.package_manager_name(), "TestPkg");
        assert_eq!(core.package_command(), "testpkg");
        assert_eq!(core.config_file_name(), "testpkg.txt");
        assert_eq!(core.icon(), "📦");
        assert_eq!(core.allowed_extensions(), &["txt"]);

        let restore_dir = core.get_default_restore_dir().unwrap();
        assert!(restore_dir.is_dir() || restore_dir == PathBuf::from("."));
    }

    #[test]
    fn test_package_plugin_with_config() {
        let core = MockPackageCore;
        let config_toml = toml::toml! {
            target_path = "custom/path"
            output_file = "custom_config.txt"
            restore_target_dir = "/custom/restore"
        };

        let plugin = PackagePlugin::with_config(core, toml::Value::Table(config_toml));
        assert!(plugin.config.is_some());

        let config = plugin.config.as_ref().unwrap();
        assert_eq!(config.target_path, Some("custom/path".to_string()));
        assert_eq!(config.output_file, Some("custom_config.txt".to_string()));
        assert_eq!(
            config.restore_target_dir,
            Some("/custom/restore".to_string())
        );
    }

    #[test]
    fn test_package_plugin_with_invalid_config() {
        let core = MockPackageCore;
        let config_toml = toml::toml! {
            target_path = "custom/path"
            output_file = "invalid_file.exe" // Invalid extension
        };

        let plugin = PackagePlugin::with_config(core, toml::Value::Table(config_toml));
        assert!(plugin.config.is_some());
        // Config is still created even with invalid extension (warning is logged)
    }

    #[test]
    fn test_config_mixin_methods() {
        let core = MockPackageCore;
        let config_toml = toml::toml! {
            target_path = "test/path"
            output_file = "test.txt"
            restore_target_dir = "/test/restore"
        };

        let plugin = PackagePlugin::with_config(core, toml::Value::Table(config_toml));

        assert_eq!(
            ConfigMixin::get_target_path(&plugin),
            Some("test/path".to_string())
        );
        assert_eq!(
            ConfigMixin::get_output_file(&plugin),
            Some("test.txt".to_string())
        );
        assert_eq!(
            ConfigMixin::get_restore_target_dir(&plugin),
            Some("/test/restore".to_string())
        );
    }

    #[test]
    fn test_command_mixin_delegation() {
        let core = MockPackageCore;
        let plugin = PackagePlugin::new(core);

        assert!(plugin.command_exists("testpkg"));
    }

    #[tokio::test]
    async fn test_command_mixin_async_delegation() {
        let core = MockPackageCore;
        let plugin = PackagePlugin::new(core);

        let result = plugin.execute_command("testpkg", &["--version"]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "command output");

        let validate_result = plugin.validate_command_exists("testpkg").await;
        assert!(validate_result.is_ok());
    }

    #[test]
    fn test_plugin_trait_methods() {
        let core = MockPackageCore;
        let plugin = PackagePlugin::new(core);

        assert_eq!(plugin.icon(), "📦");
        assert_eq!(
            plugin.description(),
            "Manages package manager configuration and state"
        );

        let hooks = plugin.get_hooks();
        assert!(hooks.is_empty());

        let default_restore_dir = plugin.get_default_restore_target_dir().unwrap();
        assert!(default_restore_dir.is_dir() || default_restore_dir == PathBuf::from("."));
    }

    #[tokio::test]
    async fn test_package_core_validate_package_manager() {
        let core = MockPackageCore;
        let result = core.validate_package_manager().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_package_plugin_restore_with_custom_config() {
        let core = MockPackageCore;
        let config_toml = toml::toml! {
            output_file = "custom_packages.txt"
        };
        let plugin = PackagePlugin::with_config(core, toml::Value::Table(config_toml));

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test config file with custom name
        let test_config = "package1==1.0.0\npackage2==2.0.0";
        let config_path = snapshot_dir.join("custom_packages.txt");
        fs::write(&config_path, test_config).await.unwrap();

        // Test restore
        let result = plugin
            .restore(&snapshot_dir, &target_dir, false)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], target_dir);
    }

    // Test custom PackageCore implementation with different defaults
    struct CustomPackageCore;

    impl PackageCore for CustomPackageCore {
        fn package_manager_name(&self) -> &'static str {
            "CustomPkg"
        }

        fn package_command(&self) -> &'static str {
            "custompkg"
        }

        fn config_file_name(&self) -> String {
            "custom_packages.json".to_string()
        }

        fn icon(&self) -> &'static str {
            "🔧"
        }

        fn allowed_extensions(&self) -> &'static [&'static str] {
            &["json", "yml"]
        }

        fn get_default_restore_dir(&self) -> Result<PathBuf> {
            Ok(PathBuf::from("/custom/restore"))
        }

        fn get_package_config(
            &self,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
        {
            Box::pin(async { Ok("{\"packages\": [\"pkg1\", \"pkg2\"]}".to_string()) })
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

    impl CommandMixin for CustomPackageCore {
        fn execute_command(
            &self,
            _cmd: &str,
            _args: &[&str],
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
        {
            Box::pin(async { Ok("custom output".to_string()) })
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

    #[test]
    fn test_custom_package_core() {
        let core = CustomPackageCore;

        assert_eq!(core.package_manager_name(), "CustomPkg");
        assert_eq!(core.package_command(), "custompkg");
        assert_eq!(core.config_file_name(), "custom_packages.json");
        assert_eq!(core.icon(), "🔧");
        assert_eq!(core.allowed_extensions(), &["json", "yml"]);

        let restore_dir = core.get_default_restore_dir().unwrap();
        assert_eq!(restore_dir, PathBuf::from("/custom/restore"));
    }

    #[tokio::test]
    async fn test_custom_package_plugin() {
        let core = CustomPackageCore;
        let plugin = PackagePlugin::new(core);

        assert_eq!(plugin.icon(), "🔧");

        let result = plugin.execute().await.unwrap();
        assert_eq!(result, "{\"packages\": [\"pkg1\", \"pkg2\"]}");

        let default_restore_dir = plugin.get_default_restore_target_dir().unwrap();
        assert_eq!(default_restore_dir, PathBuf::from("/custom/restore"));
    }

    // Test error scenarios
    struct ErrorPackageCore;

    impl PackageCore for ErrorPackageCore {
        fn package_manager_name(&self) -> &'static str {
            "ErrorPkg"
        }

        fn package_command(&self) -> &'static str {
            "errorpkg"
        }

        fn config_file_name(&self) -> String {
            "error.txt".to_string()
        }

        fn get_package_config(
            &self,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
        {
            Box::pin(async { Err(anyhow::anyhow!("Package config error")) })
        }

        fn restore_packages(
            &self,
            _config_content: &str,
            _target_dir: &std::path::Path,
            _dry_run: bool,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
            Box::pin(async { Err(anyhow::anyhow!("Restore error")) })
        }
    }

    impl CommandMixin for ErrorPackageCore {
        fn execute_command(
            &self,
            _cmd: &str,
            _args: &[&str],
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>>
        {
            Box::pin(async { Err(anyhow::anyhow!("Command error")) })
        }

        fn validate_command_exists(
            &self,
            _cmd: &str,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
            Box::pin(async { Err(anyhow::anyhow!("Command not found")) })
        }

        fn command_exists(&self, _cmd: &str) -> bool {
            false
        }
    }

    #[tokio::test]
    async fn test_package_plugin_execute_error() {
        let core = ErrorPackageCore;
        let plugin = PackagePlugin::new(core);

        let result = plugin.execute().await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Package config error"));
    }

    #[tokio::test]
    async fn test_package_plugin_validate_error() {
        let core = ErrorPackageCore;
        let plugin = PackagePlugin::new(core);

        let result = plugin.validate().await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Command not found"));
    }

    #[tokio::test]
    async fn test_package_plugin_restore_error() {
        let core = ErrorPackageCore;
        let plugin = PackagePlugin::new(core);

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&snapshot_dir).await.unwrap();
        fs::create_dir_all(&target_dir).await.unwrap();

        // Create test config file
        let test_config = "package1==1.0.0";
        let config_path = snapshot_dir.join("error.txt");
        fs::write(&config_path, test_config).await.unwrap();

        // Test restore error
        let result = plugin.restore(&snapshot_dir, &target_dir, false).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Restore error"));
    }

    #[test]
    fn test_error_package_core_command_methods() {
        let core = ErrorPackageCore;
        assert!(!core.command_exists("errorpkg"));
    }

    #[tokio::test]
    async fn test_error_package_core_async_methods() {
        let core = ErrorPackageCore;

        let cmd_result = core.execute_command("errorpkg", &[]).await;
        assert!(cmd_result.is_err());

        let validate_result = core.validate_command_exists("errorpkg").await;
        assert!(validate_result.is_err());
    }
}

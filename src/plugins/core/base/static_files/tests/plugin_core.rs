//! Tests for basic StaticFilesPlugin core functionality

#[cfg(test)]
mod tests {
    use super::super::test_utils::{MinimalStaticFilesCore, MockStaticFilesCore};
    use crate::core::plugin::Plugin;
    use crate::plugins::core::base::static_files::StaticFilesPlugin;
    use crate::plugins::core::mixins::files::FilesMixin;

    /// Test basic StaticFilesPlugin creation with MockStaticFilesCore
    /// Verifies plugin can be instantiated and provides correct description
    #[tokio::test]
    async fn test_static_files_plugin_creation() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());
        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );
        assert_eq!(plugin.icon(), "📄");
    }

    /// Test StaticFilesPlugin with MinimalStaticFilesCore
    /// Verifies plugin works with minimal mock implementation
    #[tokio::test]
    async fn test_static_files_plugin_with_minimal_core() {
        let plugin = StaticFilesPlugin::new(MinimalStaticFilesCore);
        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );
        assert_eq!(plugin.icon(), "📁");
    }

    /// Test Plugin trait method implementations
    /// Verifies all required Plugin trait methods work correctly
    #[test]
    fn test_static_files_plugin_trait_methods() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());

        // Test basic trait methods
        assert!(!plugin.description().is_empty());
        assert!(!plugin.icon().is_empty());
        assert_eq!(plugin.get_target_path(), None);
        assert_eq!(plugin.get_output_file(), None);
        assert!(plugin.creates_own_output_files());
        assert_eq!(plugin.get_restore_target_dir(), None);
    }

    /// Test FilesMixin default implementation
    /// Verifies StaticFilesPlugin implements FilesMixin correctly
    #[tokio::test]
    async fn test_static_files_plugin_files_mixin() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());

        // FilesMixin should provide default implementations
        // The actual methods depend on the FilesMixin trait definition
        // This test verifies the mixin is properly implemented
        // Test basic FilesMixin functionality - just verify it compiles and works
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        assert!(plugin.is_dir_accessible(temp_dir.path()).await);
    }

    /// Test CommandMixin default implementation  
    /// Verifies StaticFilesPlugin implements CommandMixin correctly
    #[test]
    fn test_static_files_plugin_command_mixin() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());

        // CommandMixin should provide default implementations
        // The actual methods depend on the CommandMixin trait definition
        // This test verifies the mixin is properly implemented
        // CommandMixin provides description method - test it works
        assert!(!plugin.description().is_empty());
    }

    /// Test default restore target directory functionality
    /// Verifies plugin returns correct default restore target
    #[test]
    fn test_default_restore_target_dir() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());
        let default_dir = plugin.get_default_restore_target_dir().unwrap();
        assert_eq!(default_dir.to_string_lossy(), "/");
    }

    /// Test plugin validation functionality
    /// Verifies plugin validation passes under normal conditions
    #[tokio::test]
    async fn test_static_files_plugin_validate() {
        let plugin = StaticFilesPlugin::new(MockStaticFilesCore::new());
        let result = plugin.validate().await;
        assert!(result.is_ok());
    }

    /// Test plugin with config constructor
    /// Verifies with_config constructor works correctly
    #[test]
    fn test_static_files_plugin_with_config() {
        use crate::config::Config;
        use std::sync::Arc;

        let config = Arc::new(Config::default());
        let plugin = StaticFilesPlugin::with_config(MockStaticFilesCore::new(), config);

        assert_eq!(
            plugin.description(),
            "Copies arbitrary static files and directories based on configuration"
        );
    }
}

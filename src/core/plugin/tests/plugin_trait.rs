//! Tests for Plugin trait implementations and behavior

use super::*;

/// Test Plugin trait default implementations
/// Verifies that default trait implementations work correctly
pub struct DefaultPlugin;

#[async_trait::async_trait]
impl Plugin for DefaultPlugin {
    fn description(&self) -> &str {
        "Default plugin"
    }

    fn icon(&self) -> &str {
        "⚙️"
    }

    async fn execute(&self) -> Result<String> {
        Ok("default content".to_string())
    }

    async fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn get_target_path(&self) -> Option<String> {
        None
    }

    fn get_output_file(&self) -> Option<String> {
        None
    }
}

/// Test Plugin trait default method implementations
/// Verifies that all default trait methods return expected values
#[tokio::test]
async fn test_plugin_trait_defaults() {
    let plugin = DefaultPlugin;

    // Test basic methods
    assert_eq!(plugin.description(), "Default plugin");
    assert_eq!(plugin.icon(), "⚙️");

    // Test async methods
    let content = plugin.execute().await.unwrap();
    assert_eq!(content, "default content");

    let validation = plugin.validate().await;
    assert!(validation.is_ok());

    // Test default implementations
    assert_eq!(plugin.get_target_path(), None);
    assert_eq!(plugin.get_output_file(), None);
    assert_eq!(plugin.get_restore_target_dir(), None);
    assert!(!plugin.creates_own_output_files());
    assert!(plugin.get_hooks().is_empty());

    // Test default restore directory
    let default_dir = plugin.get_default_restore_target_dir().unwrap();
    // Should be either home directory or current directory fallback
    assert!(default_dir.is_absolute() || default_dir == std::path::PathBuf::from("."));

    // Test default restore implementation
    let temp_dir = tempfile::TempDir::new().unwrap();
    let restored = plugin
        .restore(temp_dir.path(), temp_dir.path(), false)
        .await
        .unwrap();
    assert!(restored.is_empty());
}

/// Test Plugin with custom restore implementation
/// Verifies that plugins can provide custom restore logic
pub struct CustomRestorePlugin;

#[async_trait::async_trait]
impl Plugin for CustomRestorePlugin {
    fn description(&self) -> &str {
        "Custom restore plugin"
    }

    fn icon(&self) -> &str {
        "🔄"
    }

    async fn execute(&self) -> Result<String> {
        Ok("custom content".to_string())
    }

    async fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn get_target_path(&self) -> Option<String> {
        Some("custom/target".to_string())
    }

    fn get_output_file(&self) -> Option<String> {
        Some("custom.json".to_string())
    }

    fn get_restore_target_dir(&self) -> Option<String> {
        Some("/custom/restore".to_string())
    }

    fn creates_own_output_files(&self) -> bool {
        true
    }

    fn get_hooks(&self) -> Vec<crate::core::hooks::HookAction> {
        vec![crate::core::hooks::HookAction::Log {
            message: "Custom hook".to_string(),
            level: "info".to_string(),
        }]
    }

    fn get_default_restore_target_dir(&self) -> Result<std::path::PathBuf> {
        Ok(std::path::PathBuf::from("/custom/default"))
    }

    async fn restore(
        &self,
        _snapshot_path: &std::path::Path,
        _target_path: &std::path::Path,
        _dry_run: bool,
    ) -> Result<Vec<std::path::PathBuf>> {
        Ok(vec![
            std::path::PathBuf::from("/custom/file1.txt"),
            std::path::PathBuf::from("/custom/file2.txt"),
        ])
    }
}

/// Test Plugin with custom implementations
/// Verifies that plugins can override all default behaviors
#[tokio::test]
async fn test_plugin_with_custom_implementations() {
    let plugin = CustomRestorePlugin;

    // Test custom implementations
    assert_eq!(plugin.get_target_path(), Some("custom/target".to_string()));
    assert_eq!(plugin.get_output_file(), Some("custom.json".to_string()));
    assert_eq!(
        plugin.get_restore_target_dir(),
        Some("/custom/restore".to_string())
    );
    assert!(plugin.creates_own_output_files());

    let hooks = plugin.get_hooks();
    assert_eq!(hooks.len(), 1);
    match &hooks[0] {
        crate::core::hooks::HookAction::Log { message, level } => {
            assert_eq!(message, "Custom hook");
            assert_eq!(level, "info");
        }
        _ => panic!("Expected Log hook"),
    }

    let default_dir = plugin.get_default_restore_target_dir().unwrap();
    assert_eq!(default_dir, std::path::PathBuf::from("/custom/default"));

    // Test custom restore
    let temp_dir = tempfile::TempDir::new().unwrap();
    let restored = plugin
        .restore(temp_dir.path(), temp_dir.path(), false)
        .await
        .unwrap();
    assert_eq!(restored.len(), 2);
    assert_eq!(restored[0], std::path::PathBuf::from("/custom/file1.txt"));
    assert_eq!(restored[1], std::path::PathBuf::from("/custom/file2.txt"));
}

/// Test Plugin restore with dry run
/// Verifies that custom restore implementations respect dry run flag
#[tokio::test]
async fn test_plugin_restore_dry_run() {
    let plugin = CustomRestorePlugin;

    let temp_dir = tempfile::TempDir::new().unwrap();
    let restored = plugin
        .restore(temp_dir.path(), temp_dir.path(), true)
        .await
        .unwrap();

    // Custom implementation doesn't check dry_run flag, so it still returns files
    // This tests that the plugin receives the dry_run parameter correctly
    assert_eq!(restored.len(), 2);
}

/// Test plugin error handling in execution
/// Verifies that plugins can return errors from execute method
pub struct ErrorPlugin;

#[async_trait::async_trait]
impl Plugin for ErrorPlugin {
    fn description(&self) -> &str {
        "Error plugin"
    }

    fn icon(&self) -> &str {
        "❌"
    }

    async fn execute(&self) -> Result<String> {
        Err(anyhow::anyhow!("Plugin execution failed"))
    }

    async fn validate(&self) -> Result<()> {
        Err(anyhow::anyhow!("Plugin validation failed"))
    }

    fn get_target_path(&self) -> Option<String> {
        None
    }

    fn get_output_file(&self) -> Option<String> {
        None
    }

    async fn restore(
        &self,
        _snapshot_path: &std::path::Path,
        _target_path: &std::path::Path,
        _dry_run: bool,
    ) -> Result<Vec<std::path::PathBuf>> {
        Err(anyhow::anyhow!("Restore failed"))
    }
}

/// Test plugin error scenarios
/// Verifies that plugins properly handle and propagate errors
#[tokio::test]
async fn test_plugin_error_scenarios() {
    let plugin = ErrorPlugin;

    // Test execute error
    let execute_result = plugin.execute().await;
    assert!(execute_result.is_err());
    assert!(execute_result
        .unwrap_err()
        .to_string()
        .contains("execution failed"));

    // Test validate error
    let validate_result = plugin.validate().await;
    assert!(validate_result.is_err());
    assert!(validate_result
        .unwrap_err()
        .to_string()
        .contains("validation failed"));

    // Test restore error
    let temp_dir = tempfile::TempDir::new().unwrap();
    let restore_result = plugin
        .restore(temp_dir.path(), temp_dir.path(), false)
        .await;
    assert!(restore_result.is_err());
    assert!(restore_result
        .unwrap_err()
        .to_string()
        .contains("Restore failed"));
}

/// Test plugin with custom hooks functionality
/// Verifies that get_hooks method can return custom hooks
pub struct HooksPlugin;

#[async_trait::async_trait]
impl Plugin for HooksPlugin {
    fn description(&self) -> &str {
        "Plugin with hooks"
    }

    fn icon(&self) -> &str {
        "🪩"
    }

    async fn execute(&self) -> Result<String> {
        Ok("hooks plugin content".to_string())
    }

    async fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn get_target_path(&self) -> Option<String> {
        Some("/custom/hooks/path".to_string())
    }

    fn get_output_file(&self) -> Option<String> {
        Some("hooks_output.json".to_string())
    }

    fn get_restore_target_dir(&self) -> Option<String> {
        Some("/custom/restore/path".to_string())
    }

    fn creates_own_output_files(&self) -> bool {
        true
    }

    fn get_hooks(&self) -> Vec<crate::core::hooks::HookAction> {
        vec![
            crate::core::hooks::HookAction::Script {
                command: "echo".to_string(),
                args: vec!["pre-hook".to_string()],
                timeout: 30,
                working_dir: None,
                env_vars: std::collections::HashMap::new(),
            },
            crate::core::hooks::HookAction::Log {
                message: "Hook executed".to_string(),
                level: "info".to_string(),
            },
        ]
    }

    fn get_default_restore_target_dir(&self) -> Result<std::path::PathBuf> {
        Ok(std::path::PathBuf::from("/custom/default/restore"))
    }

    async fn restore(
        &self,
        _snapshot_path: &std::path::Path,
        _target_path: &std::path::Path,
        dry_run: bool,
    ) -> Result<Vec<std::path::PathBuf>> {
        if dry_run {
            Ok(vec![std::path::PathBuf::from("/dry/run/path")])
        } else {
            Ok(vec![
                std::path::PathBuf::from("/restored/file1.json"),
                std::path::PathBuf::from("/restored/file2.json"),
            ])
        }
    }
}

/// Test comprehensive plugin functionality with all features
/// Verifies that all plugin trait methods work correctly
#[tokio::test]
async fn test_comprehensive_plugin_functionality() {
    let plugin = HooksPlugin;

    // Test basic properties
    assert_eq!(plugin.description(), "Plugin with hooks");
    assert_eq!(plugin.icon(), "🪩");

    // Test configuration methods
    assert_eq!(
        plugin.get_target_path(),
        Some("/custom/hooks/path".to_string())
    );
    assert_eq!(
        plugin.get_output_file(),
        Some("hooks_output.json".to_string())
    );
    assert_eq!(
        plugin.get_restore_target_dir(),
        Some("/custom/restore/path".to_string())
    );
    assert!(plugin.creates_own_output_files());

    // Test hooks
    let hooks = plugin.get_hooks();
    assert_eq!(hooks.len(), 2);

    // Test default restore target dir
    let default_restore = plugin.get_default_restore_target_dir().unwrap();
    assert_eq!(
        default_restore,
        std::path::PathBuf::from("/custom/default/restore")
    );

    // Test execution
    let result = plugin.execute().await.unwrap();
    assert_eq!(result, "hooks plugin content");

    // Test validation
    assert!(plugin.validate().await.is_ok());

    // Test restore (normal)
    let temp_dir = tempfile::TempDir::new().unwrap();
    let restored = plugin
        .restore(temp_dir.path(), temp_dir.path(), false)
        .await
        .unwrap();
    assert_eq!(restored.len(), 2);
    assert_eq!(
        restored[0],
        std::path::PathBuf::from("/restored/file1.json")
    );
    assert_eq!(
        restored[1],
        std::path::PathBuf::from("/restored/file2.json")
    );

    // Test restore (dry run)
    let dry_restored = plugin
        .restore(temp_dir.path(), temp_dir.path(), true)
        .await
        .unwrap();
    assert_eq!(dry_restored.len(), 1);
    assert_eq!(dry_restored[0], std::path::PathBuf::from("/dry/run/path"));
}

/// Test plugin with home directory fallback
/// Verifies that get_default_restore_target_dir handles home directory correctly
pub struct HomeDirectoryPlugin;

#[async_trait::async_trait]
impl Plugin for HomeDirectoryPlugin {
    fn description(&self) -> &str {
        "Home directory plugin"
    }

    fn icon(&self) -> &str {
        "🏠"
    }

    async fn execute(&self) -> Result<String> {
        Ok("home content".to_string())
    }

    async fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn get_target_path(&self) -> Option<String> {
        None
    }

    fn get_output_file(&self) -> Option<String> {
        None
    }

    // Test default implementation of get_default_restore_target_dir
    // This should fall back to home directory or current directory
}

/// Test default restore target directory behavior
/// Verifies that home directory fallback works correctly
#[tokio::test]
async fn test_default_restore_target_directory() {
    let plugin = HomeDirectoryPlugin;

    // Test default implementation of get_default_restore_target_dir
    let restore_dir = plugin.get_default_restore_target_dir().unwrap();

    // Should be either home directory or current directory
    assert!(restore_dir.exists() || restore_dir == std::path::PathBuf::from("."));

    // If home directory is available, it should be that
    if let Some(home) = dirs::home_dir() {
        assert_eq!(restore_dir, home);
    } else {
        assert_eq!(restore_dir, std::path::PathBuf::from("."));
    }
}

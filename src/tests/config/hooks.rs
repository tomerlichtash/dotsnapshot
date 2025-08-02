use std::path::PathBuf;

use super::test_utils::create_config_with_hooks;

/// Test hook configuration functionality
/// Verifies all hook-related configuration methods work correctly
#[tokio::test]
async fn test_config_hooks_functionality() {
    let config = create_config_with_hooks();

    // Test hook configuration methods
    let hooks_config = config.get_hooks_config();
    assert_eq!(hooks_config.scripts_dir, PathBuf::from("/test/scripts"));

    // Test global hooks
    let global_pre = config.get_global_pre_snapshot_hooks();
    assert_eq!(global_pre.len(), 1);

    let global_post = config.get_global_post_snapshot_hooks();
    assert_eq!(global_post.len(), 1);

    // Test plugin-specific hooks (should be empty for plugins without hooks config)
    let plugin_pre = config.get_plugin_pre_hooks("vscode");
    assert_eq!(plugin_pre.len(), 0);

    let plugin_post = config.get_plugin_post_hooks("vscode");
    assert_eq!(plugin_post.len(), 0);

    // Test plugin-specific hooks for non-existent plugin
    let no_plugin_pre = config.get_plugin_pre_hooks("nonexistent");
    assert_eq!(no_plugin_pre.len(), 0);

    let no_plugin_post = config.get_plugin_post_hooks("nonexistent");
    assert_eq!(no_plugin_post.len(), 0);
}

/// Test plugin configuration retrieval
/// Verifies plugin-specific configuration access works correctly
#[tokio::test]
async fn test_plugin_configuration_retrieval() {
    let config = create_config_with_hooks();

    // Test plugin configuration retrieval
    let vscode_config = config.get_raw_plugin_config("vscode");
    assert!(vscode_config.is_some());

    let homebrew_config = config.get_raw_plugin_config("homebrew");
    assert!(homebrew_config.is_some());

    let nonexistent_config = config.get_raw_plugin_config("nonexistent");
    assert!(nonexistent_config.is_none());
}

/// Test time format configuration
/// Verifies time format settings work correctly
#[tokio::test]
async fn test_time_format_configuration() {
    let config = create_config_with_hooks();

    // Test time format
    let time_format = config.get_time_format();
    assert!(!time_format.is_empty());

    // Test verbose setting (should be false by default for this config)
    assert!(!config.is_verbose_default());
}

use std::path::PathBuf;

use crate::config::Config;

use super::test_utils::{create_config_with_logging_no_verbose, create_minimal_config};

/// Test config default values
/// Verifies that Config::default() returns expected values
#[tokio::test]
async fn test_config_default() {
    let config = Config::default();
    assert_eq!(config.get_output_dir(), PathBuf::from("./snapshots"));
    assert_eq!(config.get_include_plugins(), None);
    assert!(!config.is_verbose_default());
}

/// Test config with minimal settings and edge cases
/// Verifies that missing configurations are handled correctly
#[tokio::test]
async fn test_config_minimal_and_edge_cases() {
    // Test with completely minimal config
    let minimal_config = create_minimal_config();

    // Test default behaviors
    assert_eq!(
        minimal_config.get_output_dir(),
        PathBuf::from("./snapshots")
    );
    assert_eq!(minimal_config.get_include_plugins(), None);
    assert!(!minimal_config.is_verbose_default());

    // Test default time format
    let default_time_format = minimal_config.get_time_format();
    assert_eq!(
        default_time_format,
        "[year]-[month]-[day] [hour]:[minute]:[second]"
    );

    // Test empty hooks (HooksConfig has scripts_dir field, which gets default value)
    let hooks_config = minimal_config.get_hooks_config();
    assert!(hooks_config
        .scripts_dir
        .to_string_lossy()
        .contains("dotsnapshot"));

    let global_pre = minimal_config.get_global_pre_snapshot_hooks();
    assert_eq!(global_pre.len(), 0);

    let global_post = minimal_config.get_global_post_snapshot_hooks();
    assert_eq!(global_post.len(), 0);

    // Test plugin hooks with no configuration
    let plugin_pre = minimal_config.get_plugin_pre_hooks("any_plugin");
    assert_eq!(plugin_pre.len(), 0);

    let plugin_post = minimal_config.get_plugin_post_hooks("any_plugin");
    assert_eq!(plugin_post.len(), 0);

    // Test plugin config retrieval with no plugins configured
    let no_config = minimal_config.get_raw_plugin_config("any_plugin");
    assert!(no_config.is_none());
}

/// Test config with logging but no verbose setting
/// Verifies fallback behavior when verbose is not explicitly set
#[tokio::test]
async fn test_config_logging_no_verbose() {
    // Test with logging config but no verbose setting
    let config_with_logging = create_config_with_logging_no_verbose();

    // Should still return false for verbose when not set
    assert!(!config_with_logging.is_verbose_default());

    // Should use custom time format
    assert_eq!(config_with_logging.get_time_format(), "[hour]:[minute]");
}

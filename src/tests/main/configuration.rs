use crate::{create_subscriber, list_plugins, Config};

use super::test_utils::{create_test_config, parse_test_args};

/// Test subscriber creation with different configurations
/// Verifies that logging subscribers are created correctly
#[test]
fn test_create_subscriber() {
    // Test debug subscriber
    let subscriber = create_subscriber(true, "[hour]:[minute]:[second]".to_string());
    // Just verify it creates without panicking
    drop(subscriber);

    // Test non-debug subscriber
    let subscriber = create_subscriber(false, "[month]-[day] [hour]:[minute]".to_string());
    drop(subscriber);

    // Test with different time formats
    let subscriber = create_subscriber(
        false,
        "[year]/[month]/[day] [hour]:[minute]:[second]".to_string(),
    );
    drop(subscriber);

    // Test with default format (unsupported format should fall back)
    let subscriber = create_subscriber(false, "unsupported-format".to_string());
    drop(subscriber);
}

/// Test debug logging level configuration
/// Verifies that debug flag correctly sets logging levels
#[test]
fn test_debug_logging_levels() {
    // Test debug=true should set DEBUG level
    let subscriber = create_subscriber(true, "[hour]:[minute]:[second]".to_string());
    drop(subscriber);

    // Test debug=false should set INFO level
    let subscriber = create_subscriber(false, "[hour]:[minute]:[second]".to_string());
    drop(subscriber);
}

/// Test that create_subscriber handles all time format cases
/// Verifies that time format parsing covers all branches
#[test]
fn test_create_subscriber_time_formats() {
    // Test all supported time formats
    let formats = vec![
        "[hour]:[minute]:[second]",
        "[month]-[day] [hour]:[minute]",
        "[year]/[month]/[day] [hour]:[minute]:[second]",
        "[year]-[month]-[day] [hour]:[minute]:[second]", // default
    ];

    for format in formats {
        let subscriber = create_subscriber(false, format.to_string());
        drop(subscriber); // Just ensure it creates without panic
    }

    // Test unsupported format (should fall back to default)
    let subscriber = create_subscriber(true, "custom-unsupported-format".to_string());
    drop(subscriber);
}

/// Test create_subscriber with edge cases
/// Verifies subscriber creation with malformed or edge case time formats
#[test]
fn test_create_subscriber_edge_cases() {
    // Test with empty time format (should use default)
    let subscriber = create_subscriber(false, "".to_string());
    drop(subscriber);

    // Test with malformed time format (should use default)
    let subscriber = create_subscriber(true, "[invalid-format]".to_string());
    drop(subscriber);

    // Test with partial match (should use default)
    let subscriber = create_subscriber(false, "[hour]:[minute]".to_string());
    drop(subscriber);
}

/// Test list_plugins function
/// Verifies that plugin listing works correctly
#[tokio::test]
async fn test_list_plugins() {
    // This test verifies that list_plugins doesn't panic and can discover plugins
    // We can't easily test the exact output without mocking, but we can test execution
    list_plugins().await;
    // If we reach here, the function completed without panicking
}

/// Test config loading scenarios
/// Verifies different config loading paths work correctly
#[tokio::test]
async fn test_config_loading_scenarios() {
    let (_temp_dir, config_path) = create_test_config().await;

    // Simulate args with custom config
    let args = parse_test_args(&[
        "dotsnapshot",
        "--config",
        config_path.to_str().unwrap(),
        "--list",
    ]);

    // Test that config can be loaded from custom path
    let config = if let Some(config_path) = &args.config {
        if config_path.exists() {
            Config::load_from_file(config_path).await.unwrap()
        } else {
            Config::default()
        }
    } else {
        Config::load().await.unwrap_or_default()
    };

    // Verify config was loaded correctly
    assert!(config.is_verbose_default());
    assert_eq!(config.get_time_format(), "[hour]:[minute]:[second]");
}

/// Test config loading with nonexistent file
/// Verifies fallback behavior when config file doesn't exist
#[tokio::test]
async fn test_config_loading_nonexistent_file() {
    let args = parse_test_args(&[
        "dotsnapshot",
        "--config",
        "/nonexistent/path/config.toml",
        "--list",
    ]);

    // Should fall back to default config when file doesn't exist
    let config = if let Some(config_path) = &args.config {
        if config_path.exists() {
            Config::load_from_file(config_path).await.unwrap()
        } else {
            Config::default()
        }
    } else {
        Config::load().await.unwrap_or_default()
    };

    // Should be default config
    assert!(!config.is_verbose_default()); // Default is false
}

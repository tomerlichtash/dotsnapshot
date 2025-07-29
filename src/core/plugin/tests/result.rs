//! Tests for PluginResult creation and serialization

use super::*;

/// Test PluginResult creation and serialization
/// Verifies that PluginResult can be created and serialized correctly
#[test]
fn test_plugin_result_creation_and_serialization() {
    let result = PluginResult {
        plugin_name: "test_plugin".to_string(),
        content: "test content".to_string(),
        checksum: "abc123".to_string(),
        success: true,
        error_message: None,
    };

    assert_eq!(result.plugin_name, "test_plugin");
    assert_eq!(result.content, "test content");
    assert_eq!(result.checksum, "abc123");
    assert!(result.success);
    assert!(result.error_message.is_none());

    // Test serialization
    let serialized = serde_json::to_string(&result).unwrap();
    assert!(serialized.contains("test_plugin"));
    assert!(serialized.contains("test content"));
    assert!(serialized.contains("abc123"));
}

/// Test PluginResult creation with error
/// Verifies that PluginResult handles error cases correctly
#[test]
fn test_plugin_result_with_error() {
    let result = PluginResult {
        plugin_name: "failing_plugin".to_string(),
        content: String::new(),
        checksum: String::new(),
        success: false,
        error_message: Some("Plugin execution failed".to_string()),
    };

    assert_eq!(result.plugin_name, "failing_plugin");
    assert!(!result.success);
    assert_eq!(
        result.error_message.as_ref().unwrap(),
        "Plugin execution failed"
    );
}

/// Test PluginResult deserialization
/// Verifies that PluginResult can be deserialized from JSON
#[test]
fn test_plugin_result_deserialization() {
    let json = r#"
    {
        "plugin_name": "test_plugin",
        "content": "test content",
        "checksum": "abc123",
        "success": true,
        "error_message": null
    }"#;

    let result: PluginResult = serde_json::from_str(json).unwrap();
    assert_eq!(result.plugin_name, "test_plugin");
    assert_eq!(result.content, "test content");
    assert_eq!(result.checksum, "abc123");
    assert!(result.success);
    assert!(result.error_message.is_none());
}

/// Test plugin result error handling comprehensive scenarios
/// Verifies PluginResult creation with various error conditions
#[test]
fn test_plugin_result_comprehensive_error_scenarios() {
    // Test with very long error message
    let long_error = "a".repeat(1000);
    let result_with_long_error = PluginResult {
        plugin_name: "test_plugin".to_string(),
        content: "".to_string(),
        checksum: "abc123".to_string(),
        success: false,
        error_message: Some(long_error.clone()),
    };

    assert!(!result_with_long_error.success);
    assert_eq!(
        result_with_long_error.error_message.as_ref().unwrap().len(),
        1000
    );

    // Test with special characters in error message
    let special_error =
        "Error with special chars: \n\t\"quotes\" and 'apostrophes' and unicode: 🚨";
    let result_with_special_error = PluginResult {
        plugin_name: "special_plugin".to_string(),
        content: "partial content".to_string(),
        checksum: "def456".to_string(),
        success: false,
        error_message: Some(special_error.to_string()),
    };

    assert!(!result_with_special_error.success);
    assert!(result_with_special_error
        .error_message
        .as_ref()
        .unwrap()
        .contains("🚨"));

    // Test serialization of complex error scenarios
    let json = serde_json::to_string(&result_with_special_error).unwrap();
    assert!(json.contains("special_plugin"));
    assert!(json.contains("partial content"));

    // Test deserialization
    let deserialized: PluginResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.plugin_name, "special_plugin");
    assert!(!deserialized.success);
    assert!(deserialized.error_message.as_ref().unwrap().contains("🚨"));
}

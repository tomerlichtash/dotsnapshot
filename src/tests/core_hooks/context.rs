use std::path::PathBuf;

use crate::core::hooks::{HookContext, HooksConfig};

/// Test hook context interpolation with all variables
/// Verifies that context variables are properly substituted in templates
#[test]
fn test_hook_context_interpolation() {
    let hooks_config = HooksConfig::default();
    let mut context = HookContext::new(
        "test_snapshot".to_string(),
        PathBuf::from("/tmp/snapshots/test"),
        hooks_config,
    );
    context = context
        .with_plugin("homebrew".to_string())
        .with_file_count(42)
        .with_variable("custom_var".to_string(), "custom_value".to_string());

    let template = "Snapshot: {snapshot_name}, Plugin: {plugin_name}, Files: {file_count}, Custom: {custom_var}";
    let result = context.interpolate(template);

    assert_eq!(
        result,
        "Snapshot: test_snapshot, Plugin: homebrew, Files: 42, Custom: custom_value"
    );
}

/// Test hook context partial interpolation with missing variables
/// Verifies that missing variables are left unchanged in the template
#[test]
fn test_hook_context_partial_interpolation() {
    let hooks_config = HooksConfig::default();
    let context = HookContext::new(
        "test_snapshot".to_string(),
        PathBuf::from("/tmp/snapshots/test"),
        hooks_config,
    );

    // Test without plugin context (should leave {plugin_name} as-is)
    let template = "Snapshot: {snapshot_name}, Plugin: {plugin_name}, Unknown: {unknown_var}";
    let result = context.interpolate(template);

    assert_eq!(
        result,
        "Snapshot: test_snapshot, Plugin: {plugin_name}, Unknown: {unknown_var}"
    );
}

/// Test hook context builder pattern functionality
/// Verifies that context can be built incrementally with method chaining
#[test]
fn test_hook_context_builder() {
    let hooks_config = HooksConfig::default();
    let context = HookContext::new(
        "test_snapshot".to_string(),
        PathBuf::from("/tmp/snapshots/test"),
        hooks_config,
    )
    .with_plugin("test_plugin".to_string())
    .with_file_count(10)
    .with_variable("key".to_string(), "value".to_string());

    let template = "{snapshot_name}-{plugin_name}-{file_count}-{key}";
    let result = context.interpolate(template);

    assert_eq!(result, "test_snapshot-test_plugin-10-value");
}

use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

use crate::core::hooks::{
    DefaultHookExecutor, HookAction, HookContext, HookExecutor, HookType, HooksConfig,
};

use super::test_utils::create_test_script;

/// Test hook type display formatting
/// Verifies that hook types are displayed with correct string representation
#[test]
fn test_hook_type_display() {
    assert_eq!(HookType::PreSnapshot.to_string(), "pre-snapshot");
    assert_eq!(HookType::PostSnapshot.to_string(), "post-snapshot");
    assert_eq!(HookType::PrePlugin.to_string(), "pre-plugin");
    assert_eq!(HookType::PostPlugin.to_string(), "post-plugin");
}

/// Test hook action display formatting for different action types
/// Verifies that hook actions are displayed with appropriate string representations
#[test]
fn test_hook_action_display() {
    let script_action = HookAction::Script {
        command: "test-script.sh".to_string(),
        args: vec!["arg1".to_string(), "arg2".to_string()],
        timeout: 30,
        working_dir: None,
        env_vars: HashMap::new(),
    };
    assert_eq!(script_action.to_string(), "script: test-script.sh");

    let log_action = HookAction::Log {
        message:
            "Test log message that is very long and should be truncated at fifty characters or so"
                .to_string(),
        level: "info".to_string(),
    };
    assert_eq!(
        log_action.to_string(),
        "log: \"Test log message that is very long and should be t\""
    );

    let backup_action = HookAction::Backup {
        path: PathBuf::from("/source"),
        destination: PathBuf::from("/dest"),
    };
    assert_eq!(backup_action.to_string(), "backup: /source → /dest");
}

/// Test hook action display formatting for cleanup action variations
/// Verifies that cleanup actions display correctly with different patterns
#[test]
fn test_hook_action_display_cleanup_variations() {
    let cleanup_action = HookAction::Cleanup {
        patterns: vec!["*.tmp".to_string(), "*.log".to_string()],
        directories: vec![PathBuf::from("/tmp")],
        temp_files: true,
    };

    let display_str = cleanup_action.to_string();
    assert!(display_str.starts_with("cleanup:"));
    assert!(display_str.contains("*.tmp"));
}

/// Test hook action display formatting for notify actions
/// Verifies that notify actions display correctly with and without titles
#[test]
fn test_hook_action_display_notify() {
    let notify_with_title = HookAction::Notify {
        message: "Test notification".to_string(),
        title: Some("Test Title".to_string()),
    };
    assert_eq!(
        notify_with_title.to_string(),
        "notify: \"Test notification\""
    );

    let notify_without_title = HookAction::Notify {
        message: "Simple notification".to_string(),
        title: None,
    };
    assert_eq!(
        notify_without_title.to_string(),
        "notify: \"Simple notification\""
    );
}

/// Test hook action validation for log and notify actions
/// Verifies that action validation correctly identifies valid and invalid actions
#[tokio::test]
async fn test_hook_action_validation() {
    let temp_dir = TempDir::new().unwrap();
    let hooks_config = HooksConfig {
        scripts_dir: temp_dir.path().to_path_buf(),
    };
    let context = HookContext::new("test".to_string(), PathBuf::from("/tmp"), hooks_config);
    let executor = DefaultHookExecutor;

    // Valid log action
    let valid_log = HookAction::Log {
        message: "Test message".to_string(),
        level: "info".to_string(),
    };
    assert!(executor.validate(&valid_log, &context).is_ok());

    // Invalid log action (bad level)
    let invalid_log = HookAction::Log {
        message: "Test message".to_string(),
        level: "invalid_level".to_string(),
    };
    assert!(executor.validate(&invalid_log, &context).is_err());

    // Valid notify action
    let valid_notify = HookAction::Notify {
        message: "Test notification".to_string(),
        title: Some("Test Title".to_string()),
    };
    assert!(executor.validate(&valid_notify, &context).is_ok());

    // Invalid notify action (empty message)
    let invalid_notify = HookAction::Notify {
        message: "".to_string(),
        title: None,
    };
    assert!(executor.validate(&invalid_notify, &context).is_err());
}

/// Test script action validation with existing and non-existing scripts
/// Verifies that script validation correctly checks for script existence
#[tokio::test]
async fn test_script_action_validation() {
    let temp_dir = TempDir::new().unwrap();
    let hooks_config = HooksConfig {
        scripts_dir: temp_dir.path().to_path_buf(),
    };
    let context = HookContext::new("test".to_string(), PathBuf::from("/tmp"), hooks_config);
    let executor = DefaultHookExecutor;

    // Create a test script
    create_test_script(&temp_dir, "valid-script.sh", "#!/bin/bash\necho 'test'").await;

    // Valid script action (script exists)
    let valid_script = HookAction::Script {
        command: "valid-script.sh".to_string(),
        args: vec![],
        timeout: 30,
        working_dir: None,
        env_vars: HashMap::new(),
    };
    assert!(executor.validate(&valid_script, &context).is_ok());

    // Invalid script action (script doesn't exist)
    let invalid_script = HookAction::Script {
        command: "nonexistent-script.sh".to_string(),
        args: vec![],
        timeout: 30,
        working_dir: None,
        env_vars: HashMap::new(),
    };
    assert!(executor.validate(&invalid_script, &context).is_err());
}

/// Test backup action validation with valid and invalid paths
/// Verifies that backup validation correctly checks path requirements
#[tokio::test]
async fn test_backup_action_validation() {
    let temp_dir = TempDir::new().unwrap();
    let hooks_config = HooksConfig {
        scripts_dir: temp_dir.path().to_path_buf(),
    };
    let context = HookContext::new("test".to_string(), PathBuf::from("/tmp"), hooks_config);
    let executor = DefaultHookExecutor;

    // Create a test source file
    let source_file = temp_dir.path().join("source.txt");
    tokio::fs::write(&source_file, "test content")
        .await
        .unwrap();

    // Valid backup action
    let valid_backup = HookAction::Backup {
        path: source_file.clone(),
        destination: temp_dir.path().join("backup.txt"),
    };
    assert!(executor.validate(&valid_backup, &context).is_ok());
}

/// Test cleanup action validation for different cleanup scenarios
/// Verifies that cleanup validation handles patterns and directories correctly
#[tokio::test]
async fn test_cleanup_action_validation() {
    let temp_dir = TempDir::new().unwrap();
    let hooks_config = HooksConfig {
        scripts_dir: temp_dir.path().to_path_buf(),
    };
    let context = HookContext::new("test".to_string(), PathBuf::from("/tmp"), hooks_config);
    let executor = DefaultHookExecutor;

    // Valid cleanup action
    let valid_cleanup = HookAction::Cleanup {
        patterns: vec!["*.tmp".to_string()],
        directories: vec![temp_dir.path().to_path_buf()],
        temp_files: false,
    };
    assert!(executor.validate(&valid_cleanup, &context).is_ok());
}

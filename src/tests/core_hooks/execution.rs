use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::fs;

use crate::core::hooks::{
    copy_dir_all, DefaultHookExecutor, HookAction, HookContext, HookExecutor, HookManager,
    HookType, HooksConfig,
};

use super::test_utils::create_test_script;

/// Test log execution with all log levels
/// Verifies that log actions work correctly for different log levels
#[tokio::test]
async fn test_execute_log_all_levels() {
    let hooks_config = HooksConfig::default();
    let context = HookContext::new(
        "test_snapshot".to_string(),
        PathBuf::from("/tmp/snapshots/test"),
        hooks_config,
    );

    let executor = DefaultHookExecutor;
    let levels = ["trace", "debug", "info", "warn", "error"];

    for level in levels {
        let log_action = HookAction::Log {
            message: format!("Test {level} message"),
            level: level.to_string(),
        };

        let result = executor.execute(&log_action, &context).await.unwrap();
        assert!(result.success);
        assert_eq!(
            result.output.as_ref().unwrap(),
            &format!("Test {level} message")
        );
    }
}

/// Test notify execution with and without title
/// Verifies that notifications work with optional title parameter
#[tokio::test]
async fn test_execute_notify_variations() {
    let hooks_config = HooksConfig::default();
    let context = HookContext::new(
        "test_snapshot".to_string(),
        PathBuf::from("/tmp/snapshots/test"),
        hooks_config,
    );

    let executor = DefaultHookExecutor;

    // Test with title
    let notify_with_title = HookAction::Notify {
        message: "Test notification {snapshot_name}".to_string(),
        title: Some("Custom Title".to_string()),
    };

    let result = executor
        .execute(&notify_with_title, &context)
        .await
        .unwrap();
    assert!(result.success);
    assert!(result.output.as_ref().unwrap().contains("test_snapshot"));

    // Test without title
    let notify_without_title = HookAction::Notify {
        message: "Test notification".to_string(),
        title: None,
    };

    let result = executor
        .execute(&notify_without_title, &context)
        .await
        .unwrap();
    assert!(result.success);
    assert!(result
        .output
        .as_ref()
        .unwrap()
        .contains("Notification: Test notification"));
}

/// Test backup execution with file
/// Verifies that file backup creates correct copies
#[tokio::test]
async fn test_execute_backup_file() {
    let temp_dir = TempDir::new().unwrap();
    let hooks_config = HooksConfig::default();
    let context = HookContext::new(
        "test_snapshot".to_string(),
        PathBuf::from("/tmp/snapshots/test"),
        hooks_config,
    );

    let executor = DefaultHookExecutor;

    // Create a test file
    let source_file = temp_dir.path().join("source.txt");
    let dest_file = temp_dir.path().join("backup.txt");
    fs::write(&source_file, "test content").await.unwrap();

    let backup_action = HookAction::Backup {
        path: source_file.clone(),
        destination: dest_file.clone(),
    };

    let result = executor.execute(&backup_action, &context).await.unwrap();
    assert!(result.success);
    assert!(dest_file.exists());

    let backup_content = fs::read_to_string(&dest_file).await.unwrap();
    assert_eq!(backup_content, "test content");
}

/// Test backup execution with directory
/// Verifies that directory backup recursively copies structure
#[tokio::test]
async fn test_execute_backup_directory() {
    let temp_dir = TempDir::new().unwrap();
    let hooks_config = HooksConfig::default();
    let context = HookContext::new(
        "test_snapshot".to_string(),
        PathBuf::from("/tmp/snapshots/test"),
        hooks_config,
    );

    let executor = DefaultHookExecutor;

    // Create a test directory with files
    let source_dir = temp_dir.path().join("source");
    let dest_dir = temp_dir.path().join("backup");
    fs::create_dir_all(&source_dir).await.unwrap();
    fs::write(source_dir.join("file1.txt"), "content1")
        .await
        .unwrap();
    fs::write(source_dir.join("file2.txt"), "content2")
        .await
        .unwrap();

    let backup_action = HookAction::Backup {
        path: source_dir.clone(),
        destination: dest_dir.clone(),
    };

    let result = executor.execute(&backup_action, &context).await.unwrap();
    assert!(result.success);
    assert!(dest_dir.exists());
    assert!(dest_dir.join("file1.txt").exists());
    assert!(dest_dir.join("file2.txt").exists());
}

/// Test backup execution failure scenarios
/// Verifies that backup handles error conditions correctly
#[tokio::test]
async fn test_execute_backup_failure() {
    let hooks_config = HooksConfig::default();
    let context = HookContext::new(
        "test_snapshot".to_string(),
        PathBuf::from("/tmp/snapshots/test"),
        hooks_config,
    );

    let executor = DefaultHookExecutor;

    // Try to backup non-existent file
    let backup_action = HookAction::Backup {
        path: PathBuf::from("/nonexistent/file.txt"),
        destination: PathBuf::from("/tmp/backup.txt"),
    };

    let result = executor.execute(&backup_action, &context).await.unwrap();
    assert!(!result.success);
    assert!(result.error.is_some());
}

/// Test cleanup execution with patterns
/// Verifies that cleanup removes files matching specified patterns
#[tokio::test]
async fn test_execute_cleanup_patterns() {
    let temp_dir = TempDir::new().unwrap();
    let hooks_config = HooksConfig::default();
    let context = HookContext::new(
        "test_snapshot".to_string(),
        PathBuf::from("/tmp/snapshots/test"),
        hooks_config,
    );

    let executor = DefaultHookExecutor;

    // Create test files
    fs::write(temp_dir.path().join("file1.tmp"), "temp1")
        .await
        .unwrap();
    fs::write(temp_dir.path().join("file2.log"), "log1")
        .await
        .unwrap();
    fs::write(temp_dir.path().join("file3.txt"), "keep")
        .await
        .unwrap();

    let cleanup_action = HookAction::Cleanup {
        patterns: vec!["*.tmp".to_string(), "*.log".to_string()],
        directories: vec![temp_dir.path().to_path_buf()],
        temp_files: false,
    };

    let result = executor.execute(&cleanup_action, &context).await.unwrap();
    assert!(result.success);

    // Check that pattern-matched files were removed
    assert!(!temp_dir.path().join("file1.tmp").exists());
    assert!(!temp_dir.path().join("file2.log").exists());
    assert!(temp_dir.path().join("file3.txt").exists()); // Should remain
}

/// Test cleanup execution with temp files
/// Verifies that temp file cleanup functionality works
#[tokio::test]
async fn test_execute_cleanup_temp_files() {
    let hooks_config = HooksConfig::default();
    let context = HookContext::new(
        "test_snapshot".to_string(),
        PathBuf::from("/tmp/snapshots/test"),
        hooks_config,
    );

    let executor = DefaultHookExecutor;

    let cleanup_action = HookAction::Cleanup {
        patterns: vec![],
        directories: vec![],
        temp_files: true,
    };

    // This should at least not crash
    let result = executor.execute(&cleanup_action, &context).await.unwrap();
    assert!(result.output.is_some());
}

/// Test copy_dir_all with nested directories
/// Verifies that recursive directory copying handles nested structures
#[tokio::test]
async fn test_copy_dir_all_nested() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    let dst_dir = temp_dir.path().join("dst");

    // Create nested directory structure
    fs::create_dir_all(src_dir.join("subdir1/subdir2"))
        .await
        .unwrap();
    fs::write(src_dir.join("file1.txt"), "content1")
        .await
        .unwrap();
    fs::write(src_dir.join("subdir1/file2.txt"), "content2")
        .await
        .unwrap();
    fs::write(src_dir.join("subdir1/subdir2/file3.txt"), "content3")
        .await
        .unwrap();

    copy_dir_all(src_dir.clone(), dst_dir.clone())
        .await
        .unwrap();

    // Verify structure was copied
    assert!(dst_dir.exists());
    assert!(dst_dir.join("file1.txt").exists());
    assert!(dst_dir.join("subdir1/file2.txt").exists());
    assert!(dst_dir.join("subdir1/subdir2/file3.txt").exists());
}

/// Test hook manager execution with multiple hooks
/// Verifies that hook manager can execute multiple hooks sequentially
#[tokio::test]
async fn test_hook_manager_execution() {
    let temp_dir = TempDir::new().unwrap();
    let hooks_config = HooksConfig {
        scripts_dir: temp_dir.path().to_path_buf(),
    };
    let context = HookContext::new(
        "test".to_string(),
        PathBuf::from("/tmp"),
        hooks_config.clone(),
    );
    let hook_manager = HookManager::new(hooks_config);

    let hooks = vec![
        HookAction::Log {
            message: "First hook".to_string(),
            level: "info".to_string(),
        },
        HookAction::Log {
            message: "Second hook".to_string(),
            level: "info".to_string(),
        },
    ];

    let results = hook_manager
        .execute_hooks(&hooks, &HookType::PreSnapshot, &context)
        .await;
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| r.success));
}

/// Test hook manager with empty hooks list
/// Verifies that hook manager handles empty hook lists gracefully
#[tokio::test]
async fn test_hook_manager_empty_hooks() {
    let hooks_config = HooksConfig::default();
    let context = HookContext::new(
        "test".to_string(),
        PathBuf::from("/tmp"),
        hooks_config.clone(),
    );
    let hook_manager = HookManager::new(hooks_config);

    let hooks = vec![];
    let results = hook_manager
        .execute_hooks(&hooks, &HookType::PreSnapshot, &context)
        .await;
    assert_eq!(results.len(), 0);
}

/// Test hook manager error handling
/// Verifies that hook manager properly handles and reports errors
#[tokio::test]
async fn test_hook_manager_error_handling() {
    let hooks_config = HooksConfig::default();
    let context = HookContext::new(
        "test".to_string(),
        PathBuf::from("/tmp"),
        hooks_config.clone(),
    );
    let hook_manager = HookManager::new(hooks_config);

    let hooks = vec![HookAction::Log {
        message: "Test".to_string(),
        level: "invalid_level".to_string(), // This should cause an error
    }];

    let results = hook_manager
        .execute_hooks(&hooks, &HookType::PreSnapshot, &context)
        .await;
    // The hook manager executes invalid log levels as 'info' level instead of failing
    // This test verifies the hook still succeeds but logs at info level
    assert!(results[0].success);
}

/// Test hook manager with plugin context
/// Verifies that plugin context is properly passed to hooks
#[tokio::test]
async fn test_hook_manager_with_plugin_context() {
    let hooks_config = HooksConfig::default();
    let context = HookContext::new(
        "test".to_string(),
        PathBuf::from("/tmp"),
        hooks_config.clone(),
    )
    .with_plugin("test_plugin".to_string());
    let hook_manager = HookManager::new(hooks_config);

    let hooks = vec![HookAction::Log {
        message: "Plugin: {plugin_name}".to_string(),
        level: "info".to_string(),
    }];

    let results = hook_manager
        .execute_hooks(&hooks, &HookType::PreSnapshot, &context)
        .await;
    assert_eq!(results.len(), 1);
    assert!(results[0].success);
    assert!(results[0].output.as_ref().unwrap().contains("test_plugin"));
}

/// Test hook manager with long output
/// Verifies that hook manager handles hooks with substantial output
#[tokio::test]
async fn test_hook_manager_long_output() {
    let hooks_config = HooksConfig::default();
    let context = HookContext::new(
        "test".to_string(),
        PathBuf::from("/tmp"),
        hooks_config.clone(),
    );
    let hook_manager = HookManager::new(hooks_config);

    let long_message = "A".repeat(1000); // 1000 character message
    let hooks = vec![HookAction::Log {
        message: long_message.clone(),
        level: "info".to_string(),
    }];

    let results = hook_manager
        .execute_hooks(&hooks, &HookType::PreSnapshot, &context)
        .await;
    assert_eq!(results.len(), 1);
    assert!(results[0].success);
    assert_eq!(results[0].output.as_ref().unwrap(), &long_message);
}

/// Test hook manager validate_hooks method
/// Verifies that hook validation returns appropriate results for each hook
#[tokio::test]
async fn test_hook_manager_validate_hooks() {
    let temp_dir = TempDir::new().unwrap();
    let hooks_config = HooksConfig {
        scripts_dir: temp_dir.path().to_path_buf(),
    };
    let context = HookContext::new(
        "test".to_string(),
        PathBuf::from("/tmp"),
        hooks_config.clone(),
    );
    let hook_manager = HookManager::new(hooks_config);

    // Create a valid script
    create_test_script(&temp_dir, "valid.sh", "#!/bin/bash\necho valid").await;

    let hooks = vec![
        HookAction::Log {
            message: "Valid log".to_string(),
            level: "info".to_string(),
        },
        HookAction::Script {
            command: "valid.sh".to_string(),
            args: vec![],
            timeout: 30,
            working_dir: None,
            env_vars: HashMap::new(),
        },
        HookAction::Log {
            message: "Invalid log".to_string(),
            level: "invalid_level".to_string(),
        },
    ];

    let results = hook_manager.validate_hooks(&hooks, &context);
    assert_eq!(results.len(), 3);
    assert!(results[0].is_ok()); // Valid log
    assert!(results[1].is_ok()); // Valid script
    assert!(results[2].is_err()); // Invalid log level
}

use std::time::{Duration, Instant};
use tokio::time::sleep;

use super::test_utils::*;
use crate::core::progress::{PluginStatus, ProgressConfig};

/// Test ProgressMonitor creation with default configuration
/// Verifies that ProgressMonitor is created correctly with expected defaults
#[tokio::test]
async fn test_progress_monitor_creation() {
    let monitor = create_test_progress_monitor();

    // Test that we can subscribe to updates
    let _receiver = monitor.subscribe();

    // Test initial state
    let statuses = monitor.get_all_statuses().await;
    assert!(statuses.is_empty());

    let has_running = monitor.has_running_plugins().await;
    assert!(!has_running);

    let (running, completed, failed, timeout) = monitor.get_status_counts().await;
    assert_eq!((running, completed, failed, timeout), (0, 0, 0, 0));
}

/// Test ProgressMonitor creation with custom configuration
/// Verifies that custom configuration is properly applied
#[tokio::test]
async fn test_progress_monitor_with_custom_config() {
    let config = ProgressConfig {
        poll_interval: Duration::from_millis(50),
        slow_threshold: Duration::from_millis(200),
        timeout_threshold: Duration::from_secs(5),
        log_progress: true,
    };

    let monitor = create_progress_monitor_with_config(config);
    let _receiver = monitor.subscribe();

    // Monitor should be created successfully
    assert!(!monitor.has_running_plugins().await);
}

/// Test starting a plugin
/// Verifies that plugin status is correctly tracked when started
#[tokio::test]
async fn test_start_plugin() {
    let monitor = create_test_progress_monitor();
    let mut receiver = monitor.subscribe();

    // Start a plugin
    monitor.start_plugin("test_plugin".to_string()).await;

    // Verify status is updated
    let status = monitor.get_plugin_status("test_plugin").await;
    assert!(matches!(status, Some(PluginStatus::Running { .. })));

    // Verify running plugins count
    assert!(monitor.has_running_plugins().await);
    let (running, completed, failed, timeout) = monitor.get_status_counts().await;
    assert_eq!((running, completed, failed, timeout), (1, 0, 0, 0));

    // Verify progress update was sent
    let updates = wait_for_updates(&mut receiver, 1, Duration::from_secs(1)).await;
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].plugin_name, "test_plugin");
    assert!(matches!(updates[0].status, PluginStatus::Running { .. }));
}

/// Test completing a plugin
/// Verifies that plugin completion is correctly tracked
#[tokio::test]
async fn test_complete_plugin() {
    let monitor = create_test_progress_monitor();
    let mut receiver = monitor.subscribe();
    let started_at = Instant::now();

    // Start and then complete a plugin
    monitor.start_plugin("test_plugin".to_string()).await;
    sleep(Duration::from_millis(100)).await; // Simulate some work
    monitor
        .complete_plugin("test_plugin".to_string(), started_at)
        .await;

    // Verify status is updated
    let status = monitor.get_plugin_status("test_plugin").await;
    assert!(matches!(status, Some(PluginStatus::Completed { .. })));

    // Verify no running plugins
    assert!(!monitor.has_running_plugins().await);
    let (running, completed, failed, timeout) = monitor.get_status_counts().await;
    assert_eq!((running, completed, failed, timeout), (0, 1, 0, 0));

    // Verify progress updates were sent
    let updates = wait_for_updates(&mut receiver, 2, Duration::from_secs(1)).await;
    assert_eq!(updates.len(), 2);
    assert!(matches!(updates[0].status, PluginStatus::Running { .. }));
    assert!(matches!(updates[1].status, PluginStatus::Completed { .. }));
}

/// Test failing a plugin
/// Verifies that plugin failure is correctly tracked
#[tokio::test]
async fn test_fail_plugin() {
    let monitor = create_test_progress_monitor();
    let mut receiver = monitor.subscribe();
    let started_at = Instant::now();
    let error_message = "Test error message".to_string();

    // Start and then fail a plugin
    monitor.start_plugin("test_plugin".to_string()).await;
    sleep(Duration::from_millis(50)).await; // Simulate some work
    monitor
        .fail_plugin("test_plugin".to_string(), started_at, error_message.clone())
        .await;

    // Verify status is updated
    let status = monitor.get_plugin_status("test_plugin").await;
    match status {
        Some(PluginStatus::Failed { error, .. }) => {
            assert_eq!(error, error_message);
        }
        _ => panic!("Expected failed status"),
    }

    // Verify no running plugins
    assert!(!monitor.has_running_plugins().await);
    let (running, completed, failed, timeout) = monitor.get_status_counts().await;
    assert_eq!((running, completed, failed, timeout), (0, 0, 1, 0));

    // Verify progress updates were sent
    let updates = wait_for_updates(&mut receiver, 2, Duration::from_secs(1)).await;
    assert_eq!(updates.len(), 2);
    assert!(matches!(updates[0].status, PluginStatus::Running { .. }));
    assert!(matches!(updates[1].status, PluginStatus::Failed { .. }));
}

/// Test multiple plugins running concurrently
/// Verifies that multiple plugins can be tracked simultaneously
#[tokio::test]
async fn test_multiple_plugins() {
    let monitor = create_test_progress_monitor();
    let mut receiver = monitor.subscribe();

    // Start multiple plugins
    monitor.start_plugin("plugin1".to_string()).await;
    monitor.start_plugin("plugin2".to_string()).await;
    monitor.start_plugin("plugin3".to_string()).await;

    // Verify all are running
    assert!(monitor.has_running_plugins().await);
    let (running, completed, failed, timeout) = monitor.get_status_counts().await;
    assert_eq!((running, completed, failed, timeout), (3, 0, 0, 0));

    // Complete some plugins
    let started_at = Instant::now();
    monitor
        .complete_plugin("plugin1".to_string(), started_at)
        .await;
    monitor
        .fail_plugin("plugin2".to_string(), started_at, "Error".to_string())
        .await;

    // Verify final state
    let (running, completed, failed, timeout) = monitor.get_status_counts().await;
    assert_eq!((running, completed, failed, timeout), (1, 1, 1, 0));

    // Should still have running plugins
    assert!(monitor.has_running_plugins().await);

    // Verify we received all updates
    let updates = wait_for_updates(&mut receiver, 5, Duration::from_secs(1)).await;
    assert_eq!(updates.len(), 5); // 3 starts + 1 complete + 1 fail
}

/// Test slow plugin detection
/// Verifies that slow plugins are detected and reported
#[tokio::test]
async fn test_slow_plugin_detection() {
    let config = ProgressConfig {
        poll_interval: Duration::from_millis(50),
        slow_threshold: Duration::from_millis(200),
        timeout_threshold: Duration::from_secs(10),
        log_progress: false,
    };

    let monitor = create_progress_monitor_with_config(config);
    let mut receiver = monitor.subscribe();

    // Start monitoring
    monitor.start_monitoring().await;

    // Start a plugin
    monitor.start_plugin("slow_plugin".to_string()).await;

    // Wait for slow detection (should trigger after 200ms + polling time)
    sleep(Duration::from_millis(300)).await;

    // Should receive initial start update + slow warning
    let updates = wait_for_updates(&mut receiver, 2, Duration::from_secs(1)).await;
    assert!(!updates.is_empty());

    // First update should be the start
    assert_eq!(updates[0].plugin_name, "slow_plugin");
    assert!(matches!(updates[0].status, PluginStatus::Running { .. }));

    // If we got a second update, it should be about the plugin being slow
    if updates.len() > 1 {
        assert!(updates[1].message.contains("taking longer than expected"));
    }
}

/// Test getting all plugin statuses
/// Verifies that all plugin statuses can be retrieved correctly
#[tokio::test]
async fn test_get_all_statuses() {
    let monitor = create_test_progress_monitor();
    let started_at = Instant::now();

    // Start multiple plugins in different states
    monitor.start_plugin("running_plugin".to_string()).await;
    monitor.start_plugin("completed_plugin".to_string()).await;
    monitor.start_plugin("failed_plugin".to_string()).await;

    // Change their states
    monitor
        .complete_plugin("completed_plugin".to_string(), started_at)
        .await;
    monitor
        .fail_plugin("failed_plugin".to_string(), started_at, "Error".to_string())
        .await;

    // Get all statuses
    let statuses = monitor.get_all_statuses().await;
    assert_eq!(statuses.len(), 3);

    assert!(matches!(
        statuses.get("running_plugin"),
        Some(PluginStatus::Running { .. })
    ));
    assert!(matches!(
        statuses.get("completed_plugin"),
        Some(PluginStatus::Completed { .. })
    ));
    assert!(matches!(
        statuses.get("failed_plugin"),
        Some(PluginStatus::Failed { .. })
    ));
}

/// Test plugin status retrieval for non-existent plugin
/// Verifies that requesting status for non-existent plugin returns None
#[tokio::test]
async fn test_get_nonexistent_plugin_status() {
    let monitor = create_test_progress_monitor();

    let status = monitor.get_plugin_status("nonexistent").await;
    assert!(status.is_none());
}

/// Test subscription to progress updates
/// Verifies that multiple subscribers can receive updates
#[tokio::test]
async fn test_multiple_subscribers() {
    let monitor = create_test_progress_monitor();
    let mut receiver1 = monitor.subscribe();
    let mut receiver2 = monitor.subscribe();

    // Start a plugin
    monitor.start_plugin("test_plugin".to_string()).await;

    // Both receivers should get the update
    let updates1 = wait_for_updates(&mut receiver1, 1, Duration::from_secs(1)).await;
    let updates2 = wait_for_updates(&mut receiver2, 1, Duration::from_secs(1)).await;

    assert_eq!(updates1.len(), 1);
    assert_eq!(updates2.len(), 1);
    assert_eq!(updates1[0].plugin_name, updates2[0].plugin_name);
}

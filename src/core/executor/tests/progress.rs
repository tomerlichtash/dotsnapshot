use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

use crate::config::Config;
use crate::core::executor::SnapshotExecutor;
use crate::core::plugin::{Plugin, PluginRegistry};
use crate::core::progress::{PluginStatus, ProgressConfig};
use crate::symbols::*;

/// Test plugin that simulates slow execution
struct SlowTestPlugin {
    delay: Duration,
    should_fail: bool,
}

impl SlowTestPlugin {
    fn new(delay: Duration) -> Self {
        Self {
            delay,
            should_fail: false,
        }
    }

    fn new_failing(delay: Duration) -> Self {
        Self {
            delay,
            should_fail: true,
        }
    }
}

#[async_trait::async_trait]
impl Plugin for SlowTestPlugin {
    fn description(&self) -> &str {
        "Test plugin with configurable delay"
    }

    fn icon(&self) -> &str {
        SYMBOL_TOOL_PLUGIN
    }

    async fn execute(&self) -> anyhow::Result<String> {
        sleep(self.delay).await;

        if self.should_fail {
            anyhow::bail!("Simulated plugin failure");
        }

        Ok("Test plugin output".to_string())
    }

    async fn validate(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn get_target_path(&self) -> Option<String> {
        None
    }

    fn get_output_file(&self) -> Option<String> {
        None
    }
}

/// Test SnapshotExecutor with progress monitoring enabled
/// Verifies that progress monitor is properly integrated
#[tokio::test]
async fn test_executor_with_progress_monitoring() {
    let temp_dir = TempDir::new().unwrap();
    let mut registry = PluginRegistry::new();
    registry.add_plugin(
        "test_plugin".to_string(),
        Arc::new(SlowTestPlugin::new(Duration::from_millis(100))),
    );

    let config = Config::default();
    let progress_config = ProgressConfig {
        poll_interval: Duration::from_millis(50),
        slow_threshold: Duration::from_millis(200),
        timeout_threshold: Duration::from_secs(5),
        log_progress: false,
    };

    let executor = SnapshotExecutor::with_progress_monitoring(
        Arc::new(registry),
        temp_dir.path().to_path_buf(),
        Arc::new(config),
        progress_config,
    );

    // Verify progress monitor is available
    let progress_monitor = executor.get_progress_monitor();
    assert!(progress_monitor.is_some());

    let progress_monitor = progress_monitor.unwrap();
    let mut receiver = progress_monitor.subscribe();

    // Execute snapshot
    let result = executor.execute_snapshot().await;
    assert!(result.is_ok());

    // Verify we received progress updates
    tokio::time::timeout(Duration::from_secs(1), async {
        let mut updates = Vec::new();
        while updates.len() < 2 {
            if let Ok(update) = receiver.recv().await {
                updates.push(update);
            } else {
                break;
            }
        }

        // Should have received start and completion updates
        assert!(updates.len() >= 2);
        assert_eq!(updates[0].plugin_name, "test_plugin");
        assert!(matches!(updates[0].status, PluginStatus::Running { .. }));

        if updates.len() > 1 {
            assert_eq!(updates[1].plugin_name, "test_plugin");
            assert!(matches!(updates[1].status, PluginStatus::Completed { .. }));
        }
    })
    .await
    .expect("Should receive progress updates within timeout");
}

/// Test SnapshotExecutor without progress monitoring
/// Verifies that executor works normally without progress monitoring
#[tokio::test]
async fn test_executor_without_progress_monitoring() {
    let temp_dir = TempDir::new().unwrap();
    let mut registry = PluginRegistry::new();
    registry.add_plugin(
        "test_plugin".to_string(),
        Arc::new(SlowTestPlugin::new(Duration::from_millis(10))),
    );

    let config = Config::default();
    let executor = SnapshotExecutor::with_config(
        Arc::new(registry),
        temp_dir.path().to_path_buf(),
        Arc::new(config),
    );

    // Verify no progress monitor is available
    let progress_monitor = executor.get_progress_monitor();
    assert!(progress_monitor.is_none());

    // Execute snapshot should still work
    let result = executor.execute_snapshot().await;
    assert!(result.is_ok());
}

/// Test progress monitoring with plugin failure
/// Verifies that failed plugins are properly tracked
#[tokio::test]
async fn test_progress_monitoring_with_plugin_failure() {
    let temp_dir = TempDir::new().unwrap();
    let mut registry = PluginRegistry::new();
    registry.add_plugin(
        "failing_plugin".to_string(),
        Arc::new(SlowTestPlugin::new_failing(Duration::from_millis(50))),
    );

    let config = Config::default();
    let progress_config = ProgressConfig {
        poll_interval: Duration::from_millis(25),
        slow_threshold: Duration::from_millis(200),
        timeout_threshold: Duration::from_secs(5),
        log_progress: false,
    };

    let executor = SnapshotExecutor::with_progress_monitoring(
        Arc::new(registry),
        temp_dir.path().to_path_buf(),
        Arc::new(config),
        progress_config,
    );

    let progress_monitor = executor.get_progress_monitor().unwrap();
    let mut receiver = progress_monitor.subscribe();

    // Execute snapshot
    let result = executor.execute_snapshot().await;
    assert!(result.is_ok()); // Snapshot should complete even with failed plugins

    // Verify we received failure updates
    tokio::time::timeout(Duration::from_secs(1), async {
        let mut updates = Vec::new();
        while updates.len() < 2 {
            if let Ok(update) = receiver.recv().await {
                updates.push(update);
            } else {
                break;
            }
        }

        // Should have received start and failure updates
        assert!(updates.len() >= 2);
        assert_eq!(updates[0].plugin_name, "failing_plugin");
        assert!(matches!(updates[0].status, PluginStatus::Running { .. }));

        if updates.len() > 1 {
            assert_eq!(updates[1].plugin_name, "failing_plugin");
            assert!(matches!(updates[1].status, PluginStatus::Failed { .. }));
        }
    })
    .await
    .expect("Should receive progress updates within timeout");
}

/// Test progress monitoring with multiple plugins
/// Verifies that multiple plugins are tracked concurrently
#[tokio::test]
async fn test_progress_monitoring_multiple_plugins() {
    let temp_dir = TempDir::new().unwrap();
    let mut registry = PluginRegistry::new();
    registry.add_plugin(
        "plugin1".to_string(),
        Arc::new(SlowTestPlugin::new(Duration::from_millis(50))),
    );
    registry.add_plugin(
        "plugin2".to_string(),
        Arc::new(SlowTestPlugin::new(Duration::from_millis(100))),
    );
    registry.add_plugin(
        "plugin3".to_string(),
        Arc::new(SlowTestPlugin::new_failing(Duration::from_millis(75))),
    );

    let config = Config::default();
    let progress_config = ProgressConfig {
        poll_interval: Duration::from_millis(25),
        slow_threshold: Duration::from_millis(200),
        timeout_threshold: Duration::from_secs(5),
        log_progress: false,
    };

    let executor = SnapshotExecutor::with_progress_monitoring(
        Arc::new(registry),
        temp_dir.path().to_path_buf(),
        Arc::new(config),
        progress_config,
    );

    let progress_monitor = executor.get_progress_monitor().unwrap();
    let mut receiver = progress_monitor.subscribe();

    // Execute snapshot
    let result = executor.execute_snapshot().await;
    assert!(result.is_ok());

    // Verify we received updates for all plugins
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut updates = Vec::new();
        let mut plugin_names = std::collections::HashSet::new();
        let mut completed_plugins = std::collections::HashSet::new();

        // Wait until we have completion/failure updates for all plugins
        while completed_plugins.len() < 3 && updates.len() < 20 {
            if let Ok(update) = receiver.recv().await {
                plugin_names.insert(update.plugin_name.clone());

                // Track completion or failure
                match update.status {
                    PluginStatus::Completed { .. } | PluginStatus::Failed { .. } => {
                        completed_plugins.insert(update.plugin_name.clone());
                    }
                    _ => {}
                }

                updates.push(update);
            } else {
                // Small delay to let more updates come in
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        // Should have received updates for all 3 plugins
        assert!(plugin_names.contains("plugin1"));
        assert!(plugin_names.contains("plugin2"));
        assert!(plugin_names.contains("plugin3"));

        // Should have completion/failure for all plugins
        assert_eq!(
            completed_plugins.len(),
            3,
            "All plugins should complete or fail"
        );

        // Should have at least 6 updates (start + completion/failure for each plugin)
        assert!(
            updates.len() >= 6,
            "Should have at least 6 updates, got {}",
            updates.len()
        );
    })
    .await
    .expect("Should receive progress updates for all plugins within timeout");
}

/// Test progress monitor status tracking
/// Verifies that plugin statuses are correctly tracked and retrievable
#[tokio::test]
async fn test_progress_monitor_status_tracking() {
    let temp_dir = TempDir::new().unwrap();
    let mut registry = PluginRegistry::new();
    registry.add_plugin(
        "test_plugin".to_string(),
        Arc::new(SlowTestPlugin::new(Duration::from_millis(100))),
    );

    let config = Config::default();
    let progress_config = ProgressConfig {
        poll_interval: Duration::from_millis(25),
        slow_threshold: Duration::from_millis(200),
        timeout_threshold: Duration::from_secs(5),
        log_progress: false,
    };

    let executor = SnapshotExecutor::with_progress_monitoring(
        Arc::new(registry),
        temp_dir.path().to_path_buf(),
        Arc::new(config),
        progress_config,
    );

    let progress_monitor = executor.get_progress_monitor().unwrap();

    // Execute snapshot in background
    let executor_handle = tokio::spawn(async move { executor.execute_snapshot().await });

    // Check plugin status during execution
    tokio::time::timeout(Duration::from_secs(1), async {
        let mut found_running = false;
        let mut found_completed = false;

        while !found_completed {
            sleep(Duration::from_millis(10)).await;

            if let Some(status) = progress_monitor.get_plugin_status("test_plugin").await {
                match status {
                    PluginStatus::Running { .. } => {
                        found_running = true;
                    }
                    PluginStatus::Completed { .. } => {
                        found_completed = true;
                    }
                    _ => {}
                }
            }
        }

        assert!(found_running, "Should have found plugin in running state");
        assert!(
            found_completed,
            "Should have found plugin in completed state"
        );
    })
    .await
    .expect("Should track plugin status changes within timeout");

    // Wait for executor to complete
    let result = executor_handle.await.unwrap();
    assert!(result.is_ok());
}

/// Test slow plugin detection
/// Verifies that slow plugins are detected and reported
#[tokio::test]
async fn test_slow_plugin_detection() {
    let temp_dir = TempDir::new().unwrap();
    let mut registry = PluginRegistry::new();
    // Create a plugin that takes longer than the slow threshold
    registry.add_plugin(
        "slow_plugin".to_string(),
        Arc::new(SlowTestPlugin::new(Duration::from_millis(300))),
    );

    let config = Config::default();
    let progress_config = ProgressConfig {
        poll_interval: Duration::from_millis(50),
        slow_threshold: Duration::from_millis(100), // Low threshold to trigger slow detection
        timeout_threshold: Duration::from_secs(5),
        log_progress: false,
    };

    let executor = SnapshotExecutor::with_progress_monitoring(
        Arc::new(registry),
        temp_dir.path().to_path_buf(),
        Arc::new(config),
        progress_config,
    );

    let progress_monitor = executor.get_progress_monitor().unwrap();
    let mut receiver = progress_monitor.subscribe();

    // Execute snapshot
    let executor_handle = tokio::spawn(async move { executor.execute_snapshot().await });

    // Look for slow plugin warning
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut found_slow_warning = false;

        while !found_slow_warning {
            if let Ok(update) = receiver.recv().await {
                if update.plugin_name == "slow_plugin"
                    && update.message.contains("taking longer than expected")
                {
                    found_slow_warning = true;
                }
            } else {
                break;
            }
        }

        assert!(found_slow_warning, "Should have detected slow plugin");
    })
    .await
    .expect("Should detect slow plugin within timeout");

    // Wait for executor to complete
    let result = executor_handle.await.unwrap();
    assert!(result.is_ok());
}

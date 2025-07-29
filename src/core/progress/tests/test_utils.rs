use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::core::progress::{PluginStatus, ProgressConfig, ProgressMonitor};

/// Create a test progress monitor with fast polling for testing
pub fn create_test_progress_monitor() -> ProgressMonitor {
    let config = ProgressConfig {
        poll_interval: Duration::from_millis(100),
        slow_threshold: Duration::from_millis(500),
        timeout_threshold: Duration::from_secs(2),
        log_progress: false, // Disable logging in tests
    };

    ProgressMonitor::new(config)
}

/// Create a progress monitor with custom configuration
pub fn create_progress_monitor_with_config(config: ProgressConfig) -> ProgressMonitor {
    ProgressMonitor::new(config)
}

/// Helper to wait for a specific number of progress updates
pub async fn wait_for_updates(
    receiver: &mut tokio::sync::broadcast::Receiver<crate::core::progress::ProgressUpdate>,
    count: usize,
    timeout: Duration,
) -> Vec<crate::core::progress::ProgressUpdate> {
    let mut updates = Vec::new();
    let start = Instant::now();

    while updates.len() < count && start.elapsed() < timeout {
        match tokio::time::timeout(Duration::from_millis(100), receiver.recv()).await {
            Ok(Ok(update)) => updates.push(update),
            Ok(Err(_)) => break, // Channel closed
            Err(_) => continue,  // Timeout, keep trying
        }
    }

    updates
}

/// Helper to simulate plugin execution time
#[allow(dead_code)]
pub async fn simulate_plugin_execution(duration: Duration) {
    sleep(duration).await;
}

/// Create a mock plugin status for testing
#[allow(dead_code)]
pub fn create_mock_running_status(started_at: Instant) -> PluginStatus {
    PluginStatus::Running { started_at }
}

/// Create a mock completed status for testing
#[allow(dead_code)]
pub fn create_mock_completed_status(duration: Duration) -> PluginStatus {
    PluginStatus::Completed { duration }
}

/// Create a mock failed status for testing
#[allow(dead_code)]
pub fn create_mock_failed_status(error: String, duration: Duration) -> PluginStatus {
    PluginStatus::Failed { error, duration }
}

//! Progress monitoring and feedback system for plugin execution

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use tokio::time::interval;
use tracing::{info, warn};

use crate::symbols::*;

#[cfg(test)]
mod tests;

/// Status of a plugin execution
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum PluginStatus {
    /// Plugin is waiting to start
    Pending,
    /// Plugin is currently running
    Running { started_at: Instant },
    /// Plugin completed successfully
    Completed { duration: Duration },
    /// Plugin failed with error
    Failed { error: String, duration: Duration },
    /// Plugin timed out or appears stuck
    Timeout { duration: Duration },
}

/// Progress update message sent to subscribers
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProgressUpdate {
    pub plugin_name: String,
    pub status: PluginStatus,
    pub message: String,
}

/// Configuration for progress monitoring
#[derive(Debug, Clone)]
pub struct ProgressConfig {
    /// How often to poll for progress updates
    pub poll_interval: Duration,
    /// Threshold after which a plugin is considered slow
    pub slow_threshold: Duration,
    /// Threshold after which a plugin is considered potentially stuck
    pub timeout_threshold: Duration,
    /// Whether to show progress updates in logs
    pub log_progress: bool,
}

impl Default for ProgressConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(2),
            slow_threshold: Duration::from_secs(10),
            timeout_threshold: Duration::from_secs(60),
            log_progress: true,
        }
    }
}

/// Progress monitor that tracks plugin execution status
pub struct ProgressMonitor {
    plugin_statuses: Arc<RwLock<HashMap<String, PluginStatus>>>,
    progress_sender: broadcast::Sender<ProgressUpdate>,
    config: ProgressConfig,
}

impl ProgressMonitor {
    /// Create a new progress monitor
    #[allow(dead_code)]
    pub fn new(config: ProgressConfig) -> Self {
        let (progress_sender, _) = broadcast::channel(100);

        Self {
            plugin_statuses: Arc::new(RwLock::new(HashMap::new())),
            progress_sender,
            config,
        }
    }

    /// Get a receiver for progress updates
    #[allow(dead_code)]
    pub fn subscribe(&self) -> broadcast::Receiver<ProgressUpdate> {
        self.progress_sender.subscribe()
    }

    /// Start monitoring progress in the background
    pub async fn start_monitoring(&self) {
        let plugin_statuses = Arc::clone(&self.plugin_statuses);
        let progress_sender = self.progress_sender.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = interval(config.poll_interval);

            loop {
                interval.tick().await;

                let statuses = plugin_statuses.read().await;
                let now = Instant::now();

                for (plugin_name, status) in statuses.iter() {
                    if let PluginStatus::Running { started_at } = status {
                        let duration = now.duration_since(*started_at);

                        let (message, should_update) = if duration >= config.timeout_threshold {
                            (
                                format!(
                                    "{} Plugin '{}' appears stuck (running for {:.1}s)",
                                    SYMBOL_INDICATOR_WARNING,
                                    plugin_name,
                                    duration.as_secs_f64()
                                ),
                                true,
                            )
                        } else if duration >= config.slow_threshold {
                            (
                                format!(
                                    "{} Plugin '{}' is taking longer than expected ({:.1}s)",
                                    SYMBOL_INDICATOR_INFO,
                                    plugin_name,
                                    duration.as_secs_f64()
                                ),
                                duration.as_secs() % 10 == 0, // Update every 10 seconds for slow plugins
                            )
                        } else {
                            continue;
                        };

                        if should_update {
                            if config.log_progress {
                                if duration >= config.timeout_threshold {
                                    warn!("{message}");
                                } else {
                                    info!("{message}");
                                }
                            }

                            let update = ProgressUpdate {
                                plugin_name: plugin_name.clone(),
                                status: status.clone(),
                                message,
                            };

                            let _ = progress_sender.send(update);
                        }
                    }
                }
            }
        });
    }

    /// Register a plugin as starting
    pub async fn start_plugin(&self, plugin_name: String) {
        let mut statuses = self.plugin_statuses.write().await;
        let started_at = Instant::now();

        statuses.insert(plugin_name.clone(), PluginStatus::Running { started_at });

        if self.config.log_progress {
            info!("{} Starting plugin: {}", SYMBOL_ACTION_SEARCH, plugin_name);
        }

        let update = ProgressUpdate {
            plugin_name: plugin_name.clone(),
            status: PluginStatus::Running { started_at },
            message: format!("{SYMBOL_ACTION_SEARCH} Starting plugin: {plugin_name}"),
        };

        let _ = self.progress_sender.send(update);
    }

    /// Register a plugin as completed
    pub async fn complete_plugin(&self, plugin_name: String, started_at: Instant) {
        let mut statuses = self.plugin_statuses.write().await;
        let duration = started_at.elapsed();

        statuses.insert(plugin_name.clone(), PluginStatus::Completed { duration });

        if self.config.log_progress {
            info!(
                "{} Plugin '{}' completed in {:.2}s",
                SYMBOL_INDICATOR_SUCCESS,
                plugin_name,
                duration.as_secs_f64()
            );
        }

        let update = ProgressUpdate {
            plugin_name: plugin_name.clone(),
            status: PluginStatus::Completed { duration },
            message: format!(
                "{} Plugin '{}' completed in {:.2}s",
                SYMBOL_INDICATOR_SUCCESS,
                plugin_name,
                duration.as_secs_f64()
            ),
        };

        let _ = self.progress_sender.send(update);
    }

    /// Register a plugin as failed
    pub async fn fail_plugin(&self, plugin_name: String, started_at: Instant, error: String) {
        let mut statuses = self.plugin_statuses.write().await;
        let duration = started_at.elapsed();

        statuses.insert(
            plugin_name.clone(),
            PluginStatus::Failed {
                error: error.clone(),
                duration,
            },
        );

        if self.config.log_progress {
            warn!(
                "{} Plugin '{}' failed after {:.2}s: {}",
                SYMBOL_INDICATOR_ERROR,
                plugin_name,
                duration.as_secs_f64(),
                error
            );
        }

        let update = ProgressUpdate {
            plugin_name: plugin_name.clone(),
            status: PluginStatus::Failed {
                error: error.clone(),
                duration,
            },
            message: format!(
                "{} Plugin '{}' failed after {:.2}s: {}",
                SYMBOL_INDICATOR_ERROR,
                plugin_name,
                duration.as_secs_f64(),
                error
            ),
        };

        let _ = self.progress_sender.send(update);
    }

    /// Get current status of all plugins
    #[allow(dead_code)]
    pub async fn get_all_statuses(&self) -> HashMap<String, PluginStatus> {
        self.plugin_statuses.read().await.clone()
    }

    /// Get status of a specific plugin
    #[allow(dead_code)]
    pub async fn get_plugin_status(&self, plugin_name: &str) -> Option<PluginStatus> {
        self.plugin_statuses.read().await.get(plugin_name).cloned()
    }

    /// Check if any plugins are currently running
    #[allow(dead_code)]
    pub async fn has_running_plugins(&self) -> bool {
        let statuses = self.plugin_statuses.read().await;
        statuses
            .values()
            .any(|status| matches!(status, PluginStatus::Running { .. }))
    }

    /// Get count of plugins in each status
    #[allow(dead_code)]
    pub async fn get_status_counts(&self) -> (usize, usize, usize, usize) {
        let statuses = self.plugin_statuses.read().await;
        let mut running = 0;
        let mut completed = 0;
        let mut failed = 0;
        let mut timeout = 0;

        for status in statuses.values() {
            match status {
                PluginStatus::Running { .. } => running += 1,
                PluginStatus::Completed { .. } => completed += 1,
                PluginStatus::Failed { .. } => failed += 1,
                PluginStatus::Timeout { .. } => timeout += 1,
                PluginStatus::Pending => {} // Don't count pending
            }
        }

        (running, completed, failed, timeout)
    }
}

//! Test utilities for SnapshotExecutor tests

use crate::config::Config;
use crate::core::executor::SnapshotExecutor;
use crate::core::plugin::PluginRegistry;
use crate::core::snapshot::SnapshotManager;
use std::path::PathBuf;
use std::sync::Arc;

/// Test utilities for SnapshotExecutor
impl SnapshotExecutor {
    /// Create a new executor for testing without config
    pub fn new(registry: Arc<PluginRegistry>, base_path: PathBuf) -> Self {
        Self {
            registry,
            snapshot_manager: SnapshotManager::new(base_path),
            config: None,
        }
    }

    /// Get reference to the config for testing
    pub fn config(&self) -> Option<&Arc<Config>> {
        self.config.as_ref()
    }
}

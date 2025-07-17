use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs as async_fs;
use tracing::{info, warn, error};

use crate::core::checksum::calculate_checksum;
use crate::core::plugin::{Plugin, PluginRegistry, PluginResult};
use crate::core::snapshot::SnapshotManager;

/// Executes all plugins asynchronously and creates a snapshot
pub struct SnapshotExecutor {
    registry: Arc<PluginRegistry>,
    snapshot_manager: SnapshotManager,
}

impl SnapshotExecutor {
    pub fn new(registry: Arc<PluginRegistry>, base_path: PathBuf) -> Self {
        Self {
            registry,
            snapshot_manager: SnapshotManager::new(base_path),
        }
    }
    
    /// Executes all plugins and creates a snapshot
    pub async fn execute_snapshot(&self) -> Result<PathBuf> {
        info!("Starting snapshot execution");
        
        // Create snapshot directory
        let snapshot_dir = self.snapshot_manager.create_snapshot_dir().await?;
        info!("Created snapshot directory: {}", snapshot_dir.display());
        
        // Create initial metadata
        let mut metadata = self.snapshot_manager.create_metadata();
        
        // Execute all plugins concurrently
        let plugins = self.registry.plugins();
        let mut plugin_tasks = Vec::new();
        
        for plugin in plugins {
            let plugin_clone = Arc::clone(plugin);
            let snapshot_dir_clone = snapshot_dir.clone();
            let snapshot_manager_clone = self.snapshot_manager.clone();
            
            let task = tokio::spawn(async move {
                Self::execute_plugin(plugin_clone, &snapshot_dir_clone, &snapshot_manager_clone).await
            });
            
            plugin_tasks.push(task);
        }
        
        // Wait for all plugins to complete
        let mut results = Vec::new();
        for task in plugin_tasks {
            match task.await {
                Ok(result) => {
                    match result {
                        Ok(plugin_result) => {
                            results.push(plugin_result);
                        }
                        Err(e) => {
                            error!("Plugin execution failed: {}", e);
                            // Create error result for failed plugin
                            results.push(PluginResult {
                                plugin_name: "unknown".to_string(),
                                content: String::new(),
                                checksum: String::new(),
                                success: false,
                                error_message: Some(e.to_string()),
                            });
                        }
                    }
                }
                Err(e) => {
                    error!("Plugin task failed: {}", e);
                }
            }
        }
        
        // Update metadata with plugin results
        for result in &results {
            if result.success {
                metadata.checksums.insert(result.plugin_name.clone(), result.checksum.clone());
            }
        }
        
        // Save metadata
        self.snapshot_manager.save_metadata(&snapshot_dir, &metadata).await?;
        
        // Finalize snapshot (calculate directory checksum)
        self.snapshot_manager.finalize_snapshot(&snapshot_dir).await?;
        
        info!("Snapshot execution completed: {}", snapshot_dir.display());
        Ok(snapshot_dir)
    }
    
    /// Executes a single plugin with checksum optimization
    async fn execute_plugin(
        plugin: Arc<dyn Plugin>,
        snapshot_dir: &PathBuf,
        snapshot_manager: &SnapshotManager,
    ) -> Result<PluginResult> {
        let plugin_name = plugin.name().to_string();
        info!("Executing plugin: {}", plugin_name);
        
        // Validate plugin can run
        if let Err(e) = plugin.validate().await {
            warn!("Plugin validation failed for {}: {}", plugin_name, e);
            return Ok(PluginResult {
                plugin_name: plugin_name.clone(),
                content: String::new(),
                checksum: String::new(),
                success: false,
                error_message: Some(format!("Validation failed: {}", e)),
            });
        }
        
        // Execute plugin to get content
        let content = match plugin.execute().await {
            Ok(content) => content,
            Err(e) => {
                error!("Plugin execution failed for {}: {}", plugin_name, e);
                return Ok(PluginResult {
                    plugin_name: plugin_name.clone(),
                    content: String::new(),
                    checksum: String::new(),
                    success: false,
                    error_message: Some(e.to_string()),
                });
            }
        };
        
        // Calculate checksum
        let checksum = calculate_checksum(&content);
        
        // Check if we can reuse existing file with same checksum
        let filename = plugin.filename();
        if let Ok(Some(_existing_file)) = snapshot_manager.find_file_by_checksum(&plugin_name, filename, &checksum, snapshot_dir).await {
            info!("Reusing existing file for plugin {} (checksum match)", plugin_name);
            
            // Copy file from latest snapshot
            if snapshot_manager.copy_from_latest(&plugin_name, filename, snapshot_dir).await? {
                return Ok(PluginResult {
                    plugin_name: plugin_name.clone(),
                    content,
                    checksum,
                    success: true,
                    error_message: None,
                });
            }
        }
        
        // Save new content to file
        let output_path = plugin.output_path(snapshot_dir);
        async_fs::write(&output_path, &content).await
            .context(format!("Failed to write output for plugin {}", plugin_name))?;
        
        info!("Plugin {} completed successfully", plugin_name);
        
        Ok(PluginResult {
            plugin_name: plugin_name.clone(),
            content,
            checksum,
            success: true,
            error_message: None,
        })
    }
}

// Make SnapshotManager cloneable for use in async tasks
impl Clone for SnapshotManager {
    fn clone(&self) -> Self {
        Self::new(self.base_path().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use tempfile::TempDir;

    struct TestPlugin {
        name: String,
        content: String,
    }

    #[async_trait]
    impl Plugin for TestPlugin {
        fn name(&self) -> &str {
            &self.name
        }
        
        fn filename(&self) -> &str {
            "test.txt"
        }
        
        fn description(&self) -> &str {
            "Test plugin for unit tests"
        }
        
        async fn execute(&self) -> Result<String> {
            Ok(self.content.clone())
        }
        
        async fn validate(&self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_execute_snapshot() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path().to_path_buf();
        
        let mut registry = PluginRegistry::new();
        registry.register(Arc::new(TestPlugin {
            name: "test".to_string(),
            content: "test content".to_string(),
        }));
        
        let executor = SnapshotExecutor::new(Arc::new(registry), base_path);
        let snapshot_dir = executor.execute_snapshot().await?;
        
        assert!(snapshot_dir.exists());
        assert!(snapshot_dir.join("test.txt").exists());
        assert!(snapshot_dir.join("metadata.json").exists());
        
        Ok(())
    }
}
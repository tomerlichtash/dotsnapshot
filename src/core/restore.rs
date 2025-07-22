use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs as async_fs;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::core::hooks::{HookContext, HookManager, HookType};
use crate::core::plugin::{Plugin, PluginRegistry};
use crate::core::snapshot::SnapshotManager;

/// Result of a plugin restore operation
#[derive(Debug, Clone)]
pub struct RestoreResult {
    pub plugin_name: String,
    pub success: bool,
    pub restored_files: usize,
    pub backup_path: Option<PathBuf>,
    pub error_message: Option<String>,
}

/// Manages restoration of configurations from snapshots
pub struct RestoreManager {
    registry: Arc<PluginRegistry>,
    snapshot_manager: SnapshotManager,
    config: Option<Arc<Config>>,
}

impl RestoreManager {
    #[allow(dead_code)]
    pub fn new(registry: Arc<PluginRegistry>, snapshots_dir: PathBuf) -> Self {
        Self {
            registry,
            snapshot_manager: SnapshotManager::new(snapshots_dir),
            config: None,
        }
    }

    pub fn with_config(
        registry: Arc<PluginRegistry>,
        snapshots_dir: PathBuf,
        config: Arc<Config>,
    ) -> Self {
        Self {
            registry,
            snapshot_manager: SnapshotManager::new(snapshots_dir),
            config: Some(config),
        }
    }

    /// List available snapshots
    pub async fn list_snapshots(&self) -> Result<Vec<SnapshotInfo>> {
        let mut snapshots = Vec::new();
        let snapshots_dir = self.snapshot_manager.base_path();

        if !snapshots_dir.exists() {
            warn!(
                "Snapshots directory does not exist: {}",
                snapshots_dir.display()
            );
            return Ok(snapshots);
        }

        let mut entries = async_fs::read_dir(&snapshots_dir).await.with_context(|| {
            format!(
                "Failed to read snapshots directory: {}",
                snapshots_dir.display()
            )
        })?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(snapshot_info) = self.analyze_snapshot(&path).await {
                    snapshots.push(snapshot_info);
                } else {
                    tracing::debug!(
                        "Skipping directory that doesn't appear to be a snapshot: {}",
                        path.display()
                    );
                }
            }
        }

        // Sort by creation time (newest first)
        snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(snapshots)
    }

    /// Restore configurations from a specific snapshot
    pub async fn restore_from_snapshot(
        &self,
        snapshot_name: &str,
        selected_plugins: Option<&[String]>,
        dry_run: bool,
        backup_existing: bool,
    ) -> Result<Vec<RestoreResult>> {
        info!("Starting restore from snapshot: {}", snapshot_name);

        let snapshot_path = self.snapshot_manager.base_path().join(snapshot_name);
        if !snapshot_path.exists() {
            return Err(anyhow::anyhow!(
                "Snapshot '{}' not found in {}",
                snapshot_name,
                self.snapshot_manager.base_path().display()
            ));
        }

        // Set up hooks manager and context
        let hooks_config = self
            .config
            .as_ref()
            .map(|c| c.get_hooks_config())
            .unwrap_or_default();
        let hook_manager = HookManager::new(hooks_config.clone());
        let hook_context = HookContext::new(
            snapshot_name.to_string(),
            snapshot_path.clone(),
            hooks_config.clone(),
        );

        // Execute pre-restore hooks (global)
        if let Some(config) = &self.config {
            let pre_restore_hooks = config.get_global_pre_restore_hooks();
            if !pre_restore_hooks.is_empty() {
                hook_manager
                    .execute_hooks(&pre_restore_hooks, &HookType::PreRestore, &hook_context)
                    .await;
            }
        }

        // Create backup directory if needed
        let backup_dir = if backup_existing {
            let backup_path = if dry_run {
                // In dry run mode, create a mock backup path
                self.snapshot_manager.base_path().join(format!(
                    "backup_dry_run_{}",
                    chrono::Local::now().format("%Y%m%d_%H%M%S")
                ))
            } else {
                self.create_backup_directory().await?
            };
            if !dry_run {
                info!("Created backup directory: {}", backup_path.display());
            } else {
                info!(
                    "🔍 [DRY RUN] Would create backup directory: {}",
                    backup_path.display()
                );
            }
            Some(backup_path)
        } else {
            None
        };

        let backup_context = if let Some(backup_path) = &backup_dir {
            hook_context.with_variable(
                "backup_path".to_string(),
                backup_path.to_string_lossy().to_string(),
            )
        } else {
            hook_context.clone()
        };

        // Get available plugins for restoration
        let available_plugins = self.get_restorable_plugins(&snapshot_path).await?;
        let plugins_to_restore = if let Some(selected) = selected_plugins {
            available_plugins
                .into_iter()
                .filter(|(name, _)| selected.contains(name))
                .collect()
        } else {
            available_plugins
        };

        if plugins_to_restore.is_empty() {
            warn!(
                "No plugins found for restoration in snapshot: {}",
                snapshot_name
            );
            return Ok(Vec::new());
        }

        info!(
            "Restoring {} plugins: {}",
            plugins_to_restore.len(),
            plugins_to_restore
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );

        // Execute plugin restorations
        let mut results = Vec::new();
        for (plugin_name, snapshot_file) in plugins_to_restore {
            let result = self
                .restore_plugin(
                    &plugin_name,
                    &snapshot_file,
                    backup_dir.as_ref(),
                    dry_run,
                    &hook_manager,
                    &backup_context,
                )
                .await?;
            results.push(result);
        }

        // Execute post-restore hooks (global)
        if let Some(config) = &self.config {
            let post_restore_hooks = config.get_global_post_restore_hooks();
            if !post_restore_hooks.is_empty() {
                let final_context = backup_context
                    .with_file_count(results.iter().map(|r| r.restored_files).sum())
                    .with_variable("restored_plugins".to_string(), results.len().to_string());
                hook_manager
                    .execute_hooks(&post_restore_hooks, &HookType::PostRestore, &final_context)
                    .await;
            }
        }

        info!("Restore completed for snapshot: {}", snapshot_name);
        Ok(results)
    }

    /// Restore a single plugin
    async fn restore_plugin(
        &self,
        plugin_name: &str,
        snapshot_file: &Path,
        backup_dir: Option<&PathBuf>,
        dry_run: bool,
        hook_manager: &HookManager,
        hook_context: &HookContext,
    ) -> Result<RestoreResult> {
        info!("🔄 Restoring plugin: {}", plugin_name);

        // Create plugin-specific hook context
        let plugin_hook_context = hook_context.clone().with_plugin(plugin_name.to_string());

        // Execute pre-plugin-restore hooks
        if let Some(config) = &self.config {
            let pre_plugin_hooks = config.get_plugin_pre_restore_hooks(plugin_name);
            if !pre_plugin_hooks.is_empty() {
                hook_manager
                    .execute_hooks(
                        &pre_plugin_hooks,
                        &HookType::PrePluginRestore,
                        &plugin_hook_context,
                    )
                    .await;
            }
        }

        // Find the plugin in the registry
        let plugin = self
            .registry
            .plugins()
            .iter()
            .find(|p| p.name() == plugin_name)
            .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found in registry", plugin_name))?;

        // Perform the actual restoration
        let restore_result = if dry_run {
            info!("🔍 [DRY RUN] Would restore plugin: {}", plugin_name);
            RestoreResult {
                plugin_name: plugin_name.to_string(),
                success: true,
                restored_files: 1,
                backup_path: backup_dir.cloned(),
                error_message: None,
            }
        } else {
            match self
                .perform_plugin_restore(plugin.as_ref(), snapshot_file, backup_dir)
                .await
            {
                Ok((restored_files, backup_path)) => {
                    info!("✅ Plugin {} restored successfully", plugin_name);
                    RestoreResult {
                        plugin_name: plugin_name.to_string(),
                        success: true,
                        restored_files,
                        backup_path,
                        error_message: None,
                    }
                }
                Err(e) => {
                    error!("❌ Failed to restore plugin {}: {}", plugin_name, e);
                    RestoreResult {
                        plugin_name: plugin_name.to_string(),
                        success: false,
                        restored_files: 0,
                        backup_path: backup_dir.cloned(),
                        error_message: Some(e.to_string()),
                    }
                }
            }
        };

        // Execute post-plugin-restore hooks
        if let Some(config) = &self.config {
            let post_plugin_hooks = config.get_plugin_post_restore_hooks(plugin_name);
            if !post_plugin_hooks.is_empty() {
                let result_context = plugin_hook_context
                    .with_file_count(restore_result.restored_files)
                    .with_variable("success".to_string(), restore_result.success.to_string());

                let result_context = if let Some(backup_path) = &restore_result.backup_path {
                    result_context.with_variable(
                        "backup_path".to_string(),
                        backup_path.to_string_lossy().to_string(),
                    )
                } else {
                    result_context
                };

                hook_manager
                    .execute_hooks(
                        &post_plugin_hooks,
                        &HookType::PostPluginRestore,
                        &result_context,
                    )
                    .await;
            }
        }

        Ok(restore_result)
    }

    /// Perform the actual plugin restoration (placeholder for now)
    async fn perform_plugin_restore(
        &self,
        plugin: &dyn Plugin,
        snapshot_file: &Path,
        backup_dir: Option<&PathBuf>,
    ) -> Result<(usize, Option<PathBuf>)> {
        // This is a placeholder implementation
        // Each plugin will need to implement its own restore logic
        info!(
            "Restoring {} from {}",
            plugin.name(),
            snapshot_file.display()
        );

        // For now, just copy the file to demonstrate the concept
        if let Some(backup_dir) = backup_dir {
            // Create backup if needed
            let backup_file = backup_dir.join(format!("{}.backup", plugin.name()));
            // Backup existing configuration (implementation depends on plugin)
            info!("Would backup existing config to: {}", backup_file.display());
        }

        // Restore the configuration (implementation depends on plugin)
        info!("Would restore configuration for plugin: {}", plugin.name());

        Ok((1, backup_dir.cloned()))
    }

    /// Get plugins that can be restored from a snapshot
    async fn get_restorable_plugins(&self, snapshot_path: &Path) -> Result<Vec<(String, PathBuf)>> {
        let mut plugins = Vec::new();
        let mut entries = async_fs::read_dir(snapshot_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // Skip metadata files
            if filename == "metadata.json" || filename == ".checksum" {
                continue;
            }

            // Map files to plugin names (this is a simplified mapping)
            let plugin_name = self.map_file_to_plugin(&filename);
            if let Some(name) = plugin_name {
                plugins.push((name, path));
            }
        }

        Ok(plugins)
    }

    /// Map snapshot files to plugin names (simplified)
    fn map_file_to_plugin(&self, filename: &str) -> Option<String> {
        match filename {
            "Brewfile" => Some("homebrew_brewfile".to_string()),
            "vscode_settings.json" => Some("vscode_settings".to_string()),
            "vscode_keybindings.json" => Some("vscode_keybindings".to_string()),
            "vscode_extensions.txt" => Some("vscode_extensions".to_string()),
            "cursor_settings.json" => Some("cursor_settings".to_string()),
            "cursor_keybindings.json" => Some("cursor_keybindings".to_string()),
            "cursor_extensions.txt" => Some("cursor_extensions".to_string()),
            "npm_global_packages.txt" => Some("npm_global_packages".to_string()),
            "npm_config.txt" => Some("npm_config".to_string()),
            _ => None,
        }
    }

    /// Analyze a snapshot directory to extract metadata
    async fn analyze_snapshot(&self, snapshot_path: &Path) -> Result<SnapshotInfo> {
        let name = snapshot_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Try to read metadata.json
        let metadata_path = snapshot_path.join("metadata.json");
        let (created_at, plugin_count) = if metadata_path.exists() {
            match self.read_snapshot_metadata(&metadata_path).await {
                Ok(metadata) => {
                    let created_at = chrono::DateTime::parse_from_rfc3339(&metadata.timestamp)
                        .map(|dt| dt.with_timezone(&chrono::Local))
                        .unwrap_or_else(|_| chrono::Local::now());
                    let plugin_count = metadata.plugins.len();
                    (created_at, plugin_count)
                }
                Err(e) => {
                    tracing::debug!("Failed to read metadata for {}: {}", name, e);
                    // Fallback to directory modification time
                    let created_at = self.get_directory_creation_time(snapshot_path)?;
                    (created_at, 0)
                }
            }
        } else {
            // Fallback to directory modification time
            let created_at = self.get_directory_creation_time(snapshot_path)?;
            (created_at, 0)
        };

        // Calculate directory size
        let size_bytes = self.calculate_directory_size(snapshot_path).await?;

        Ok(SnapshotInfo {
            name,
            path: snapshot_path.to_path_buf(),
            created_at,
            size_bytes,
            plugin_count,
        })
    }

    /// Read snapshot metadata
    async fn read_snapshot_metadata(&self, metadata_path: &Path) -> Result<SnapshotMetadata> {
        let content = async_fs::read_to_string(metadata_path)
            .await
            .with_context(|| {
                format!("Failed to read metadata file: {}", metadata_path.display())
            })?;

        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse metadata file: {}", metadata_path.display()))
    }

    /// Get directory creation time
    fn get_directory_creation_time(&self, path: &Path) -> Result<chrono::DateTime<chrono::Local>> {
        let metadata = std::fs::metadata(path)
            .with_context(|| format!("Failed to get metadata for: {}", path.display()))?;

        let created = metadata
            .modified()
            .or_else(|_| metadata.created())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        let datetime = chrono::DateTime::<chrono::Local>::from(created);
        Ok(datetime)
    }

    /// Calculate directory size
    async fn calculate_directory_size(&self, dir_path: &Path) -> Result<u64> {
        let mut total_size = 0u64;

        fn visit_dir_sync(dir: &Path, total: &mut u64) -> Result<()> {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    visit_dir_sync(&path, total)?;
                } else {
                    *total += entry.metadata()?.len();
                }
            }
            Ok(())
        }

        visit_dir_sync(dir_path, &mut total_size)?;
        Ok(total_size)
    }

    /// Create backup directory for existing configurations
    async fn create_backup_directory(&self) -> Result<PathBuf> {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let backup_dir = self
            .snapshot_manager
            .base_path()
            .join(format!("backup_{timestamp}"));

        async_fs::create_dir_all(&backup_dir)
            .await
            .with_context(|| {
                format!(
                    "Failed to create backup directory: {}",
                    backup_dir.display()
                )
            })?;

        Ok(backup_dir)
    }
}

/// Information about a snapshot
#[derive(Debug, Clone)]
pub struct SnapshotInfo {
    pub name: String,
    #[allow(dead_code)]
    pub path: PathBuf,
    pub created_at: chrono::DateTime<chrono::Local>,
    pub size_bytes: u64,
    pub plugin_count: usize,
}

/// Snapshot metadata structure
#[derive(Debug, serde::Deserialize)]
struct SnapshotMetadata {
    pub timestamp: String,
    pub plugins: Vec<PluginMetadata>,
}

#[derive(Debug, serde::Deserialize)]
struct PluginMetadata {
    #[allow(dead_code)]
    pub name: String,
}

impl SnapshotInfo {
    /// Format file size for human readable output
    pub fn format_size(&self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = self.size_bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", self.size_bytes, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper function to create test snapshot directory with files
    async fn create_test_snapshot_with_files(
        snapshots_dir: &Path,
        snapshot_name: &str,
    ) -> Result<PathBuf> {
        let snapshot_path = snapshots_dir.join(snapshot_name);
        async_fs::create_dir_all(&snapshot_path).await?;

        // Create test plugin files
        async_fs::write(snapshot_path.join("Brewfile"), "tap 'homebrew/core'\n").await?;
        async_fs::write(
            snapshot_path.join("vscode_settings.json"),
            r#"{"test": true}"#,
        )
        .await?;
        async_fs::write(
            snapshot_path.join("npm_global_packages.txt"),
            "typescript\n",
        )
        .await?;

        // Create metadata
        let metadata = serde_json::json!({
            "timestamp": "2025-01-22T10:30:00Z",
            "plugins": [
                {"name": "homebrew_brewfile"},
                {"name": "vscode_settings"},
                {"name": "npm_global_packages"}
            ]
        });
        async_fs::write(
            snapshot_path.join("metadata.json"),
            serde_json::to_string_pretty(&metadata)?,
        )
        .await?;

        Ok(snapshot_path)
    }

    #[tokio::test]
    async fn test_restore_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let registry = Arc::new(PluginRegistry::new());
        let restore_manager = RestoreManager::new(registry, temp_dir.path().to_path_buf());

        assert!(restore_manager.config.is_none());
    }

    #[tokio::test]
    async fn test_restore_manager_with_config() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let registry = Arc::new(PluginRegistry::new());
        let config = Config::default();

        let restore_manager =
            RestoreManager::with_config(registry, temp_dir.path().to_path_buf(), Arc::new(config));

        assert!(restore_manager.config.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn test_list_empty_snapshots() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let registry = Arc::new(PluginRegistry::new());
        let restore_manager = RestoreManager::new(registry, temp_dir.path().to_path_buf());

        let snapshots = restore_manager.list_snapshots().await?;
        assert!(snapshots.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_list_snapshots_with_metadata() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let snapshots_dir = temp_dir.path().to_path_buf();
        async_fs::create_dir_all(&snapshots_dir).await?;

        // Create test snapshots
        create_test_snapshot_with_files(&snapshots_dir, "snapshot1").await?;
        create_test_snapshot_with_files(&snapshots_dir, "snapshot2").await?;

        let registry = Arc::new(PluginRegistry::new());
        let restore_manager = RestoreManager::new(registry, snapshots_dir);

        let snapshots = restore_manager.list_snapshots().await?;
        assert_eq!(snapshots.len(), 2);

        // Check that metadata was parsed correctly
        for snapshot in &snapshots {
            assert_eq!(snapshot.plugin_count, 3);
            assert!(snapshot.size_bytes > 0);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_list_snapshots_without_metadata() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let snapshots_dir = temp_dir.path().to_path_buf();
        async_fs::create_dir_all(&snapshots_dir).await?;

        // Create snapshot without metadata
        let snapshot_path = snapshots_dir.join("no_metadata_snapshot");
        async_fs::create_dir_all(&snapshot_path).await?;
        async_fs::write(snapshot_path.join("Brewfile"), "# Test brewfile\n").await?;

        let registry = Arc::new(PluginRegistry::new());
        let restore_manager = RestoreManager::new(registry, snapshots_dir);

        let snapshots = restore_manager.list_snapshots().await?;
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].plugin_count, 0); // No metadata means 0 plugins
        assert!(snapshots[0].size_bytes > 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_analyze_snapshot() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let snapshots_dir = temp_dir.path().to_path_buf();

        let snapshot_path =
            create_test_snapshot_with_files(&snapshots_dir, "test_snapshot").await?;

        let registry = Arc::new(PluginRegistry::new());
        let restore_manager = RestoreManager::new(registry, snapshots_dir);

        let snapshot_info = restore_manager.analyze_snapshot(&snapshot_path).await?;

        assert_eq!(snapshot_info.name, "test_snapshot");
        assert_eq!(snapshot_info.plugin_count, 3);
        assert!(snapshot_info.size_bytes > 0);
        assert!(snapshot_info.path.ends_with("test_snapshot"));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_restorable_plugins() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let snapshots_dir = temp_dir.path().to_path_buf();

        let snapshot_path =
            create_test_snapshot_with_files(&snapshots_dir, "test_snapshot").await?;

        let registry = Arc::new(PluginRegistry::new());
        let restore_manager = RestoreManager::new(registry, snapshots_dir);

        let plugins = restore_manager
            .get_restorable_plugins(&snapshot_path)
            .await?;

        // Should find 3 plugins (excluding metadata.json)
        assert_eq!(plugins.len(), 3);

        let plugin_names: Vec<String> = plugins.iter().map(|(name, _)| name.clone()).collect();
        assert!(plugin_names.contains(&"homebrew_brewfile".to_string()));
        assert!(plugin_names.contains(&"vscode_settings".to_string()));
        assert!(plugin_names.contains(&"npm_global_packages".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_map_file_to_plugin() {
        let registry = Arc::new(PluginRegistry::new());
        let restore_manager = RestoreManager::new(registry, PathBuf::from("/tmp"));

        // Test known mappings
        assert_eq!(
            restore_manager.map_file_to_plugin("Brewfile"),
            Some("homebrew_brewfile".to_string())
        );
        assert_eq!(
            restore_manager.map_file_to_plugin("vscode_settings.json"),
            Some("vscode_settings".to_string())
        );
        assert_eq!(
            restore_manager.map_file_to_plugin("cursor_extensions.txt"),
            Some("cursor_extensions".to_string())
        );

        // Test unknown files
        assert_eq!(restore_manager.map_file_to_plugin("unknown.file"), None);
        assert_eq!(restore_manager.map_file_to_plugin("metadata.json"), None);
    }

    #[tokio::test]
    async fn test_restore_from_nonexistent_snapshot() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let snapshots_dir = temp_dir.path().to_path_buf();
        async_fs::create_dir_all(&snapshots_dir).await?;

        let registry = Arc::new(PluginRegistry::new());
        let restore_manager = RestoreManager::new(registry, snapshots_dir);

        let result = restore_manager
            .restore_from_snapshot("nonexistent", None, true, false)
            .await;

        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("Snapshot 'nonexistent' not found"));

        Ok(())
    }

    #[tokio::test]
    async fn test_restore_dry_run() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let snapshots_dir = temp_dir.path().to_path_buf();
        async_fs::create_dir_all(&snapshots_dir).await?;

        // Create test snapshot
        create_test_snapshot_with_files(&snapshots_dir, "test_snapshot").await?;

        // Create registry with plugins
        use crate::plugins::{
            homebrew::HomebrewBrewfilePlugin, npm::NpmGlobalPackagesPlugin,
            vscode::VSCodeSettingsPlugin,
        };
        let mut registry = PluginRegistry::new();
        registry.register(Arc::new(HomebrewBrewfilePlugin::new()));
        registry.register(Arc::new(VSCodeSettingsPlugin::new()));
        registry.register(Arc::new(NpmGlobalPackagesPlugin::new()));

        let restore_manager = RestoreManager::new(Arc::new(registry), snapshots_dir);

        // Perform dry run restore
        let results = restore_manager
            .restore_from_snapshot("test_snapshot", None, true, false)
            .await?;

        assert!(!results.is_empty());

        // In dry run mode, all should be successful
        for result in &results {
            assert!(result.success);
            assert!(result.error_message.is_none());
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_restore_with_selected_plugins() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let snapshots_dir = temp_dir.path().to_path_buf();
        async_fs::create_dir_all(&snapshots_dir).await?;

        // Create test snapshot with multiple plugins
        create_test_snapshot_with_files(&snapshots_dir, "test_snapshot").await?;

        // Create registry with multiple plugins
        use crate::plugins::{
            homebrew::HomebrewBrewfilePlugin, npm::NpmGlobalPackagesPlugin,
            vscode::VSCodeSettingsPlugin,
        };
        let mut registry = PluginRegistry::new();
        registry.register(Arc::new(HomebrewBrewfilePlugin::new()));
        registry.register(Arc::new(VSCodeSettingsPlugin::new()));
        registry.register(Arc::new(NpmGlobalPackagesPlugin::new()));

        let restore_manager = RestoreManager::new(Arc::new(registry), snapshots_dir);

        // Restore only homebrew plugin
        let selected_plugins = vec!["homebrew_brewfile".to_string()];
        let results = restore_manager
            .restore_from_snapshot("test_snapshot", Some(&selected_plugins), true, false)
            .await?;

        // Should only restore the selected plugin
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].plugin_name, "homebrew_brewfile");
        assert!(results[0].success);

        Ok(())
    }

    #[tokio::test]
    async fn test_restore_with_backup_dry_run() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let snapshots_dir = temp_dir.path().to_path_buf();
        async_fs::create_dir_all(&snapshots_dir).await?;

        create_test_snapshot_with_files(&snapshots_dir, "test_snapshot").await?;

        use crate::plugins::{
            homebrew::HomebrewBrewfilePlugin, npm::NpmGlobalPackagesPlugin,
            vscode::VSCodeSettingsPlugin,
        };
        let mut registry = PluginRegistry::new();
        registry.register(Arc::new(HomebrewBrewfilePlugin::new()));
        registry.register(Arc::new(VSCodeSettingsPlugin::new()));
        registry.register(Arc::new(NpmGlobalPackagesPlugin::new()));

        let restore_manager = RestoreManager::new(Arc::new(registry), snapshots_dir);

        // Test backup creation in dry run mode
        let results = restore_manager
            .restore_from_snapshot("test_snapshot", None, true, true)
            .await?;

        assert!(!results.is_empty());

        // In dry run mode with backup, backup_path should be set
        for result in &results {
            assert!(result.success);
            assert!(result.backup_path.is_some());
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_create_backup_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let snapshots_dir = temp_dir.path().to_path_buf();

        let registry = Arc::new(PluginRegistry::new());
        let restore_manager = RestoreManager::new(registry, snapshots_dir);

        let backup_dir = restore_manager.create_backup_directory().await?;

        assert!(backup_dir.exists());
        assert!(backup_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("backup_"));

        Ok(())
    }

    #[tokio::test]
    async fn test_calculate_directory_size() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("size_test");
        async_fs::create_dir_all(&test_dir).await?;

        // Create test files with known sizes
        async_fs::write(test_dir.join("file1.txt"), "Hello").await?; // 5 bytes
        async_fs::write(test_dir.join("file2.txt"), "World!").await?; // 6 bytes

        let registry = Arc::new(PluginRegistry::new());
        let restore_manager = RestoreManager::new(registry, temp_dir.path().to_path_buf());

        let size = restore_manager.calculate_directory_size(&test_dir).await?;
        assert_eq!(size, 11); // 5 + 6 bytes

        Ok(())
    }

    #[tokio::test]
    async fn test_get_directory_creation_time() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("time_test");
        async_fs::create_dir_all(&test_dir).await?;

        let registry = Arc::new(PluginRegistry::new());
        let restore_manager = RestoreManager::new(registry, temp_dir.path().to_path_buf());

        let creation_time = restore_manager.get_directory_creation_time(&test_dir)?;

        // Should be recent (within last minute)
        let now = chrono::Local::now();
        let diff = now.signed_duration_since(creation_time);
        assert!(diff.num_seconds() < 60);

        Ok(())
    }

    #[test]
    fn test_format_size() {
        let snapshot_info = SnapshotInfo {
            name: "test".to_string(),
            path: PathBuf::from("/test"),
            created_at: chrono::Local::now(),
            size_bytes: 1536,
            plugin_count: 2,
        };

        assert_eq!(snapshot_info.format_size(), "1.5 KB");
    }

    #[test]
    fn test_format_size_bytes() {
        let snapshot_info = SnapshotInfo {
            name: "small".to_string(),
            path: PathBuf::from("/test"),
            created_at: chrono::Local::now(),
            size_bytes: 512,
            plugin_count: 1,
        };

        assert_eq!(snapshot_info.format_size(), "512 B");
    }

    #[test]
    fn test_format_size_megabytes() {
        let snapshot_info = SnapshotInfo {
            name: "large".to_string(),
            path: PathBuf::from("/test"),
            created_at: chrono::Local::now(),
            size_bytes: 2_097_152, // 2 MB
            plugin_count: 5,
        };

        assert_eq!(snapshot_info.format_size(), "2.0 MB");
    }

    #[test]
    fn test_restore_result_creation() {
        let result = RestoreResult {
            plugin_name: "test_plugin".to_string(),
            success: true,
            restored_files: 5,
            backup_path: Some(PathBuf::from("/backup/path")),
            error_message: None,
        };

        assert_eq!(result.plugin_name, "test_plugin");
        assert!(result.success);
        assert_eq!(result.restored_files, 5);
        assert!(result.backup_path.is_some());
        assert!(result.error_message.is_none());

        let failed_result = RestoreResult {
            plugin_name: "failed_plugin".to_string(),
            success: false,
            restored_files: 0,
            backup_path: None,
            error_message: Some("Test error".to_string()),
        };

        assert!(!failed_result.success);
        assert_eq!(failed_result.restored_files, 0);
        assert!(failed_result.backup_path.is_none());
        assert_eq!(failed_result.error_message.as_deref(), Some("Test error"));
    }
}

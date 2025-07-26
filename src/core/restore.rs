use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::core::plugin::PluginRegistry;
use crate::symbols::*;

/// Manages the restoration of configuration files from snapshots
pub struct RestoreManager {
    snapshot_path: PathBuf,
    default_target_directory: PathBuf,
    global_target_override: Option<PathBuf>,
    dry_run: bool,
    backup: bool,
    force: bool,
    plugin_registry: PluginRegistry,
}

/// Information about a file restoration operation
#[derive(Debug, Clone)]
pub struct RestoreOperation {
    pub source_path: PathBuf,
    pub target_path: PathBuf,
    pub plugin_name: String,
    pub operation_type: RestoreOperationType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RestoreOperationType {
    Copy,
    Skip,
}

impl RestoreManager {
    /// Create a new RestoreManager with the specified configuration
    pub fn new(
        snapshot_path: PathBuf,
        default_target_directory: PathBuf,
        global_target_override: Option<PathBuf>,
        config: Config,
        dry_run: bool,
        backup: bool,
        force: bool,
    ) -> Self {
        // Auto-discover and register plugins for restoration
        let plugin_registry = PluginRegistry::discover_plugins(Some(&config));

        Self {
            snapshot_path,
            default_target_directory,
            global_target_override,
            dry_run,
            backup,
            force,
            plugin_registry,
        }
    }

    /// Execute the restoration process
    pub async fn execute_restore(
        &self,
        selected_plugins: Option<Vec<String>>,
    ) -> Result<Vec<PathBuf>> {
        info!("{} Analyzing snapshot structure...", SYMBOL_ACTION_SEARCH);

        // Discover available plugins in the snapshot
        let available_plugins = self.discover_snapshot_plugins().await?;

        if available_plugins.is_empty() {
            warn!(
                "{} No plugin data found in snapshot",
                SYMBOL_INDICATOR_WARNING
            );
            return Ok(vec![]);
        }

        info!(
            "{} Found {} plugin(s) in snapshot: {}",
            SYMBOL_INDICATOR_SUCCESS,
            available_plugins.len(),
            available_plugins.join(", ")
        );

        // Filter plugins based on selection
        let plugins_to_restore = if let Some(selected) = selected_plugins {
            self.filter_plugins(&available_plugins, &selected)?
        } else {
            available_plugins
        };

        if plugins_to_restore.is_empty() {
            warn!(
                "{} No plugins selected for restoration",
                SYMBOL_INDICATOR_WARNING
            );
            return Ok(vec![]);
        }

        info!(
            "{} Restoring {} plugin(s): {}",
            SYMBOL_ACTION_RESTORE,
            plugins_to_restore.len(),
            plugins_to_restore.join(", ")
        );

        // Plan restoration operations
        let operations = self.plan_restore_operations(&plugins_to_restore).await?;

        if operations.is_empty() {
            warn!("{} No files to restore", SYMBOL_INDICATOR_WARNING);
            return Ok(vec![]);
        }

        info!(
            "{} Planned {} restoration operation(s)",
            SYMBOL_INDICATOR_INFO,
            operations.len()
        );

        // Show preview if dry run
        if self.dry_run {
            self.show_restore_preview(&operations).await?;
            return Ok(operations.iter().map(|op| op.target_path.clone()).collect());
        }

        // Request confirmation if not forced
        if !self.force {
            self.request_confirmation(&operations).await?;
        }

        // Execute restoration operations
        let restored_files = self.execute_operations(&operations).await?;

        info!(
            "{} Restoration completed: {} files restored",
            SYMBOL_INDICATOR_SUCCESS,
            restored_files.len()
        );

        Ok(restored_files)
    }

    /// Discover which plugins have data in the snapshot
    async fn discover_snapshot_plugins(&self) -> Result<Vec<String>> {
        let mut plugins = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.snapshot_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    // Skip hidden directories and system directories
                    if !name.starts_with('.') && name != "metadata" {
                        plugins.push(name.to_string());
                    }
                }
            }
        }

        Ok(plugins)
    }

    /// Filter plugins based on user selection
    fn filter_plugins(&self, available: &[String], selected: &[String]) -> Result<Vec<String>> {
        let mut filtered = Vec::new();

        for selection in selected {
            let selection = selection.trim();

            // Handle wildcard patterns
            if selection.contains('*') {
                let pattern = selection.replace('*', "");
                for plugin in available {
                    if plugin.contains(&pattern) {
                        filtered.push(plugin.clone());
                    }
                }
            } else if available.contains(&selection.to_string()) {
                filtered.push(selection.to_string());
            } else {
                warn!(
                    "{} Plugin '{}' not found in snapshot",
                    SYMBOL_INDICATOR_WARNING, selection
                );
            }
        }

        if filtered.is_empty() && !selected.is_empty() {
            return Err(anyhow::anyhow!(
                "None of the selected plugins were found in the snapshot"
            ));
        }

        // Remove duplicates
        filtered.sort();
        filtered.dedup();

        Ok(filtered)
    }

    /// Plan all restoration operations
    async fn plan_restore_operations(&self, plugins: &[String]) -> Result<Vec<RestoreOperation>> {
        let mut operations = Vec::new();

        for plugin_name in plugins {
            let plugin_operations = self.plan_plugin_restore(plugin_name).await?;
            operations.extend(plugin_operations);
        }

        Ok(operations)
    }

    /// Plan restoration operations for a specific plugin
    async fn plan_plugin_restore(&self, plugin_name: &str) -> Result<Vec<RestoreOperation>> {
        let plugin_snapshot_path = self.snapshot_path.join(plugin_name);

        if !plugin_snapshot_path.exists() {
            debug!(
                "Plugin snapshot path does not exist: {}",
                plugin_snapshot_path.display()
            );
            return Ok(vec![]);
        }

        // Check if we have a plugin implementation for custom restore logic
        if let Some(plugin) = self.plugin_registry.get_plugin(plugin_name) {
            debug!("Using plugin-specific restore logic for: {}", plugin_name);

            // Determine target directory with proper precedence:
            // 1. CLI --target-dir (global_target_override) overrides everything
            // 2. Plugin's restore_target_dir configuration
            // 3. Plugin's default restore target directory
            let target_directory = if let Some(global_override) = &self.global_target_override {
                global_override.clone()
            } else if let Some(plugin_target) = plugin.get_restore_target_dir() {
                PathBuf::from(shellexpand::tilde(&plugin_target).as_ref())
            } else {
                plugin.get_default_restore_target_dir()?
            };

            // Get restored files from plugin-specific logic
            let restored_files = plugin
                .restore(&plugin_snapshot_path, &target_directory, self.dry_run)
                .await?;

            // Convert to RestoreOperations for consistency
            let mut operations = Vec::new();
            for file_path in restored_files {
                operations.push(RestoreOperation {
                    source_path: plugin_snapshot_path.clone(), // Approximate - plugin handles details
                    target_path: file_path,
                    plugin_name: plugin_name.to_string(),
                    operation_type: RestoreOperationType::Copy,
                });
            }

            // If plugin didn't handle any files, fall back to generic logic
            if operations.is_empty() {
                debug!(
                    "Plugin {} didn't handle restoration, using generic file copying",
                    plugin_name
                );
                self.plan_directory_restore(
                    &plugin_snapshot_path,
                    &target_directory,
                    plugin_name,
                    &mut operations,
                )
                .await?;
            }

            Ok(operations)
        } else {
            debug!(
                "No plugin implementation found for {}, using generic file copying",
                plugin_name
            );

            // For plugins without implementation, use default target directory
            // (no plugin-specific configuration available)
            let target_directory = self
                .global_target_override
                .as_ref()
                .unwrap_or(&self.default_target_directory)
                .clone();

            let mut operations = Vec::new();
            self.plan_directory_restore(
                &plugin_snapshot_path,
                &target_directory,
                plugin_name,
                &mut operations,
            )
            .await?;
            Ok(operations)
        }
    }

    /// Recursively plan restoration for a directory
    fn plan_directory_restore<'a>(
        &'a self,
        source_dir: &'a Path,
        target_dir: &'a Path,
        plugin_name: &'a str,
        operations: &'a mut Vec<RestoreOperation>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + 'a>> {
        Box::pin(async move {
            let mut entries = tokio::fs::read_dir(source_dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let source_path = entry.path();
                let relative_path = source_path.strip_prefix(source_dir)?;
                let target_path = target_dir.join(relative_path);

                if source_path.is_dir() {
                    // Recursively plan for subdirectories
                    self.plan_directory_restore(
                        &source_path,
                        &target_path,
                        plugin_name,
                        operations,
                    )
                    .await?;
                } else {
                    // Plan file restoration
                    let operation_type = self
                        .determine_operation_type(&source_path, &target_path)
                        .await?;

                    operations.push(RestoreOperation {
                        source_path,
                        target_path,
                        plugin_name: plugin_name.to_string(),
                        operation_type,
                    });
                }
            }

            Ok(())
        })
    }

    /// Determine the type of operation needed for a file
    async fn determine_operation_type(
        &self,
        source_path: &Path,
        target_path: &Path,
    ) -> Result<RestoreOperationType> {
        if !target_path.exists() {
            return Ok(RestoreOperationType::Copy);
        }

        // Check if files are different
        let source_content = tokio::fs::read(source_path).await?;
        let target_content = tokio::fs::read(target_path).await?;

        if source_content == target_content {
            Ok(RestoreOperationType::Skip)
        } else {
            // For now, we'll use Copy (overwrite) strategy
            // Future enhancement: implement merge strategies for specific file types
            Ok(RestoreOperationType::Copy)
        }
    }

    /// Show preview of restore operations
    async fn show_restore_preview(&self, operations: &[RestoreOperation]) -> Result<()> {
        info!("{} RESTORE PREVIEW:", SYMBOL_INDICATOR_INFO);
        info!("");

        let mut by_plugin: HashMap<String, Vec<&RestoreOperation>> = HashMap::new();
        for op in operations {
            by_plugin
                .entry(op.plugin_name.clone())
                .or_default()
                .push(op);
        }

        for (plugin_name, plugin_ops) in by_plugin {
            info!("{} {}:", SYMBOL_TOOL_PLUGIN, plugin_name);

            for op in plugin_ops {
                let symbol = match op.operation_type {
                    RestoreOperationType::Copy => SYMBOL_CONTENT_ARROW_RIGHT,
                    RestoreOperationType::Skip => SYMBOL_CONTENT_SKIP,
                };

                info!(
                    "  {} {} → {}",
                    symbol,
                    op.source_path.display(),
                    op.target_path.display()
                );
            }
            info!("");
        }

        Ok(())
    }

    /// Request user confirmation for restore operations
    async fn request_confirmation(&self, operations: &[RestoreOperation]) -> Result<()> {
        use std::io::{self, Write};

        let copy_count = operations
            .iter()
            .filter(|op| op.operation_type == RestoreOperationType::Copy)
            .count();
        let skip_count = operations
            .iter()
            .filter(|op| op.operation_type == RestoreOperationType::Skip)
            .count();

        info!("");
        info!("{} RESTORE SUMMARY:", SYMBOL_INDICATOR_WARNING);
        info!(
            "  {} Files to copy/overwrite: {}",
            SYMBOL_CONTENT_ARROW_RIGHT, copy_count
        );
        info!(
            "  {} Files to skip (identical): {}",
            SYMBOL_CONTENT_SKIP, skip_count
        );

        if self.backup && copy_count > 0 {
            info!(
                "  {} Existing files will be backed up",
                SYMBOL_CONTENT_BACKUP
            );
        }

        info!("");
        print!("{SYMBOL_EXPERIENCE_QUESTION} Continue with restoration? [y/N]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let answer = input.trim().to_lowercase();
        if !matches!(answer.as_str(), "y" | "yes") {
            info!("{} Restoration cancelled by user", SYMBOL_INDICATOR_INFO);
            return Err(anyhow::anyhow!("Restoration cancelled by user"));
        }

        Ok(())
    }

    /// Execute all restoration operations
    async fn execute_operations(&self, operations: &[RestoreOperation]) -> Result<Vec<PathBuf>> {
        let mut restored_files = Vec::new();

        for operation in operations {
            match operation.operation_type {
                RestoreOperationType::Copy => {
                    if let Err(e) = self.execute_copy_operation(operation).await {
                        error!(
                            "{} Failed to restore {}: {}",
                            SYMBOL_INDICATOR_ERROR,
                            operation.target_path.display(),
                            e
                        );
                        continue;
                    }
                    restored_files.push(operation.target_path.clone());
                }
                RestoreOperationType::Skip => {
                    debug!(
                        "Skipping identical file: {}",
                        operation.target_path.display()
                    );
                }
            }
        }

        Ok(restored_files)
    }

    /// Execute a copy operation (with optional backup)
    async fn execute_copy_operation(&self, operation: &RestoreOperation) -> Result<()> {
        // Create target directory if needed
        if let Some(parent) = operation.target_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        // Backup existing file if requested
        if self.backup && operation.target_path.exists() {
            self.backup_existing_file(&operation.target_path).await?;
        }

        // Copy file
        tokio::fs::copy(&operation.source_path, &operation.target_path)
            .await
            .with_context(|| {
                format!(
                    "Failed to copy {} to {}",
                    operation.source_path.display(),
                    operation.target_path.display()
                )
            })?;

        debug!(
            "Restored: {} → {}",
            operation.source_path.display(),
            operation.target_path.display()
        );

        Ok(())
    }

    /// Create backup of existing file
    async fn backup_existing_file(&self, file_path: &Path) -> Result<()> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_path = file_path.with_extension(format!(
            "{}.backup.{}",
            file_path.extension().and_then(|s| s.to_str()).unwrap_or(""),
            timestamp
        ));

        tokio::fs::copy(file_path, &backup_path)
            .await
            .with_context(|| {
                format!(
                    "Failed to backup {} to {}",
                    file_path.display(),
                    backup_path.display()
                )
            })?;

        debug!(
            "Backed up: {} → {}",
            file_path.display(),
            backup_path.display()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    /// Helper function to create a test RestoreManager
    async fn create_test_restore_manager(
        snapshot_path: PathBuf,
        target_dir: PathBuf,
        dry_run: bool,
    ) -> RestoreManager {
        let config = Config::default();
        RestoreManager::new(
            snapshot_path,
            target_dir.clone(),
            None, // global_target_override
            config,
            dry_run,
            false, // backup
            true,  // force
        )
    }

    /// Test RestoreManager creation
    /// Verifies that RestoreManager is created with correct configuration
    #[tokio::test]
    async fn test_restore_manager_new() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_path = temp_dir.path().join("snapshot");
        let target_dir = temp_dir.path().join("target");
        let config = Config::default();

        let manager = RestoreManager::new(
            snapshot_path.clone(),
            target_dir.clone(),
            Some(PathBuf::from("/override")),
            config,
            true,  // dry_run
            true,  // backup
            false, // force
        );

        assert_eq!(manager.snapshot_path, snapshot_path);
        assert_eq!(manager.default_target_directory, target_dir);
        assert_eq!(
            manager.global_target_override,
            Some(PathBuf::from("/override"))
        );
        assert!(manager.dry_run);
        assert!(manager.backup);
        assert!(!manager.force);
    }

    /// Test RestoreOperation creation and equality
    /// Verifies RestoreOperation struct functionality
    #[test]
    fn test_restore_operation() {
        let op = RestoreOperation {
            source_path: PathBuf::from("/source/file.txt"),
            target_path: PathBuf::from("/target/file.txt"),
            plugin_name: "test_plugin".to_string(),
            operation_type: RestoreOperationType::Copy,
        };

        assert_eq!(op.source_path, PathBuf::from("/source/file.txt"));
        assert_eq!(op.target_path, PathBuf::from("/target/file.txt"));
        assert_eq!(op.plugin_name, "test_plugin");
        assert_eq!(op.operation_type, RestoreOperationType::Copy);

        // Test clone
        let cloned = op.clone();
        assert_eq!(cloned.source_path, op.source_path);
        assert_eq!(cloned.plugin_name, op.plugin_name);
    }

    /// Test RestoreOperationType variants
    /// Verifies operation type functionality
    #[test]
    fn test_restore_operation_type() {
        assert_eq!(RestoreOperationType::Copy, RestoreOperationType::Copy);
        assert_eq!(RestoreOperationType::Skip, RestoreOperationType::Skip);
        assert_ne!(RestoreOperationType::Copy, RestoreOperationType::Skip);
    }

    /// Test discovering plugins in empty snapshot
    /// Verifies behavior when snapshot has no plugin directories
    #[tokio::test]
    async fn test_discover_snapshot_plugins_empty() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_path = temp_dir.path().join("snapshot");
        fs::create_dir_all(&snapshot_path).await.unwrap();

        let manager =
            create_test_restore_manager(snapshot_path, temp_dir.path().join("target"), true).await;

        let plugins = manager.discover_snapshot_plugins().await.unwrap();
        assert!(plugins.is_empty());
    }

    /// Test discovering plugins in snapshot
    /// Verifies correct plugin discovery from snapshot structure
    #[tokio::test]
    async fn test_discover_snapshot_plugins_with_plugins() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_path = temp_dir.path().join("snapshot");

        // Create plugin directories
        fs::create_dir_all(snapshot_path.join("vscode"))
            .await
            .unwrap();
        fs::create_dir_all(snapshot_path.join("homebrew"))
            .await
            .unwrap();
        fs::create_dir_all(snapshot_path.join(".hidden"))
            .await
            .unwrap(); // Should be ignored
        fs::create_dir_all(snapshot_path.join("metadata"))
            .await
            .unwrap(); // Should be ignored

        // Create a file (should be ignored)
        fs::write(snapshot_path.join("file.txt"), "test")
            .await
            .unwrap();

        let manager =
            create_test_restore_manager(snapshot_path, temp_dir.path().join("target"), true).await;

        let mut plugins = manager.discover_snapshot_plugins().await.unwrap();
        plugins.sort(); // Sort for consistent test results

        assert_eq!(plugins, vec!["homebrew", "vscode"]);
    }

    /// Test filter plugins with exact matches
    /// Verifies plugin filtering with exact names
    #[test]
    fn test_filter_plugins_exact_match() {
        let temp_dir = TempDir::new().unwrap();
        let manager = RestoreManager::new(
            temp_dir.path().to_path_buf(),
            temp_dir.path().to_path_buf(),
            None,
            Config::default(),
            false,
            false,
            false,
        );

        let available = vec![
            "vscode".to_string(),
            "homebrew".to_string(),
            "npm".to_string(),
        ];
        let selected = vec!["vscode".to_string(), "npm".to_string()];

        let filtered = manager.filter_plugins(&available, &selected).unwrap();
        assert_eq!(filtered, vec!["npm", "vscode"]); // Sorted
    }

    /// Test filter plugins with wildcards
    /// Verifies wildcard pattern matching
    #[test]
    fn test_filter_plugins_wildcard() {
        let temp_dir = TempDir::new().unwrap();
        let manager = RestoreManager::new(
            temp_dir.path().to_path_buf(),
            temp_dir.path().to_path_buf(),
            None,
            Config::default(),
            false,
            false,
            false,
        );

        let available = vec![
            "vscode_settings".to_string(),
            "vscode_extensions".to_string(),
            "homebrew".to_string(),
            "npm".to_string(),
        ];
        let selected = vec!["vscode*".to_string()];

        let filtered = manager.filter_plugins(&available, &selected).unwrap();
        assert_eq!(filtered, vec!["vscode_extensions", "vscode_settings"]); // Sorted
    }

    /// Test filter plugins with no matches
    /// Verifies error when no plugins match selection
    #[test]
    fn test_filter_plugins_no_matches() {
        let temp_dir = TempDir::new().unwrap();
        let manager = RestoreManager::new(
            temp_dir.path().to_path_buf(),
            temp_dir.path().to_path_buf(),
            None,
            Config::default(),
            false,
            false,
            false,
        );

        let available = vec!["vscode".to_string(), "homebrew".to_string()];
        let selected = vec!["nonexistent".to_string()];

        let result = manager.filter_plugins(&available, &selected);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("None of the selected plugins"));
    }

    /// Test filter plugins with duplicates
    /// Verifies duplicate removal in filtered results
    #[test]
    fn test_filter_plugins_duplicates() {
        let temp_dir = TempDir::new().unwrap();
        let manager = RestoreManager::new(
            temp_dir.path().to_path_buf(),
            temp_dir.path().to_path_buf(),
            None,
            Config::default(),
            false,
            false,
            false,
        );

        let available = vec!["vscode".to_string(), "homebrew".to_string()];
        let selected = vec![
            "vscode".to_string(),
            "vscode".to_string(),
            "homebrew".to_string(),
        ];

        let filtered = manager.filter_plugins(&available, &selected).unwrap();
        assert_eq!(filtered, vec!["homebrew", "vscode"]); // Sorted and deduplicated
    }

    /// Test determine operation type for new file
    /// Verifies Copy operation for non-existent target
    #[tokio::test]
    async fn test_determine_operation_type_new_file() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target.txt");

        fs::write(&source, "content").await.unwrap();
        // target doesn't exist

        let manager = create_test_restore_manager(
            temp_dir.path().to_path_buf(),
            temp_dir.path().to_path_buf(),
            true,
        )
        .await;

        let op_type = manager
            .determine_operation_type(&source, &target)
            .await
            .unwrap();
        assert_eq!(op_type, RestoreOperationType::Copy);
    }

    /// Test determine operation type for identical files
    /// Verifies Skip operation for identical files
    #[tokio::test]
    async fn test_determine_operation_type_identical_files() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target.txt");

        let content = "identical content";
        fs::write(&source, content).await.unwrap();
        fs::write(&target, content).await.unwrap();

        let manager = create_test_restore_manager(
            temp_dir.path().to_path_buf(),
            temp_dir.path().to_path_buf(),
            true,
        )
        .await;

        let op_type = manager
            .determine_operation_type(&source, &target)
            .await
            .unwrap();
        assert_eq!(op_type, RestoreOperationType::Skip);
    }

    /// Test determine operation type for different files
    /// Verifies Copy operation for different files
    #[tokio::test]
    async fn test_determine_operation_type_different_files() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target.txt");

        fs::write(&source, "source content").await.unwrap();
        fs::write(&target, "target content").await.unwrap();

        let manager = create_test_restore_manager(
            temp_dir.path().to_path_buf(),
            temp_dir.path().to_path_buf(),
            true,
        )
        .await;

        let op_type = manager
            .determine_operation_type(&source, &target)
            .await
            .unwrap();
        assert_eq!(op_type, RestoreOperationType::Copy);
    }

    /// Test execute restore with empty snapshot
    /// Verifies behavior when snapshot contains no plugins
    #[tokio::test]
    async fn test_execute_restore_empty_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_path = temp_dir.path().join("snapshot");
        fs::create_dir_all(&snapshot_path).await.unwrap();

        let manager =
            create_test_restore_manager(snapshot_path, temp_dir.path().join("target"), false).await;

        let result = manager.execute_restore(None).await.unwrap();
        assert!(result.is_empty());
    }

    /// Test execute restore with plugin selection
    /// Verifies restore with specific plugin selection
    #[tokio::test]
    async fn test_execute_restore_with_plugin_selection() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_path = temp_dir.path().join("snapshot");

        // Create plugin directories with files
        let vscode_dir = snapshot_path.join("vscode");
        fs::create_dir_all(&vscode_dir).await.unwrap();
        fs::write(vscode_dir.join("settings.json"), "{}")
            .await
            .unwrap();

        let homebrew_dir = snapshot_path.join("homebrew");
        fs::create_dir_all(&homebrew_dir).await.unwrap();
        fs::write(homebrew_dir.join("Brewfile"), "brew 'git'")
            .await
            .unwrap();

        let manager = create_test_restore_manager(
            snapshot_path,
            temp_dir.path().join("target"),
            true, // dry_run
        )
        .await;

        // Select only vscode
        let result = manager
            .execute_restore(Some(vec!["vscode".to_string()]))
            .await
            .unwrap();
        assert!(!result.is_empty());
    }

    /// Test execute restore with no matching plugins
    /// Verifies behavior when selected plugins don't exist
    #[tokio::test]
    async fn test_execute_restore_no_matching_plugins() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_path = temp_dir.path().join("snapshot");

        // Create a plugin directory
        fs::create_dir_all(snapshot_path.join("vscode"))
            .await
            .unwrap();

        let manager =
            create_test_restore_manager(snapshot_path, temp_dir.path().join("target"), true).await;

        // Try to restore non-existent plugin
        let result = manager
            .execute_restore(Some(vec!["nonexistent".to_string()]))
            .await;
        assert!(result.is_err());
    }

    /// Test plan directory restore
    /// Verifies recursive directory restoration planning
    #[tokio::test]
    async fn test_plan_directory_restore() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");

        // Create nested directory structure
        fs::create_dir_all(source_dir.join("subdir")).await.unwrap();
        fs::write(source_dir.join("file1.txt"), "content1")
            .await
            .unwrap();
        fs::write(source_dir.join("subdir/file2.txt"), "content2")
            .await
            .unwrap();

        let manager =
            create_test_restore_manager(temp_dir.path().to_path_buf(), target_dir.clone(), true)
                .await;

        let mut operations = Vec::new();
        manager
            .plan_directory_restore(&source_dir, &target_dir, "test_plugin", &mut operations)
            .await
            .unwrap();

        assert_eq!(operations.len(), 2);
        assert!(operations
            .iter()
            .any(|op| op.target_path.ends_with("file1.txt")));
        assert!(operations
            .iter()
            .any(|op| op.target_path.ends_with("subdir/file2.txt")));
    }

    /// Test backup existing file
    /// Verifies file backup functionality
    #[tokio::test]
    async fn test_backup_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "original content").await.unwrap();

        let manager = RestoreManager::new(
            temp_dir.path().to_path_buf(),
            temp_dir.path().to_path_buf(),
            None,
            Config::default(),
            false,
            true, // backup enabled
            false,
        );

        manager.backup_existing_file(&file_path).await.unwrap();

        // Check that backup file was created
        let mut entries_stream = fs::read_dir(temp_dir.path()).await.unwrap();
        let mut entries = Vec::new();
        while let Some(entry) = entries_stream.next_entry().await.unwrap() {
            entries.push(entry);
        }

        assert_eq!(entries.len(), 2); // Original + backup

        // Verify backup file exists with correct naming pattern
        let has_backup = entries
            .iter()
            .any(|entry| entry.file_name().to_string_lossy().contains(".backup."));
        assert!(has_backup);
    }

    /// Test execute copy operation
    /// Verifies file copy with directory creation
    #[tokio::test]
    async fn test_execute_copy_operation() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("nested/dir/target.txt");

        fs::write(&source, "test content").await.unwrap();

        let manager = RestoreManager::new(
            temp_dir.path().to_path_buf(),
            temp_dir.path().to_path_buf(),
            None,
            Config::default(),
            false,
            false, // no backup
            false,
        );

        let operation = RestoreOperation {
            source_path: source,
            target_path: target.clone(),
            plugin_name: "test".to_string(),
            operation_type: RestoreOperationType::Copy,
        };

        manager.execute_copy_operation(&operation).await.unwrap();

        // Verify file was copied
        assert!(target.exists());
        let content = fs::read_to_string(&target).await.unwrap();
        assert_eq!(content, "test content");
    }

    /// Test execute copy operation with backup
    /// Verifies file copy with existing file backup
    #[tokio::test]
    async fn test_execute_copy_operation_with_backup() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let target = temp_dir.path().join("target.txt");

        fs::write(&source, "new content").await.unwrap();
        fs::write(&target, "old content").await.unwrap();

        let manager = RestoreManager::new(
            temp_dir.path().to_path_buf(),
            temp_dir.path().to_path_buf(),
            None,
            Config::default(),
            false,
            true, // backup enabled
            false,
        );

        let operation = RestoreOperation {
            source_path: source,
            target_path: target.clone(),
            plugin_name: "test".to_string(),
            operation_type: RestoreOperationType::Copy,
        };

        manager.execute_copy_operation(&operation).await.unwrap();

        // Verify file was overwritten
        let content = fs::read_to_string(&target).await.unwrap();
        assert_eq!(content, "new content");

        // Verify backup was created
        let mut entries_stream = fs::read_dir(temp_dir.path()).await.unwrap();
        let mut entries = Vec::new();
        while let Some(entry) = entries_stream.next_entry().await.unwrap() {
            entries.push(entry);
        }

        let backup_count = entries
            .iter()
            .filter(|entry| entry.file_name().to_string_lossy().contains(".backup."))
            .count();
        assert_eq!(backup_count, 1);
    }

    /// Test execute operations with mixed types
    /// Verifies execution of both Copy and Skip operations
    #[tokio::test]
    async fn test_execute_operations_mixed() {
        let temp_dir = TempDir::new().unwrap();

        // Create source files
        let source1 = temp_dir.path().join("source1.txt");
        let source2 = temp_dir.path().join("source2.txt");
        fs::write(&source1, "content1").await.unwrap();
        fs::write(&source2, "content2").await.unwrap();

        let target1 = temp_dir.path().join("target1.txt");
        let target2 = temp_dir.path().join("target2.txt");

        let manager = RestoreManager::new(
            temp_dir.path().to_path_buf(),
            temp_dir.path().to_path_buf(),
            None,
            Config::default(),
            false,
            false,
            false,
        );

        let operations = vec![
            RestoreOperation {
                source_path: source1,
                target_path: target1.clone(),
                plugin_name: "test".to_string(),
                operation_type: RestoreOperationType::Copy,
            },
            RestoreOperation {
                source_path: source2,
                target_path: target2.clone(),
                plugin_name: "test".to_string(),
                operation_type: RestoreOperationType::Skip,
            },
        ];

        let restored = manager.execute_operations(&operations).await.unwrap();

        // Only Copy operations should be in restored files
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0], target1);
        assert!(target1.exists());
        assert!(!target2.exists()); // Skip operation shouldn't create file
    }

    /// Test plan plugin restore with no plugin implementation
    /// Verifies fallback to generic restore logic
    #[tokio::test]
    async fn test_plan_plugin_restore_no_implementation() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_path = temp_dir.path().join("snapshot");
        let plugin_dir = snapshot_path.join("custom_plugin");

        fs::create_dir_all(&plugin_dir).await.unwrap();
        fs::write(plugin_dir.join("config.toml"), "[config]")
            .await
            .unwrap();

        let manager =
            create_test_restore_manager(snapshot_path, temp_dir.path().join("target"), true).await;

        let operations = manager.plan_plugin_restore("custom_plugin").await.unwrap();
        assert_eq!(operations.len(), 1);
        assert!(operations[0].source_path.ends_with("config.toml"));
    }

    /// Test plan plugin restore with nonexistent plugin
    /// Verifies handling of missing plugin directory
    #[tokio::test]
    async fn test_plan_plugin_restore_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let snapshot_path = temp_dir.path().join("snapshot");
        fs::create_dir_all(&snapshot_path).await.unwrap();

        let manager =
            create_test_restore_manager(snapshot_path, temp_dir.path().join("target"), true).await;

        let operations = manager.plan_plugin_restore("nonexistent").await.unwrap();
        assert!(operations.is_empty());
    }
}

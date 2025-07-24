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
        info!("{} Analyzing snapshot structure...", ACTION_SEARCH);

        // Discover available plugins in the snapshot
        let available_plugins = self.discover_snapshot_plugins().await?;

        if available_plugins.is_empty() {
            warn!("{} No plugin data found in snapshot", INDICATOR_WARNING);
            return Ok(vec![]);
        }

        info!(
            "{} Found {} plugin(s) in snapshot: {}",
            INDICATOR_SUCCESS,
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
            warn!("{} No plugins selected for restoration", INDICATOR_WARNING);
            return Ok(vec![]);
        }

        info!(
            "{} Restoring {} plugin(s): {}",
            ACTION_RESTORE,
            plugins_to_restore.len(),
            plugins_to_restore.join(", ")
        );

        // Plan restoration operations
        let operations = self.plan_restore_operations(&plugins_to_restore).await?;

        if operations.is_empty() {
            warn!("{} No files to restore", INDICATOR_WARNING);
            return Ok(vec![]);
        }

        info!(
            "{} Planned {} restoration operation(s)",
            INDICATOR_INFO,
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
            INDICATOR_SUCCESS,
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
                    INDICATOR_WARNING, selection
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
                        target_path.parent().unwrap_or(target_dir),
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
        info!("{} RESTORE PREVIEW:", INDICATOR_INFO);
        info!("");

        let mut by_plugin: HashMap<String, Vec<&RestoreOperation>> = HashMap::new();
        for op in operations {
            by_plugin
                .entry(op.plugin_name.clone())
                .or_default()
                .push(op);
        }

        for (plugin_name, plugin_ops) in by_plugin {
            info!("{} {}:", TOOL_PLUGIN, plugin_name);

            for op in plugin_ops {
                let symbol = match op.operation_type {
                    RestoreOperationType::Copy => CONTENT_ARROW_RIGHT,
                    RestoreOperationType::Skip => CONTENT_SKIP,
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
        info!("{} RESTORE SUMMARY:", INDICATOR_WARNING);
        info!(
            "  {} Files to copy/overwrite: {}",
            CONTENT_ARROW_RIGHT, copy_count
        );
        info!(
            "  {} Files to skip (identical): {}",
            CONTENT_SKIP, skip_count
        );

        if self.backup && copy_count > 0 {
            info!("  {} Existing files will be backed up", CONTENT_BACKUP);
        }

        info!("");
        print!("{EXPERIENCE_QUESTION} Continue with restoration? [y/N]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let answer = input.trim().to_lowercase();
        if !matches!(answer.as_str(), "y" | "yes") {
            info!("{} Restoration cancelled by user", INDICATOR_INFO);
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
                            INDICATOR_ERROR,
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

use anyhow::Result;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::symbols::*;

/// Types of hooks that can be executed
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HookType {
    /// Executed before any plugins run (global setup, validation)
    PreSnapshot,
    /// Executed after all plugins complete (cleanup, notifications)
    PostSnapshot,
    /// Executed before a specific plugin runs (plugin-specific setup)
    PrePlugin,
    /// Executed after a specific plugin completes (plugin-specific cleanup)
    PostPlugin,
}

impl std::fmt::Display for HookType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookType::PreSnapshot => write!(f, "pre-snapshot"),
            HookType::PostSnapshot => write!(f, "post-snapshot"),
            HookType::PrePlugin => write!(f, "pre-plugin"),
            HookType::PostPlugin => write!(f, "post-plugin"),
        }
    }
}

/// Hook action types that can be executed
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum HookAction {
    /// Execute a script or command
    Script {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default = "default_timeout")]
        timeout: u64,
        working_dir: Option<PathBuf>,
        #[serde(default)]
        env_vars: HashMap<String, String>,
    },
    /// Log a message
    Log {
        message: String,
        #[serde(default = "default_log_level")]
        level: String,
    },
    /// Send a system notification (if supported)
    Notify {
        message: String,
        title: Option<String>,
    },
    /// Backup files/directories
    Backup { path: PathBuf, destination: PathBuf },
    /// Cleanup temporary files
    Cleanup {
        #[serde(default)]
        patterns: Vec<String>,
        #[serde(default)]
        directories: Vec<PathBuf>,
        #[serde(default)]
        temp_files: bool,
    },
}

impl std::fmt::Display for HookAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookAction::Script { command, .. } => write!(f, "script: {command}"),
            HookAction::Log { message, .. } => write!(
                f,
                "log: \"{}\"",
                message.chars().take(50).collect::<String>()
            ),
            HookAction::Notify { message, .. } => write!(
                f,
                "notify: \"{}\"",
                message.chars().take(50).collect::<String>()
            ),
            HookAction::Backup { path, destination } => {
                write!(f, "backup: {} → {}", path.display(), destination.display())
            }
            HookAction::Cleanup {
                patterns,
                directories,
                temp_files,
            } => {
                let mut parts = Vec::new();
                if !patterns.is_empty() {
                    parts.push(format!("patterns: {}", patterns.join(", ")));
                }
                if !directories.is_empty() {
                    parts.push(format!("dirs: {}", directories.len()));
                }
                if *temp_files {
                    parts.push("temp_files".to_string());
                }
                write!(f, "cleanup: {}", parts.join(", "))
            }
        }
    }
}

pub fn default_timeout() -> u64 {
    30
}

pub fn default_log_level() -> String {
    "info".to_string()
}

/// Configuration for hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksConfig {
    /// Directory where hook scripts are located
    #[serde(default = "default_scripts_dir")]
    pub scripts_dir: PathBuf,
}

pub fn default_scripts_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("dotsnapshot")
        .join("scripts")
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            scripts_dir: default_scripts_dir(),
        }
    }
}

impl HooksConfig {
    /// Resolve a script command to its full path
    pub fn resolve_script_path(&self, command: &str) -> PathBuf {
        let path = PathBuf::from(command);

        // If absolute path, use as-is
        if path.is_absolute() {
            return path;
        }

        // If relative path, resolve relative to scripts_dir
        self.scripts_dir.join(command)
    }

    /// Expand tilde (~) to home directory if present
    pub fn expand_tilde(path: &Path) -> PathBuf {
        if let Some(path_str) = path.to_str() {
            if let Some(stripped) = path_str.strip_prefix("~/") {
                if let Some(home_dir) = dirs::home_dir() {
                    return home_dir.join(stripped);
                }
            } else if path_str == "~" {
                if let Some(home_dir) = dirs::home_dir() {
                    return home_dir;
                }
            }
        }
        path.to_path_buf()
    }
}

/// Context provided to hooks during execution
#[derive(Debug, Clone)]
pub struct HookContext {
    /// Name of the plugin (if applicable)
    pub plugin_name: Option<String>,
    /// Snapshot name/ID
    pub snapshot_name: String,
    /// Snapshot directory path
    pub snapshot_dir: PathBuf,
    /// Number of files processed so far
    pub file_count: usize,
    /// Additional variables for template interpolation
    pub variables: HashMap<String, String>,
    /// Hook configuration for path resolution
    pub hooks_config: HooksConfig,
}

impl HookContext {
    /// Create a new hook context
    pub fn new(snapshot_name: String, snapshot_dir: PathBuf, hooks_config: HooksConfig) -> Self {
        Self {
            plugin_name: None,
            snapshot_name,
            snapshot_dir,
            file_count: 0,
            variables: HashMap::new(),
            hooks_config,
        }
    }

    /// Set the plugin name for plugin-specific hooks
    pub fn with_plugin(mut self, plugin_name: String) -> Self {
        self.plugin_name = Some(plugin_name);
        self
    }

    /// Set the file count
    pub fn with_file_count(mut self, count: usize) -> Self {
        self.file_count = count;
        self
    }

    /// Add a custom variable for template interpolation
    pub fn with_variable(mut self, key: String, value: String) -> Self {
        self.variables.insert(key, value);
        self
    }

    /// Interpolate template variables in a string
    pub fn interpolate(&self, template: &str) -> String {
        let mut result = template.to_string();

        // Replace standard variables
        result = result.replace("{snapshot_name}", &self.snapshot_name);
        result = result.replace("{snapshot_dir}", &self.snapshot_dir.to_string_lossy());
        result = result.replace("{file_count}", &self.file_count.to_string());

        if let Some(plugin_name) = &self.plugin_name {
            result = result.replace("{plugin_name}", plugin_name);
        }

        // Replace custom variables
        for (key, value) in &self.variables {
            result = result.replace(&format!("{{{key}}}"), value);
        }

        result
    }
}

/// Result of hook execution
#[derive(Debug, Clone)]
pub struct HookResult {
    /// Whether the hook executed successfully
    pub success: bool,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Output from the hook (if any)
    pub output: Option<String>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Hook action that was executed
    pub action: String,
}

/// Trait for executing hook actions
#[async_trait]
pub trait HookExecutor: Send + Sync {
    /// Execute a hook action with the given context
    async fn execute(&self, action: &HookAction, context: &HookContext) -> Result<HookResult>;

    /// Validate that the hook action can be executed
    fn validate(&self, action: &HookAction, context: &HookContext) -> Result<()>;
}

/// Manager for executing hooks at various stages
pub struct HookManager {
    executor: Box<dyn HookExecutor>,
}

impl HookManager {
    /// Create a new hook manager with the default executor
    pub fn new(_hooks_config: HooksConfig) -> Self {
        Self {
            executor: Box::new(DefaultHookExecutor),
        }
    }

    /// Execute hooks with comprehensive logging
    pub async fn execute_hooks(
        &self,
        hooks: &[HookAction],
        hook_type: &HookType,
        context: &HookContext,
    ) -> Vec<HookResult> {
        if hooks.is_empty() {
            return Vec::new();
        }

        let plugin_context = if let Some(plugin_name) = &context.plugin_name {
            format!(" for plugin '{plugin_name}'")
        } else {
            " (global)".to_string()
        };

        info!(
            "{} Executing {} hooks{} ({} hooks)",
            SYMBOL_ACTION_HOOK,
            hook_type,
            plugin_context,
            hooks.len()
        );

        let mut results = Vec::new();

        for (index, hook) in hooks.iter().enumerate() {
            let start_time = std::time::Instant::now();

            // Log hook start with clear identification
            info!(
                "  {} [{}/{}] Starting {hook_type} hook: {hook}",
                SYMBOL_ACTION_HOOK,
                index + 1,
                hooks.len()
            );
            debug!("     Hook details: {hook:#?}");

            let result = self.executor.execute(hook, context).await;

            match result {
                Ok(mut hook_result) => {
                    hook_result.execution_time_ms = start_time.elapsed().as_millis() as u64;
                    hook_result.action = hook.to_string();

                    if hook_result.success {
                        info!(
                            "  {} [{}/{}] {hook_type} hook completed: {hook} ({}ms)",
                            SYMBOL_INDICATOR_SUCCESS,
                            index + 1,
                            hooks.len(),
                            hook_result.execution_time_ms
                        );

                        // Log hook output if available and not too long
                        if let Some(output) = &hook_result.output {
                            if !output.trim().is_empty() && output.len() < 200 {
                                debug!("     Output: {}", output.trim());
                            }
                        }
                    } else {
                        error!(
                            "  {} [{}/{}] {hook_type} hook failed: {hook} ({}ms)",
                            SYMBOL_INDICATOR_ERROR,
                            index + 1,
                            hooks.len(),
                            hook_result.execution_time_ms
                        );

                        if let Some(error) = &hook_result.error {
                            error!("     Error: {error}");
                        }
                    }

                    results.push(hook_result);
                }
                Err(e) => {
                    let execution_time = start_time.elapsed().as_millis() as u64;
                    error!(
                        "  {} [{}/{}] {hook_type} hook execution failed: {hook} ({}ms)",
                        SYMBOL_INDICATOR_ERROR,
                        index + 1,
                        hooks.len(),
                        execution_time
                    );
                    error!("     Error: {e}");

                    results.push(HookResult {
                        success: false,
                        execution_time_ms: execution_time,
                        output: None,
                        error: Some(e.to_string()),
                        action: hook.to_string(),
                    });
                }
            }
        }

        // Summary log
        let successful = results.iter().filter(|r| r.success).count();
        let total_time: u64 = results.iter().map(|r| r.execution_time_ms).sum();

        if successful == results.len() {
            info!(
                "{} {} All {} {hook_type} hooks{} completed successfully (total: {}ms)",
                SYMBOL_ACTION_HOOK,
                SYMBOL_INDICATOR_SUCCESS,
                results.len(),
                plugin_context,
                total_time
            );
        } else {
            warn!(
                "{} {} {}/{} {hook_type} hooks{} completed successfully (total: {}ms)",
                SYMBOL_ACTION_HOOK,
                SYMBOL_INDICATOR_WARNING,
                successful,
                results.len(),
                plugin_context,
                total_time
            );
        }

        results
    }

    /// Validate hooks configuration
    pub fn validate_hooks(&self, hooks: &[HookAction], context: &HookContext) -> Vec<Result<()>> {
        hooks
            .iter()
            .map(|hook| self.executor.validate(hook, context))
            .collect()
    }
}

/// Default implementation of hook executor
pub struct DefaultHookExecutor;

#[async_trait]
impl HookExecutor for DefaultHookExecutor {
    async fn execute(&self, action: &HookAction, context: &HookContext) -> Result<HookResult> {
        match action {
            HookAction::Script {
                command,
                args,
                timeout: timeout_secs,
                working_dir,
                env_vars,
            } => {
                self.execute_script(
                    command,
                    args,
                    *timeout_secs,
                    working_dir.as_ref(),
                    env_vars,
                    context,
                )
                .await
            }
            HookAction::Log { message, level } => self.execute_log(message, level, context).await,
            HookAction::Notify { message, title } => {
                self.execute_notify(message, title.as_deref(), context)
                    .await
            }
            HookAction::Backup { path, destination } => {
                self.execute_backup(path.as_path(), destination.as_path(), context)
                    .await
            }
            HookAction::Cleanup {
                patterns,
                directories,
                temp_files,
            } => {
                self.execute_cleanup(patterns, directories, *temp_files, context)
                    .await
            }
        }
    }

    fn validate(&self, action: &HookAction, context: &HookContext) -> Result<()> {
        match action {
            HookAction::Script {
                command,
                working_dir,
                ..
            } => {
                // Check if command exists (basic validation)
                if command.trim().is_empty() {
                    return Err(anyhow::anyhow!("Script command cannot be empty"));
                }

                // Resolve script path
                let script_path = context.hooks_config.resolve_script_path(command);
                let expanded_path = HooksConfig::expand_tilde(&script_path);

                if !expanded_path.exists() {
                    return Err(anyhow::anyhow!(
                        "Script not found: {} → {}",
                        command,
                        expanded_path.display()
                    ));
                }

                // Check working directory exists if specified
                if let Some(dir) = working_dir {
                    let expanded_dir = HooksConfig::expand_tilde(dir);
                    if !expanded_dir.exists() {
                        return Err(anyhow::anyhow!(
                            "Working directory does not exist: {}",
                            expanded_dir.display()
                        ));
                    }
                }

                Ok(())
            }
            HookAction::Log { message, level } => {
                if message.trim().is_empty() {
                    return Err(anyhow::anyhow!("Log message cannot be empty"));
                }

                match level.as_str() {
                    "trace" | "debug" | "info" | "warn" | "error" => Ok(()),
                    _ => Err(anyhow::anyhow!("Invalid log level: {}", level)),
                }
            }
            HookAction::Notify { message, .. } => {
                if message.trim().is_empty() {
                    return Err(anyhow::anyhow!("Notification message cannot be empty"));
                }
                Ok(())
            }
            HookAction::Backup { path, destination } => {
                let expanded_path = HooksConfig::expand_tilde(path);
                if !expanded_path.exists() {
                    return Err(anyhow::anyhow!(
                        "Backup source path does not exist: {}",
                        expanded_path.display()
                    ));
                }

                let expanded_dest = HooksConfig::expand_tilde(destination);
                if let Some(parent) = expanded_dest.parent() {
                    if !parent.exists() {
                        return Err(anyhow::anyhow!(
                            "Backup destination parent directory does not exist: {}",
                            parent.display()
                        ));
                    }
                }

                Ok(())
            }
            HookAction::Cleanup {
                patterns,
                directories,
                ..
            } => {
                for dir in directories {
                    let expanded_dir = HooksConfig::expand_tilde(dir);
                    if !expanded_dir.exists() {
                        return Err(anyhow::anyhow!(
                            "Cleanup directory does not exist: {}",
                            expanded_dir.display()
                        ));
                    }
                }

                // Basic pattern validation
                for pattern in patterns {
                    if pattern.trim().is_empty() {
                        return Err(anyhow::anyhow!("Cleanup pattern cannot be empty"));
                    }
                }

                Ok(())
            }
        }
    }
}

impl DefaultHookExecutor {
    async fn execute_script(
        &self,
        command: &str,
        args: &[String],
        timeout_secs: u64,
        working_dir: Option<&PathBuf>,
        env_vars: &HashMap<String, String>,
        context: &HookContext,
    ) -> Result<HookResult> {
        // Resolve script path
        let script_path = context.hooks_config.resolve_script_path(command);
        let expanded_path = HooksConfig::expand_tilde(&script_path);

        // Interpolate variables in args
        let interpolated_args: Vec<String> =
            args.iter().map(|arg| context.interpolate(arg)).collect();

        debug!(
            "     Executing script: {} {:?}",
            expanded_path.display(),
            interpolated_args
        );

        let mut cmd = Command::new(&expanded_path);
        cmd.args(&interpolated_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set working directory if specified
        if let Some(dir) = working_dir {
            let expanded_dir = HooksConfig::expand_tilde(dir);
            cmd.current_dir(expanded_dir);
        }

        // Set environment variables
        for (key, value) in env_vars {
            cmd.env(key, context.interpolate(value));
        }

        // Execute with timeout
        let result = timeout(Duration::from_secs(timeout_secs), cmd.output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if output.status.success() {
                    debug!("     Script executed successfully");
                    Ok(HookResult {
                        success: true,
                        execution_time_ms: 0, // Will be set by caller
                        output: Some(stdout.to_string()),
                        error: None,
                        action: command.to_string(),
                    })
                } else {
                    debug!(
                        "     Script failed with exit code: {:?}",
                        output.status.code()
                    );
                    Ok(HookResult {
                        success: false,
                        execution_time_ms: 0,
                        output: Some(stdout.to_string()),
                        error: Some(stderr.to_string()),
                        action: command.to_string(),
                    })
                }
            }
            Ok(Err(e)) => {
                debug!("     Failed to execute script: {}", e);
                Ok(HookResult {
                    success: false,
                    execution_time_ms: 0,
                    output: None,
                    error: Some(format!("Failed to execute: {e}")),
                    action: command.to_string(),
                })
            }
            Err(_) => {
                debug!("     Script timed out after {} seconds", timeout_secs);
                Ok(HookResult {
                    success: false,
                    execution_time_ms: 0,
                    output: None,
                    error: Some(format!("Timeout after {timeout_secs} seconds")),
                    action: command.to_string(),
                })
            }
        }
    }

    async fn execute_log(
        &self,
        message: &str,
        level: &str,
        context: &HookContext,
    ) -> Result<HookResult> {
        let interpolated_message = context.interpolate(message);

        match level {
            "trace" => tracing::trace!("     Hook log: {}", interpolated_message),
            "debug" => tracing::debug!("     Hook log: {}", interpolated_message),
            "info" => tracing::info!("     Hook log: {}", interpolated_message),
            "warn" => tracing::warn!("     Hook log: {}", interpolated_message),
            "error" => tracing::error!("     Hook log: {}", interpolated_message),
            _ => tracing::info!("     Hook log: {}", interpolated_message),
        }

        Ok(HookResult {
            success: true,
            execution_time_ms: 0,
            output: Some(interpolated_message),
            error: None,
            action: format!("log: {}", message.chars().take(50).collect::<String>()),
        })
    }

    async fn execute_notify(
        &self,
        message: &str,
        title: Option<&str>,
        context: &HookContext,
    ) -> Result<HookResult> {
        let interpolated_message = context.interpolate(message);
        let interpolated_title = title.map(|t| context.interpolate(t));

        // For now, just log the notification. In the future, this could integrate with system notifications
        info!(
            "     {} {}: {}",
            SYMBOL_DOC_ANNOUNCEMENT,
            interpolated_title.unwrap_or_else(|| "dotsnapshot".to_string()),
            interpolated_message
        );

        Ok(HookResult {
            success: true,
            execution_time_ms: 0,
            output: Some(format!("Notification: {interpolated_message}")),
            error: None,
            action: format!("notify: {}", message.chars().take(50).collect::<String>()),
        })
    }

    async fn execute_backup(
        &self,
        path: &Path,
        destination: &Path,
        _context: &HookContext,
    ) -> Result<HookResult> {
        let expanded_path = HooksConfig::expand_tilde(path);
        let expanded_dest = HooksConfig::expand_tilde(destination);

        // Simple backup implementation - copy files/directories
        let result = if expanded_path.is_dir() {
            copy_dir_all(expanded_path.clone(), expanded_dest.clone()).await
        } else {
            tokio::fs::copy(&expanded_path, &expanded_dest)
                .await
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!(e))
        };

        match result {
            Ok(_) => {
                debug!(
                    "     Backed up {} to {}",
                    expanded_path.display(),
                    expanded_dest.display()
                );
                Ok(HookResult {
                    success: true,
                    execution_time_ms: 0,
                    output: Some(format!(
                        "Backed up {} to {}",
                        expanded_path.display(),
                        expanded_dest.display()
                    )),
                    error: None,
                    action: format!("backup: {} → {}", path.display(), destination.display()),
                })
            }
            Err(e) => {
                debug!(
                    "     Failed to backup {} to {}: {}",
                    expanded_path.display(),
                    expanded_dest.display(),
                    e
                );
                Ok(HookResult {
                    success: false,
                    execution_time_ms: 0,
                    output: None,
                    error: Some(format!("Backup failed: {e}")),
                    action: format!("backup: {} → {}", path.display(), destination.display()),
                })
            }
        }
    }

    async fn execute_cleanup(
        &self,
        patterns: &[String],
        directories: &[PathBuf],
        temp_files: bool,
        _context: &HookContext,
    ) -> Result<HookResult> {
        let mut cleaned_files = 0;
        let mut errors = Vec::new();

        // Clean up specified directories with patterns
        for dir in directories {
            let expanded_dir = HooksConfig::expand_tilde(dir);
            for pattern in patterns {
                match cleanup_pattern(&expanded_dir, pattern).await {
                    Ok(count) => cleaned_files += count,
                    Err(e) => errors.push(format!(
                        "Failed to clean {}/{}: {}",
                        expanded_dir.display(),
                        pattern,
                        e
                    )),
                }
            }
        }

        // Clean up temp files if requested
        if temp_files {
            match cleanup_temp_files().await {
                Ok(count) => cleaned_files += count,
                Err(e) => errors.push(format!("Failed to clean temp files: {e}")),
            }
        }

        let success = errors.is_empty();
        let message = format!("Cleaned up {cleaned_files} files");

        if success {
            debug!("     {}", message);
        } else {
            debug!("     {} (with {} errors)", message, errors.len());
        }

        Ok(HookResult {
            success,
            execution_time_ms: 0,
            output: Some(message),
            error: if errors.is_empty() {
                None
            } else {
                Some(errors.join("; "))
            },
            action: "cleanup".to_string(),
        })
    }
}

// Helper functions

pub fn copy_dir_all(
    src: PathBuf,
    dst: PathBuf,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>> {
    Box::pin(async move {
        tokio::fs::create_dir_all(&dst)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let mut entries = tokio::fs::read_dir(&src)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| anyhow::anyhow!(e))? {
            let path = entry.path();
            let name = entry.file_name();
            let dest_path = dst.join(name);

            if path.is_dir() {
                copy_dir_all(path, dest_path).await?;
            } else {
                tokio::fs::copy(&path, &dest_path)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
            }
        }

        Ok(())
    })
}

async fn cleanup_pattern(dir: &PathBuf, pattern: &str) -> Result<usize> {
    // Simple glob-like pattern matching - this could be enhanced with proper glob support
    let mut count = 0;
    let mut entries = tokio::fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            if simple_pattern_match(pattern, filename) {
                if let Err(e) = tokio::fs::remove_file(&path).await {
                    warn!("Failed to remove {}: {}", path.display(), e);
                } else {
                    count += 1;
                }
            }
        }
    }

    Ok(count)
}

async fn cleanup_temp_files() -> Result<usize> {
    // Clean up common temp file locations
    let temp_dirs = vec![
        std::env::temp_dir(),
        PathBuf::from("/tmp"),
        PathBuf::from("/var/tmp"),
    ];

    let mut count = 0;
    for temp_dir in temp_dirs {
        if temp_dir.exists() {
            // Only clean dotsnapshot-related temp files to be safe
            if let Ok(temp_count) = cleanup_pattern(&temp_dir, "dotsnapshot*").await {
                count += temp_count;
            }
        }
    }

    Ok(count)
}

pub fn simple_pattern_match(pattern: &str, filename: &str) -> bool {
    // Simple wildcard matching - could be enhanced with proper glob
    if pattern == "*" {
        return true;
    }

    if pattern.starts_with('*') && pattern.ends_with('*') {
        let middle = &pattern[1..pattern.len() - 1];
        return filename.contains(middle);
    }

    if let Some(suffix) = pattern.strip_prefix('*') {
        return filename.ends_with(suffix);
    }

    if let Some(prefix) = pattern.strip_suffix('*') {
        return filename.starts_with(prefix);
    }

    pattern == filename
}

#[cfg(test)]
mod tests;

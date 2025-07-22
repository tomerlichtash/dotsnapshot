use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Types of hooks that can be executed
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookType {
    /// Executed before any plugins run (global setup, validation)
    PreSnapshot,
    /// Executed after all plugins complete (cleanup, notifications)
    PostSnapshot,
    /// Executed before a specific plugin runs during snapshot (plugin-specific setup)
    PrePluginSnapshot,
    /// Executed after a specific plugin completes during snapshot (plugin-specific cleanup)
    PostPluginSnapshot,
    /// Executed before any plugins are restored (global restore setup)
    PreRestore,
    /// Executed after all plugins are restored (global restore cleanup)
    PostRestore,
    /// Executed before a specific plugin is restored (plugin-specific restore setup)
    PrePluginRestore,
    /// Executed after a specific plugin is restored (plugin-specific restore cleanup)
    PostPluginRestore,
}

impl std::fmt::Display for HookType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookType::PreSnapshot => write!(f, "pre-snapshot"),
            HookType::PostSnapshot => write!(f, "post-snapshot"),
            HookType::PrePluginSnapshot => write!(f, "pre-plugin-snapshot"),
            HookType::PostPluginSnapshot => write!(f, "post-plugin-snapshot"),
            HookType::PreRestore => write!(f, "pre-restore"),
            HookType::PostRestore => write!(f, "post-restore"),
            HookType::PrePluginRestore => write!(f, "pre-plugin-restore"),
            HookType::PostPluginRestore => write!(f, "post-plugin-restore"),
        }
    }
}

/// Hook action types that can be executed
#[derive(Debug, Clone, Serialize, Deserialize)]
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

fn default_timeout() -> u64 {
    30
}

fn default_log_level() -> String {
    "info".to_string()
}

/// Configuration for hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksConfig {
    /// Directory where hook scripts are located
    #[serde(default = "default_scripts_dir")]
    pub scripts_dir: PathBuf,
}

fn default_scripts_dir() -> PathBuf {
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
    #[allow(dead_code)]
    hooks_config: HooksConfig,
}

impl HookManager {
    /// Create a new hook manager with the default executor
    pub fn new(hooks_config: HooksConfig) -> Self {
        Self {
            executor: Box::new(DefaultHookExecutor),
            hooks_config,
        }
    }

    /// Create a hook manager with a custom executor
    #[allow(dead_code)]
    pub fn with_executor(hooks_config: HooksConfig, executor: Box<dyn HookExecutor>) -> Self {
        Self {
            executor,
            hooks_config,
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
            "🪝 Executing {} hooks{} ({} hooks)",
            hook_type,
            plugin_context,
            hooks.len()
        );

        let mut results = Vec::new();

        for (index, hook) in hooks.iter().enumerate() {
            let start_time = std::time::Instant::now();

            // Log hook start with clear identification
            info!(
                "  🪝 [{}/{}] Starting {hook_type} hook: {hook}",
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
                            "  ✅ [{}/{}] {hook_type} hook completed: {hook} ({}ms)",
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
                            "  ❌ [{}/{}] {hook_type} hook failed: {hook} ({}ms)",
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
                        "  ❌ [{}/{}] {hook_type} hook execution failed: {hook} ({}ms)",
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
                "🪝 ✅ All {} {hook_type} hooks{} completed successfully (total: {}ms)",
                results.len(),
                plugin_context,
                total_time
            );
        } else {
            warn!(
                "🪝 ⚠️  {}/{} {hook_type} hooks{} completed successfully (total: {}ms)",
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
            "     📢 {}: {}",
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

fn copy_dir_all(
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

fn simple_pattern_match(pattern: &str, filename: &str) -> bool {
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
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;
    use tokio::fs;

    // Helper function to create a test script
    async fn create_test_script(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let script_path = dir.path().join(name);
        fs::write(&script_path, content)
            .await
            .expect("Failed to create test script");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path).await.unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).await.unwrap();
        }

        script_path
    }

    #[test]
    fn test_hook_context_interpolation() {
        let hooks_config = HooksConfig::default();
        let mut context = HookContext::new(
            "test_snapshot".to_string(),
            PathBuf::from("/tmp/snapshots/test"),
            hooks_config,
        );
        context = context
            .with_plugin("homebrew".to_string())
            .with_file_count(42)
            .with_variable("custom_var".to_string(), "custom_value".to_string());

        let template = "Snapshot: {snapshot_name}, Plugin: {plugin_name}, Files: {file_count}, Custom: {custom_var}";
        let result = context.interpolate(template);

        assert_eq!(
            result,
            "Snapshot: test_snapshot, Plugin: homebrew, Files: 42, Custom: custom_value"
        );
    }

    #[test]
    fn test_hook_context_partial_interpolation() {
        let hooks_config = HooksConfig::default();
        let context = HookContext::new(
            "test_snapshot".to_string(),
            PathBuf::from("/tmp/snapshots/test"),
            hooks_config,
        );

        // Test without plugin context (should leave {plugin_name} as-is)
        let template = "Snapshot: {snapshot_name}, Plugin: {plugin_name}, Unknown: {unknown_var}";
        let result = context.interpolate(template);

        assert_eq!(
            result,
            "Snapshot: test_snapshot, Plugin: {plugin_name}, Unknown: {unknown_var}"
        );
    }

    #[test]
    fn test_simple_pattern_match() {
        assert!(simple_pattern_match("*", "anything.txt"));
        assert!(simple_pattern_match("*.txt", "file.txt"));
        assert!(!simple_pattern_match("*.txt", "file.log"));
        assert!(simple_pattern_match("test*", "test123"));
        assert!(!simple_pattern_match("test*", "other123"));
        assert!(simple_pattern_match("*tmp*", "file.tmp.bak"));
        assert!(!simple_pattern_match("*tmp*", "file.log"));
        assert!(simple_pattern_match("exact.txt", "exact.txt"));
        assert!(!simple_pattern_match("exact.txt", "other.txt"));
    }

    #[test]
    fn test_hook_type_display() {
        assert_eq!(HookType::PreSnapshot.to_string(), "pre-snapshot");
        assert_eq!(HookType::PostSnapshot.to_string(), "post-snapshot");
        assert_eq!(
            HookType::PrePluginSnapshot.to_string(),
            "pre-plugin-snapshot"
        );
        assert_eq!(
            HookType::PostPluginSnapshot.to_string(),
            "post-plugin-snapshot"
        );
    }

    #[test]
    fn test_hook_action_display() {
        let script_action = HookAction::Script {
            command: "test-script.sh".to_string(),
            args: vec!["arg1".to_string(), "arg2".to_string()],
            timeout: 30,
            working_dir: None,
            env_vars: HashMap::new(),
        };
        assert_eq!(script_action.to_string(), "script: test-script.sh");

        let log_action = HookAction::Log {
            message: "Test log message that is very long and should be truncated at fifty characters or so".to_string(),
            level: "info".to_string(),
        };
        assert_eq!(
            log_action.to_string(),
            "log: \"Test log message that is very long and should be t\""
        );

        let backup_action = HookAction::Backup {
            path: PathBuf::from("/source"),
            destination: PathBuf::from("/dest"),
        };
        assert_eq!(backup_action.to_string(), "backup: /source → /dest");
    }

    #[test]
    fn test_hooks_config_path_resolution() {
        let temp_dir = TempDir::new().unwrap();
        let hooks_config = HooksConfig {
            scripts_dir: temp_dir.path().to_path_buf(),
        };

        // Test relative path resolution
        let relative_path = hooks_config.resolve_script_path("test-script.sh");
        assert_eq!(relative_path, temp_dir.path().join("test-script.sh"));

        // Test absolute path (should be unchanged)
        #[cfg(unix)]
        {
            let absolute_path = hooks_config.resolve_script_path("/usr/bin/test");
            assert_eq!(absolute_path, PathBuf::from("/usr/bin/test"));
        }
        #[cfg(windows)]
        {
            let absolute_path = hooks_config.resolve_script_path("C:\\Windows\\System32\\cmd.exe");
            assert_eq!(
                absolute_path,
                PathBuf::from("C:\\Windows\\System32\\cmd.exe")
            );
        }

        // Test subdirectory path
        let subdir_path = hooks_config.resolve_script_path("hooks/test.sh");
        assert_eq!(subdir_path, temp_dir.path().join("hooks/test.sh"));
    }

    #[test]
    fn test_hooks_config_tilde_expansion() {
        // Test tilde expansion
        let home_path = PathBuf::from("~/test/path");
        let expanded = HooksConfig::expand_tilde(&home_path);

        if let Some(home_dir) = dirs::home_dir() {
            assert_eq!(expanded, home_dir.join("test/path"));
        } else {
            // If no home directory, should return original path
            assert_eq!(expanded, home_path);
        }

        // Test non-tilde path (should be unchanged)
        let regular_path = PathBuf::from("/regular/path");
        let not_expanded = HooksConfig::expand_tilde(&regular_path);
        assert_eq!(not_expanded, regular_path);
    }

    #[tokio::test]
    async fn test_hook_action_validation() {
        let temp_dir = TempDir::new().unwrap();
        let hooks_config = HooksConfig {
            scripts_dir: temp_dir.path().to_path_buf(),
        };
        let context = HookContext::new("test".to_string(), PathBuf::from("/tmp"), hooks_config);
        let executor = DefaultHookExecutor;

        // Valid log action
        let valid_log = HookAction::Log {
            message: "Test message".to_string(),
            level: "info".to_string(),
        };
        assert!(executor.validate(&valid_log, &context).is_ok());

        // Invalid log action (bad level)
        let invalid_log = HookAction::Log {
            message: "Test message".to_string(),
            level: "invalid_level".to_string(),
        };
        assert!(executor.validate(&invalid_log, &context).is_err());

        // Valid notify action
        let valid_notify = HookAction::Notify {
            message: "Test notification".to_string(),
            title: Some("Test Title".to_string()),
        };
        assert!(executor.validate(&valid_notify, &context).is_ok());

        // Invalid notify action (empty message)
        let invalid_notify = HookAction::Notify {
            message: "".to_string(),
            title: None,
        };
        assert!(executor.validate(&invalid_notify, &context).is_err());
    }

    #[tokio::test]
    async fn test_script_action_validation() {
        let temp_dir = TempDir::new().unwrap();
        let hooks_config = HooksConfig {
            scripts_dir: temp_dir.path().to_path_buf(),
        };
        let context = HookContext::new("test".to_string(), PathBuf::from("/tmp"), hooks_config);
        let executor = DefaultHookExecutor;

        // Create a test script
        create_test_script(&temp_dir, "valid-script.sh", "#!/bin/bash\necho 'test'").await;

        // Valid script action (script exists)
        let valid_script = HookAction::Script {
            command: "valid-script.sh".to_string(),
            args: vec![],
            timeout: 30,
            working_dir: None,
            env_vars: HashMap::new(),
        };
        assert!(executor.validate(&valid_script, &context).is_ok());

        // Invalid script action (script doesn't exist)
        let invalid_script = HookAction::Script {
            command: "nonexistent-script.sh".to_string(),
            args: vec![],
            timeout: 30,
            working_dir: None,
            env_vars: HashMap::new(),
        };
        assert!(executor.validate(&invalid_script, &context).is_err());

        // Invalid script action (empty command)
        let empty_script = HookAction::Script {
            command: "".to_string(),
            args: vec![],
            timeout: 30,
            working_dir: None,
            env_vars: HashMap::new(),
        };
        assert!(executor.validate(&empty_script, &context).is_err());
    }

    #[tokio::test]
    async fn test_backup_action_validation() {
        let temp_dir = TempDir::new().unwrap();
        let hooks_config = HooksConfig::default();
        let context = HookContext::new("test".to_string(), PathBuf::from("/tmp"), hooks_config);
        let executor = DefaultHookExecutor;

        // Create a test file to backup
        let test_file = temp_dir.path().join("test-file.txt");
        fs::write(&test_file, "test content").await.unwrap();

        // Valid backup action
        let valid_backup = HookAction::Backup {
            path: test_file.clone(),
            destination: temp_dir.path().join("backup.txt"),
        };
        assert!(executor.validate(&valid_backup, &context).is_ok());

        // Invalid backup action (source doesn't exist)
        let invalid_backup = HookAction::Backup {
            path: temp_dir.path().join("nonexistent.txt"),
            destination: temp_dir.path().join("backup.txt"),
        };
        assert!(executor.validate(&invalid_backup, &context).is_err());
    }

    #[tokio::test]
    async fn test_cleanup_action_validation() {
        let temp_dir = TempDir::new().unwrap();
        let hooks_config = HooksConfig::default();
        let context = HookContext::new("test".to_string(), PathBuf::from("/tmp"), hooks_config);
        let executor = DefaultHookExecutor;

        // Valid cleanup action
        let valid_cleanup = HookAction::Cleanup {
            patterns: vec!["*.tmp".to_string(), "*.log".to_string()],
            directories: vec![temp_dir.path().to_path_buf()],
            temp_files: true,
        };
        assert!(executor.validate(&valid_cleanup, &context).is_ok());

        // Invalid cleanup action (directory doesn't exist)
        let invalid_cleanup = HookAction::Cleanup {
            patterns: vec!["*.tmp".to_string()],
            directories: vec![PathBuf::from("/nonexistent/directory")],
            temp_files: false,
        };
        assert!(executor.validate(&invalid_cleanup, &context).is_err());

        // Invalid cleanup action (empty pattern)
        let empty_pattern_cleanup = HookAction::Cleanup {
            patterns: vec!["".to_string()],
            directories: vec![temp_dir.path().to_path_buf()],
            temp_files: false,
        };
        assert!(executor.validate(&empty_pattern_cleanup, &context).is_err());
    }

    #[tokio::test]
    async fn test_hook_manager_execution() {
        let temp_dir = TempDir::new().unwrap();
        let hooks_config = HooksConfig {
            scripts_dir: temp_dir.path().to_path_buf(),
        };
        let context = HookContext::new(
            "test_snapshot".to_string(),
            PathBuf::from("/tmp/snapshots/test"),
            hooks_config.clone(),
        );
        let hook_manager = HookManager::new(hooks_config);

        // Test log action execution
        let log_hooks = vec![HookAction::Log {
            message: "Test log message with {snapshot_name}".to_string(),
            level: "info".to_string(),
        }];

        let results = hook_manager
            .execute_hooks(&log_hooks, &HookType::PreSnapshot, &context)
            .await;

        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert_eq!(
            results[0].action,
            "log: \"Test log message with {snapshot_name}\""
        );
        assert!(results[0].output.is_some());
        assert!(results[0]
            .output
            .as_ref()
            .unwrap()
            .contains("test_snapshot"));
    }

    #[tokio::test]
    async fn test_hook_manager_with_plugin_context() {
        let hooks_config = HooksConfig::default();
        let context = HookContext::new(
            "test_snapshot".to_string(),
            PathBuf::from("/tmp/snapshots/test"),
            hooks_config.clone(),
        )
        .with_plugin("homebrew_brewfile".to_string());

        let hook_manager = HookManager::new(hooks_config);

        let hooks = vec![HookAction::Log {
            message: "Processing plugin {plugin_name} for snapshot {snapshot_name}".to_string(),
            level: "info".to_string(),
        }];

        let results = hook_manager
            .execute_hooks(&hooks, &HookType::PrePluginSnapshot, &context)
            .await;

        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        let output = results[0].output.as_ref().unwrap();
        assert!(output.contains("homebrew_brewfile"));
        assert!(output.contains("test_snapshot"));
    }

    #[tokio::test]
    async fn test_hook_manager_script_execution() {
        let temp_dir = TempDir::new().unwrap();
        let hooks_config = HooksConfig {
            scripts_dir: temp_dir.path().to_path_buf(),
        };
        let context = HookContext::new(
            "test_snapshot".to_string(),
            PathBuf::from("/tmp/snapshots/test"),
            hooks_config.clone(),
        );
        let hook_manager = HookManager::new(hooks_config);

        // Create a simple test script
        #[cfg(unix)]
        let script_content = "#!/bin/bash\necho 'Hello from hook script'";
        #[cfg(windows)]
        let script_content = "@echo off\necho Hello from hook script";

        #[cfg(unix)]
        let script_name = "test-script.sh";
        #[cfg(windows)]
        let script_name = "test-script.bat";

        create_test_script(&temp_dir, script_name, script_content).await;

        let script_hooks = vec![HookAction::Script {
            command: script_name.to_string(),
            args: vec![],
            timeout: 10,
            working_dir: None,
            env_vars: HashMap::new(),
        }];

        let results = hook_manager
            .execute_hooks(&script_hooks, &HookType::PrePluginSnapshot, &context)
            .await;

        assert_eq!(results.len(), 1);
        assert!(
            results[0].success,
            "Script execution should succeed: {:?}",
            results[0].error
        );
        assert!(results[0].output.is_some());
        assert!(results[0]
            .output
            .as_ref()
            .unwrap()
            .contains("Hello from hook script"));
    }

    #[tokio::test]
    async fn test_hook_manager_multiple_hooks() {
        let temp_dir = TempDir::new().unwrap();
        let hooks_config = HooksConfig {
            scripts_dir: temp_dir.path().to_path_buf(),
        };
        let context = HookContext::new(
            "test_snapshot".to_string(),
            PathBuf::from("/tmp/snapshots/test"),
            hooks_config.clone(),
        );
        let hook_manager = HookManager::new(hooks_config);

        let mixed_hooks = vec![
            HookAction::Log {
                message: "First hook: {snapshot_name}".to_string(),
                level: "info".to_string(),
            },
            HookAction::Notify {
                message: "Second hook notification".to_string(),
                title: Some("Test".to_string()),
            },
            HookAction::Log {
                message: "Third hook: complete".to_string(),
                level: "debug".to_string(),
            },
        ];

        let results = hook_manager
            .execute_hooks(&mixed_hooks, &HookType::PostSnapshot, &context)
            .await;

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.success));

        // Check that all hooks executed and have output
        for result in &results {
            assert!(result.output.is_some());
            assert!(result.execution_time_ms < 1000); // Should be very fast
        }
    }

    #[tokio::test]
    async fn test_hook_manager_error_handling() {
        let temp_dir = TempDir::new().unwrap();
        let hooks_config = HooksConfig {
            scripts_dir: temp_dir.path().to_path_buf(),
        };
        let context = HookContext::new(
            "test_snapshot".to_string(),
            PathBuf::from("/tmp/snapshots/test"),
            hooks_config.clone(),
        );
        let hook_manager = HookManager::new(hooks_config);

        // Mix of valid and invalid hooks
        let mixed_hooks = vec![
            HookAction::Log {
                message: "Valid hook".to_string(),
                level: "info".to_string(),
            },
            HookAction::Script {
                command: "nonexistent-script.sh".to_string(),
                args: vec![],
                timeout: 10,
                working_dir: None,
                env_vars: HashMap::new(),
            },
            HookAction::Log {
                message: "Another valid hook".to_string(),
                level: "info".to_string(),
            },
        ];

        let results = hook_manager
            .execute_hooks(&mixed_hooks, &HookType::PreSnapshot, &context)
            .await;

        assert_eq!(results.len(), 3);

        // First and third hooks should succeed
        assert!(results[0].success);
        assert!(results[2].success);

        // Second hook (nonexistent script) should fail
        assert!(!results[1].success);
        assert!(results[1].error.is_some());
        assert!(
            results[1].error.as_ref().unwrap().contains("not found")
                || results[1]
                    .error
                    .as_ref()
                    .unwrap()
                    .contains("Failed to execute")
        );
    }

    #[tokio::test]
    async fn test_hook_manager_empty_hooks() {
        let hooks_config = HooksConfig::default();
        let context = HookContext::new(
            "test_snapshot".to_string(),
            PathBuf::from("/tmp/snapshots/test"),
            hooks_config.clone(),
        );
        let hook_manager = HookManager::new(hooks_config);

        let results = hook_manager
            .execute_hooks(&[], &HookType::PreSnapshot, &context)
            .await;

        assert_eq!(results.len(), 0);
    }
}

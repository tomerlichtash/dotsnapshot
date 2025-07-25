use anyhow::Result;
use tracing::{debug, info, warn};

use crate::core::hooks::HookAction;

/// Mixin trait for centralized hook execution logic
#[allow(async_fn_in_trait)]
#[allow(dead_code)]
pub trait HooksMixin {
    /// Get all hooks for this plugin
    fn get_hooks(&self) -> Vec<HookAction>;

    /// Execute pre-plugin hooks
    async fn execute_pre_hooks(&self) -> Result<()> {
        let hooks = self.get_hooks();

        if hooks.is_empty() {
            debug!("No hooks to execute");
            return Ok(());
        }

        info!("Executing {} hooks", hooks.len());
        for hook in &hooks {
            self.execute_hook(hook).await?;
        }

        Ok(())
    }

    /// Execute post-plugin hooks
    async fn execute_post_hooks(&self) -> Result<()> {
        // For now, this is the same as pre-hooks since hooks in StandardHooks
        // are already separated into pre_plugin and post_plugin vectors
        Ok(())
    }

    /// Execute a single hook action
    async fn execute_hook(&self, hook: &HookAction) -> Result<()> {
        match hook {
            HookAction::Script {
                command,
                args,
                timeout,
                ..
            } => {
                self.execute_script_hook(command, Some(args), Some(*timeout))
                    .await
            }
            HookAction::Log { message, level } => self.execute_log_hook(message, Some(level)).await,
            HookAction::Notify { message, title } => {
                self.execute_notify_hook(message, title.as_deref()).await
            }
            HookAction::Backup { path, destination } => {
                self.execute_backup_hook(path, Some(destination)).await
            }
            HookAction::Cleanup {
                patterns,
                directories,
                temp_files,
            } => {
                self.execute_cleanup_hook(Some(patterns), Some(directories), *temp_files)
                    .await
            }
        }
    }

    /// Execute a script hook
    async fn execute_script_hook(
        &self,
        command: &str,
        args: Option<&Vec<String>>,
        timeout: Option<u64>,
    ) -> Result<()> {
        use tokio::process::Command;
        use tokio::time::{timeout as tokio_timeout, Duration};

        info!("Executing script hook: {}", command);

        let mut cmd = Command::new(command);

        if let Some(args) = args {
            for arg in args {
                cmd.arg(arg);
            }
        }

        let timeout_duration = Duration::from_secs(timeout.unwrap_or(30));

        match tokio_timeout(timeout_duration, cmd.output()).await {
            Ok(Ok(output)) => {
                if output.status.success() {
                    info!("Script hook completed successfully");
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(anyhow::anyhow!("Script hook failed: {}", stderr));
                }
            }
            Ok(Err(e)) => {
                return Err(anyhow::anyhow!("Failed to execute script hook: {}", e));
            }
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "Script hook timed out after {} seconds",
                    timeout_duration.as_secs()
                ));
            }
        }

        Ok(())
    }

    /// Execute a log hook
    async fn execute_log_hook(&self, message: &str, level: Option<&String>) -> Result<()> {
        let level_str = level.map(|s| s.as_str()).unwrap_or("info");
        match level_str {
            "trace" => tracing::trace!("Hook: {}", message),
            "debug" => tracing::debug!("Hook: {}", message),
            "info" => tracing::info!("Hook: {}", message),
            "warn" => tracing::warn!("Hook: {}", message),
            "error" => tracing::error!("Hook: {}", message),
            _ => tracing::info!("Hook: {}", message),
        }
        Ok(())
    }

    /// Execute a notify hook
    async fn execute_notify_hook(&self, message: &str, title: Option<&str>) -> Result<()> {
        let title = title.unwrap_or("dotsnapshot");
        info!("Notification [{}]: {}", title, message);
        // TODO: Implement actual system notifications if needed
        Ok(())
    }

    /// Execute a backup hook
    async fn execute_backup_hook(
        &self,
        path: &std::path::Path,
        destination: Option<&std::path::PathBuf>,
    ) -> Result<()> {
        use tokio::fs;

        if !path.exists() {
            debug!("Backup path does not exist: {}", path.display());
            return Ok(());
        }

        let backup_path = if let Some(dest) = destination {
            dest.clone()
        } else {
            // Default backup location
            let mut backup = path.to_path_buf();
            let ext = path.extension().unwrap_or_default().to_string_lossy();
            backup.set_extension(format!("{ext}.backup"));
            backup
        };

        info!(
            "Creating backup: {} -> {}",
            path.display(),
            backup_path.display()
        );

        if path.is_dir() {
            // For directories, we need to copy recursively
            // TODO: Implement recursive directory copy
            warn!("Directory backup not yet implemented: {}", path.display());
        } else {
            // For files, simple copy
            if let Some(parent) = backup_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::copy(path, &backup_path).await?;
        }

        Ok(())
    }

    /// Execute a cleanup hook
    async fn execute_cleanup_hook(
        &self,
        patterns: Option<&Vec<String>>,
        directories: Option<&Vec<std::path::PathBuf>>,
        temp_files: bool,
    ) -> Result<()> {
        info!("Executing cleanup hook");

        if let Some(patterns) = patterns {
            for pattern in patterns {
                self.cleanup_pattern(pattern).await?;
            }
        }

        if let Some(directories) = directories {
            for dir in directories {
                self.cleanup_directory(&dir.to_string_lossy()).await?;
            }
        }

        if temp_files {
            self.cleanup_temp_files().await?;
        }

        Ok(())
    }

    /// Helper method to copy directory recursively
    /// TODO: Implement this method properly with correct lifetime handling
    async fn copy_dir_recursive(
        &self,
        _src: &std::path::Path,
        _dst: &std::path::Path,
    ) -> Result<()> {
        // Placeholder implementation
        warn!("copy_dir_recursive not yet implemented");
        Ok(())
    }

    /// Helper method to cleanup files matching a pattern
    async fn cleanup_pattern(&self, pattern: &str) -> Result<()> {
        info!("Cleaning up files matching pattern: {}", pattern);
        // TODO: Implement glob pattern matching and cleanup
        Ok(())
    }

    /// Helper method to cleanup a directory
    async fn cleanup_directory(&self, dir: &str) -> Result<()> {
        use tokio::fs;

        let path = std::path::Path::new(dir);
        if path.exists() && path.is_dir() {
            info!("Cleaning up directory: {}", dir);
            fs::remove_dir_all(path).await?;
        }
        Ok(())
    }

    /// Helper method to cleanup temporary files
    async fn cleanup_temp_files(&self) -> Result<()> {
        info!("Cleaning up temporary files");
        // TODO: Implement temp file cleanup logic
        Ok(())
    }
}

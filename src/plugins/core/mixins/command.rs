use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::debug;
use which::which;

/// Mixin trait for CLI command execution patterns
#[allow(async_fn_in_trait)]
pub trait CommandMixin {
    /// Execute a command and return its output as a string
    fn execute_command(
        &self,
        cmd: &str,
        args: &[&str],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + '_>> {
        let cmd = cmd.to_string();
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        Box::pin(async move {
            debug!("Executing command: {} {:?}", cmd, args);

            let output = Command::new(&cmd)
                .args(&args)
                .output()
                .await
                .with_context(|| format!("Failed to execute command: {cmd} {args:?}"))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                return Err(anyhow::anyhow!(
                    "Command failed: {} {:?}\nStdout: {}\nStderr: {}",
                    cmd,
                    args,
                    stdout,
                    stderr
                ));
            }

            let result = String::from_utf8_lossy(&output.stdout).to_string();
            debug!("Command output: {} bytes", result.len());
            Ok(result)
        })
    }

    /// Validate that a command exists on the system
    fn validate_command_exists(
        &self,
        cmd: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
        let cmd = cmd.to_string();
        Box::pin(async move {
            which(&cmd).with_context(|| format!("{cmd} command not found. Please install it."))?;
            Ok(())
        })
    }

    /// Check if a command exists without failing
    fn command_exists(&self, cmd: &str) -> bool {
        which(cmd).is_ok()
    }
}

// Note: Removed blanket implementation to avoid conflicts with specific implementations
// Each type that needs CommandMixin should implement it explicitly

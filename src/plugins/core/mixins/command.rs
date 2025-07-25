use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::{debug, info};
use which::which;

/// Mixin trait for CLI command execution patterns
#[allow(async_fn_in_trait)]
#[allow(dead_code)]
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

    /// Execute a command and return its output, handling errors gracefully
    async fn execute_command_safe(&self, cmd: &str, args: &[&str]) -> Result<Option<String>> {
        match self.execute_command(cmd, args).await {
            Ok(output) => Ok(Some(output)),
            Err(e) => {
                info!("Command failed (continuing): {} {:?} - {}", cmd, args, e);
                Ok(None)
            }
        }
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

    /// Execute a command in a specific directory
    async fn execute_command_in_dir(
        &self,
        cmd: &str,
        args: &[&str],
        dir: &std::path::Path,
    ) -> Result<String> {
        debug!("Executing command in {}: {} {:?}", dir.display(), cmd, args);

        let output = Command::new(cmd)
            .args(args)
            .current_dir(dir)
            .output()
            .await
            .with_context(|| {
                format!(
                    "Failed to execute command in {}: {} {:?}",
                    dir.display(),
                    cmd,
                    args
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(anyhow::anyhow!(
                "Command failed in {}: {} {:?}\nStdout: {}\nStderr: {}",
                dir.display(),
                cmd,
                args,
                stdout,
                stderr
            ));
        }

        let result = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(result)
    }

    /// Execute a command with a timeout
    async fn execute_command_with_timeout(
        &self,
        cmd: &str,
        args: &[&str],
        timeout_secs: u64,
    ) -> Result<String> {
        use tokio::time::{timeout, Duration};

        debug!(
            "Executing command with {}s timeout: {} {:?}",
            timeout_secs, cmd, args
        );

        let command_future = Command::new(cmd).args(args).output();

        let output = timeout(Duration::from_secs(timeout_secs), command_future)
            .await
            .with_context(|| format!("Command timed out after {timeout_secs}s: {cmd} {args:?}"))?
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
        Ok(result)
    }

    /// Parse command output lines, filtering empty lines
    fn parse_command_lines(&self, output: &str) -> Vec<String> {
        output
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect()
    }

    /// Execute a command and parse its output as lines
    async fn execute_command_lines(&self, cmd: &str, args: &[&str]) -> Result<Vec<String>> {
        let output = self.execute_command(cmd, args).await?;
        Ok(self.parse_command_lines(&output))
    }

    /// Execute a command and check if it succeeds (ignore output)
    async fn execute_command_check(&self, cmd: &str, args: &[&str]) -> Result<bool> {
        debug!("Checking command: {} {:?}", cmd, args);

        let status = Command::new(cmd)
            .args(args)
            .status()
            .await
            .with_context(|| format!("Failed to execute command: {cmd} {args:?}"))?;

        Ok(status.success())
    }

    /// Get the version of a command (assumes --version flag)
    async fn get_command_version(&self, cmd: &str) -> Result<String> {
        let output = self.execute_command_safe(cmd, &["--version"]).await?;

        match output {
            Some(version_output) => {
                // Extract the first line which usually contains the version
                let first_line = version_output.lines().next().unwrap_or("unknown").trim();
                Ok(first_line.to_string())
            }
            None => Ok("unknown".to_string()),
        }
    }
}

// Note: Removed blanket implementation to avoid conflicts with specific implementations
// Each type that needs CommandMixin should implement it explicitly

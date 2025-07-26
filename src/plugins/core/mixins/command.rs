use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::process::Command;
use tracing::debug;
use which::which;

/// Provides command execution capabilities to plugins
#[async_trait::async_trait]
pub trait CommandMixin: Send + Sync {
    /// Execute a command and return its output
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
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .with_context(|| format!("Failed to execute command: {cmd}"))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!(
                    "Command '{}' failed with exit code {}: {}",
                    cmd,
                    output.status.code().unwrap_or(-1),
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
    // WORKAROUND: False positive dead code warning in Rust beta toolchain
    // This method is actually used in extensions.rs, package.rs, static_files.rs and tests
    // but the beta compiler's dead code analysis doesn't detect trait method usage properly
    #[allow(dead_code)]
    fn command_exists(&self, cmd: &str) -> bool {
        which(cmd).is_ok()
    }
}

// Note: Removed blanket implementation to avoid conflicts with specific implementations
// Each type that needs CommandMixin should implement it explicitly

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    /// Mock implementation for testing CommandMixin functionality
    struct MockPlugin;

    impl CommandMixin for MockPlugin {}

    /// Test basic command execution with successful command
    /// Verifies that commands can be executed and return correct output
    #[tokio::test]
    async fn test_execute_command_success() {
        let plugin = MockPlugin;

        // Use a command that should exist on all systems
        let result = plugin.execute_command("echo", &["hello", "world"]).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("hello world"));
    }

    /// Test command execution with non-existent command
    /// Verifies that proper error handling occurs when command doesn't exist
    #[tokio::test]
    async fn test_execute_command_not_found() {
        let plugin = MockPlugin;

        let result = plugin
            .execute_command("nonexistent_command_12345", &[])
            .await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Failed to execute command"));
    }

    /// Test command execution that returns non-zero exit code
    /// Verifies that commands with failure exit codes are handled properly
    #[tokio::test]
    async fn test_execute_command_failure_exit_code() {
        let plugin = MockPlugin;

        // Use 'false' command which always returns exit code 1
        let result = plugin.execute_command("false", &[]).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("failed with exit code"));
    }

    /// Test command validation for existing command
    /// Verifies that validation passes for commands that exist in PATH
    #[tokio::test]
    async fn test_validate_command_exists_success() {
        let plugin = MockPlugin;

        let result = plugin.validate_command_exists("echo").await;

        assert!(result.is_ok());
    }

    /// Test command validation for non-existent command
    /// Verifies that validation fails appropriately for missing commands
    #[tokio::test]
    async fn test_validate_command_exists_failure() {
        let plugin = MockPlugin;

        let result = plugin
            .validate_command_exists("nonexistent_command_12345")
            .await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("command not found"));
    }

    /// Test synchronous command existence check for existing command
    /// Verifies that the sync check correctly identifies existing commands
    #[test]
    fn test_command_exists_true() {
        let plugin = MockPlugin;

        let exists = plugin.command_exists("echo");

        assert!(exists);
    }

    /// Test synchronous command existence check for non-existent command
    /// Verifies that the sync check correctly identifies missing commands
    #[test]
    fn test_command_exists_false() {
        let plugin = MockPlugin;

        let exists = plugin.command_exists("nonexistent_command_12345");

        assert!(!exists);
    }

    /// Test command execution with multiple arguments
    /// Verifies that complex command arguments are handled correctly
    #[tokio::test]
    async fn test_execute_command_with_args() {
        let plugin = MockPlugin;

        // Test with multiple arguments
        let result = plugin.execute_command("echo", &["-n", "test"]).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.trim(), "test");
    }

    /// Test command execution with empty arguments
    /// Verifies that commands work correctly with no arguments
    #[tokio::test]
    async fn test_execute_command_no_args() {
        let plugin = MockPlugin;

        let result = plugin.execute_command("pwd", &[]).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        // pwd should return some directory path
        assert!(!output.trim().is_empty());
    }
}

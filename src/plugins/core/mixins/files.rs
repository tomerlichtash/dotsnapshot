use anyhow::{Context, Result};
use std::path::Path;
use tokio::fs;
use tracing::{debug, info};

/// Mixin trait for common file operations
#[allow(async_fn_in_trait)]
pub trait FilesMixin {
    /// Restore a file from source to target, creating directories as needed
    async fn restore_file(&self, source: &Path, target: &Path) -> Result<()> {
        debug!(
            "Restoring file: {} -> {}",
            source.display(),
            target.display()
        );

        if !source.exists() {
            return Err(anyhow::anyhow!(
                "Source file does not exist: {}",
                source.display()
            ));
        }

        // Create parent directories if they don't exist
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).await.with_context(|| {
                format!(
                    "Failed to create parent directories for {}",
                    target.display()
                )
            })?;
        }

        // Copy the file
        fs::copy(source, target).await.with_context(|| {
            format!(
                "Failed to copy file from {} to {}",
                source.display(),
                target.display()
            )
        })?;

        info!("Restored file: {}", target.display());
        Ok(())
    }

    /// Check if a directory exists and is accessible
    async fn is_dir_accessible(&self, path: &Path) -> bool {
        match fs::metadata(path).await {
            Ok(metadata) => metadata.is_dir(),
            Err(_) => false,
        }
    }
}

// Note: Removed blanket implementation to avoid conflicts with specific implementations
// Each type that needs FilesMixin should implement it explicitly

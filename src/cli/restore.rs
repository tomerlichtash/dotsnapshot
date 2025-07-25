use anyhow::Result;
use std::path::PathBuf;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::core::restore::RestoreManager;
use crate::symbols::*;

/// Handle restore subcommand
#[allow(clippy::too_many_arguments)]
pub async fn handle_restore_command(
    snapshot_path: Option<PathBuf>,
    latest: bool,
    plugins: Option<String>,
    dry_run: bool,
    backup: bool,
    force: bool,
    target_dir: Option<PathBuf>,
    config_path: Option<PathBuf>,
) -> Result<()> {
    // Load configuration
    let config = if let Some(config_path) = config_path {
        if config_path.exists() {
            Config::load_from_file(&config_path).await?
        } else {
            Config::default()
        }
    } else {
        Config::load().await.unwrap_or_default()
    };

    // Determine the actual snapshot path
    let actual_snapshot_path = if latest {
        // Find the latest snapshot in the default output directory
        find_latest_snapshot(&config).await?
    } else if let Some(path) = snapshot_path {
        path
    } else {
        return Err(anyhow::anyhow!(
            "Either provide a snapshot path or use --latest flag"
        ));
    };

    // Validate snapshot path exists
    if !actual_snapshot_path.exists() {
        error!(
            "{} Snapshot path does not exist: {}",
            SYMBOL_INDICATOR_ERROR,
            actual_snapshot_path.display()
        );
        return Err(anyhow::anyhow!(
            "Snapshot path does not exist: {}",
            actual_snapshot_path.display()
        ));
    }

    if !actual_snapshot_path.is_dir() {
        error!(
            "{} Snapshot path is not a directory: {}",
            SYMBOL_INDICATOR_ERROR,
            actual_snapshot_path.display()
        );
        return Err(anyhow::anyhow!(
            "Snapshot path is not a directory: {}",
            actual_snapshot_path.display()
        ));
    }

    // Parse plugins filter
    let selected_plugins = plugins.map(|p| {
        p.split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    });

    // Determine target directory - this is the global override if provided
    let global_target_override = target_dir;

    info!(
        "{} Starting restore from snapshot: {}",
        SYMBOL_ACTION_RESTORE,
        actual_snapshot_path.display()
    );
    if let Some(ref target) = global_target_override {
        info!(
            "{} Global target directory: {}",
            SYMBOL_CONTENT_FOLDER,
            target.display()
        );
    } else {
        info!(
            "{} Target directory: per-plugin configuration or home directory",
            SYMBOL_CONTENT_FOLDER
        );
    }

    if dry_run {
        info!(
            "{} DRY RUN MODE: No changes will be made",
            SYMBOL_INDICATOR_WARNING
        );
    }

    if let Some(ref plugins) = selected_plugins {
        info!(
            "{} Restoring plugins: {}",
            SYMBOL_TOOL_PLUGIN,
            plugins.join(", ")
        );
    } else {
        info!("{} Restoring all plugins from snapshot", SYMBOL_SCOPE_WORLD);
    }

    // Create restore manager
    let default_target = global_target_override
        .clone()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
    let restore_manager = RestoreManager::new(
        actual_snapshot_path,
        default_target,
        global_target_override,
        config,
        dry_run,
        backup,
        force,
    );

    // Execute restoration
    match restore_manager.execute_restore(selected_plugins).await {
        Ok(restored_files) => {
            if dry_run {
                info!(
                    "{} DRY RUN: Would restore {} files",
                    SYMBOL_INDICATOR_SUCCESS,
                    restored_files.len()
                );
                info!(
                    "{} Preview completed successfully",
                    SYMBOL_EXPERIENCE_SUCCESS
                );
            } else {
                info!(
                    "{} Successfully restored {} files",
                    SYMBOL_INDICATOR_SUCCESS,
                    restored_files.len()
                );
                info!(
                    "{} Restoration completed successfully",
                    SYMBOL_EXPERIENCE_SUCCESS
                );
            }

            // Show summary of restored files
            if !restored_files.is_empty() {
                info!("{} Restored files:", SYMBOL_DOC_NOTE);
                for file in restored_files.iter().take(10) {
                    info!("   {} {}", SYMBOL_CONTENT_FILE, file.display());
                }
                if restored_files.len() > 10 {
                    info!(
                        "   {} ... and {} more files",
                        SYMBOL_DOC_NOTE,
                        restored_files.len() - 10
                    );
                }
            }
        }
        Err(e) => {
            error!("{} Restoration failed: {}", SYMBOL_INDICATOR_ERROR, e);
            if !dry_run {
                warn!(
                    "{} Some files may have been partially restored",
                    SYMBOL_INDICATOR_WARNING
                );
                warn!(
                    "{} Check the logs above for specific failures",
                    SYMBOL_EXPERIENCE_IDEA
                );
            }
            return Err(e);
        }
    }

    Ok(())
}

/// Find the latest snapshot directory in the default snapshot directory
async fn find_latest_snapshot(config: &Config) -> Result<PathBuf> {
    let snapshot_base_dir = config.get_output_dir();

    if !snapshot_base_dir.exists() {
        return Err(anyhow::anyhow!(
            "Snapshot directory does not exist: {}. No snapshots found.",
            snapshot_base_dir.display()
        ));
    }

    let mut entries = tokio::fs::read_dir(&snapshot_base_dir).await?;
    let mut snapshot_dirs = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                // Check if directory name matches snapshot pattern (YYYYMMDD_HHMMSS)
                if is_snapshot_directory(dir_name) {
                    snapshot_dirs.push((dir_name.to_string(), path));
                }
            }
        }
    }

    if snapshot_dirs.is_empty() {
        return Err(anyhow::anyhow!(
            "No snapshot directories found in: {}",
            snapshot_base_dir.display()
        ));
    }

    // Sort by directory name (which is timestamp-based) in descending order
    snapshot_dirs.sort_by(|a, b| b.0.cmp(&a.0));

    let latest_snapshot_path = snapshot_dirs[0].1.clone();
    info!(
        "{} Found latest snapshot: {}",
        SYMBOL_EXPERIENCE_IDEA,
        latest_snapshot_path.display()
    );

    Ok(latest_snapshot_path)
}

/// Check if a directory name matches the snapshot pattern (YYYYMMDD_HHMMSS)
fn is_snapshot_directory(dir_name: &str) -> bool {
    // Pattern: 8 digits + underscore + 6 digits (e.g., 20240117_143022)
    if dir_name.len() != 15 {
        return false;
    }

    let parts: Vec<&str> = dir_name.split('_').collect();
    if parts.len() != 2 {
        return false;
    }

    // Check that both parts are numeric
    parts[0].len() == 8
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[1].len() == 6
        && parts[1].chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_snapshot_directory() {
        // Valid snapshot directory names
        assert!(is_snapshot_directory("20240117_143022"));
        assert!(is_snapshot_directory("20231201_000000"));
        assert!(is_snapshot_directory("20250101_235959"));

        // Invalid snapshot directory names
        assert!(!is_snapshot_directory("20240117"));
        assert!(!is_snapshot_directory("143022"));
        assert!(!is_snapshot_directory("20240117-143022"));
        assert!(!is_snapshot_directory("2024011_143022"));
        assert!(!is_snapshot_directory("20240117_14302"));
        assert!(!is_snapshot_directory("20240117_143022_extra"));
        assert!(!is_snapshot_directory("abcd1234_143022"));
        assert!(!is_snapshot_directory("20240117_abcdef"));
        assert!(!is_snapshot_directory(""));
        assert!(!is_snapshot_directory("not_a_snapshot"));
    }

    #[tokio::test]
    async fn test_find_latest_snapshot_empty_directory() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            output_dir: Some(temp_dir.path().to_path_buf()),
            ..Default::default()
        };

        let result = find_latest_snapshot(&config).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No snapshot directories found"));
    }

    #[tokio::test]
    async fn test_find_latest_snapshot_with_snapshots() {
        use tempfile::TempDir;
        use tokio::fs;

        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            output_dir: Some(temp_dir.path().to_path_buf()),
            ..Default::default()
        };

        // Create some snapshot directories
        let snapshot1 = temp_dir.path().join("20240115_120000");
        let snapshot2 = temp_dir.path().join("20240117_143022");
        let snapshot3 = temp_dir.path().join("20240116_090000");
        let non_snapshot = temp_dir.path().join("not_a_snapshot");

        fs::create_dir_all(&snapshot1).await.unwrap();
        fs::create_dir_all(&snapshot2).await.unwrap();
        fs::create_dir_all(&snapshot3).await.unwrap();
        fs::create_dir_all(&non_snapshot).await.unwrap();

        let result = find_latest_snapshot(&config).await.unwrap();
        assert_eq!(result, snapshot2); // Should be the latest (20240117_143022)
    }

    #[tokio::test]
    async fn test_find_latest_snapshot_nonexistent_directory() {
        let config = Config {
            output_dir: Some(PathBuf::from("/nonexistent/directory")),
            ..Default::default()
        };

        let result = find_latest_snapshot(&config).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Snapshot directory does not exist"));
    }
}

use anyhow::Result;
use std::path::PathBuf;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::core::restore::RestoreManager;
use crate::symbols::*;

/// Handle restore subcommand
pub async fn handle_restore_command(
    snapshot_path: PathBuf,
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

    // Validate snapshot path exists
    if !snapshot_path.exists() {
        error!(
            "{} Snapshot path does not exist: {}",
            INDICATOR_ERROR,
            snapshot_path.display()
        );
        return Err(anyhow::anyhow!(
            "Snapshot path does not exist: {}",
            snapshot_path.display()
        ));
    }

    if !snapshot_path.is_dir() {
        error!(
            "{} Snapshot path is not a directory: {}",
            INDICATOR_ERROR,
            snapshot_path.display()
        );
        return Err(anyhow::anyhow!(
            "Snapshot path is not a directory: {}",
            snapshot_path.display()
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
        ACTION_RESTORE,
        snapshot_path.display()
    );
    if let Some(ref target) = global_target_override {
        info!(
            "{} Global target directory: {}",
            CONTENT_FOLDER,
            target.display()
        );
    } else {
        info!(
            "{} Target directory: per-plugin configuration or home directory",
            CONTENT_FOLDER
        );
    }

    if dry_run {
        info!(
            "{} DRY RUN MODE: No changes will be made",
            INDICATOR_WARNING
        );
    }

    if let Some(ref plugins) = selected_plugins {
        info!("{} Restoring plugins: {}", TOOL_PLUGIN, plugins.join(", "));
    } else {
        info!("{} Restoring all plugins from snapshot", SCOPE_WORLD);
    }

    // Create restore manager
    let default_target = global_target_override
        .clone()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
    let restore_manager = RestoreManager::new(
        snapshot_path,
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
                    INDICATOR_SUCCESS,
                    restored_files.len()
                );
                info!("{} Preview completed successfully", EXPERIENCE_SUCCESS);
            } else {
                info!(
                    "{} Successfully restored {} files",
                    INDICATOR_SUCCESS,
                    restored_files.len()
                );
                info!("{} Restoration completed successfully", EXPERIENCE_SUCCESS);
            }

            // Show summary of restored files
            if !restored_files.is_empty() {
                info!("{} Restored files:", DOC_NOTE);
                for file in restored_files.iter().take(10) {
                    info!("   {} {}", CONTENT_FILE, file.display());
                }
                if restored_files.len() > 10 {
                    info!(
                        "   {} ... and {} more files",
                        DOC_NOTE,
                        restored_files.len() - 10
                    );
                }
            }
        }
        Err(e) => {
            error!("{} Restoration failed: {}", INDICATOR_ERROR, e);
            if !dry_run {
                warn!(
                    "{} Some files may have been partially restored",
                    INDICATOR_WARNING
                );
                warn!(
                    "{} Check the logs above for specific failures",
                    EXPERIENCE_IDEA
                );
            }
            return Err(e);
        }
    }

    Ok(())
}

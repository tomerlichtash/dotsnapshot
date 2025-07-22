use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

use crate::config::Config;
use crate::core::plugin::PluginRegistry;
use crate::core::restore::RestoreManager;

#[derive(Parser)]
pub enum RestoreCommands {
    /// List available snapshots
    List {
        /// Snapshots directory (uses config if not specified)
        #[arg(long)]
        snapshots_dir: Option<PathBuf>,
    },
    /// Restore configurations from a snapshot
    Restore {
        /// Name of the snapshot to restore from
        snapshot: String,

        /// Specific plugins to restore (comma-separated)
        #[arg(short, long)]
        plugins: Option<String>,

        /// Snapshots directory (uses config if not specified)
        #[arg(long)]
        snapshots_dir: Option<PathBuf>,

        /// Show what would be restored without making changes
        #[arg(long)]
        dry_run: bool,

        /// Ask for confirmation before restoring
        #[arg(short, long)]
        interactive: bool,

        /// Backup existing configurations before restoring
        #[arg(short, long)]
        backup: bool,
    },
}

/// Handle restore subcommands
pub async fn handle_restore_command(
    command: RestoreCommands,
    config_path: Option<PathBuf>,
) -> Result<()> {
    match command {
        RestoreCommands::List { snapshots_dir } => {
            handle_list_snapshots(snapshots_dir, config_path).await
        }
        RestoreCommands::Restore {
            snapshot,
            plugins,
            snapshots_dir,
            dry_run,
            interactive,
            backup,
        } => {
            handle_restore_snapshot(
                snapshot,
                plugins,
                snapshots_dir,
                dry_run,
                interactive,
                backup,
                config_path,
            )
            .await
        }
    }
}

async fn handle_list_snapshots(
    snapshots_dir: Option<PathBuf>,
    config_path: Option<PathBuf>,
) -> Result<()> {
    // Load configuration
    let config = load_config(config_path).await?;
    let snapshots_path = snapshots_dir.unwrap_or_else(|| config.get_output_dir());

    // Create restore manager
    let registry = Arc::new(PluginRegistry::new());
    let restore_manager =
        RestoreManager::with_config(registry, snapshots_path.clone(), Arc::new(config));

    info!("📸 Available snapshots in: {}", snapshots_path.display());

    let snapshots = restore_manager.list_snapshots().await?;
    if snapshots.is_empty() {
        info!("No snapshots found in: {}", snapshots_path.display());
        return Ok(());
    }

    info!("");
    for snapshot in snapshots {
        info!(
            "  {} | {} | {} | {} plugins",
            snapshot.name,
            snapshot.created_at.format("%Y-%m-%d %H:%M:%S"),
            snapshot.format_size(),
            snapshot.plugin_count
        );
    }

    Ok(())
}

async fn handle_restore_snapshot(
    snapshot: String,
    plugins: Option<String>,
    snapshots_dir: Option<PathBuf>,
    dry_run: bool,
    interactive: bool,
    backup: bool,
    config_path: Option<PathBuf>,
) -> Result<()> {
    // Load configuration
    let config = load_config(config_path).await?;
    let snapshots_path = snapshots_dir.unwrap_or_else(|| config.get_output_dir());

    // Parse selected plugins
    let selected_plugins = plugins.as_ref().map(|p| {
        p.split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    });

    // Create plugin registry with all plugins
    let mut registry = PluginRegistry::new();
    register_all_plugins(&mut registry);

    // Create restore manager
    let restore_manager =
        RestoreManager::with_config(Arc::new(registry), snapshots_path.clone(), Arc::new(config));

    // Interactive confirmation
    if interactive && !dry_run {
        let confirmation = if let Some(ref plugins) = selected_plugins {
            format!(
                "Are you sure you want to restore plugins {} from snapshot '{snapshot}'? (y/N)",
                plugins.join(", ")
            )
        } else {
            format!(
                "Are you sure you want to restore all plugins from snapshot '{snapshot}'? (y/N)"
            )
        };

        println!("{confirmation}");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
            info!("Restore cancelled.");
            return Ok(());
        }
    }

    // Perform restore
    let mode = if dry_run { " (DRY RUN)" } else { "" };
    info!("🔄 Starting restore{} from snapshot: {}", mode, snapshot);

    let results = restore_manager
        .restore_from_snapshot(&snapshot, selected_plugins.as_deref(), dry_run, backup)
        .await?;

    // Display results
    info!("");
    info!("📊 Restore Summary:");
    let mut successful = 0;
    let mut failed = 0;
    let mut total_files = 0;

    for result in &results {
        let status = if result.success { "✅" } else { "❌" };
        let action = if dry_run { "would restore" } else { "restored" };

        if result.success {
            successful += 1;
            total_files += result.restored_files;
            info!(
                "  {} {} {} ({} files)",
                status, action, result.plugin_name, result.restored_files
            );
        } else {
            failed += 1;
            let error_msg = result.error_message.as_deref().unwrap_or("Unknown error");
            warn!(
                "  {} Failed to restore {}: {}",
                status, result.plugin_name, error_msg
            );
        }

        // Show backup location if created
        if !dry_run && backup {
            if let Some(backup_path) = &result.backup_path {
                info!("    💾 Backup created at: {}", backup_path.display());
            }
        }
    }

    info!("");
    if dry_run {
        info!(
            "🔍 Dry run completed: {} plugins would be restored ({} files)",
            successful, total_files
        );
    } else {
        info!(
            "✅ Restore completed: {} successful, {} failed ({} files restored)",
            successful, failed, total_files
        );
    }

    if failed > 0 {
        warn!("Some plugins failed to restore. Check the logs above for details.");
    }

    Ok(())
}

async fn load_config(config_path: Option<PathBuf>) -> Result<Config> {
    if let Some(path) = config_path {
        if path.exists() {
            Config::load_from_file(&path).await
        } else {
            Ok(Config::default())
        }
    } else {
        Ok(Config::load().await.unwrap_or_default())
    }
}

fn register_all_plugins(registry: &mut PluginRegistry) {
    use crate::plugins::{
        cursor::{CursorExtensionsPlugin, CursorKeybindingsPlugin, CursorSettingsPlugin},
        homebrew::HomebrewBrewfilePlugin,
        npm::{NpmConfigPlugin, NpmGlobalPackagesPlugin},
        static_files::StaticFilesPlugin,
        vscode::{VSCodeExtensionsPlugin, VSCodeKeybindingsPlugin, VSCodeSettingsPlugin},
    };

    // Register all plugins
    registry.register(Arc::new(HomebrewBrewfilePlugin::new()));
    registry.register(Arc::new(VSCodeSettingsPlugin::new()));
    registry.register(Arc::new(VSCodeKeybindingsPlugin::new()));
    registry.register(Arc::new(VSCodeExtensionsPlugin::new()));
    registry.register(Arc::new(CursorSettingsPlugin::new()));
    registry.register(Arc::new(CursorKeybindingsPlugin::new()));
    registry.register(Arc::new(CursorExtensionsPlugin::new()));
    registry.register(Arc::new(NpmGlobalPackagesPlugin::new()));
    registry.register(Arc::new(NpmConfigPlugin::new()));
    registry.register(Arc::new(StaticFilesPlugin::new()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_config_default() -> Result<()> {
        let config = load_config(None).await?;
        assert!(config.output_dir.is_some());
        Ok(())
    }

    #[test]
    fn test_register_all_plugins() {
        let mut registry = PluginRegistry::new();
        register_all_plugins(&mut registry);

        // Should have at least the core plugins
        let plugins = registry.plugins();
        assert!(!plugins.is_empty());

        // Check that we have the expected plugin types
        let plugin_names: Vec<String> = plugins.iter().map(|p| p.name().to_string()).collect();
        assert!(plugin_names.contains(&"homebrew_brewfile".to_string()));
        assert!(plugin_names.contains(&"vscode_settings".to_string()));
    }
}

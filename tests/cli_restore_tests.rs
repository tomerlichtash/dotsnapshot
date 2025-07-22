use anyhow::Result;
use chrono::Local;
use dotsnapshot::cli::restore::{handle_restore_command, RestoreCommands};
use dotsnapshot::config::{
    Config, GlobalConfig, GlobalHooks, PluginConfig, PluginHooks, PluginsConfig,
};
use dotsnapshot::core::hooks::HooksConfig;
use dotsnapshot::core::plugin::PluginRegistry;
use dotsnapshot::core::restore::{RestoreManager, SnapshotInfo};
use dotsnapshot::plugins::homebrew::HomebrewBrewfilePlugin;
use dotsnapshot::plugins::vscode::{VSCodeExtensionsPlugin, VSCodeSettingsPlugin};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs;

/// Helper function to create a mock snapshot directory with test data
async fn create_test_snapshot(
    snapshots_dir: &Path,
    snapshot_name: &str,
    with_metadata: bool,
) -> Result<PathBuf> {
    let snapshot_path = snapshots_dir.join(snapshot_name);
    fs::create_dir_all(&snapshot_path).await?;

    // Create some test plugin files
    fs::write(
        snapshot_path.join("Brewfile"),
        "tap 'homebrew/core'\nbrew 'git'\n",
    )
    .await?;
    fs::write(
        snapshot_path.join("vscode_settings.json"),
        r#"{"editor.fontSize": 14}"#,
    )
    .await?;
    fs::write(
        snapshot_path.join("vscode_extensions.txt"),
        "ms-python.python\nrust-lang.rust-analyzer\n",
    )
    .await?;

    // Create metadata if requested
    if with_metadata {
        let metadata = serde_json::json!({
            "timestamp": Local::now().to_rfc3339(),
            "plugins": [
                {"name": "homebrew_brewfile"},
                {"name": "vscode_settings"},
                {"name": "vscode_extensions"}
            ]
        });
        fs::write(
            snapshot_path.join("metadata.json"),
            serde_json::to_string_pretty(&metadata)?,
        )
        .await?;
    }

    Ok(snapshot_path)
}

/// Helper function to create a test config with restore hooks
async fn create_test_config_with_restore_hooks(config_path: &PathBuf) -> Result<Config> {
    use dotsnapshot::core::hooks::HookAction;

    let config = Config {
        output_dir: None,
        include_plugins: None,
        logging: None,
        global: Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![],
                post_snapshot: vec![],
                pre_restore: vec![HookAction::Log {
                    message: "Starting restore operation: {snapshot_name}".to_string(),
                    level: "info".to_string(),
                }],
                post_restore: vec![HookAction::Notify {
                    message: "Restore completed successfully!".to_string(),
                    title: Some("dotsnapshot".to_string()),
                }],
            }),
        }),
        plugins: Some(PluginsConfig {
            homebrew_brewfile: Some(PluginConfig {
                target_path: None,
                hooks: Some(PluginHooks {
                    pre_plugin: vec![],
                    post_plugin: vec![],
                    pre_plugin_restore: vec![HookAction::Log {
                        message: "Preparing homebrew restore".to_string(),
                        level: "debug".to_string(),
                    }],
                    post_plugin_restore: vec![HookAction::Log {
                        message: "Homebrew configuration restored".to_string(),
                        level: "info".to_string(),
                    }],
                }),
            }),
            vscode_settings: Some(PluginConfig {
                target_path: None,
                hooks: Some(PluginHooks {
                    pre_plugin: vec![],
                    post_plugin: vec![],
                    pre_plugin_restore: vec![],
                    post_plugin_restore: vec![HookAction::Backup {
                        path: PathBuf::from("~/.config/Code/User/settings.json"),
                        destination: PathBuf::from("/tmp/vscode-backup"),
                    }],
                }),
            }),
            vscode_keybindings: None,
            vscode_extensions: None,
            cursor_settings: None,
            cursor_keybindings: None,
            cursor_extensions: None,
            npm_global_packages: None,
            npm_config: None,
            static_files: None,
        }),
        static_files: None,
        hooks: Some(HooksConfig {
            scripts_dir: PathBuf::from("~/.config/dotsnapshot/scripts"),
        }),
    };

    config.save_to_file(config_path).await?;
    Ok(config)
}

#[tokio::test]
async fn test_restore_manager_creation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    let registry = Arc::new(PluginRegistry::new());

    let _restore_manager = RestoreManager::new(registry, snapshots_dir);
    // RestoreManager created successfully

    Ok(())
}

#[tokio::test]
async fn test_restore_manager_with_config() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    let config_path = temp_dir.path().join("config.toml");

    let config = create_test_config_with_restore_hooks(&config_path).await?;
    let registry = Arc::new(PluginRegistry::new());

    let _restore_manager = RestoreManager::with_config(registry, snapshots_dir, Arc::new(config));
    // RestoreManager with config created successfully

    Ok(())
}

#[tokio::test]
async fn test_list_empty_snapshots() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    let registry = Arc::new(PluginRegistry::new());

    let restore_manager = RestoreManager::new(registry, snapshots_dir);
    let snapshots = restore_manager.list_snapshots().await?;

    assert!(snapshots.is_empty());
    Ok(())
}

#[tokio::test]
async fn test_list_snapshots_with_metadata() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    fs::create_dir_all(&snapshots_dir).await?;

    // Create test snapshots
    create_test_snapshot(&snapshots_dir, "20250122_120000", true).await?;
    create_test_snapshot(&snapshots_dir, "20250121_150000", true).await?;
    create_test_snapshot(&snapshots_dir, "20250120_100000", false).await?; // No metadata

    let registry = Arc::new(PluginRegistry::new());
    let restore_manager = RestoreManager::new(registry, snapshots_dir);
    let snapshots = restore_manager.list_snapshots().await?;

    assert_eq!(snapshots.len(), 3);

    // Should be sorted by creation time (newest first)
    // Check that all expected snapshots are present
    let snapshot_names: Vec<String> = snapshots.iter().map(|s| s.name.clone()).collect();
    assert!(snapshot_names.contains(&"20250122_120000".to_string()));
    assert!(snapshot_names.contains(&"20250121_150000".to_string()));
    assert!(snapshot_names.contains(&"20250120_100000".to_string()));

    // Snapshots with metadata should have plugin count > 0
    let with_metadata = snapshots.iter().filter(|s| s.plugin_count > 0).count();
    let without_metadata = snapshots.iter().filter(|s| s.plugin_count == 0).count();
    assert_eq!(with_metadata, 2);
    assert_eq!(without_metadata, 1);

    Ok(())
}

#[tokio::test]
async fn test_restore_from_nonexistent_snapshot() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    fs::create_dir_all(&snapshots_dir).await?;

    let registry = Arc::new(PluginRegistry::new());
    let restore_manager = RestoreManager::new(registry, snapshots_dir);

    let result = restore_manager
        .restore_from_snapshot("nonexistent_snapshot", None, true, false)
        .await;

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Snapshot 'nonexistent_snapshot' not found"));

    Ok(())
}

#[tokio::test]
async fn test_dry_run_restore() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    fs::create_dir_all(&snapshots_dir).await?;

    // Create test snapshot
    create_test_snapshot(&snapshots_dir, "test_snapshot", true).await?;

    // Create registry with some plugins
    let mut registry = PluginRegistry::new();
    registry.register(Arc::new(HomebrewBrewfilePlugin::new()));
    registry.register(Arc::new(VSCodeSettingsPlugin::new()));
    registry.register(Arc::new(VSCodeExtensionsPlugin::new()));
    registry.register(Arc::new(VSCodeSettingsPlugin::new()));
    registry.register(Arc::new(VSCodeExtensionsPlugin::new()));

    let restore_manager = RestoreManager::new(Arc::new(registry), snapshots_dir);

    // Perform dry run restore
    let results = restore_manager
        .restore_from_snapshot("test_snapshot", None, true, false)
        .await?;

    assert!(!results.is_empty());

    // All results should be successful in dry run mode
    for result in &results {
        assert!(result.success);
        assert_eq!(result.restored_files, 1); // Mock value
    }

    Ok(())
}

#[tokio::test]
async fn test_restore_with_selected_plugins() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    fs::create_dir_all(&snapshots_dir).await?;

    // Create test snapshot
    create_test_snapshot(&snapshots_dir, "test_snapshot", true).await?;

    // Create registry with plugins
    let mut registry = PluginRegistry::new();
    registry.register(Arc::new(HomebrewBrewfilePlugin::new()));
    registry.register(Arc::new(VSCodeSettingsPlugin::new()));
    registry.register(Arc::new(VSCodeExtensionsPlugin::new()));
    registry.register(Arc::new(VSCodeSettingsPlugin::new()));
    registry.register(Arc::new(VSCodeExtensionsPlugin::new()));

    let restore_manager = RestoreManager::new(Arc::new(registry), snapshots_dir);

    // Restore only homebrew plugin
    let selected_plugins = vec!["homebrew_brewfile".to_string()];
    let results = restore_manager
        .restore_from_snapshot("test_snapshot", Some(&selected_plugins), true, false)
        .await?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].plugin_name, "homebrew_brewfile");
    assert!(results[0].success);

    Ok(())
}

#[tokio::test]
async fn test_restore_with_backup() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    fs::create_dir_all(&snapshots_dir).await?;

    // Create test snapshot
    create_test_snapshot(&snapshots_dir, "test_snapshot", true).await?;

    let mut registry = PluginRegistry::new();
    registry.register(Arc::new(HomebrewBrewfilePlugin::new()));
    registry.register(Arc::new(VSCodeSettingsPlugin::new()));
    registry.register(Arc::new(VSCodeExtensionsPlugin::new()));

    let restore_manager = RestoreManager::new(Arc::new(registry), snapshots_dir);

    // Perform restore with backup (dry run to avoid actual file operations)
    let results = restore_manager
        .restore_from_snapshot("test_snapshot", None, true, true)
        .await?;

    assert!(!results.is_empty());

    // In dry run mode, backup paths should still be set
    for result in &results {
        assert!(result.success);
        assert!(result.backup_path.is_some());
    }

    Ok(())
}

#[tokio::test]
async fn test_restore_with_hooks() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    let config_path = temp_dir.path().join("config.toml");
    fs::create_dir_all(&snapshots_dir).await?;

    // Create test snapshot
    create_test_snapshot(&snapshots_dir, "test_snapshot", true).await?;

    // Create config with restore hooks
    let config = create_test_config_with_restore_hooks(&config_path).await?;

    let mut registry = PluginRegistry::new();
    registry.register(Arc::new(HomebrewBrewfilePlugin::new()));
    registry.register(Arc::new(VSCodeSettingsPlugin::new()));
    registry.register(Arc::new(VSCodeExtensionsPlugin::new()));

    let restore_manager =
        RestoreManager::with_config(Arc::new(registry), snapshots_dir, Arc::new(config));

    // Perform restore (dry run to avoid actual operations)
    let results = restore_manager
        .restore_from_snapshot("test_snapshot", None, true, false)
        .await?;

    assert!(!results.is_empty());
    assert!(results[0].success);

    // This test primarily ensures hooks don't cause crashes
    // Detailed hook execution testing is done in hook-specific tests

    Ok(())
}

#[tokio::test]
async fn test_snapshot_info_format_size() -> Result<()> {
    let snapshot_info = SnapshotInfo {
        name: "test_snapshot".to_string(),
        path: PathBuf::from("/test"),
        created_at: Local::now(),
        size_bytes: 1024,
        plugin_count: 3,
    };

    assert_eq!(snapshot_info.format_size(), "1.0 KB");

    let large_snapshot = SnapshotInfo {
        name: "large_snapshot".to_string(),
        path: PathBuf::from("/test"),
        created_at: Local::now(),
        size_bytes: 1048576, // 1 MB
        plugin_count: 5,
    };

    assert_eq!(large_snapshot.format_size(), "1.0 MB");

    let small_snapshot = SnapshotInfo {
        name: "small_snapshot".to_string(),
        path: PathBuf::from("/test"),
        created_at: Local::now(),
        size_bytes: 512,
        plugin_count: 1,
    };

    assert_eq!(small_snapshot.format_size(), "512 B");

    Ok(())
}

#[tokio::test]
async fn test_cli_restore_list_command() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    let config_path = temp_dir.path().join("config.toml");
    fs::create_dir_all(&snapshots_dir).await?;

    // Create test snapshots
    create_test_snapshot(&snapshots_dir, "20250122_120000", true).await?;
    create_test_snapshot(&snapshots_dir, "20250121_150000", true).await?;

    // Create minimal config
    let config = Config {
        output_dir: Some(snapshots_dir),
        ..Config::default()
    };
    config.save_to_file(&config_path).await?;

    let list_command = RestoreCommands::List {
        snapshots_dir: None, // Use config default
    };

    // This should not panic or error
    let result = handle_restore_command(list_command, Some(config_path)).await;
    assert!(result.is_ok());

    Ok(())
}

#[tokio::test]
async fn test_cli_restore_list_with_custom_dir() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    fs::create_dir_all(&snapshots_dir).await?;

    // Create test snapshot
    create_test_snapshot(&snapshots_dir, "test_snapshot", true).await?;

    let list_command = RestoreCommands::List {
        snapshots_dir: Some(snapshots_dir),
    };

    // This should work even without a config file
    let result = handle_restore_command(list_command, None).await;
    assert!(result.is_ok());

    Ok(())
}

#[tokio::test]
async fn test_cli_restore_restore_command_dry_run() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    let config_path = temp_dir.path().join("config.toml");
    fs::create_dir_all(&snapshots_dir).await?;

    // Create test snapshot
    create_test_snapshot(&snapshots_dir, "test_snapshot", true).await?;

    // Create config
    let config = Config {
        output_dir: Some(snapshots_dir),
        ..Config::default()
    };
    config.save_to_file(&config_path).await?;

    let restore_command = RestoreCommands::Restore {
        snapshot: "test_snapshot".to_string(),
        plugins: None,
        snapshots_dir: None,
        dry_run: true,
        interactive: false,
        backup: false,
    };

    // This should complete successfully
    let result = handle_restore_command(restore_command, Some(config_path)).await;
    assert!(result.is_ok());

    Ok(())
}

#[tokio::test]
async fn test_cli_restore_with_specific_plugins() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    let config_path = temp_dir.path().join("config.toml");
    fs::create_dir_all(&snapshots_dir).await?;

    // Create test snapshot
    create_test_snapshot(&snapshots_dir, "test_snapshot", true).await?;

    // Create config
    let config = Config {
        output_dir: Some(snapshots_dir),
        ..Config::default()
    };
    config.save_to_file(&config_path).await?;

    let restore_command = RestoreCommands::Restore {
        snapshot: "test_snapshot".to_string(),
        plugins: Some("homebrew_brewfile,vscode_settings".to_string()),
        snapshots_dir: None,
        dry_run: true,
        interactive: false,
        backup: false,
    };

    let result = handle_restore_command(restore_command, Some(config_path)).await;
    assert!(result.is_ok());

    Ok(())
}

#[tokio::test]
async fn test_cli_restore_with_backup() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    let config_path = temp_dir.path().join("config.toml");
    fs::create_dir_all(&snapshots_dir).await?;

    // Create test snapshot
    create_test_snapshot(&snapshots_dir, "test_snapshot", true).await?;

    // Create config
    let config = Config {
        output_dir: Some(snapshots_dir),
        ..Config::default()
    };
    config.save_to_file(&config_path).await?;

    let restore_command = RestoreCommands::Restore {
        snapshot: "test_snapshot".to_string(),
        plugins: None,
        snapshots_dir: None,
        dry_run: true, // Keep as dry run to avoid actual operations
        interactive: false,
        backup: true,
    };

    let result = handle_restore_command(restore_command, Some(config_path)).await;
    assert!(result.is_ok());

    Ok(())
}

#[tokio::test]
async fn test_cli_restore_nonexistent_snapshot() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    let config_path = temp_dir.path().join("config.toml");
    fs::create_dir_all(&snapshots_dir).await?;

    // Create config
    let config = Config {
        output_dir: Some(snapshots_dir),
        ..Config::default()
    };
    config.save_to_file(&config_path).await?;

    let restore_command = RestoreCommands::Restore {
        snapshot: "nonexistent".to_string(),
        plugins: None,
        snapshots_dir: None,
        dry_run: true,
        interactive: false,
        backup: false,
    };

    let result = handle_restore_command(restore_command, Some(config_path)).await;
    assert!(result.is_err());

    Ok(())
}

// Removed test for private method map_file_to_plugin - tested indirectly through other tests

// Integration test that covers the full restore workflow
#[tokio::test]
async fn test_full_restore_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    let config_path = temp_dir.path().join("config.toml");
    fs::create_dir_all(&snapshots_dir).await?;

    // Create multiple test snapshots
    create_test_snapshot(&snapshots_dir, "snapshot_2025_01_01", true).await?;
    create_test_snapshot(&snapshots_dir, "snapshot_2025_01_02", true).await?;
    create_test_snapshot(&snapshots_dir, "snapshot_2025_01_03", false).await?; // No metadata

    // Create config with restore hooks
    let _config = create_test_config_with_restore_hooks(&config_path).await?;

    // Test 1: List snapshots
    let list_command = RestoreCommands::List {
        snapshots_dir: Some(snapshots_dir.clone()),
    };
    let result = handle_restore_command(list_command, Some(config_path.clone())).await;
    assert!(result.is_ok());

    // Test 2: Restore all plugins from newest snapshot (dry run)
    let restore_all_command = RestoreCommands::Restore {
        snapshot: "snapshot_2025_01_03".to_string(), // Use the one without metadata
        plugins: None,
        snapshots_dir: Some(snapshots_dir.clone()),
        dry_run: true,
        interactive: false,
        backup: false,
    };
    let result = handle_restore_command(restore_all_command, Some(config_path.clone())).await;
    assert!(result.is_ok());

    // Test 3: Restore specific plugins with backup (dry run)
    let restore_specific_command = RestoreCommands::Restore {
        snapshot: "snapshot_2025_01_02".to_string(),
        plugins: Some("homebrew_brewfile,vscode_settings".to_string()),
        snapshots_dir: Some(snapshots_dir.clone()),
        dry_run: true,
        interactive: false,
        backup: true,
    };
    let result = handle_restore_command(restore_specific_command, Some(config_path.clone())).await;
    assert!(result.is_ok());

    Ok(())
}

// Test error handling scenarios
#[tokio::test]
async fn test_restore_error_scenarios() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let snapshots_dir = temp_dir.path().to_path_buf();
    let config_path = temp_dir.path().join("config.toml");

    // Create config pointing to non-existent snapshots directory
    let config = Config {
        output_dir: Some(snapshots_dir.join("nonexistent")),
        ..Config::default()
    };
    config.save_to_file(&config_path).await?;

    // Test 1: List snapshots in non-existent directory
    let list_command = RestoreCommands::List {
        snapshots_dir: None,
    };
    let result = handle_restore_command(list_command, Some(config_path.clone())).await;
    // This should succeed but return empty list
    assert!(result.is_ok());

    // Test 2: Restore from non-existent directory
    let restore_command = RestoreCommands::Restore {
        snapshot: "any_snapshot".to_string(),
        plugins: None,
        snapshots_dir: None,
        dry_run: true,
        interactive: false,
        backup: false,
    };
    let result = handle_restore_command(restore_command, Some(config_path.clone())).await;
    assert!(result.is_err());

    Ok(())
}

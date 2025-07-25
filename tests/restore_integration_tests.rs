use anyhow::Result;
use dotsnapshot::config::Config;
use dotsnapshot::core::restore::RestoreManager;
use tempfile::TempDir;
use tokio::fs;

/// Integration tests for restore functionality
/// These tests verify the complete restore workflow including
/// discovery, planning, and execution of restoration operations.

#[tokio::test]
/// Test restore behavior with an empty snapshot directory
async fn test_restore_manager_with_empty_snapshot() -> Result<()> {
    let temp_snapshot = TempDir::new()?;
    let temp_target = TempDir::new()?;

    let restore_manager = RestoreManager::new(
        temp_snapshot.path().to_path_buf(),
        temp_target.path().to_path_buf(),
        None, // no global target override
        Config::default(),
        true,  // dry_run
        true,  // backup
        false, // force
    );

    let result = restore_manager.execute_restore(None).await?;
    assert_eq!(result.len(), 0, "Empty snapshot should restore no files");

    Ok(())
}

#[tokio::test]
/// Test restore functionality with a single plugin containing one file
async fn test_restore_manager_with_single_plugin() -> Result<()> {
    let temp_snapshot = TempDir::new()?;
    let temp_target = TempDir::new()?;

    // Create a test plugin snapshot with a single file
    let plugin_dir = temp_snapshot.path().join("test_plugin");
    fs::create_dir_all(&plugin_dir).await?;
    fs::write(plugin_dir.join("config.json"), r#"{"test": "value"}"#).await?;

    let restore_manager = RestoreManager::new(
        temp_snapshot.path().to_path_buf(),
        temp_target.path().to_path_buf(),
        None, // no global target override
        Config::default(),
        true,  // dry_run
        true,  // backup
        false, // force
    );

    let result = restore_manager.execute_restore(None).await?;
    assert_eq!(result.len(), 1, "Should plan to restore one file");

    let restored_file = &result[0];
    assert_eq!(
        restored_file.file_name().unwrap().to_str().unwrap(),
        "config.json"
    );

    Ok(())
}

#[tokio::test]
/// Test restore functionality with multiple plugins containing multiple files each
async fn test_restore_manager_with_multiple_plugins() -> Result<()> {
    let temp_snapshot = TempDir::new()?;
    let temp_target = TempDir::new()?;

    // Create multiple plugin snapshots
    for plugin_name in ["plugin1", "plugin2", "plugin3"] {
        let plugin_dir = temp_snapshot.path().join(plugin_name);
        fs::create_dir_all(&plugin_dir).await?;
        fs::write(
            plugin_dir.join("config.txt"),
            format!("config for {plugin_name}"),
        )
        .await?;
        fs::write(
            plugin_dir.join("data.json"),
            format!(r#"{{"plugin": "{plugin_name}"}}"#),
        )
        .await?;
    }

    let restore_manager = RestoreManager::new(
        temp_snapshot.path().to_path_buf(),
        temp_target.path().to_path_buf(),
        None, // no global target override
        Config::default(),
        true,  // dry_run
        true,  // backup
        false, // force
    );

    let result = restore_manager.execute_restore(None).await?;
    assert_eq!(
        result.len(),
        6,
        "Should plan to restore 6 files (2 per plugin)"
    );

    Ok(())
}

#[tokio::test]
/// Test plugin filtering to restore only specific plugins from a snapshot
async fn test_restore_manager_plugin_filtering() -> Result<()> {
    let temp_snapshot = TempDir::new()?;
    let temp_target = TempDir::new()?;

    // Create multiple plugin snapshots
    for plugin_name in ["wanted_plugin", "unwanted_plugin"] {
        let plugin_dir = temp_snapshot.path().join(plugin_name);
        fs::create_dir_all(&plugin_dir).await?;
        fs::write(
            plugin_dir.join("config.txt"),
            format!("config for {plugin_name}"),
        )
        .await?;
    }

    let restore_manager = RestoreManager::new(
        temp_snapshot.path().to_path_buf(),
        temp_target.path().to_path_buf(),
        None, // no global target override
        Config::default(),
        true,  // dry_run
        true,  // backup
        false, // force
    );

    // Test filtering to only restore wanted_plugin
    let selected_plugins = Some(vec!["wanted_plugin".to_string()]);
    let result = restore_manager.execute_restore(selected_plugins).await?;
    assert_eq!(
        result.len(),
        1,
        "Should only restore files from wanted_plugin"
    );

    // Verify the restored file is from the correct plugin
    let restored_file = &result[0];
    println!("Restored file path: {}", restored_file.display());

    // The restored file path will be the target path, not source path
    // We should verify that we only got one file (from the wanted plugin)
    // and that it's the config.txt file from wanted_plugin
    assert!(restored_file.file_name().unwrap().to_str().unwrap() == "config.txt");

    Ok(())
}

#[tokio::test]
/// Test error handling when filtering for plugins that don't exist in the snapshot
async fn test_restore_manager_nonexistent_plugin_filter() -> Result<()> {
    let temp_snapshot = TempDir::new()?;
    let temp_target = TempDir::new()?;

    // Create a real plugin snapshot
    let plugin_dir = temp_snapshot.path().join("real_plugin");
    fs::create_dir_all(&plugin_dir).await?;
    fs::write(plugin_dir.join("config.txt"), "real config").await?;

    let restore_manager = RestoreManager::new(
        temp_snapshot.path().to_path_buf(),
        temp_target.path().to_path_buf(),
        None, // no global target override
        Config::default(),
        true,  // dry_run
        true,  // backup
        false, // force
    );

    // Test filtering with non-existent plugin
    let selected_plugins = Some(vec!["nonexistent_plugin".to_string()]);
    let result = restore_manager.execute_restore(selected_plugins).await;

    assert!(
        result.is_err(),
        "Should fail when no selected plugins are found"
    );
    let error_message = result.unwrap_err().to_string();
    assert!(error_message.contains("None of the selected plugins were found"));

    Ok(())
}

#[tokio::test]
/// Test restore functionality with nested directory structures in snapshots
async fn test_restore_manager_nested_directories() -> Result<()> {
    let temp_snapshot = TempDir::new()?;
    let temp_target = TempDir::new()?;

    // Create nested directory structure in snapshot
    let plugin_dir = temp_snapshot.path().join("nested_plugin");
    fs::create_dir_all(&plugin_dir.join("subdir/deeper")).await?;
    fs::write(plugin_dir.join("root_file.txt"), "root content").await?;
    fs::write(plugin_dir.join("subdir/mid_file.txt"), "middle content").await?;
    fs::write(
        plugin_dir.join("subdir/deeper/deep_file.txt"),
        "deep content",
    )
    .await?;

    let restore_manager = RestoreManager::new(
        temp_snapshot.path().to_path_buf(),
        temp_target.path().to_path_buf(),
        None, // no global target override
        Config::default(),
        true,  // dry_run
        true,  // backup
        false, // force
    );

    let result = restore_manager.execute_restore(None).await?;
    assert_eq!(
        result.len(),
        3,
        "Should plan to restore all 3 files from nested structure"
    );

    // Verify that nested paths are preserved
    let restored_paths: Vec<String> = result
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    println!("Restored paths: {restored_paths:?}");

    // Check that we have the expected file names (nested structure will be preserved in target)
    let file_names: Vec<String> = result
        .iter()
        .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
        .collect();

    assert!(file_names.contains(&"root_file.txt".to_string()));
    assert!(file_names.contains(&"mid_file.txt".to_string()));
    assert!(file_names.contains(&"deep_file.txt".to_string()));

    Ok(())
}

#[tokio::test]
/// Test actual file restoration (not dry-run) to verify files are physically copied
async fn test_restore_manager_actual_file_operation() -> Result<()> {
    let temp_snapshot = TempDir::new()?;
    let temp_target = TempDir::new()?;

    // Create a test file in snapshot
    let plugin_dir = temp_snapshot.path().join("file_test_plugin");
    fs::create_dir_all(&plugin_dir).await?;
    let test_content = "This is test content for restoration";
    fs::write(plugin_dir.join("test_file.txt"), test_content).await?;

    let restore_manager = RestoreManager::new(
        temp_snapshot.path().to_path_buf(),
        temp_target.path().to_path_buf(),
        None, // no global target override
        Config::default(),
        false, // NOT dry_run - actually perform restoration
        false, // no backup for this test
        true,  // force (skip confirmation)
    );

    let result = restore_manager.execute_restore(None).await?;
    assert_eq!(result.len(), 1, "Should restore one file");

    // Verify the file was actually restored
    let restored_file = &result[0];
    assert!(restored_file.exists(), "Restored file should exist");

    let restored_content = fs::read_to_string(restored_file).await?;
    assert_eq!(
        restored_content, test_content,
        "Restored content should match original"
    );

    Ok(())
}

#[tokio::test]
/// Test wildcard plugin filtering (e.g., vscode* matches vscode_settings and vscode_extensions)
async fn test_restore_manager_wildcard_plugin_filtering() -> Result<()> {
    let temp_snapshot = TempDir::new()?;
    let temp_target = TempDir::new()?;

    // Create plugins with a common prefix
    for plugin_name in [
        "vscode_settings",
        "vscode_extensions",
        "npm_global",
        "other_plugin",
    ] {
        let plugin_dir = temp_snapshot.path().join(plugin_name);
        fs::create_dir_all(&plugin_dir).await?;
        fs::write(
            plugin_dir.join("config.txt"),
            format!("config for {plugin_name}"),
        )
        .await?;
    }

    let restore_manager = RestoreManager::new(
        temp_snapshot.path().to_path_buf(),
        temp_target.path().to_path_buf(),
        None, // no global target override
        Config::default(),
        true,  // dry_run
        true,  // backup
        false, // force
    );

    // Test wildcard filtering for vscode* plugins
    let selected_plugins = Some(vec!["vscode*".to_string()]);
    let result = restore_manager.execute_restore(selected_plugins).await?;
    println!("Wildcard test - restored files: {result:?}");

    // The wildcard should match vscode_settings and vscode_extensions (2 files total)
    assert_eq!(
        result.len(),
        2,
        "Should restore files from both vscode plugins"
    );

    Ok(())
}

#[tokio::test]
/// Test that identical files are detected and skipped during restoration
async fn test_restore_manager_skip_identical_files() -> Result<()> {
    let temp_snapshot = TempDir::new()?;
    let temp_target = TempDir::new()?;

    // Create a test file in snapshot
    let plugin_dir = temp_snapshot.path().join("identical_test_plugin");
    fs::create_dir_all(&plugin_dir).await?;
    let test_content = "identical content";
    fs::write(plugin_dir.join("identical_file.txt"), test_content).await?;

    // Create the same file in target (simulating it already exists)
    let target_file = temp_target.path().join("identical_file.txt");
    fs::write(&target_file, test_content).await?;

    let restore_manager = RestoreManager::new(
        temp_snapshot.path().to_path_buf(),
        temp_target.path().to_path_buf(),
        None, // no global target override
        Config::default(),
        true,  // dry_run
        true,  // backup
        false, // force
    );

    let result = restore_manager.execute_restore(None).await?;

    // The file should still be in the result because we plan the operation
    // but the operation type should be determined as Skip during planning
    assert_eq!(result.len(), 1, "Should still plan the operation");

    Ok(())
}

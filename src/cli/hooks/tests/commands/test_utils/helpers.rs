//! Helper functions for CLI hooks command tests

use crate::config::Config;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tokio::fs;

/// Creates a temporary directory and config file for testing
/// Returns (temp_dir, config_path)
pub fn create_test_environment() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test_config.toml");
    (temp_dir, config_path)
}

/// Sets up a config file with the given configuration
pub async fn setup_config_file(config: &Config, config_path: &Path) {
    config.save_to_file(config_path).await.unwrap();
}

/// Creates a scripts directory with test scripts
pub async fn create_test_scripts_dir(base_dir: &Path) -> PathBuf {
    let scripts_dir = base_dir.join("scripts");
    fs::create_dir_all(&scripts_dir).await.unwrap();

    // Create test scripts
    fs::write(scripts_dir.join("test.sh"), "#!/bin/bash\necho test")
        .await
        .unwrap();
    fs::write(
        scripts_dir.join("test.py"),
        "#!/usr/bin/env python\nprint('test')",
    )
    .await
    .unwrap();

    scripts_dir
}

/// Creates multiple test scripts in a directory
pub async fn create_multiple_test_scripts(scripts_dir: &Path) {
    fs::create_dir_all(scripts_dir).await.unwrap();

    fs::write(scripts_dir.join("test1.sh"), "#!/bin/bash\necho test1")
        .await
        .unwrap();
    fs::write(
        scripts_dir.join("test2.py"),
        "#!/usr/bin/env python\nprint('test2')",
    )
    .await
    .unwrap();
    fs::write(scripts_dir.join("test3.js"), "console.log('test3')")
        .await
        .unwrap();
}

/// Verifies that a hook was added to the configuration
pub async fn verify_hook_added(config_path: &Path, expected_count: usize) {
    let updated_config = Config::load_from_file(config_path).await.unwrap();
    let pre_hooks = updated_config.get_global_pre_snapshot_hooks();
    assert_eq!(pre_hooks.len(), expected_count);
}

/// Verifies that hooks were removed from the configuration  
pub async fn verify_hooks_removed(config_path: &Path, expected_count: usize) {
    let updated_config = Config::load_from_file(config_path).await.unwrap();
    let pre_hooks = updated_config.get_global_pre_snapshot_hooks();
    assert_eq!(pre_hooks.len(), expected_count);
}

/// Creates a test environment with a scripts directory
pub async fn create_test_environment_with_scripts() -> (TempDir, PathBuf, PathBuf) {
    let (temp_dir, config_path) = create_test_environment();
    let scripts_dir = create_test_scripts_dir(temp_dir.path()).await;
    (temp_dir, config_path, scripts_dir)
}

use anyhow::Result;
use std::path::PathBuf;

use crate::config::Config;

use super::test_utils::{create_basic_config, create_temp_dir};

/// Test config loading and saving
/// Verifies that configurations can be saved and loaded correctly
#[tokio::test]
async fn test_config_load_and_save() -> Result<()> {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join("config.toml");
    let config = create_basic_config();

    // Save config
    config.save_to_file(&config_path).await?;

    // Load config
    let loaded_config = Config::load_from_file(&config_path).await?;
    assert_eq!(
        loaded_config.get_output_dir(),
        PathBuf::from("/tmp/snapshots")
    );
    assert_eq!(
        loaded_config.get_include_plugins(),
        Some(vec!["homebrew".to_string(), "vscode".to_string()])
    );
    assert!(loaded_config.is_verbose_default());

    Ok(())
}

/// Test config paths discovery
/// Verifies that config file paths are discovered correctly
#[tokio::test]
async fn test_config_paths() {
    let paths = Config::get_config_paths();
    assert!(!paths.is_empty());
    assert!(paths
        .iter()
        .any(|p| p.file_name().unwrap() == "dotsnapshot.toml"));
}

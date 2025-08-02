use anyhow::Result;
use std::path::PathBuf;

use crate::config::Config;

use super::test_utils::{create_complex_config, create_temp_dir};

/// Test config serialization and deserialization edge cases
/// Verifies that complex configurations can be properly saved and loaded
#[tokio::test]
async fn test_config_serialization_edge_cases() -> Result<()> {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join("complex_config.toml");

    // Create a complex configuration with all features
    let complex_config = create_complex_config();

    // Save the complex configuration
    complex_config.save_to_file(&config_path).await?;

    // Load and verify the configuration
    let loaded_config = Config::load_from_file(&config_path).await?;

    // Verify all aspects were preserved
    assert_eq!(
        loaded_config.get_output_dir(),
        PathBuf::from("/complex/output")
    );
    assert!(loaded_config.is_verbose_default());
    assert_eq!(loaded_config.get_include_plugins().unwrap().len(), 3);

    Ok(())
}

/// Test hooks preservation during serialization
/// Verifies that hook configurations are preserved across save/load cycles
#[tokio::test]
async fn test_hooks_serialization_preservation() -> Result<()> {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join("hooks_config.toml");

    let complex_config = create_complex_config();

    // Save and reload
    complex_config.save_to_file(&config_path).await?;
    let loaded_config = Config::load_from_file(&config_path).await?;

    // Verify hooks were preserved
    let hooks = loaded_config.get_hooks_config();
    assert_eq!(hooks.scripts_dir, PathBuf::from("/usr/local/bin/scripts"));

    let global_pre = loaded_config.get_global_pre_snapshot_hooks();
    assert_eq!(global_pre.len(), 1);

    let vscode_pre = loaded_config.get_plugin_pre_hooks("vscode");
    assert_eq!(vscode_pre.len(), 0); // No plugin-level hooks configured

    Ok(())
}

/// Test plugin configurations preservation during serialization
/// Verifies that plugin-specific configurations are preserved across save/load cycles
#[tokio::test]
async fn test_plugin_configurations_serialization_preservation() -> Result<()> {
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join("plugins_config.toml");

    let complex_config = create_complex_config();

    // Save and reload
    complex_config.save_to_file(&config_path).await?;
    let loaded_config = Config::load_from_file(&config_path).await?;

    // Verify plugin configurations were preserved
    let vscode_config = loaded_config.get_raw_plugin_config("vscode");
    assert!(vscode_config.is_some());

    let homebrew_config = loaded_config.get_raw_plugin_config("homebrew");
    assert!(homebrew_config.is_some());

    let npm_config = loaded_config.get_raw_plugin_config("npm");
    assert!(npm_config.is_some());

    Ok(())
}

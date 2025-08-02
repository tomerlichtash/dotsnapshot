use clap::Parser;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::fs;

use crate::Args;

/// Creates a temporary configuration file for testing
/// Returns the path to the created config file
pub async fn create_test_config() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test_config.toml");
    let config_content = r#"
        output_dir = "/tmp/test-snapshots"
        
        [logging]
        verbose = true
        time_format = "[hour]:[minute]:[second]"
    "#;
    fs::write(&config_path, config_content).await.unwrap();
    (temp_dir, config_path)
}

/// Helper function to parse command line arguments for testing
/// Returns parsed Args struct
pub fn parse_test_args(args: &[&str]) -> Args {
    Args::parse_from(args)
}

/// Plugin selection test cases for validation
/// Returns vector of (input, expected) tuples
pub fn get_plugin_selection_test_cases() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("vscode", vec!["vscode"]),
        ("vscode,homebrew", vec!["vscode", "homebrew"]),
        ("vscode,homebrew,npm", vec!["vscode", "homebrew", "npm"]),
        ("single", vec!["single"]),
        ("", vec![""]), // Edge case: empty plugin
    ]
}

/// Shell completion options for testing
/// Returns list of shell names supported for completions
pub fn get_shell_completion_options() -> Vec<&'static str> {
    vec!["bash", "zsh", "fish", "powershell", "elvish"]
}

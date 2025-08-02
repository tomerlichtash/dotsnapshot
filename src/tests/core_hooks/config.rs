use std::path::PathBuf;
use tempfile::TempDir;

use crate::core::hooks::HooksConfig;

/// Test hooks config path resolution with relative and absolute paths
/// Verifies that script paths are resolved correctly based on scripts directory
#[test]
fn test_hooks_config_path_resolution() {
    let temp_dir = TempDir::new().unwrap();
    let hooks_config = HooksConfig {
        scripts_dir: temp_dir.path().to_path_buf(),
    };

    // Test relative path resolution
    let relative_path = hooks_config.resolve_script_path("test-script.sh");
    assert_eq!(relative_path, temp_dir.path().join("test-script.sh"));

    // Test absolute path (should be unchanged)
    #[cfg(unix)]
    {
        let absolute_path = hooks_config.resolve_script_path("/usr/bin/test");
        assert_eq!(absolute_path, PathBuf::from("/usr/bin/test"));
    }
    #[cfg(windows)]
    {
        let absolute_path = hooks_config.resolve_script_path("C:\\Windows\\System32\\cmd.exe");
        assert_eq!(
            absolute_path,
            PathBuf::from("C:\\Windows\\System32\\cmd.exe")
        );
    }

    // Test subdirectory path
    let subdir_path = hooks_config.resolve_script_path("hooks/test.sh");
    assert_eq!(subdir_path, temp_dir.path().join("hooks/test.sh"));
}

/// Test hooks config tilde expansion functionality
/// Verifies that tilde paths are expanded to home directory correctly
#[test]
fn test_hooks_config_tilde_expansion() {
    // Test tilde expansion
    let home_path = PathBuf::from("~/test/path");
    let expanded = HooksConfig::expand_tilde(&home_path);

    if let Some(home_dir) = dirs::home_dir() {
        assert_eq!(expanded, home_dir.join("test/path"));
    } else {
        // If no home directory, should return original path
        assert_eq!(expanded, home_path);
    }

    // Test non-tilde path (should be unchanged)
    let regular_path = PathBuf::from("/regular/path");
    let not_expanded = HooksConfig::expand_tilde(&regular_path);
    assert_eq!(not_expanded, regular_path);
}

/// Test hooks config default values
/// Verifies that default configuration provides sensible values
#[test]
fn test_hooks_config_default() {
    let config = HooksConfig::default();

    // Default scripts directory should contain "dotsnapshot"
    assert!(config.scripts_dir.to_string_lossy().contains("dotsnapshot"));
}

/// Test hooks config single tilde expansion edge case
/// Verifies that a standalone tilde is expanded to home directory
#[test]
fn test_hooks_config_expand_tilde_single() {
    let tilde_path = PathBuf::from("~");
    let expanded = HooksConfig::expand_tilde(&tilde_path);

    if let Some(home_dir) = dirs::home_dir() {
        assert_eq!(expanded, home_dir);
    } else {
        assert_eq!(expanded, tilde_path);
    }
}

/// Test default timeout value for hook actions
/// Verifies that a reasonable default timeout is provided
#[test]
fn test_default_timeout() {
    // Default timeout should be reasonable (30 seconds)
    assert_eq!(crate::core::hooks::default_timeout(), 30);
}

/// Test default log level for hook actions
/// Verifies that a sensible default log level is provided
#[test]
fn test_default_log_level() {
    // Default log level should be "info"
    assert_eq!(crate::core::hooks::default_log_level(), "info");
}

/// Test default scripts directory configuration
/// Verifies that default scripts directory is properly configured
#[test]
fn test_default_scripts_dir() {
    let config = HooksConfig::default();
    let scripts_dir_str = config.scripts_dir.to_string_lossy();

    // Should contain dotsnapshot and scripts
    assert!(scripts_dir_str.contains("dotsnapshot"));
    assert!(scripts_dir_str.contains("scripts"));
}

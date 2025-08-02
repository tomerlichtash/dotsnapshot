use clap_complete::Shell;

use super::test_utils::{get_shell_completion_options, parse_test_args};

/// Test parsing of info and utility flags
/// Verifies that special flags like --info, --man, --completions are parsed correctly
#[test]
fn test_utility_flags_parsing() {
    // Test --info flag
    let args = parse_test_args(&["dotsnapshot", "--info"]);
    assert!(args.info);

    // Test --man flag
    let args = parse_test_args(&["dotsnapshot", "--man"]);
    assert!(args.man);

    // Test --completions flag
    let args = parse_test_args(&["dotsnapshot", "--completions", "bash"]);
    assert_eq!(args.completions, Some(Shell::Bash));

    let args = parse_test_args(&["dotsnapshot", "--completions", "zsh"]);
    assert_eq!(args.completions, Some(Shell::Zsh));
}

/// Test completions with different shells
/// Verifies that all supported shell types can be parsed
#[test]
fn test_completions_all_shells() {
    // Test completions with different shells
    let shell_options = get_shell_completion_options();
    for shell in shell_options {
        let args = parse_test_args(&["dotsnapshot", "--completions", shell]);
        assert!(args.completions.is_some());
    }
}

/// Test version information access
/// Verifies that version info is available for --info command
#[test]
fn test_version_info() {
    // Test that version info can be accessed (used in --info command)
    let version = env!("CARGO_PKG_VERSION");
    let description = env!("CARGO_PKG_DESCRIPTION");
    let repository = env!("CARGO_PKG_REPOSITORY");
    let license = env!("CARGO_PKG_LICENSE");

    assert!(!version.is_empty());
    assert!(!description.is_empty());
    assert!(!repository.is_empty());
    assert!(!license.is_empty());
}

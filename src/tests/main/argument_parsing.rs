use std::path::PathBuf;

use super::test_utils::{get_plugin_selection_test_cases, parse_test_args};

/// Test basic argument parsing with default values
/// Verifies that command-line arguments are parsed correctly
#[test]
fn test_args_parsing() {
    // Test default values
    let args = parse_test_args(&["dotsnapshot"]);
    assert!(args.output.is_none());
    assert!(!args.verbose);
    assert!(args.plugins.is_none());
    assert!(args.config.is_none());
    assert!(!args.list);

    // Test custom values
    let args = parse_test_args(&[
        "dotsnapshot",
        "--output",
        "/tmp/test",
        "--verbose",
        "--plugins",
        "homebrew,npm",
        "--config",
        "/path/to/config.toml",
    ]);
    assert_eq!(args.output.unwrap(), PathBuf::from("/tmp/test"));
    assert!(args.verbose);
    assert_eq!(args.plugins.unwrap(), "homebrew,npm");
    assert_eq!(args.config.unwrap(), PathBuf::from("/path/to/config.toml"));
    assert!(!args.list);

    // Test --list flag
    let args = parse_test_args(&["dotsnapshot", "--list"]);
    assert!(args.list);
}

/// Test debug flag parsing
/// Verifies that the debug flag is parsed correctly in CLI arguments
#[test]
fn test_debug_flag_parsing() {
    // Test default debug value (should be false)
    let args = parse_test_args(&["dotsnapshot"]);
    assert!(!args.debug);

    // Test --debug flag
    let args = parse_test_args(&["dotsnapshot", "--debug"]);
    assert!(args.debug);

    // Test --debug with other flags
    let args = parse_test_args(&["dotsnapshot", "--debug", "--verbose", "--list"]);
    assert!(args.debug);
    assert!(args.verbose);
    assert!(args.list);
}

/// Test argument validation and conflicts
/// Verifies that certain argument combinations work correctly
#[test]
fn test_argument_validation() {
    // Test that certain argument combinations work
    let args = parse_test_args(&[
        "dotsnapshot",
        "--verbose",
        "--output",
        "/tmp/test",
        "--plugins",
        "vscode,homebrew",
    ]);

    assert!(args.verbose);
    assert_eq!(args.output, Some(PathBuf::from("/tmp/test")));
    assert_eq!(args.plugins, Some("vscode,homebrew".to_string()));
}

/// Test edge cases in argument parsing
/// Verifies parsing of edge cases like empty values and paths with spaces
#[test]
fn test_argument_parsing_edge_cases() {
    // Test with empty plugin list (should still parse)
    let args = parse_test_args(&["dotsnapshot", "--plugins", ""]);
    assert_eq!(args.plugins, Some("".to_string()));

    // Test with multiple output formats
    let args = parse_test_args(&["dotsnapshot", "--output", "/path/with spaces/snapshots"]);
    assert_eq!(
        args.output,
        Some(PathBuf::from("/path/with spaces/snapshots"))
    );
}

/// Test plugin selection parsing logic
/// Verifies that plugin string parsing works for various formats
#[test]
fn test_plugin_selection_parsing() {
    // Test various plugin string formats
    let test_cases = get_plugin_selection_test_cases();

    for (input, expected) in test_cases {
        let parsed: Vec<String> = input.split(',').map(|s| s.to_string()).collect();
        let expected_strings: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            parsed, expected_strings,
            "Failed to parse plugin string: {input}"
        );
    }
}

/// Test default argument values
/// Verifies that all CLI arguments have correct default values
#[test]
fn test_default_argument_values() {
    let args = parse_test_args(&["dotsnapshot"]);

    // Verify all default values
    assert!(!args.verbose);
    assert!(!args.debug);
    assert!(args.config.is_none());
    assert!(args.command.is_none());
    assert!(args.output.is_none());
    assert!(args.plugins.is_none());
    assert!(!args.list);
    assert!(!args.info);
    assert!(args.completions.is_none());
    assert!(!args.man);
}

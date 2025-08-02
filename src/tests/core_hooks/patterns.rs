use crate::core::hooks::simple_pattern_match;

/// Test simple pattern matching with various wildcard patterns
/// Verifies that glob-like pattern matching works correctly
#[test]
fn test_simple_pattern_match() {
    assert!(simple_pattern_match("*", "anything.txt"));
    assert!(simple_pattern_match("*.txt", "file.txt"));
    assert!(!simple_pattern_match("*.txt", "file.log"));
    assert!(simple_pattern_match("test*", "test123"));
    assert!(!simple_pattern_match("test*", "other123"));
    assert!(simple_pattern_match("*tmp*", "file.tmp.bak"));
    assert!(!simple_pattern_match("*tmp*", "file.log"));
    assert!(simple_pattern_match("exact.txt", "exact.txt"));
    assert!(!simple_pattern_match("exact.txt", "other.txt"));
}

/// Test simple pattern matching edge cases and special scenarios
/// Verifies pattern matching handles edge cases correctly
#[test]
fn test_simple_pattern_match_edge_cases() {
    // Empty patterns and strings
    assert!(simple_pattern_match("", ""));
    assert!(!simple_pattern_match("", "something"));
    assert!(!simple_pattern_match("something", ""));

    // Multiple wildcards
    assert!(simple_pattern_match("*.*", "file.txt"));
    assert!(simple_pattern_match("*.*", "file.backup.txt"));
    assert!(!simple_pattern_match("*.*", "file"));

    // Leading/trailing wildcards
    assert!(simple_pattern_match("*test", "mytest"));
    assert!(simple_pattern_match("test*", "testing"));
    assert!(simple_pattern_match("*test*", "mytesting"));

    // Special characters (should be treated literally, not as regex)
    assert!(simple_pattern_match("file[123]", "file[123]"));
    assert!(!simple_pattern_match("file[123]", "file1"));
    assert!(simple_pattern_match("file.txt", "file.txt"));
    assert!(!simple_pattern_match("file.txt", "filetxt"));
}

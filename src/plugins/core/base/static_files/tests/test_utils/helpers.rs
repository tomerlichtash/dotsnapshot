//! Helper functions and utilities for static_files tests

use std::path::PathBuf;
use tempfile::TempDir;

/// Create test file paths for mock scenarios
/// Returns a vector of PathBuf for consistent testing
pub fn create_test_file_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/test/file1.txt"),
        PathBuf::from("/test/file2.txt"),
        PathBuf::from("/test/subdir/file3.txt"),
    ]
}

/// Validate JSON response structure
/// Checks if a JSON string contains expected fields
pub fn validate_json_response(json_str: &str) -> bool {
    // Handle the case where response has STATIC_DIR_CHECKSUM: prefix
    let json_part = if json_str.starts_with("STATIC_DIR_CHECKSUM:") {
        // Find the newline after the checksum prefix
        if let Some(newline_pos) = json_str.find('\n') {
            &json_str[newline_pos + 1..]
        } else {
            json_str
        }
    } else {
        json_str
    };

    match serde_json::from_str::<serde_json::Value>(json_part) {
        Ok(json) => {
            json.get("summary").is_some()
                && json["summary"].get("total_files").is_some()
                && json["summary"].get("copied").is_some()
                && json["summary"].get("failed").is_some()
        }
        Err(_) => false,
    }
}

/// Extract file count from JSON response
/// Returns the total_files count from a JSON response
pub fn extract_file_count_from_json(json_str: &str) -> Option<usize> {
    // Handle the case where response has STATIC_DIR_CHECKSUM: prefix
    let json_part = if json_str.starts_with("STATIC_DIR_CHECKSUM:") {
        // Find the newline after the checksum prefix
        if let Some(newline_pos) = json_str.find('\n') {
            &json_str[newline_pos + 1..]
        } else {
            json_str
        }
    } else {
        json_str
    };

    serde_json::from_str::<serde_json::Value>(json_part)
        .ok()?
        .get("summary")?
        .get("total_files")?
        .as_u64()
        .map(|n| n as usize)
}

/// Create a mock snapshot directory structure
/// Creates temporary directories that simulate snapshot structure
pub async fn create_mock_snapshot_dir(temp_dir: &TempDir) -> PathBuf {
    let snapshot_dir = temp_dir.path().join("snapshot_123");
    let static_dir = snapshot_dir.join("static");

    tokio::fs::create_dir_all(&static_dir)
        .await
        .expect("Failed to create mock snapshot directory");

    // Create some mock files
    tokio::fs::write(static_dir.join("file1.txt"), "content1")
        .await
        .expect("Failed to create mock file");
    tokio::fs::write(static_dir.join("file2.txt"), "content2")
        .await
        .expect("Failed to create mock file");

    snapshot_dir
}

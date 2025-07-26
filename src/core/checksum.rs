use anyhow::Result;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

/// Calculates SHA256 checksum of a string
pub fn calculate_checksum(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Calculates checksum of directory contents (recursive)
pub fn calculate_directory_checksum(dir_path: &Path) -> Result<String> {
    let mut file_checksums = Vec::new();

    // Collect all files in directory recursively
    let mut files = Vec::new();
    collect_files(dir_path, &mut files)?;

    // Sort files for consistent ordering
    files.sort();

    // Calculate checksum for each file
    for file in files {
        let content = fs::read(&file)?;
        let mut hasher = Sha256::new();
        hasher.update(&content);

        // Include relative path in checksum calculation for uniqueness
        let relative_path = file.strip_prefix(dir_path)?;
        let file_info = format!("{}:{:x}", relative_path.display(), hasher.finalize());
        file_checksums.push(file_info);
    }

    // Calculate final checksum from all file checksums
    let combined = file_checksums.join("\n");
    Ok(calculate_checksum(&combined))
}

/// Recursively collects all files in a directory
fn collect_files(dir: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            files.push(path);
        } else if path.is_dir() {
            collect_files(&path, files)?;
        }
    }
    Ok(())
}

/// Compares two checksums for equality
pub fn checksums_equal(checksum1: &str, checksum2: &str) -> bool {
    checksum1 == checksum2
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_calculate_checksum() {
        let content = "test content";
        let checksum = calculate_checksum(content);
        assert_eq!(checksum.len(), 64); // SHA256 produces 64-char hex string

        // Same content should produce same checksum
        let checksum2 = calculate_checksum(content);
        assert_eq!(checksum, checksum2);
    }

    #[test]
    fn test_checksums_equal() {
        let checksum1 = "abc123";
        let checksum2 = "abc123";
        let checksum3 = "def456";

        assert!(checksums_equal(checksum1, checksum2));
        assert!(!checksums_equal(checksum1, checksum3));
    }

    #[test]
    fn test_calculate_directory_checksum() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let dir_path = temp_dir.path();

        // Create some test files
        let mut file1 = File::create(dir_path.join("file1.txt"))?;
        file1.write_all(b"content1")?;

        let mut file2 = File::create(dir_path.join("file2.txt"))?;
        file2.write_all(b"content2")?;

        let checksum = calculate_directory_checksum(dir_path)?;
        assert_eq!(checksum.len(), 64);

        // Same directory should produce same checksum
        let checksum2 = calculate_directory_checksum(dir_path)?;
        assert_eq!(checksum, checksum2);

        Ok(())
    }
}

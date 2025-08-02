use std::path::PathBuf;
use tempfile::TempDir;
use tokio::fs;

/// Creates a test script in the given directory
/// Returns the path to the created script file
pub async fn create_test_script(dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let script_path = dir.path().join(name);
    fs::write(&script_path, content)
        .await
        .expect("Failed to create test script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path).await.unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).await.unwrap();
    }

    script_path
}

use assert_cmd::Command;
use tempfile::TempDir;

/// Test basic CLI functionality - covers main.rs argument parsing and basic execution paths
#[test]
fn test_cli_help_command() {
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "A CLI utility to create snapshots",
        ));
}

#[test]
fn test_cli_version_command() {
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_cli_hooks_help() {
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .args(["hooks", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Manage plugin hooks"));
}

#[test]
fn test_cli_restore_help() {
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .args(["restore", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Restore configuration"));
}

#[test]
fn test_cli_list_flag() {
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .arg("--list")
        .assert()
        .success()
        .stdout(predicates::str::contains("Available plugins"));
}

#[test]
fn test_cli_info_flag() {
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .arg("--info")
        .assert()
        .success();
}

#[test]
fn test_cli_invalid_command() {
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .arg("invalid-command")
        .assert()
        .failure()
        .stderr(predicates::str::contains("error: unrecognized subcommand"));
}

#[test]
fn test_cli_with_verbose_flag() {
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .args(["--verbose", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "A CLI utility to create snapshots",
        ));
}

#[test]
fn test_cli_with_debug_flag() {
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .args(["--debug", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "A CLI utility to create snapshots",
        ));
}

#[test]
fn test_cli_with_config_flag() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test_config.toml");
    std::fs::write(&config_path, "# test config").unwrap();

    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .args(["--config", config_path.to_str().unwrap(), "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "A CLI utility to create snapshots",
        ));
}

#[test]
fn test_cli_completions_bash() {
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .arg("--completions=bash")
        .assert()
        .success()
        .stdout(predicates::str::contains("complete"));
}

#[test]
fn test_cli_completions_zsh() {
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .arg("--completions=zsh")
        .assert()
        .success()
        .stdout(predicates::str::contains("compdef"));
}

#[test]
fn test_cli_completions_fish() {
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .arg("--completions=fish")
        .assert()
        .success()
        .stdout(predicates::str::contains("complete"));
}

#[test]
fn test_cli_man_generation() {
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .arg("--man")
        .assert()
        .success()
        .stdout(predicates::str::contains(".TH"));
}

#[test]
fn test_cli_with_output_flag() {
    let temp_dir = TempDir::new().unwrap();
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .args(["--output", temp_dir.path().to_str().unwrap(), "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "A CLI utility to create snapshots",
        ));
}

#[test]
fn test_cli_with_plugins_flag() {
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .args(["--plugins", "vscode,homebrew", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "A CLI utility to create snapshots",
        ));
}

#[test]
fn test_cli_with_nonexistent_config() {
    // CLI gracefully handles nonexistent config by using defaults
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .args(["--config", "/nonexistent/config.toml", "--list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Available plugins"));
}

#[test]
fn test_cli_restore_without_args() {
    Command::cargo_bin("dotsnapshot")
        .unwrap()
        .arg("restore")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "Either provide a snapshot path or use --latest",
        ));
}

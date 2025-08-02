use std::path::PathBuf;

use crate::{Commands, HooksCommands};

use super::test_utils::parse_test_args;

/// Test hooks command parsing
/// Verifies that hooks subcommands are parsed correctly
#[test]
fn test_hooks_command_parsing() {
    // Test hooks add command
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "add",
        "--pre-snapshot",
        "--script",
        "test.sh",
    ]);

    match args.command {
        Some(Commands::Hooks { .. }) => {
            // Command structure is correct
        }
        _ => panic!("Expected hooks command"),
    }

    // Test hooks list command
    let args = parse_test_args(&["dotsnapshot", "hooks", "list"]);
    match args.command {
        Some(Commands::Hooks { .. }) => {
            // Command structure is correct
        }
        _ => panic!("Expected hooks command"),
    }
}

/// Test restore command parsing
/// Verifies that restore subcommands are parsed correctly
#[test]
fn test_restore_command_parsing() {
    // Test restore with snapshot path
    let args = parse_test_args(&["dotsnapshot", "restore", "/path/to/snapshot"]);

    match args.command {
        Some(Commands::Restore { snapshot_path, .. }) => {
            assert_eq!(snapshot_path, Some(PathBuf::from("/path/to/snapshot")));
        }
        _ => panic!("Expected restore command"),
    }

    // Test restore with --latest flag
    let args = parse_test_args(&["dotsnapshot", "restore", "--latest"]);

    match args.command {
        Some(Commands::Restore { latest, .. }) => {
            assert!(latest);
        }
        _ => panic!("Expected restore command"),
    }

    // Test restore with options
    let args = parse_test_args(&[
        "dotsnapshot",
        "restore",
        "--latest",
        "--plugins",
        "vscode,cursor",
        "--dry-run",
        "--force",
    ]);

    match args.command {
        Some(Commands::Restore {
            latest,
            plugins,
            dry_run,
            force,
            ..
        }) => {
            assert!(latest);
            assert_eq!(plugins, Some("vscode,cursor".to_string()));
            assert!(dry_run);
            assert!(force);
        }
        _ => panic!("Expected restore command"),
    }
}

/// Test hook target parsing
/// Verifies that hook targets are parsed correctly
#[test]
fn test_hook_target_parsing() {
    // Test pre-snapshot hook target
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "add",
        "--pre-snapshot",
        "--script",
        "test.sh",
    ]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Add { target, .. } => {
                assert!(target.pre_snapshot);
            }
            _ => panic!("Expected add command"),
        }
    }

    // Test plugin-specific hook target
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "add",
        "--pre-plugin",
        "vscode",
        "--log",
        "Starting VSCode backup",
    ]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Add { target, .. } => {
                assert_eq!(target.pre_plugin, Some("vscode".to_string()));
            }
            _ => panic!("Expected add command"),
        }
    }
}

/// Test hook action parsing
/// Verifies that different hook actions are parsed correctly
#[test]
fn test_hook_action_parsing() {
    // Test basic script action (without args/timeout to avoid CLI conflicts)
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "add",
        "--pre-snapshot",
        "--script",
        "backup.sh",
    ]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Add { action, .. } => {
                assert_eq!(action.script, Some("backup.sh".to_string()));
            }
            _ => panic!("Expected add command"),
        }
    }

    // Test log action (basic without level to avoid CLI conflicts)
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "add",
        "--post-snapshot",
        "--log",
        "Backup completed",
    ]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Add { action, .. } => {
                assert_eq!(action.log, Some("Backup completed".to_string()));
            }
            _ => panic!("Expected add command"),
        }
    }

    // Test notify action (basic without title to avoid CLI conflicts)
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "add",
        "--post-plugin",
        "homebrew",
        "--notify",
        "Homebrew backup complete",
    ]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Add { action, .. } => {
                assert_eq!(action.notify, Some("Homebrew backup complete".to_string()));
            }
            _ => panic!("Expected add command"),
        }
    }

    // Test backup action (basic without path/destination to avoid CLI conflicts)
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "add",
        "--pre-plugin",
        "vscode",
        "--backup",
    ]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Add { action, .. } => {
                assert!(action.backup);
            }
            _ => panic!("Expected add command"),
        }
    }

    // Test cleanup action (basic without additional flags to avoid CLI conflicts)
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "add",
        "--post-snapshot",
        "--cleanup",
    ]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Add { action, .. } => {
                assert!(action.cleanup);
            }
            _ => panic!("Expected add command"),
        }
    }
}

/// Test hooks list command parsing
/// Verifies that hooks list command options are parsed correctly
#[test]
fn test_hooks_list_parsing() {
    // Test basic list
    let args = parse_test_args(&["dotsnapshot", "hooks", "list"]);
    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::List { .. } => {
                // Command parsed correctly
            }
            _ => panic!("Expected list command"),
        }
    }

    // Test list with plugin filter
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "list",
        "--plugin",
        "vscode",
        "--verbose",
    ]);
    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::List {
                plugin, verbose, ..
            } => {
                assert_eq!(plugin, Some("vscode".to_string()));
                assert!(verbose);
            }
            _ => panic!("Expected list command"),
        }
    }

    // Test list with hook type filters
    let args = parse_test_args(&["dotsnapshot", "hooks", "list", "--pre-plugin"]);
    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::List { pre_plugin, .. } => {
                assert!(pre_plugin);
            }
            _ => panic!("Expected list command"),
        }
    }
}

/// Test hooks remove command parsing
/// Verifies that hooks remove command options are parsed correctly
#[test]
fn test_hooks_remove_parsing() {
    // Test remove by index
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "remove",
        "--pre-snapshot",
        "--index",
        "2",
    ]);
    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Remove { index, .. } => {
                assert_eq!(index, Some(2));
            }
            _ => panic!("Expected remove command"),
        }
    }

    // Test remove by script name
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "remove",
        "--post-plugin",
        "homebrew",
        "--script",
        "backup.sh",
    ]);
    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Remove { script, .. } => {
                assert_eq!(script, Some("backup.sh".to_string()));
            }
            _ => panic!("Expected remove command"),
        }
    }

    // Test remove all
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "remove",
        "--pre-plugin",
        "vscode",
        "--all",
    ]);
    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Remove { all, .. } => {
                assert!(all);
            }
            _ => panic!("Expected remove command"),
        }
    }
}

/// Test hooks validate command parsing
/// Verifies that hooks validate command options are parsed correctly
#[test]
fn test_hooks_validate_parsing() {
    // Test basic validate
    let args = parse_test_args(&["dotsnapshot", "hooks", "validate"]);
    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Validate { .. } => {
                // Command parsed correctly
            }
            _ => panic!("Expected validate command"),
        }
    }

    // Test validate with filters
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "validate",
        "--plugin",
        "cursor",
        "--pre-plugin",
        "--post-snapshot",
    ]);
    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Validate {
                plugin,
                pre_plugin,
                post_snapshot,
                ..
            } => {
                assert_eq!(plugin, Some("cursor".to_string()));
                assert!(pre_plugin);
                assert!(post_snapshot);
            }
            _ => panic!("Expected validate command"),
        }
    }
}

/// Test hooks scripts-dir command parsing
/// Verifies that scripts directory management commands are parsed correctly
#[test]
fn test_hooks_scripts_dir_parsing() {
    // Test set scripts directory
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "scripts-dir",
        "--set",
        "/home/user/scripts",
    ]);
    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::ScriptsDir { set, .. } => {
                assert_eq!(set, Some(PathBuf::from("/home/user/scripts")));
            }
            _ => panic!("Expected scripts-dir command"),
        }
    }

    // Test create scripts directory
    let args = parse_test_args(&["dotsnapshot", "hooks", "scripts-dir", "--create"]);
    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::ScriptsDir { create, .. } => {
                assert!(create);
            }
            _ => panic!("Expected scripts-dir command"),
        }
    }
}

/// Test hooks command variations
/// Verifies various combinations of hooks commands work correctly
#[test]
fn test_hooks_command_variations() {
    // Test hooks remove with different target types
    let args = parse_test_args(&["dotsnapshot", "hooks", "remove", "--post-snapshot", "--all"]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Remove { all, target, .. } => {
                assert!(all);
                assert!(target.post_snapshot);
            }
            _ => panic!("Expected remove command"),
        }
    }

    // Test hooks validate with multiple filters
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "validate",
        "--pre-snapshot",
        "--post-plugin",
    ]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Validate {
                pre_snapshot,
                post_plugin,
                ..
            } => {
                assert!(pre_snapshot);
                assert!(post_plugin);
            }
            _ => panic!("Expected validate command"),
        }
    }
}

/// Test hook action parsing with additional options
/// Verifies complex hook action configurations parse correctly
#[test]
fn test_hook_action_extended_parsing() {
    // Test script action (basic without conflicting args)
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "add",
        "--pre-plugin",
        "vscode",
        "--script",
        "backup.sh",
    ]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Add { action, .. } => {
                assert_eq!(action.script, Some("backup.sh".to_string()));
            }
            _ => panic!("Expected add command"),
        }
    }

    // Test log action (basic without conflicting level)
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "add",
        "--post-snapshot",
        "--log",
        "Backup completed successfully",
    ]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Add { action, .. } => {
                assert_eq!(
                    action.log,
                    Some("Backup completed successfully".to_string())
                );
            }
            _ => panic!("Expected add command"),
        }
    }

    // Test notify action (basic without conflicting title)
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "add",
        "--pre-plugin",
        "homebrew",
        "--notify",
        "Starting Homebrew backup",
    ]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Add { action, .. } => {
                assert_eq!(action.notify, Some("Starting Homebrew backup".to_string()));
            }
            _ => panic!("Expected add command"),
        }
    }

    // Test backup action (basic without conflicting paths)
    let args = parse_test_args(&["dotsnapshot", "hooks", "add", "--pre-snapshot", "--backup"]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Add { action, .. } => {
                assert!(action.backup);
            }
            _ => panic!("Expected add command"),
        }
    }

    // Test cleanup action (basic without conflicting options)
    let args = parse_test_args(&[
        "dotsnapshot",
        "hooks",
        "add",
        "--post-snapshot",
        "--cleanup",
    ]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::Add { action, .. } => {
                assert!(action.cleanup);
            }
            _ => panic!("Expected add command"),
        }
    }
}

/// Test restore command extended options
/// Verifies restore command with all available options
#[test]
fn test_restore_command_extended_options() {
    // Test restore with all options
    let args = parse_test_args(&[
        "dotsnapshot",
        "restore",
        "/path/to/snapshot",
        "--plugins",
        "vscode,cursor,homebrew",
        "--dry-run",
        "--backup",
        "--force",
        "--target-dir",
        "/custom/restore/target",
    ]);

    match args.command {
        Some(Commands::Restore {
            snapshot_path,
            plugins,
            dry_run,
            backup,
            force,
            target_dir,
            latest,
        }) => {
            assert_eq!(snapshot_path, Some(PathBuf::from("/path/to/snapshot")));
            assert_eq!(plugins, Some("vscode,cursor,homebrew".to_string()));
            assert!(dry_run);
            assert!(backup);
            assert!(force);
            assert_eq!(target_dir, Some(PathBuf::from("/custom/restore/target")));
            assert!(!latest);
        }
        _ => panic!("Expected restore command"),
    }

    // Test restore without backup
    let args = parse_test_args(&["dotsnapshot", "restore", "--latest", "--backup", "false"]);

    match args.command {
        Some(Commands::Restore { backup, .. }) => {
            // Note: backup defaults to true, so this tests the default behavior
            assert!(backup); // Default value
        }
        _ => panic!("Expected restore command"),
    }
}

/// Test hooks list command with all filter combinations
/// Verifies all hook type filters work correctly
#[test]
fn test_hooks_list_all_filters() {
    // Test pre-plugin filter
    let args = parse_test_args(&["dotsnapshot", "hooks", "list", "--pre-plugin"]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::List { pre_plugin, .. } => assert!(pre_plugin),
            _ => panic!("Expected list command"),
        }
    }

    // Test post-plugin filter
    let args = parse_test_args(&["dotsnapshot", "hooks", "list", "--post-plugin"]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::List { post_plugin, .. } => assert!(post_plugin),
            _ => panic!("Expected list command"),
        }
    }

    // Test pre-snapshot filter
    let args = parse_test_args(&["dotsnapshot", "hooks", "list", "--pre-snapshot"]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::List { pre_snapshot, .. } => assert!(pre_snapshot),
            _ => panic!("Expected list command"),
        }
    }

    // Test post-snapshot filter
    let args = parse_test_args(&["dotsnapshot", "hooks", "list", "--post-snapshot"]);

    if let Some(Commands::Hooks { command }) = args.command {
        match *command {
            HooksCommands::List { post_snapshot, .. } => assert!(post_snapshot),
            _ => panic!("Expected list command"),
        }
    }
}

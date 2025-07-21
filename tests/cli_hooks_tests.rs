use anyhow::Result;
use dotsnapshot::cli::hooks::handle_hooks_command;
use dotsnapshot::config::Config;
use dotsnapshot::{HookActionArgs, HookTarget, HooksCommands};
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::fs;

#[tokio::test]
async fn test_hooks_add_script_command() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.toml");

    // Start with empty config
    let initial_config = Config::default();
    initial_config.save_to_file(&config_path).await?;

    // Add a pre-plugin hook
    let add_command = HooksCommands::Add {
        target: HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: Some("homebrew_brewfile".to_string()),
            post_plugin: None,
        },
        action: HookActionArgs {
            script: Some("brew-update.sh".to_string()),
            log: None,
            notify: None,
            backup: false,
            cleanup: false,
            args: Some("--update,--verbose".to_string()),
            timeout: Some(60),
            level: None,
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        },
    };

    // Execute the command (this should not panic or error)
    handle_hooks_command(add_command, Some(config_path.clone())).await?;

    // Load the config and verify the hook was added
    let updated_config = Config::load_from_file(&config_path).await?;
    let hooks = updated_config.get_plugin_pre_hooks("homebrew_brewfile");

    assert_eq!(hooks.len(), 1);
    if let dotsnapshot::core::hooks::HookAction::Script {
        command,
        args,
        timeout,
        ..
    } = &hooks[0]
    {
        assert_eq!(command, "brew-update.sh");
        assert_eq!(args, &vec!["--update".to_string(), "--verbose".to_string()]);
        assert_eq!(*timeout, 60);
    } else {
        panic!("Expected script action");
    }

    Ok(())
}

#[tokio::test]
async fn test_hooks_add_global_hook() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.toml");

    // Add a global post-snapshot notification hook
    let add_command = HooksCommands::Add {
        target: HookTarget {
            pre_snapshot: false,
            post_snapshot: true,
            pre_plugin: None,
            post_plugin: None,
        },
        action: HookActionArgs {
            script: None,
            log: None,
            notify: Some("Snapshot completed successfully!".to_string()),
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: Some("dotsnapshot".to_string()),
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        },
    };

    handle_hooks_command(add_command, Some(config_path.clone())).await?;

    // Verify the hook was added
    let config = Config::load_from_file(&config_path).await?;
    let hooks = config.get_global_post_snapshot_hooks();

    assert_eq!(hooks.len(), 1);
    if let dotsnapshot::core::hooks::HookAction::Notify { message, title } = &hooks[0] {
        assert_eq!(message, "Snapshot completed successfully!");
        assert_eq!(title.as_deref(), Some("dotsnapshot"));
    } else {
        panic!("Expected notify action");
    }

    Ok(())
}

#[tokio::test]
async fn test_hooks_add_log_hook() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.toml");

    // Add a log hook
    let add_command = HooksCommands::Add {
        target: HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        },
        action: HookActionArgs {
            script: None,
            log: Some("Starting snapshot creation: {snapshot_name}".to_string()),
            notify: None,
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            level: Some("debug".to_string()),
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        },
    };

    handle_hooks_command(add_command, Some(config_path.clone())).await?;

    // Verify the hook was added
    let config = Config::load_from_file(&config_path).await?;
    let hooks = config.get_global_pre_snapshot_hooks();

    assert_eq!(hooks.len(), 1);
    if let dotsnapshot::core::hooks::HookAction::Log { message, level } = &hooks[0] {
        assert_eq!(message, "Starting snapshot creation: {snapshot_name}");
        assert_eq!(level, "debug");
    } else {
        panic!("Expected log action");
    }

    Ok(())
}

#[tokio::test]
async fn test_hooks_add_backup_hook() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.toml");

    // Create test directories
    let source_dir = temp_dir.path().join("source");
    let backup_dir = temp_dir.path().join("backup");
    fs::create_dir_all(&source_dir).await?;
    fs::create_dir_all(&backup_dir).await?;

    // Add a backup hook
    let add_command = HooksCommands::Add {
        target: HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: Some("vscode_settings".to_string()),
        },
        action: HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: true,
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: None,
            path: Some(source_dir.clone()),
            destination: Some(backup_dir.clone()),
            patterns: None,
            directories: None,
            temp_files: false,
        },
    };

    handle_hooks_command(add_command, Some(config_path.clone())).await?;

    // Verify the hook was added
    let config = Config::load_from_file(&config_path).await?;
    let hooks = config.get_plugin_post_hooks("vscode_settings");

    assert_eq!(hooks.len(), 1);
    if let dotsnapshot::core::hooks::HookAction::Backup { path, destination } = &hooks[0] {
        assert_eq!(path, &source_dir);
        assert_eq!(destination, &backup_dir);
    } else {
        panic!("Expected backup action");
    }

    Ok(())
}

#[tokio::test]
async fn test_hooks_add_cleanup_hook() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.toml");

    // Add a cleanup hook
    let add_command = HooksCommands::Add {
        target: HookTarget {
            pre_snapshot: false,
            post_snapshot: true,
            pre_plugin: None,
            post_plugin: None,
        },
        action: HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: false,
            cleanup: true,
            args: None,
            timeout: None,
            level: None,
            title: None,
            path: None,
            destination: None,
            patterns: Some("*.tmp,*.log,*.bak".to_string()),
            directories: Some("/tmp,/var/tmp".to_string()),
            temp_files: true,
        },
    };

    handle_hooks_command(add_command, Some(config_path.clone())).await?;

    // Verify the hook was added
    let config = Config::load_from_file(&config_path).await?;
    let hooks = config.get_global_post_snapshot_hooks();

    assert_eq!(hooks.len(), 1);
    if let dotsnapshot::core::hooks::HookAction::Cleanup {
        patterns,
        directories,
        temp_files,
    } = &hooks[0]
    {
        assert_eq!(
            patterns,
            &vec![
                "*.tmp".to_string(),
                "*.log".to_string(),
                "*.bak".to_string()
            ]
        );
        assert_eq!(
            directories,
            &vec![PathBuf::from("/tmp"), PathBuf::from("/var/tmp")]
        );
        assert!(*temp_files);
    } else {
        panic!("Expected cleanup action");
    }

    Ok(())
}

#[tokio::test]
async fn test_hooks_add_multiple_hooks() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.toml");

    // Add first hook
    let add_command1 = HooksCommands::Add {
        target: HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: Some("homebrew_brewfile".to_string()),
            post_plugin: None,
        },
        action: HookActionArgs {
            script: Some("first-script.sh".to_string()),
            log: None,
            notify: None,
            backup: false,
            cleanup: false,
            args: None,
            timeout: Some(30),
            level: None,
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        },
    };

    handle_hooks_command(add_command1, Some(config_path.clone())).await?;

    // Add second hook to same plugin and hook type
    let add_command2 = HooksCommands::Add {
        target: HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: Some("homebrew_brewfile".to_string()),
            post_plugin: None,
        },
        action: HookActionArgs {
            script: None,
            log: Some("Second hook executing".to_string()),
            notify: None,
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            level: Some("info".to_string()),
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        },
    };

    handle_hooks_command(add_command2, Some(config_path.clone())).await?;

    // Verify both hooks were added
    let config = Config::load_from_file(&config_path).await?;
    let hooks = config.get_plugin_pre_hooks("homebrew_brewfile");

    assert_eq!(hooks.len(), 2);

    // First hook should be script
    if let dotsnapshot::core::hooks::HookAction::Script {
        command, timeout, ..
    } = &hooks[0]
    {
        assert_eq!(command, "first-script.sh");
        assert_eq!(*timeout, 30);
    } else {
        panic!("Expected first hook to be script action");
    }

    // Second hook should be log
    if let dotsnapshot::core::hooks::HookAction::Log { message, level } = &hooks[1] {
        assert_eq!(message, "Second hook executing");
        assert_eq!(level, "info");
    } else {
        panic!("Expected second hook to be log action");
    }

    Ok(())
}

#[tokio::test]
async fn test_hooks_remove_by_index() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.toml");

    // First, add multiple hooks
    for i in 0..3 {
        let add_command = HooksCommands::Add {
            target: HookTarget {
                pre_snapshot: true,
                post_snapshot: false,
                pre_plugin: None,
                post_plugin: None,
            },
            action: HookActionArgs {
                script: None,
                log: Some(format!("Hook number {i}")),
                notify: None,
                backup: false,
                cleanup: false,
                args: None,
                timeout: None,
                level: Some("info".to_string()),
                title: None,
                path: None,
                destination: None,
                patterns: None,
                directories: None,
                temp_files: false,
            },
        };
        handle_hooks_command(add_command, Some(config_path.clone())).await?;
    }

    // Verify we have 3 hooks
    let config = Config::load_from_file(&config_path).await?;
    assert_eq!(config.get_global_pre_snapshot_hooks().len(), 3);

    // Remove the middle hook (index 1)
    let remove_command = HooksCommands::Remove {
        target: HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        },
        index: Some(1),
        all: false,
        script: None,
    };

    handle_hooks_command(remove_command, Some(config_path.clone())).await?;

    // Verify we now have 2 hooks, and hook 1 was removed
    let config = Config::load_from_file(&config_path).await?;
    let hooks = config.get_global_pre_snapshot_hooks();
    assert_eq!(hooks.len(), 2);

    if let dotsnapshot::core::hooks::HookAction::Log { message, .. } = &hooks[0] {
        assert_eq!(message, "Hook number 0");
    } else {
        panic!("Expected log action");
    }

    if let dotsnapshot::core::hooks::HookAction::Log { message, .. } = &hooks[1] {
        assert_eq!(message, "Hook number 2");
    } else {
        panic!("Expected log action");
    }

    Ok(())
}

#[tokio::test]
async fn test_hooks_remove_all() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.toml");

    // Add several hooks
    for i in 0..5 {
        let add_command = HooksCommands::Add {
            target: HookTarget {
                pre_snapshot: false,
                post_snapshot: false,
                pre_plugin: Some("vscode_settings".to_string()),
                post_plugin: None,
            },
            action: HookActionArgs {
                script: None,
                log: Some(format!("VSCode hook {i}")),
                notify: None,
                backup: false,
                cleanup: false,
                args: None,
                timeout: None,
                level: Some("debug".to_string()),
                title: None,
                path: None,
                destination: None,
                patterns: None,
                directories: None,
                temp_files: false,
            },
        };
        handle_hooks_command(add_command, Some(config_path.clone())).await?;
    }

    // Verify we have 5 hooks
    let config = Config::load_from_file(&config_path).await?;
    assert_eq!(config.get_plugin_pre_hooks("vscode_settings").len(), 5);

    // Remove all hooks
    let remove_command = HooksCommands::Remove {
        target: HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: Some("vscode_settings".to_string()),
            post_plugin: None,
        },
        index: None,
        all: true,
        script: None,
    };

    handle_hooks_command(remove_command, Some(config_path.clone())).await?;

    // Verify all hooks were removed
    let config = Config::load_from_file(&config_path).await?;
    assert_eq!(config.get_plugin_pre_hooks("vscode_settings").len(), 0);

    Ok(())
}

#[tokio::test]
async fn test_hooks_remove_by_script_name() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.toml");

    // Add hooks with different script names
    let scripts = vec!["script1.sh", "script2.sh", "script1.sh", "other.sh"];

    for script in &scripts {
        let add_command = HooksCommands::Add {
            target: HookTarget {
                pre_snapshot: false,
                post_snapshot: false,
                pre_plugin: None,
                post_plugin: Some("homebrew_brewfile".to_string()),
            },
            action: HookActionArgs {
                script: Some(script.to_string()),
                log: None,
                notify: None,
                backup: false,
                cleanup: false,
                args: None,
                timeout: Some(30),
                level: None,
                title: None,
                path: None,
                destination: None,
                patterns: None,
                directories: None,
                temp_files: false,
            },
        };
        handle_hooks_command(add_command, Some(config_path.clone())).await?;
    }

    // Verify we have 4 hooks
    let config = Config::load_from_file(&config_path).await?;
    assert_eq!(config.get_plugin_post_hooks("homebrew_brewfile").len(), 4);

    // Remove hooks with "script1.sh" in the name
    let remove_command = HooksCommands::Remove {
        target: HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: Some("homebrew_brewfile".to_string()),
        },
        index: None,
        all: false,
        script: Some("script1.sh".to_string()),
    };

    handle_hooks_command(remove_command, Some(config_path.clone())).await?;

    // Should have removed 2 hooks (both script1.sh instances), leaving 2
    let config = Config::load_from_file(&config_path).await?;
    let remaining_hooks = config.get_plugin_post_hooks("homebrew_brewfile");
    assert_eq!(remaining_hooks.len(), 2);

    // Check that remaining hooks are script2.sh and other.sh
    for hook in &remaining_hooks {
        if let dotsnapshot::core::hooks::HookAction::Script { command, .. } = hook {
            assert!(command == "script2.sh" || command == "other.sh");
            assert_ne!(command, "script1.sh");
        } else {
            panic!("Expected script action");
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_scripts_dir_management() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.toml");
    let custom_scripts_dir = temp_dir.path().join("custom-scripts");

    // Test setting scripts directory
    let scripts_dir_command = HooksCommands::ScriptsDir {
        set: Some(custom_scripts_dir.clone()),
        create: true,
    };

    handle_hooks_command(scripts_dir_command, Some(config_path.clone())).await?;

    // Verify the scripts directory was set and created
    let config = Config::load_from_file(&config_path).await?;
    let hooks_config = config.get_hooks_config();
    assert_eq!(hooks_config.scripts_dir, custom_scripts_dir);
    assert!(custom_scripts_dir.exists());

    Ok(())
}

// Mock test for commands that interact with the filesystem
#[tokio::test]
async fn test_hooks_validation_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.toml");

    // Create scripts directory and a test script
    let scripts_dir = temp_dir.path().join("scripts");
    fs::create_dir_all(&scripts_dir).await?;

    let test_script = scripts_dir.join("test-script.sh");
    fs::write(&test_script, "#!/bin/bash\necho 'Hello from test script'\n").await?;

    // Set executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&test_script).await?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&test_script, perms).await?;
    }

    // Set up config with custom scripts directory
    let setup_command = HooksCommands::ScriptsDir {
        set: Some(scripts_dir.clone()),
        create: false,
    };
    handle_hooks_command(setup_command, Some(config_path.clone())).await?;

    // Add a hook that uses the test script
    let add_command = HooksCommands::Add {
        target: HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        },
        action: HookActionArgs {
            script: Some("test-script.sh".to_string()),
            log: None,
            notify: None,
            backup: false,
            cleanup: false,
            args: Some("arg1,arg2".to_string()),
            timeout: Some(10),
            level: None,
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        },
    };

    // This should succeed without error since the script exists
    handle_hooks_command(add_command, Some(config_path.clone())).await?;

    // Verify the hook was added correctly
    let config = Config::load_from_file(&config_path).await?;
    let hooks = config.get_global_pre_snapshot_hooks();
    assert_eq!(hooks.len(), 1);

    if let dotsnapshot::core::hooks::HookAction::Script {
        command,
        args,
        timeout,
        ..
    } = &hooks[0]
    {
        assert_eq!(command, "test-script.sh");
        assert_eq!(args, &vec!["arg1".to_string(), "arg2".to_string()]);
        assert_eq!(*timeout, 10);
    } else {
        panic!("Expected script action");
    }

    // Test validation command (this is more of a smoke test since we can't easily
    // capture the validation output in a unit test)
    let validate_command = HooksCommands::Validate {
        plugin: None,
        pre_plugin: false,
        post_plugin: false,
        pre_snapshot: true,
        post_snapshot: false,
    };

    // This should complete without error
    handle_hooks_command(validate_command, Some(config_path.clone())).await?;

    Ok(())
}

use anyhow::Result;
use dotsnapshot::config::{
    Config, GlobalConfig, GlobalHooks, PluginConfig, PluginHooks, PluginsConfig,
};
use dotsnapshot::core::hooks::{HookAction, HooksConfig};
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::fs;

#[tokio::test]
async fn test_config_with_hooks_serialization() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.toml");

    // Create a comprehensive config with hooks
    let config = Config {
        output_dir: Some(PathBuf::from("./snapshots")),
        include_plugins: Some(vec!["homebrew".to_string(), "vscode".to_string()]),
        logging: None,
        hooks: Some(HooksConfig {
            scripts_dir: PathBuf::from("~/.config/dotsnapshot/scripts"),
        }),
        global: Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![
                    HookAction::Script {
                        command: "pre-snapshot-setup.sh".to_string(),
                        args: vec![],
                        timeout: 30,
                        working_dir: None,
                        env_vars: HashMap::new(),
                    },
                    HookAction::Log {
                        message: "Starting snapshot creation for {snapshot_name}".to_string(),
                        level: "info".to_string(),
                    },
                ],
                post_snapshot: vec![
                    HookAction::Notify {
                        message: "Snapshot {snapshot_name} completed successfully".to_string(),
                        title: Some("dotsnapshot".to_string()),
                    },
                    HookAction::Cleanup {
                        patterns: vec!["*.tmp".to_string(), "*.log".to_string()],
                        directories: vec![PathBuf::from("/tmp/dotsnapshot")],
                        temp_files: true,
                    },
                ],
            }),
        }),
        static_files: None,
        plugins: Some(PluginsConfig {
            plugins: {
                let mut map = std::collections::HashMap::new();
                map.insert(
                    "homebrew_brewfile".to_string(),
                    toml::Value::try_from(PluginConfig {
                        target_path: Some("homebrew".to_string()),
                        output_file: None,
                        hooks: Some(PluginHooks {
                            pre_plugin: vec![
                                HookAction::Script {
                                    command: "homebrew/pre-backup.sh".to_string(),
                                    args: vec!["--update".to_string()],
                                    timeout: 60,
                                    working_dir: None,
                                    env_vars: HashMap::from([(
                                        "HOMEBREW_NO_AUTO_UPDATE".to_string(),
                                        "1".to_string(),
                                    )]),
                                },
                                HookAction::Log {
                                    message: "Starting Homebrew backup for {plugin_name}"
                                        .to_string(),
                                    level: "info".to_string(),
                                },
                            ],
                            post_plugin: vec![
                                HookAction::Script {
                                    command: "homebrew/validate-brewfile.sh".to_string(),
                                    args: vec![],
                                    timeout: 30,
                                    working_dir: None,
                                    env_vars: HashMap::new(),
                                },
                                HookAction::Backup {
                                    path: PathBuf::from("~/.homebrew/backup"),
                                    destination: PathBuf::from("/tmp/homebrew-backup"),
                                },
                            ],
                        }),
                    })
                    .unwrap(),
                );
                map.insert(
                    "vscode_settings".to_string(),
                    toml::Value::try_from(PluginConfig {
                        target_path: Some("vscode".to_string()),
                        output_file: None,
                        hooks: Some(PluginHooks {
                            pre_plugin: vec![HookAction::Script {
                                command: "vscode/backup-extensions.sh".to_string(),
                                args: vec![],
                                timeout: 30,
                                working_dir: None,
                                env_vars: HashMap::new(),
                            }],
                            post_plugin: vec![HookAction::Log {
                                message: "VSCode settings backed up: {file_count} files"
                                    .to_string(),
                                level: "info".to_string(),
                            }],
                        }),
                    })
                    .unwrap(),
                );
                map
            },
        }),
        ui: None,
    };

    // Save config
    config.save_to_file(&config_path).await?;

    // Read the TOML content and verify structure
    let toml_content = fs::read_to_string(&config_path).await?;
    println!("Generated TOML:\n{toml_content}");

    // Verify it contains expected sections
    assert!(toml_content.contains("[hooks]"));
    assert!(toml_content.contains("[[global.hooks.pre-snapshot]]"));
    assert!(toml_content.contains("[[global.hooks.post-snapshot]]"));
    assert!(toml_content.contains("[plugins.homebrew_brewfile]"));
    assert!(toml_content.contains("[[plugins.homebrew_brewfile.hooks.pre-plugin]]"));
    assert!(toml_content.contains("[[plugins.homebrew_brewfile.hooks.post-plugin]]"));

    // Load config and verify it matches
    let loaded_config = Config::load_from_file(&config_path).await?;

    // Verify hooks configuration
    assert!(loaded_config.hooks.is_some());
    let hooks_config = loaded_config.hooks.unwrap();
    assert_eq!(
        hooks_config.scripts_dir,
        PathBuf::from("~/.config/dotsnapshot/scripts")
    );

    // Verify global hooks
    assert!(loaded_config.global.is_some());
    let global = loaded_config.global.unwrap();
    assert!(global.hooks.is_some());
    let global_hooks = global.hooks.unwrap();

    assert_eq!(global_hooks.pre_snapshot.len(), 2);
    assert_eq!(global_hooks.post_snapshot.len(), 2);

    // Verify first pre-snapshot hook (script)
    if let HookAction::Script {
        command,
        args,
        timeout,
        ..
    } = &global_hooks.pre_snapshot[0]
    {
        assert_eq!(command, "pre-snapshot-setup.sh");
        assert!(args.is_empty());
        assert_eq!(*timeout, 30);
    } else {
        panic!("Expected first pre-snapshot hook to be a Script");
    }

    // Verify second pre-snapshot hook (log)
    if let HookAction::Log { message, level } = &global_hooks.pre_snapshot[1] {
        assert_eq!(message, "Starting snapshot creation for {snapshot_name}");
        assert_eq!(level, "info");
    } else {
        panic!("Expected second pre-snapshot hook to be a Log");
    }

    // Verify plugin hooks
    assert!(loaded_config.plugins.is_some());
    let plugins = loaded_config.plugins.unwrap();

    assert!(plugins.plugins.contains_key("homebrew_brewfile"));
    let homebrew_value = &plugins.plugins["homebrew_brewfile"];
    let homebrew_config: PluginConfig = homebrew_value.clone().try_into().unwrap();
    assert!(homebrew_config.hooks.is_some());
    let homebrew_hooks = homebrew_config.hooks.unwrap();

    assert_eq!(homebrew_hooks.pre_plugin.len(), 2);
    assert_eq!(homebrew_hooks.post_plugin.len(), 2);

    // Verify homebrew pre-plugin script hook
    if let HookAction::Script {
        command,
        args,
        timeout,
        env_vars,
        ..
    } = &homebrew_hooks.pre_plugin[0]
    {
        assert_eq!(command, "homebrew/pre-backup.sh");
        assert_eq!(args, &vec!["--update".to_string()]);
        assert_eq!(*timeout, 60);
        assert_eq!(
            env_vars.get("HOMEBREW_NO_AUTO_UPDATE"),
            Some(&"1".to_string())
        );
    } else {
        panic!("Expected first homebrew pre-plugin hook to be a Script");
    }

    Ok(())
}

#[tokio::test]
async fn test_config_hooks_helper_methods() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.toml");

    // Create config with various hooks
    let config = Config {
        output_dir: None,
        include_plugins: None,
        logging: None,
        hooks: Some(HooksConfig {
            scripts_dir: PathBuf::from("/custom/scripts"),
        }),
        global: Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![HookAction::Log {
                    message: "Pre-snapshot global hook".to_string(),
                    level: "info".to_string(),
                }],
                post_snapshot: vec![HookAction::Log {
                    message: "Post-snapshot global hook".to_string(),
                    level: "info".to_string(),
                }],
            }),
        }),
        static_files: None,
        plugins: Some(PluginsConfig {
            plugins: {
                let mut map = std::collections::HashMap::new();
                map.insert(
                    "homebrew_brewfile".to_string(),
                    toml::Value::try_from(PluginConfig {
                        target_path: None,
                        output_file: None,
                        hooks: Some(PluginHooks {
                            pre_plugin: vec![HookAction::Script {
                                command: "homebrew-pre.sh".to_string(),
                                args: vec![],
                                timeout: 30,
                                working_dir: None,
                                env_vars: HashMap::new(),
                            }],
                            post_plugin: vec![HookAction::Script {
                                command: "homebrew-post.sh".to_string(),
                                args: vec![],
                                timeout: 30,
                                working_dir: None,
                                env_vars: HashMap::new(),
                            }],
                        }),
                    })
                    .unwrap(),
                );
                map.insert(
                    "vscode_settings".to_string(),
                    toml::Value::try_from(PluginConfig {
                        target_path: None,
                        output_file: None,
                        hooks: Some(PluginHooks {
                            pre_plugin: vec![HookAction::Log {
                                message: "VSCode pre-plugin".to_string(),
                                level: "debug".to_string(),
                            }],
                            post_plugin: vec![],
                        }),
                    })
                    .unwrap(),
                );
                map
            },
        }),
        ui: None,
    };

    // Save and reload config
    config.save_to_file(&config_path).await?;
    let loaded_config = Config::load_from_file(&config_path).await?;

    // Test hooks config helper
    let hooks_config = loaded_config.get_hooks_config();
    assert_eq!(hooks_config.scripts_dir, PathBuf::from("/custom/scripts"));

    // Test global hooks helpers
    let pre_snapshot_hooks = loaded_config.get_global_pre_snapshot_hooks();
    assert_eq!(pre_snapshot_hooks.len(), 1);
    if let HookAction::Log { message, .. } = &pre_snapshot_hooks[0] {
        assert_eq!(message, "Pre-snapshot global hook");
    } else {
        panic!("Expected log action");
    }

    let post_snapshot_hooks = loaded_config.get_global_post_snapshot_hooks();
    assert_eq!(post_snapshot_hooks.len(), 1);

    // Test plugin hooks helpers
    let homebrew_pre_hooks = loaded_config.get_plugin_pre_hooks("homebrew_brewfile");
    assert_eq!(homebrew_pre_hooks.len(), 1);
    if let HookAction::Script { command, .. } = &homebrew_pre_hooks[0] {
        assert_eq!(command, "homebrew-pre.sh");
    } else {
        panic!("Expected script action");
    }

    let homebrew_post_hooks = loaded_config.get_plugin_post_hooks("homebrew_brewfile");
    assert_eq!(homebrew_post_hooks.len(), 1);

    let vscode_pre_hooks = loaded_config.get_plugin_pre_hooks("vscode_settings");
    assert_eq!(vscode_pre_hooks.len(), 1);

    let vscode_post_hooks = loaded_config.get_plugin_post_hooks("vscode_settings");
    assert_eq!(vscode_post_hooks.len(), 0);

    // Test non-existent plugin hooks
    let nonexistent_hooks = loaded_config.get_plugin_pre_hooks("nonexistent_plugin");
    assert_eq!(nonexistent_hooks.len(), 0);

    Ok(())
}

#[tokio::test]
async fn test_minimal_config_with_hooks() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("minimal.toml");

    // Create minimal config with just hooks config
    let minimal_toml = r#"
[hooks]
scripts_dir = "~/scripts"

[global.hooks]
[[global.hooks.pre-snapshot]]
action = "log"
message = "Starting snapshot"
level = "info"
"#;

    fs::write(&config_path, minimal_toml).await?;

    // Load and verify
    let config = Config::load_from_file(&config_path).await?;

    // Should use defaults for other fields
    assert_eq!(config.get_output_dir(), PathBuf::from("./snapshots"));
    assert!(config.get_include_plugins().is_none());

    // Should have hooks configuration
    let hooks_config = config.get_hooks_config();
    assert_eq!(hooks_config.scripts_dir, PathBuf::from("~/scripts"));

    // Should have global pre-snapshot hook
    let pre_hooks = config.get_global_pre_snapshot_hooks();
    assert_eq!(pre_hooks.len(), 1);
    if let HookAction::Log { message, level } = &pre_hooks[0] {
        assert_eq!(message, "Starting snapshot");
        assert_eq!(level, "info");
    } else {
        panic!("Expected log action");
    }

    Ok(())
}

#[tokio::test]
async fn test_config_without_hooks() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("no-hooks.toml");

    // Create config without any hooks
    let no_hooks_toml = r#"
output_dir = "./custom-snapshots"
include_plugins = ["homebrew", "vscode"]

[plugins.homebrew_brewfile]
target_path = "homebrew"
"#;

    fs::write(&config_path, no_hooks_toml).await?;

    // Load and verify
    let config = Config::load_from_file(&config_path).await?;

    // Should use default hooks config
    let hooks_config = config.get_hooks_config();
    assert!(hooks_config
        .scripts_dir
        .to_string_lossy()
        .contains("dotsnapshot"));
    assert!(hooks_config
        .scripts_dir
        .to_string_lossy()
        .contains("scripts"));

    // Should have no global hooks
    assert_eq!(config.get_global_pre_snapshot_hooks().len(), 0);
    assert_eq!(config.get_global_post_snapshot_hooks().len(), 0);

    // Should have no plugin hooks
    assert_eq!(config.get_plugin_pre_hooks("homebrew_brewfile").len(), 0);
    assert_eq!(config.get_plugin_post_hooks("homebrew_brewfile").len(), 0);

    Ok(())
}

#[tokio::test]
async fn test_config_partial_hooks() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("partial-hooks.toml");

    // Create config with only some hooks defined
    let partial_toml = r#"
[hooks]
scripts_dir = "/opt/scripts"

[global.hooks]
[[global.hooks.post-snapshot]]
action = "notify"
message = "Snapshot completed"
title = "dotsnapshot"

[plugins.vscode_settings]
[[plugins.vscode_settings.hooks.pre-plugin]]
action = "script"
command = "vscode-prep.sh"
timeout = 45
"#;

    fs::write(&config_path, partial_toml).await?;

    // Load and verify
    let config = Config::load_from_file(&config_path).await?;

    // Should have custom scripts directory
    let hooks_config = config.get_hooks_config();
    assert_eq!(hooks_config.scripts_dir, PathBuf::from("/opt/scripts"));

    // Should have no pre-snapshot hooks, but one post-snapshot hook
    assert_eq!(config.get_global_pre_snapshot_hooks().len(), 0);
    let post_hooks = config.get_global_post_snapshot_hooks();
    assert_eq!(post_hooks.len(), 1);
    if let HookAction::Notify { message, title } = &post_hooks[0] {
        assert_eq!(message, "Snapshot completed");
        assert_eq!(title.as_deref(), Some("dotsnapshot"));
    } else {
        panic!("Expected notify action");
    }

    // Should have vscode pre-plugin hook but no post-plugin hooks
    let vscode_pre_hooks = config.get_plugin_pre_hooks("vscode_settings");
    assert_eq!(vscode_pre_hooks.len(), 1);
    if let HookAction::Script {
        command, timeout, ..
    } = &vscode_pre_hooks[0]
    {
        assert_eq!(command, "vscode-prep.sh");
        assert_eq!(*timeout, 45);
    } else {
        panic!("Expected script action");
    }

    let vscode_post_hooks = config.get_plugin_post_hooks("vscode_settings");
    assert_eq!(vscode_post_hooks.len(), 0);

    // Should have no hooks for other plugins
    assert_eq!(config.get_plugin_pre_hooks("homebrew_brewfile").len(), 0);
    assert_eq!(config.get_plugin_post_hooks("homebrew_brewfile").len(), 0);

    Ok(())
}

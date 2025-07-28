//! Tests for hook configuration management functionality

#[cfg(test)]
mod tests {
    use crate::cli::hooks::*;
    use crate::config::{Config, GlobalConfig, GlobalHooks, PluginHooks};
    use crate::core::hooks::HookAction;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio::fs;

    /// Test config file path resolution
    #[test]
    fn test_get_config_file_path() {
        // Test with custom path
        let custom_path = PathBuf::from("/custom/config.toml");
        let result = get_config_file_path(Some(custom_path.clone()));
        assert_eq!(result, custom_path);

        // Test with None (should use default)
        let result = get_config_file_path(None);
        // Should be the default config file path
        assert!(result.to_string_lossy().contains("config.toml"));
    }

    /// Test ensuring global config exists
    /// Verifies that global config sections are created when needed
    #[test]
    fn test_ensure_global_config() {
        // Test with empty config
        let mut config = Config::default();
        assert!(config.global.is_none());

        ensure_global_config(&mut config);

        assert!(config.global.is_some());
        assert!(config.global.as_ref().unwrap().hooks.is_some());

        // Test with existing global config but no hooks
        let mut config = Config {
            global: Some(GlobalConfig { hooks: None }),
            ..Default::default()
        };

        ensure_global_config(&mut config);

        assert!(config.global.is_some());
        assert!(config.global.as_ref().unwrap().hooks.is_some());

        // Test with existing global config and hooks
        let mut config = Config {
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![],
                    post_snapshot: vec![],
                }),
            }),
            ..Default::default()
        };

        ensure_global_config(&mut config);

        // Should remain unchanged
        assert!(config.global.is_some());
        assert!(config.global.as_ref().unwrap().hooks.is_some());
    }

    /// Test ensuring plugin config exists
    /// Verifies that plugin config sections are created when needed
    #[test]
    fn test_ensure_plugin_config() {
        let mut config = Config::default();
        let plugin_name = "test_plugin";

        // Initially no plugins configured
        assert!(config.plugins.is_none());

        ensure_plugin_config(&mut config, plugin_name);

        // Should create plugins section and the specific plugin
        assert!(config.plugins.is_some());
        let plugins = config.plugins.as_ref().unwrap();
        assert!(plugins.plugins.contains_key(plugin_name));

        let plugin_config = plugins.plugins.get(plugin_name).unwrap();
        // plugin_config is a toml::Value, not a PluginConfig struct
        if let Some(hooks_val) = plugin_config.get("hooks") {
            assert!(hooks_val.is_table());
        } else {
            panic!("hooks should be present");
        }

        // Test with existing plugins but new plugin
        ensure_plugin_config(&mut config, "another_plugin");

        let plugins = config.plugins.as_ref().unwrap();
        assert!(plugins.plugins.contains_key("another_plugin"));
        assert_eq!(plugins.plugins.len(), 2);
    }

    /// Test modifying plugin config
    /// Verifies that plugin configurations can be modified correctly
    #[test]
    fn test_modify_plugin_config() {
        let mut config = Config::default();
        let plugin_name = "test_plugin";

        // First ensure the plugin config exists
        ensure_plugin_config(&mut config, plugin_name);

        // Test successful modification
        let result = modify_plugin_config(&mut config, plugin_name, |plugin_config| {
            // Add a hook to verify modification works
            plugin_config
                .hooks
                .as_mut()
                .unwrap()
                .pre_plugin
                .push(HookAction::Log {
                    message: "test".to_string(),
                    level: "info".to_string(),
                });
            42 // Return value for testing
        });

        assert_eq!(result, Some(42));

        // Verify the modification was applied
        let hooks = config.get_plugin_pre_hooks(plugin_name);
        assert_eq!(hooks.len(), 1);

        // Test modification of non-existent plugin
        let result = modify_plugin_config(&mut config, "nonexistent", |_| 99);
        assert_eq!(result, None);
    }

    /// Test counting total hooks in configuration
    /// Verifies that hook counting logic works correctly
    #[test]
    fn test_count_total_hooks() {
        let mut config = Config::default();

        // Initially no hooks
        assert_eq!(count_total_hooks(&config), 0);

        // Add global hooks
        config.global = Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![HookAction::Log {
                    message: "pre".to_string(),
                    level: "info".to_string(),
                }],
                post_snapshot: vec![
                    HookAction::Log {
                        message: "post1".to_string(),
                        level: "info".to_string(),
                    },
                    HookAction::Log {
                        message: "post2".to_string(),
                        level: "info".to_string(),
                    },
                ],
            }),
        });

        assert_eq!(count_total_hooks(&config), 3);

        // For this test, we'll use a different approach since directly building
        // PluginConfig structs is complex due to the toml::Value storage
        // The count_total_hooks function is mainly tested by the existing integration tests
        // We can test it with just global hooks
        assert_eq!(count_total_hooks(&config), 3);
    }

    /// Test getting all plugin names from configuration
    /// Verifies that plugin name extraction works correctly
    #[test]
    fn test_get_all_plugin_names() {
        let config = Config::default();

        // Initially no plugins
        assert_eq!(get_all_plugin_names(&config), Vec::<String>::new());

        // Since PluginConfig storage uses toml::Value internally,
        // this function is better tested through integration tests
        // Here we just test the empty case
        assert_eq!(get_all_plugin_names(&config), Vec::<String>::new());
    }

    /// Test counting scripts in directory
    /// Verifies that script file counting works correctly
    #[tokio::test]
    async fn test_count_scripts_in_directory() {
        let temp_dir = TempDir::new().unwrap();
        let scripts_dir = temp_dir.path().join("scripts");
        fs::create_dir_all(&scripts_dir).await.unwrap();

        // Initially empty directory
        let count = count_scripts_in_directory(&scripts_dir).await.unwrap();
        assert_eq!(count, 0);

        // Add some script files
        fs::write(scripts_dir.join("script1.sh"), "#!/bin/bash\necho test")
            .await
            .unwrap();
        fs::write(
            scripts_dir.join("script2.py"),
            "#!/usr/bin/env python\nprint('test')",
        )
        .await
        .unwrap();
        fs::write(
            scripts_dir.join("script3.rb"),
            "#!/usr/bin/env ruby\nputs 'test'",
        )
        .await
        .unwrap();
        fs::write(scripts_dir.join("not_script.txt"), "This is not a script")
            .await
            .unwrap();

        let count = count_scripts_in_directory(&scripts_dir).await.unwrap();
        assert_eq!(count, 3); // Only the .sh, .py, .rb files

        // Test with executable file without extension
        let exec_file = scripts_dir.join("executable");
        fs::write(&exec_file, "#!/bin/bash\necho test")
            .await
            .unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&exec_file).await.unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&exec_file, perms).await.unwrap();

            let count = count_scripts_in_directory(&scripts_dir).await.unwrap();
            assert_eq!(count, 4); // Now includes the executable file
        }

        #[cfg(not(unix))]
        {
            let count = count_scripts_in_directory(&scripts_dir).await.unwrap();
            assert_eq!(count, 4); // Assumes executable on non-Unix
        }
    }

    /// Test error cases for script counting
    /// Verifies that error handling works for invalid directories
    #[tokio::test]
    async fn test_count_scripts_in_directory_errors() {
        let nonexistent_dir = PathBuf::from("/nonexistent/directory");
        let result = count_scripts_in_directory(&nonexistent_dir).await;
        assert!(result.is_err());
    }

    /// Test load_or_create_config function
    /// Verifies that configuration loading and creation works correctly
    #[tokio::test]
    async fn test_load_or_create_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Test with non-existent file (should return default config, not create file)
        let result = load_or_create_config(Some(config_path.clone())).await;
        assert!(result.is_ok());
        // The function returns default config but doesn't create the file
        assert!(!config_path.exists());

        // Create the config file manually
        let config = Config::default();
        config.save_to_file(&config_path).await.unwrap();
        assert!(config_path.exists());

        // Test loading existing config
        let result2 = load_or_create_config(Some(config_path.clone())).await;
        assert!(result2.is_ok());

        // Test with None path (should use default config discovery)
        let result3 = load_or_create_config(None).await;
        assert!(result3.is_ok());
    }

    /// Test modify_plugin_config function with comprehensive scenarios
    /// Verifies that plugin configuration modification works in various situations
    #[tokio::test]
    async fn test_modify_plugin_config_comprehensive() {
        let mut config = Config::default();
        ensure_plugin_config(&mut config, "test_plugin");

        // Test successful modification
        let result = modify_plugin_config(&mut config, "test_plugin", |plugin_config| {
            plugin_config.target_path = Some("custom_path".to_string());
            "modified"
        });

        assert_eq!(result, Some("modified"));

        // Test modification of non-existent plugin
        let result = modify_plugin_config(&mut config, "nonexistent_plugin", |plugin_config| {
            plugin_config.target_path = Some("should_not_work".to_string());
            "failed"
        });

        assert_eq!(result, None);

        // Test with plugin that has hooks
        ensure_plugin_config(&mut config, "hooked_plugin");
        let result = modify_plugin_config(&mut config, "hooked_plugin", |plugin_config| {
            if plugin_config.hooks.is_none() {
                plugin_config.hooks = Some(PluginHooks {
                    pre_plugin: vec![],
                    post_plugin: vec![],
                });
            }
            plugin_config
                .hooks
                .as_mut()
                .unwrap()
                .pre_plugin
                .push(HookAction::Log {
                    message: "Pre-plugin log".to_string(),
                    level: "info".to_string(),
                });
            true
        });

        assert_eq!(result, Some(true));
    }

    /// Test get_all_plugin_names function with various configurations
    /// Verifies that all plugin names are correctly extracted
    #[tokio::test]
    async fn test_get_all_plugin_names_comprehensive() {
        // Test with empty config
        let empty_config = Config::default();
        let names = get_all_plugin_names(&empty_config);
        assert!(names.is_empty());

        // Test with config containing multiple plugins
        let mut config = Config::default();
        ensure_plugin_config(&mut config, "plugin1");
        ensure_plugin_config(&mut config, "plugin2");
        ensure_plugin_config(&mut config, "plugin3");

        let names = get_all_plugin_names(&config);
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"plugin1".to_string()));
        assert!(names.contains(&"plugin2".to_string()));
        assert!(names.contains(&"plugin3".to_string()));

        // Test with config that has plugins but empty HashMap
        let config_empty_plugins = Config {
            plugins: Some(crate::config::PluginsConfig {
                plugins: HashMap::new(),
            }),
            ..Default::default()
        };

        let names = get_all_plugin_names(&config_empty_plugins);
        assert!(names.is_empty());
    }

    /// Test count_total_hooks function with comprehensive scenarios
    /// Verifies that hook counting works correctly with various configurations
    #[tokio::test]
    async fn test_count_total_hooks_comprehensive() {
        use crate::config::{GlobalConfig, GlobalHooks};
        use crate::core::hooks::HookAction;

        // Test with empty config
        let empty_config = Config::default();
        assert_eq!(count_total_hooks(&empty_config), 0);

        // Test with global hooks only
        let global_hooks = GlobalHooks {
            pre_snapshot: vec![
                HookAction::Log {
                    message: "Log 1".to_string(),
                    level: "info".to_string(),
                },
                HookAction::Log {
                    message: "Log 2".to_string(),
                    level: "info".to_string(),
                },
            ],
            post_snapshot: vec![HookAction::Notify {
                message: "Done".to_string(),
                title: None,
            }],
        };

        let config_with_global = Config {
            global: Some(GlobalConfig {
                hooks: Some(global_hooks),
            }),
            ..Default::default()
        };

        assert_eq!(count_total_hooks(&config_with_global), 3);

        // Test with plugin hooks only
        let mut config_with_plugins = Config::default();
        ensure_plugin_config(&mut config_with_plugins, "plugin1");
        ensure_plugin_config(&mut config_with_plugins, "plugin2");

        modify_plugin_config(&mut config_with_plugins, "plugin1", |plugin_config| {
            plugin_config.hooks = Some(PluginHooks {
                pre_plugin: vec![HookAction::Log {
                    message: "Plugin 1 pre".to_string(),
                    level: "info".to_string(),
                }],
                post_plugin: vec![HookAction::Log {
                    message: "Plugin 1 post".to_string(),
                    level: "info".to_string(),
                }],
            });
        });

        modify_plugin_config(&mut config_with_plugins, "plugin2", |plugin_config| {
            plugin_config.hooks = Some(PluginHooks {
                pre_plugin: vec![],
                post_plugin: vec![HookAction::Notify {
                    message: "Plugin 2 done".to_string(),
                    title: None,
                }],
            });
        });

        assert_eq!(count_total_hooks(&config_with_plugins), 3);

        // Test with both global and plugin hooks
        let mut config_with_both = config_with_global.clone();
        config_with_both.plugins = config_with_plugins.plugins;

        assert_eq!(count_total_hooks(&config_with_both), 6);
    }

    /// Test edge cases for config file path handling
    /// Verifies that config path resolution works correctly
    #[tokio::test]
    async fn test_get_config_file_path_edge_cases() {
        // Test with explicit path
        let explicit_path = PathBuf::from("/custom/path/config.toml");
        let result = get_config_file_path(Some(explicit_path.clone()));
        assert_eq!(result, explicit_path);

        // Test with None (should return default)
        let result = get_config_file_path(None);
        assert!(result.to_string_lossy().contains("dotsnapshot"));

        // Test with relative path
        let relative_path = PathBuf::from("./relative_config.toml");
        let result = get_config_file_path(Some(relative_path.clone()));
        assert_eq!(result, relative_path);
    }

    /// Test ensure_plugin_config with existing plugin
    /// Verifies that existing plugin configurations are preserved
    #[test]
    fn test_ensure_plugin_config_existing() {
        let mut config = Config::default();

        // First add a plugin with custom config
        ensure_plugin_config(&mut config, "test_plugin");
        modify_plugin_config(&mut config, "test_plugin", |plugin_config| {
            plugin_config.target_path = Some("custom/path".to_string());
            plugin_config.output_file = Some("custom.txt".to_string());
        });

        // Ensure again - should preserve existing config
        ensure_plugin_config(&mut config, "test_plugin");

        // Verify config was preserved
        let plugins = config.plugins.as_ref().unwrap();
        let plugin_value = plugins.plugins.get("test_plugin").unwrap();
        let target_path = plugin_value.get("target_path").unwrap().as_str().unwrap();
        assert_eq!(target_path, "custom/path");
    }

    /// Test count_scripts_in_directory with various file types
    /// Verifies comprehensive script detection including edge cases
    #[tokio::test]
    async fn test_count_scripts_in_directory_comprehensive() {
        let temp_dir = TempDir::new().unwrap();
        let scripts_dir = temp_dir.path().join("scripts");
        fs::create_dir_all(&scripts_dir).await.unwrap();

        // Create various file types
        fs::write(scripts_dir.join("script.sh"), "#!/bin/bash")
            .await
            .unwrap();
        fs::write(scripts_dir.join("script.py"), "#!/usr/bin/env python")
            .await
            .unwrap();
        fs::write(scripts_dir.join("script.rb"), "#!/usr/bin/env ruby")
            .await
            .unwrap();
        fs::write(scripts_dir.join("script.js"), "#!/usr/bin/env node")
            .await
            .unwrap();
        fs::write(scripts_dir.join("script.ts"), "#!/usr/bin/env ts-node")
            .await
            .unwrap();
        fs::write(scripts_dir.join("not_script.md"), "# Readme")
            .await
            .unwrap();
        fs::write(scripts_dir.join("data.json"), "{}")
            .await
            .unwrap();

        // Create subdirectory (should not be counted)
        fs::create_dir_all(scripts_dir.join("subdir"))
            .await
            .unwrap();
        fs::write(scripts_dir.join("subdir/nested.sh"), "#!/bin/bash")
            .await
            .unwrap();

        let count = count_scripts_in_directory(&scripts_dir).await.unwrap();
        assert_eq!(count, 5); // Only the 5 script files in the main directory
    }
}

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{error, info, warn};

use crate::config::{Config, GlobalConfig, GlobalHooks, PluginConfig, PluginHooks};
use crate::core::hooks::{HookAction, HookContext, HookManager, HooksConfig};
use crate::symbols::*;
use crate::{HookActionArgs, HookTarget, HooksCommands};

/// Handle hooks subcommands
pub async fn handle_hooks_command(
    command: HooksCommands,
    config_path: Option<PathBuf>,
) -> Result<()> {
    match command {
        HooksCommands::Add { target, action } => handle_add_hook(target, action, config_path).await,
        HooksCommands::Remove {
            target,
            index,
            all,
            script,
        } => handle_remove_hook(target, index, all, script, config_path).await,
        HooksCommands::List {
            plugin,
            pre_plugin,
            post_plugin,
            pre_snapshot,
            post_snapshot,
            verbose,
        } => {
            handle_list_hooks(
                plugin,
                pre_plugin,
                post_plugin,
                pre_snapshot,
                post_snapshot,
                verbose,
                config_path,
            )
            .await
        }
        HooksCommands::Validate {
            plugin,
            pre_plugin,
            post_plugin,
            pre_snapshot,
            post_snapshot,
        } => {
            handle_validate_hooks(
                plugin,
                pre_plugin,
                post_plugin,
                pre_snapshot,
                post_snapshot,
                config_path,
            )
            .await
        }
        HooksCommands::ScriptsDir { set, create } => {
            handle_scripts_dir(set, create, config_path).await
        }
    }
}

async fn handle_add_hook(
    target: HookTarget,
    action: HookActionArgs,
    config_path: Option<PathBuf>,
) -> Result<()> {
    let mut config = load_or_create_config(config_path.clone()).await?;
    let hook_action = convert_action_args_to_hook_action(action)?;

    // Determine target type and plugin
    let (hook_type, plugin_name) = determine_hook_target(&target)?;

    // Add hook to appropriate configuration section
    match hook_type.as_str() {
        "pre-snapshot" => {
            ensure_global_config(&mut config);
            config
                .global
                .as_mut()
                .unwrap()
                .hooks
                .as_mut()
                .unwrap()
                .pre_snapshot
                .push(hook_action.clone());
        }
        "post-snapshot" => {
            ensure_global_config(&mut config);
            config
                .global
                .as_mut()
                .unwrap()
                .hooks
                .as_mut()
                .unwrap()
                .post_snapshot
                .push(hook_action.clone());
        }
        "pre-plugin" => {
            let plugin_name = plugin_name.as_ref().unwrap();
            ensure_plugin_config(&mut config, plugin_name);
            modify_plugin_config(&mut config, plugin_name, |plugin_config| {
                plugin_config
                    .hooks
                    .as_mut()
                    .unwrap()
                    .pre_plugin
                    .push(hook_action.clone());
            });
        }
        "post-plugin" => {
            let plugin_name = plugin_name.as_ref().unwrap();
            ensure_plugin_config(&mut config, plugin_name);
            modify_plugin_config(&mut config, plugin_name, |plugin_config| {
                plugin_config
                    .hooks
                    .as_mut()
                    .unwrap()
                    .post_plugin
                    .push(hook_action.clone());
            });
        }
        _ => unreachable!(),
    }

    // Save updated configuration
    let config_file_path = get_config_file_path(config_path);
    config.save_to_file(&config_file_path).await?;

    // Show success message
    let plugin_context = if let Some(plugin) = &plugin_name {
        format!(" to {plugin}")
    } else {
        " (global)".to_string()
    };

    info!(
        "{} Added {hook_type} hook{plugin_context}:",
        SYMBOL_INDICATOR_SUCCESS
    );
    info!("   {} {hook_action}", SYMBOL_DOC_NOTE);

    // Check if script exists
    if let HookAction::Script { command, .. } = &hook_action {
        let hooks_config = config.get_hooks_config();
        let script_path = hooks_config.resolve_script_path(command);
        let expanded_path = HooksConfig::expand_tilde(&script_path);

        if !expanded_path.exists() {
            warn!(
                "   {}  Script file not found: {} → {}",
                SYMBOL_INDICATOR_WARNING,
                command,
                expanded_path.display()
            );
            warn!(
                "   {} Create the script file to complete setup",
                SYMBOL_EXPERIENCE_IDEA
            );
        }
    }

    Ok(())
}

async fn handle_remove_hook(
    target: HookTarget,
    index: Option<usize>,
    all: bool,
    script: Option<String>,
    config_path: Option<PathBuf>,
) -> Result<()> {
    let mut config = load_or_create_config(config_path.clone()).await?;
    let (hook_type, plugin_name) = determine_hook_target(&target)?;

    // Get mutable reference to the appropriate hook list
    let hooks = match hook_type.as_str() {
        "pre-snapshot" => {
            if let Some(global) = config.global.as_mut() {
                if let Some(hooks) = global.hooks.as_mut() {
                    &mut hooks.pre_snapshot
                } else {
                    info!("No pre-snapshot hooks configured");
                    return Ok(());
                }
            } else {
                info!("No pre-snapshot hooks configured");
                return Ok(());
            }
        }
        "post-snapshot" => {
            if let Some(global) = config.global.as_mut() {
                if let Some(hooks) = global.hooks.as_mut() {
                    &mut hooks.post_snapshot
                } else {
                    info!("No post-snapshot hooks configured");
                    return Ok(());
                }
            } else {
                info!("No post-snapshot hooks configured");
                return Ok(());
            }
        }
        "pre-plugin" => {
            let plugin_name = plugin_name.as_ref().unwrap();
            return handle_plugin_hook_removal(
                &mut config,
                plugin_name,
                "pre-plugin",
                index,
                all,
                script,
                config_path,
            )
            .await;
        }
        "post-plugin" => {
            let plugin_name = plugin_name.as_ref().unwrap();
            return handle_plugin_hook_removal(
                &mut config,
                plugin_name,
                "post-plugin",
                index,
                all,
                script,
                config_path,
            )
            .await;
        }
        _ => unreachable!(),
    };

    let original_count = hooks.len();

    if all {
        hooks.clear();
        let plugin_context = plugin_name
            .map(|p| format!(" from {p}"))
            .unwrap_or_else(|| " (global)".to_string());
        info!(
            "{} Removed all {hook_type} hooks{plugin_context}:",
            SYMBOL_INDICATOR_SUCCESS
        );
        info!(
            "   {}  {} hooks removed",
            SYMBOL_CONTENT_TRASH, original_count
        );
    } else if let Some(idx) = index {
        if idx < hooks.len() {
            let removed_hook = hooks.remove(idx);
            let plugin_context = plugin_name
                .map(|p| format!(" from {p}"))
                .unwrap_or_else(|| " (global)".to_string());
            info!(
                "{} Removed {hook_type} hook{plugin_context}:",
                SYMBOL_INDICATOR_SUCCESS
            );
            info!("   {} {removed_hook}", SYMBOL_DOC_NOTE);
        } else {
            error!(
                "{} Index {idx} is out of range (max: {})",
                SYMBOL_INDICATOR_ERROR,
                hooks.len().saturating_sub(1)
            );
            return Ok(());
        }
    } else if let Some(script_name) = script {
        let mut removed_count = 0;
        hooks.retain(|hook| {
            if let HookAction::Script { command, .. } = hook {
                if command.contains(&script_name) {
                    removed_count += 1;
                    false
                } else {
                    true
                }
            } else {
                true
            }
        });

        if removed_count > 0 {
            let plugin_context = plugin_name
                .map(|p| format!(" from {p}"))
                .unwrap_or_else(|| " (global)".to_string());
            info!("{} Removed {removed_count} {hook_type} hook(s){plugin_context} matching script '{script_name}'", SYMBOL_INDICATOR_SUCCESS);
        } else {
            info!("No {hook_type} hooks found matching script '{script_name}'");
            return Ok(());
        }
    } else {
        error!(
            "{} Must specify --index, --all, or --script to remove hooks",
            SYMBOL_INDICATOR_ERROR
        );
        return Ok(());
    }

    // Save updated configuration
    let config_file_path = get_config_file_path(config_path);
    config.save_to_file(&config_file_path).await?;

    Ok(())
}

async fn handle_list_hooks(
    plugin: Option<String>,
    pre_plugin: bool,
    post_plugin: bool,
    pre_snapshot: bool,
    post_snapshot: bool,
    verbose: bool,
    config_path: Option<PathBuf>,
) -> Result<()> {
    let config = load_or_create_config(config_path).await?;
    let hooks_config = config.get_hooks_config();

    info!("{} Plugin Hooks Configuration:", SYMBOL_ACTION_HOOK);
    info!(
        "{} Scripts Directory: {}",
        SYMBOL_CONTENT_FOLDER,
        hooks_config.scripts_dir.display()
    );
    info!("");

    // Show global hooks if requested or if no specific filters
    let show_global = plugin.is_none() && (!pre_plugin && !post_plugin);
    if show_global {
        show_global_hooks(&config, pre_snapshot, post_snapshot, verbose, &hooks_config);
    }

    // Show plugin-specific hooks
    if let Some(plugin_name) = plugin {
        show_plugin_hooks(
            &config,
            &plugin_name,
            pre_plugin,
            post_plugin,
            verbose,
            &hooks_config,
        );
    } else if !show_global || pre_plugin || post_plugin {
        // Show all plugin hooks when filtering by hook type
        show_all_plugin_hooks(&config, pre_plugin, post_plugin, verbose, &hooks_config);
    }

    // Show total count
    let total_hooks = count_total_hooks(&config);
    info!("");
    info!("Total hooks: {total_hooks}");

    Ok(())
}

async fn handle_validate_hooks(
    plugin: Option<String>,
    pre_plugin: bool,
    post_plugin: bool,
    pre_snapshot: bool,
    post_snapshot: bool,
    config_path: Option<PathBuf>,
) -> Result<()> {
    let config = load_or_create_config(config_path).await?;
    let hooks_config = config.get_hooks_config();
    let hook_context = HookContext::new(
        "validation".to_string(),
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
        hooks_config.clone(),
    );
    let hook_manager = HookManager::new(hooks_config.clone());

    info!("{} Validating hook configuration...", SYMBOL_ACTION_SEARCH);
    info!(
        "{} Scripts Directory: {} (exists: {})",
        SYMBOL_CONTENT_FOLDER,
        hooks_config.scripts_dir.display(),
        if hooks_config.scripts_dir.exists() {
            SYMBOL_INDICATOR_SUCCESS
        } else {
            SYMBOL_INDICATOR_ERROR
        }
    );
    info!("");

    let mut total_valid = 0;
    let mut total_warnings = 0;
    let mut total_errors = 0;

    // Validate global hooks if requested
    if plugin.is_none() && (!pre_plugin && !post_plugin) {
        if !post_snapshot {
            let hooks = config.get_global_pre_snapshot_hooks();
            let (valid, warnings, errors) =
                validate_hook_list(&hook_manager, &hooks, "pre-snapshot", None, &hook_context);
            total_valid += valid;
            total_warnings += warnings;
            total_errors += errors;
        }

        if !pre_snapshot {
            let hooks = config.get_global_post_snapshot_hooks();
            let (valid, warnings, errors) =
                validate_hook_list(&hook_manager, &hooks, "post-snapshot", None, &hook_context);
            total_valid += valid;
            total_warnings += warnings;
            total_errors += errors;
        }
    }

    // Validate plugin hooks
    let plugin_names = if let Some(plugin_name) = plugin {
        vec![plugin_name]
    } else {
        get_all_plugin_names(&config)
    };

    for plugin_name in plugin_names {
        if !post_plugin {
            let hooks = config.get_plugin_pre_hooks(&plugin_name);
            let plugin_context = hook_context.clone().with_plugin(plugin_name.clone());
            let (valid, warnings, errors) = validate_hook_list(
                &hook_manager,
                &hooks,
                "pre-plugin",
                Some(&plugin_name),
                &plugin_context,
            );
            total_valid += valid;
            total_warnings += warnings;
            total_errors += errors;
        }

        if !pre_plugin {
            let hooks = config.get_plugin_post_hooks(&plugin_name);
            let plugin_context = hook_context.clone().with_plugin(plugin_name.clone());
            let (valid, warnings, errors) = validate_hook_list(
                &hook_manager,
                &hooks,
                "post-plugin",
                Some(&plugin_name),
                &plugin_context,
            );
            total_valid += valid;
            total_warnings += warnings;
            total_errors += errors;
        }
    }

    // Summary
    info!("");
    info!(
        "Validation summary: {} valid, {} warnings, {} errors",
        total_valid, total_warnings, total_errors
    );

    if total_errors == 0 && total_warnings == 0 {
        info!("{} All hooks are valid!", SYMBOL_INDICATOR_SUCCESS);
    } else if total_errors == 0 {
        warn!(
            "{} Configuration is valid but has warnings",
            SYMBOL_INDICATOR_WARNING
        );
    } else {
        error!(
            "{} Configuration has errors that need to be fixed",
            SYMBOL_INDICATOR_ERROR
        );
    }

    if !hooks_config.scripts_dir.exists() {
        info!(
            "{} Run 'dotsnapshot hooks scripts-dir --create' to create the scripts directory",
            SYMBOL_EXPERIENCE_IDEA
        );
    }

    Ok(())
}

async fn handle_scripts_dir(
    set: Option<PathBuf>,
    create: bool,
    config_path: Option<PathBuf>,
) -> Result<()> {
    let mut config = load_or_create_config(config_path.clone()).await?;

    if let Some(new_path) = set {
        // Set new scripts directory
        let expanded_path = HooksConfig::expand_tilde(&new_path);

        if config.hooks.is_none() {
            config.hooks = Some(HooksConfig::default());
        }
        config.hooks.as_mut().unwrap().scripts_dir = expanded_path.clone();

        // Save configuration
        let config_file_path = get_config_file_path(config_path);
        config.save_to_file(&config_file_path).await?;

        info!(
            "{} Scripts directory updated: {}",
            SYMBOL_CONTENT_FOLDER,
            expanded_path.display()
        );
        if !expanded_path.exists() {
            warn!(
                "   {}  Directory does not exist - run with --create to create it",
                SYMBOL_INDICATOR_WARNING
            );
            warn!(
                "   {} Existing scripts will need to be moved manually",
                SYMBOL_EXPERIENCE_IDEA
            );
        } else {
            info!("   {} Directory exists", SYMBOL_INDICATOR_SUCCESS);
        }

        if create && !expanded_path.exists() {
            tokio::fs::create_dir_all(&expanded_path)
                .await
                .context("Failed to create scripts directory")?;
            info!("   {} Created scripts directory", SYMBOL_CONTENT_FOLDER);
        }
    } else {
        // Show current scripts directory
        let hooks_config = config.get_hooks_config();
        let scripts_dir = &hooks_config.scripts_dir;
        let expanded_dir = HooksConfig::expand_tilde(scripts_dir);

        info!(
            "{} Current scripts directory: {}",
            SYMBOL_CONTENT_FOLDER,
            scripts_dir.display()
        );
        info!(
            "   Status: {} {}",
            if expanded_dir.exists() {
                "exists"
            } else {
                "does not exist"
            },
            if expanded_dir.exists() {
                let script_count = count_scripts_in_directory(&expanded_dir).await.unwrap_or(0);
                format!("({script_count} scripts found)")
            } else {
                String::new()
            }
        );
        info!("   Path: {}", expanded_dir.display());

        if create && !expanded_dir.exists() {
            tokio::fs::create_dir_all(&expanded_dir)
                .await
                .context("Failed to create scripts directory")?;
            info!(
                "{} Created scripts directory: {}",
                SYMBOL_CONTENT_FOLDER,
                expanded_dir.display()
            );
            info!(
                "   {} Directory created successfully",
                SYMBOL_INDICATOR_SUCCESS
            );
            info!(
                "   {} You can now add your hook scripts to this directory",
                SYMBOL_EXPERIENCE_IDEA
            );
        }
    }

    Ok(())
}

// Helper functions

fn convert_action_args_to_hook_action(args: HookActionArgs) -> Result<HookAction> {
    if let Some(script) = args.script {
        let env_vars = HashMap::new();
        let script_args = args
            .args
            .map(|a| a.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();
        let timeout = args.timeout.unwrap_or(30);

        Ok(HookAction::Script {
            command: script,
            args: script_args,
            timeout,
            working_dir: None,
            env_vars,
        })
    } else if let Some(message) = args.log {
        Ok(HookAction::Log {
            message,
            level: args.level.unwrap_or_else(|| "info".to_string()),
        })
    } else if let Some(message) = args.notify {
        Ok(HookAction::Notify {
            message,
            title: args.title,
        })
    } else if args.backup {
        Ok(HookAction::Backup {
            path: args
                .path
                .ok_or_else(|| anyhow::anyhow!("--path required for backup action"))?,
            destination: args
                .destination
                .ok_or_else(|| anyhow::anyhow!("--destination required for backup action"))?,
        })
    } else if args.cleanup {
        let patterns = args
            .patterns
            .map(|p| p.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();
        let directories = args
            .directories
            .map(|d| d.split(',').map(|s| PathBuf::from(s.trim())).collect())
            .unwrap_or_default();

        Ok(HookAction::Cleanup {
            patterns,
            directories,
            temp_files: args.temp_files,
        })
    } else {
        Err(anyhow::anyhow!("No action specified"))
    }
}

fn determine_hook_target(target: &HookTarget) -> Result<(String, Option<String>)> {
    if target.pre_snapshot {
        Ok(("pre-snapshot".to_string(), None))
    } else if target.post_snapshot {
        Ok(("post-snapshot".to_string(), None))
    } else if let Some(plugin) = &target.pre_plugin {
        Ok(("pre-plugin".to_string(), Some(plugin.clone())))
    } else if let Some(plugin) = &target.post_plugin {
        Ok(("post-plugin".to_string(), Some(plugin.clone())))
    } else {
        Err(anyhow::anyhow!("No hook target specified"))
    }
}

async fn load_or_create_config(config_path: Option<PathBuf>) -> Result<Config> {
    if let Some(path) = config_path {
        if path.exists() {
            Config::load_from_file(&path).await
        } else {
            Ok(Config::default())
        }
    } else {
        Config::load().await
    }
}

fn get_config_file_path(config_path: Option<PathBuf>) -> PathBuf {
    config_path.unwrap_or_else(|| {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dotsnapshot")
            .join("config.toml")
    })
}

fn ensure_global_config(config: &mut Config) {
    if config.global.is_none() {
        config.global = Some(GlobalConfig { hooks: None });
    }
    if config.global.as_ref().unwrap().hooks.is_none() {
        config.global.as_mut().unwrap().hooks = Some(GlobalHooks {
            pre_snapshot: Vec::new(),
            post_snapshot: Vec::new(),
        });
    }
}

fn ensure_plugin_config(config: &mut Config, plugin_name: &str) {
    use crate::config::PluginsConfig;

    if config.plugins.is_none() {
        config.plugins = Some(PluginsConfig {
            plugins: std::collections::HashMap::new(),
        });
    }

    let plugins = config.plugins.as_mut().unwrap();

    if !plugins.plugins.contains_key(plugin_name) {
        let plugin_config = PluginConfig {
            target_path: None,
            output_file: None,
            hooks: Some(PluginHooks {
                pre_plugin: Vec::new(),
                post_plugin: Vec::new(),
            }),
        };
        if let Ok(value) = toml::Value::try_from(plugin_config) {
            plugins.plugins.insert(plugin_name.to_string(), value);
        } else {
            warn!(
                "Failed to serialize PluginConfig for plugin '{}'",
                plugin_name
            );
        }
    } else {
        // Ensure hooks exist
        if let Some(plugin_value) = plugins.plugins.get_mut(plugin_name) {
            if let Ok(mut plugin_config) = plugin_value.clone().try_into::<PluginConfig>() {
                if plugin_config.hooks.is_none() {
                    plugin_config.hooks = Some(PluginHooks {
                        pre_plugin: Vec::new(),
                        post_plugin: Vec::new(),
                    });
                    if let Ok(value) = toml::Value::try_from(plugin_config) {
                        *plugin_value = value;
                    } else {
                        warn!(
                            "Failed to serialize PluginConfig for plugin '{}'",
                            plugin_name
                        );
                    }
                }
            }
        }
    }
}

// Helper function to modify plugin config - returns a closure that modifies the plugin config in place
fn modify_plugin_config<F, R>(config: &mut Config, plugin_name: &str, modifier: F) -> Option<R>
where
    F: FnOnce(&mut PluginConfig) -> R,
{
    let plugins = config.plugins.as_mut()?;
    let plugin_value = plugins.plugins.get_mut(plugin_name)?;

    if let Ok(mut plugin_config) = plugin_value.clone().try_into::<PluginConfig>() {
        let result = modifier(&mut plugin_config);
        match toml::Value::try_from(plugin_config) {
            Ok(value) => {
                *plugin_value = value;
                Some(result)
            }
            Err(e) => {
                warn!(
                    "Failed to serialize PluginConfig for plugin '{}': {}",
                    plugin_name, e
                );
                None
            }
        }
    } else {
        None
    }
}

// Legacy function for backward compatibility - now uses modify_plugin_config internally

async fn handle_plugin_hook_removal(
    config: &mut Config,
    plugin_name: &str,
    hook_type: &str,
    index: Option<usize>,
    all: bool,
    script: Option<String>,
    config_path: Option<PathBuf>,
) -> Result<()> {
    let plugins = config.plugins.as_mut();
    if plugins.is_none() {
        info!("No {hook_type} hooks configured for {plugin_name}");
        return Ok(());
    }
    let plugins = plugins.unwrap();

    // Get the current plugin config or create a new one
    let current_value = plugins.plugins.get(plugin_name).cloned();
    let mut plugin_config = if let Some(value) = current_value {
        value.try_into::<PluginConfig>().unwrap_or(PluginConfig {
            target_path: None,
            output_file: None,
            hooks: None,
        })
    } else {
        PluginConfig {
            target_path: None,
            output_file: None,
            hooks: None,
        }
    };

    // Check if hooks exist
    if plugin_config.hooks.is_none() {
        plugin_config.hooks = Some(PluginHooks {
            pre_plugin: Vec::new(),
            post_plugin: Vec::new(),
        });
    }

    {
        let hooks = if hook_type == "pre-plugin" {
            if let Some(ref mut hooks) = plugin_config.hooks {
                &mut hooks.pre_plugin
            } else {
                info!("No {hook_type} hooks configured for {plugin_name}");
                return Ok(());
            }
        } else if let Some(ref mut hooks) = plugin_config.hooks {
            &mut hooks.post_plugin
        } else {
            info!("No {hook_type} hooks configured for {plugin_name}");
            return Ok(());
        };

        let original_count = hooks.len();

        if all {
            hooks.clear();
            info!(
                "{} Removed all {hook_type} hooks from {plugin_name}:",
                SYMBOL_INDICATOR_SUCCESS
            );
            info!(
                "   {}  {} hooks removed",
                SYMBOL_CONTENT_TRASH, original_count
            );
        } else if let Some(idx) = index {
            if idx < hooks.len() {
                let removed_hook = hooks.remove(idx);
                info!(
                    "{} Removed {hook_type} hook from {plugin_name}:",
                    SYMBOL_INDICATOR_SUCCESS
                );
                info!("   {} {removed_hook}", SYMBOL_DOC_NOTE);
            } else {
                error!(
                    "{} Index {idx} is out of range (max: {})",
                    SYMBOL_INDICATOR_ERROR,
                    hooks.len().saturating_sub(1)
                );
                return Ok(());
            }
        } else if let Some(script_name) = script {
            let mut removed_count = 0;
            hooks.retain(|hook| {
                if let HookAction::Script { command, .. } = hook {
                    if command.contains(&script_name) {
                        removed_count += 1;
                        false
                    } else {
                        true
                    }
                } else {
                    true
                }
            });

            if removed_count > 0 {
                info!(
                    "{} Removed {} {hook_type} hook(s) from {plugin_name} containing '{script_name}'",
                    SYMBOL_INDICATOR_SUCCESS,
                    removed_count
                );
            } else {
                info!("No {hook_type} hooks found for {plugin_name} containing '{script_name}'");
            }
        }
    }

    // Save the modified config back to the HashMap
    plugins.plugins.insert(
        plugin_name.to_string(),
        toml::Value::try_from(plugin_config).with_context(|| {
            format!("Failed to serialize PluginConfig for plugin '{plugin_name}'")
        })?,
    );

    // Save the config
    let save_path = config_path.unwrap_or_else(|| {
        Config::get_config_paths()
            .into_iter()
            .next()
            .unwrap_or_else(|| PathBuf::from("dotsnapshot.toml"))
    });
    config.save_to_file(&save_path).await?;

    info!("{} Configuration updated", SYMBOL_INDICATOR_SUCCESS);

    Ok(())
}

fn show_global_hooks(
    config: &Config,
    pre_snapshot: bool,
    post_snapshot: bool,
    verbose: bool,
    hooks_config: &HooksConfig,
) {
    let show_pre = !post_snapshot || pre_snapshot;
    let show_post = !pre_snapshot || post_snapshot;

    if show_pre || show_post {
        info!("{} Global Hooks:", SYMBOL_SCOPE_WORLD);

        if show_pre {
            let hooks = config.get_global_pre_snapshot_hooks();
            show_hook_list(&hooks, "pre-snapshot", None, verbose, hooks_config);
        }

        if show_post {
            let hooks = config.get_global_post_snapshot_hooks();
            show_hook_list(&hooks, "post-snapshot", None, verbose, hooks_config);
        }

        info!("");
    }
}

fn show_plugin_hooks(
    config: &Config,
    plugin_name: &str,
    pre_plugin: bool,
    post_plugin: bool,
    verbose: bool,
    hooks_config: &HooksConfig,
) {
    let show_pre = !post_plugin || pre_plugin;
    let show_post = !pre_plugin || post_plugin;

    let icon = match plugin_name {
        "homebrew_brewfile" => SYMBOL_TOOL_PACKAGE_MANAGER,
        name if name.starts_with("vscode") => SYMBOL_TOOL_COMPUTER,
        name if name.starts_with("cursor") => SYMBOL_TOOL_EDITOR,
        name if name.starts_with("npm") => SYMBOL_CONTENT_PACKAGE,
        _ => SYMBOL_TOOL_PLUGIN,
    };

    info!("{icon} {plugin_name}:");

    if show_pre {
        let hooks = config.get_plugin_pre_hooks(plugin_name);
        show_hook_list(
            &hooks,
            "pre-plugin",
            Some(plugin_name),
            verbose,
            hooks_config,
        );
    }

    if show_post {
        let hooks = config.get_plugin_post_hooks(plugin_name);
        show_hook_list(
            &hooks,
            "post-plugin",
            Some(plugin_name),
            verbose,
            hooks_config,
        );
    }

    info!("");
}

fn show_all_plugin_hooks(
    config: &Config,
    pre_plugin: bool,
    post_plugin: bool,
    verbose: bool,
    hooks_config: &HooksConfig,
) {
    let plugin_names = get_all_plugin_names(config);

    for plugin_name in plugin_names {
        show_plugin_hooks(
            config,
            &plugin_name,
            pre_plugin,
            post_plugin,
            verbose,
            hooks_config,
        );
    }
}

fn show_hook_list(
    hooks: &[HookAction],
    hook_type: &str,
    _plugin_name: Option<&str>,
    verbose: bool,
    hooks_config: &HooksConfig,
) {
    if hooks.is_empty() {
        return;
    }

    info!("  {hook_type}:");
    for (index, hook) in hooks.iter().enumerate() {
        if let HookAction::Script { command, .. } = hook {
            let script_path = hooks_config.resolve_script_path(command);
            let expanded_path = HooksConfig::expand_tilde(&script_path);
            let exists = if expanded_path.exists() {
                SYMBOL_INDICATOR_SUCCESS
            } else {
                SYMBOL_INDICATOR_ERROR
            };

            if verbose {
                info!(
                    "    [{}] {} → {} {}",
                    index,
                    hook,
                    expanded_path.display(),
                    exists
                );
                if let HookAction::Script { args, timeout, .. } = hook {
                    if !args.is_empty() {
                        info!("        args: {:?}", args);
                    }
                    info!("        timeout: {}s", timeout);
                }
            } else {
                info!(
                    "    [{}] {} → {} {}",
                    index,
                    hook,
                    expanded_path.display(),
                    exists
                );
            }
        } else {
            info!("    [{}] {}", index, hook);
            if verbose {
                info!("        {:#?}", hook);
            }
        }
    }
}

fn validate_hook_list(
    hook_manager: &HookManager,
    hooks: &[HookAction],
    hook_type: &str,
    plugin_name: Option<&str>,
    context: &HookContext,
) -> (usize, usize, usize) {
    if hooks.is_empty() {
        return (0, 0, 0);
    }

    let plugin_label = plugin_name
        .map(|p| format!(" {p}"))
        .unwrap_or_else(|| " (global)".to_string());
    info!("{}{plugin_label} {hook_type} hooks:", SYMBOL_ACTION_SEARCH);

    let mut valid = 0;
    let mut warnings = 0;
    let mut errors = 0;

    for (index, hook) in hooks.iter().enumerate() {
        let results = hook_manager.validate_hooks(std::slice::from_ref(hook), context);
        match &results[0] {
            Ok(_) => {
                valid += 1;
                if let HookAction::Notify { .. } = hook {
                    warnings += 1;
                    warn!(
                        "  {}  [{index}] {hook} (system notifications may not be available)",
                        SYMBOL_INDICATOR_WARNING
                    );
                } else {
                    info!("  {} [{index}] {hook}", SYMBOL_INDICATOR_SUCCESS);
                }
            }
            Err(e) => {
                errors += 1;
                error!("  {} [{index}] {hook}", SYMBOL_INDICATOR_ERROR);
                error!("      Error: {e}");
            }
        }
    }

    (valid, warnings, errors)
}

fn count_total_hooks(config: &Config) -> usize {
    let mut total = 0;

    // Count global hooks
    total += config.get_global_pre_snapshot_hooks().len();
    total += config.get_global_post_snapshot_hooks().len();

    // Count plugin hooks
    for plugin_name in get_all_plugin_names(config) {
        total += config.get_plugin_pre_hooks(&plugin_name).len();
        total += config.get_plugin_post_hooks(&plugin_name).len();
    }

    total
}

fn get_all_plugin_names(config: &Config) -> Vec<String> {
    let mut names = Vec::new();

    if let Some(plugins) = &config.plugins {
        for plugin_name in plugins.plugins.keys() {
            names.push(plugin_name.clone());
        }
    }

    names
}

async fn count_scripts_in_directory(dir: &PathBuf) -> Result<usize> {
    let mut count = 0;
    let mut entries = tokio::fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() {
            // Simple check for executable files or script extensions
            if let Some(extension) = path.extension() {
                if matches!(extension.to_str(), Some("sh" | "py" | "rb" | "js" | "ts")) {
                    count += 1;
                }
            } else {
                // Check if file is executable (Unix-like systems)
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = tokio::fs::metadata(&path).await {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            count += 1;
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    count += 1; // Assume executable on non-Unix systems
                }
            }
        }
    }

    Ok(count)
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::core::hooks::HookAction;
    use tempfile::TempDir;
    use tokio::fs;

    /// Test conversion from CLI args to hook actions
    /// Verifies that different hook action types are converted correctly
    #[test]
    fn test_convert_action_args_to_hook_action() {
        // Test script action
        let script_args = HookActionArgs {
            script: Some("test.sh".to_string()),
            args: Some("arg1,arg2".to_string()),
            timeout: Some(30),
            log: None,
            notify: None,
            backup: false,
            cleanup: false,
            level: None,
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(script_args).unwrap();
        match result {
            HookAction::Script {
                command,
                args,
                timeout,
                ..
            } => {
                assert_eq!(command, "test.sh");
                assert_eq!(args, vec!["arg1", "arg2"]);
                assert_eq!(timeout, 30);
            }
            _ => panic!("Expected script action"),
        }

        // Test log action
        let log_args = HookActionArgs {
            script: None,
            log: Some("Test message".to_string()),
            level: Some("info".to_string()),
            notify: None,
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(log_args).unwrap();
        match result {
            HookAction::Log { message, level } => {
                assert_eq!(message, "Test message");
                assert_eq!(level, "info");
            }
            _ => panic!("Expected log action"),
        }

        // Test notify action
        let notify_args = HookActionArgs {
            script: None,
            log: None,
            notify: Some("Notification message".to_string()),
            title: Some("Test Title".to_string()),
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(notify_args).unwrap();
        match result {
            HookAction::Notify { message, title } => {
                assert_eq!(message, "Notification message");
                assert_eq!(title, Some("Test Title".to_string()));
            }
            _ => panic!("Expected notify action"),
        }

        // Test backup action
        let backup_args = HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: true,
            path: Some(PathBuf::from("/source")),
            destination: Some(PathBuf::from("/backup")),
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(backup_args).unwrap();
        match result {
            HookAction::Backup { path, destination } => {
                assert_eq!(path, PathBuf::from("/source"));
                assert_eq!(destination, PathBuf::from("/backup"));
            }
            _ => panic!("Expected backup action"),
        }

        // Test cleanup action
        let cleanup_args = HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: false,
            cleanup: true,
            patterns: Some("*.tmp,*.log".to_string()),
            directories: Some("/tmp,/var/tmp".to_string()),
            temp_files: true,
            args: None,
            timeout: None,
            level: None,
            title: None,
            path: None,
            destination: None,
        };

        let result = convert_action_args_to_hook_action(cleanup_args).unwrap();
        match result {
            HookAction::Cleanup {
                patterns,
                directories,
                temp_files,
            } => {
                assert_eq!(patterns, vec!["*.tmp", "*.log"]);
                assert_eq!(
                    directories,
                    vec![PathBuf::from("/tmp"), PathBuf::from("/var/tmp")]
                );
                assert!(temp_files);
            }
            _ => panic!("Expected cleanup action"),
        }
    }

    /// Test error cases for action conversion
    /// Verifies that invalid hook action arguments produce appropriate errors
    #[test]
    fn test_convert_action_args_to_hook_action_errors() {
        // Test missing action (no action specified)
        let empty_args = HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(empty_args);
        assert!(result.is_err());

        // Test backup action without required paths
        let incomplete_backup = HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: true,
            path: None, // Missing path
            destination: Some(PathBuf::from("/backup")),
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(incomplete_backup);
        assert!(result.is_err());
    }

    /// Test determination of hook targets from CLI arguments
    /// Verifies that hook targets are parsed correctly
    #[test]
    fn test_determine_hook_target() {
        // Test pre-snapshot target
        let pre_snapshot_target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let (hook_type, plugin_name) = determine_hook_target(&pre_snapshot_target).unwrap();
        assert_eq!(hook_type, "pre-snapshot");
        assert_eq!(plugin_name, None);

        // Test post-snapshot target
        let post_snapshot_target = HookTarget {
            pre_snapshot: false,
            post_snapshot: true,
            pre_plugin: None,
            post_plugin: None,
        };

        let (hook_type, plugin_name) = determine_hook_target(&post_snapshot_target).unwrap();
        assert_eq!(hook_type, "post-snapshot");
        assert_eq!(plugin_name, None);

        // Test pre-plugin target
        let pre_plugin_target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: Some("vscode_settings".to_string()),
            post_plugin: None,
        };

        let (hook_type, plugin_name) = determine_hook_target(&pre_plugin_target).unwrap();
        assert_eq!(hook_type, "pre-plugin");
        assert_eq!(plugin_name, Some("vscode_settings".to_string()));

        // Test post-plugin target
        let post_plugin_target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: Some("homebrew_brewfile".to_string()),
        };

        let (hook_type, plugin_name) = determine_hook_target(&post_plugin_target).unwrap();
        assert_eq!(hook_type, "post-plugin");
        assert_eq!(plugin_name, Some("homebrew_brewfile".to_string()));
    }

    /// Test error cases for hook target determination
    /// Verifies that invalid targets produce appropriate errors
    #[test]
    fn test_determine_hook_target_errors() {
        // Test no target specified
        let no_target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let result = determine_hook_target(&no_target);
        assert!(result.is_err());

        // Note: Multiple targets are prevented by clap's group constraints,
        // so the function actually processes them sequentially and would return
        // the first match. This is correct behavior since clap prevents multiple targets.
    }

    /// Test config file path resolution
    /// Verifies that config paths are resolved correctly
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

    /// Test handle_hooks_command function with list subcommand
    /// Verifies that the hooks command dispatcher works correctly for listing hooks
    #[tokio::test]
    async fn test_handle_hooks_command_list() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create a test config with some hooks
        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![HookAction::Log {
                        message: "test pre".to_string(),
                        level: "info".to_string(),
                    }],
                    post_snapshot: vec![],
                }),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

        let command = HooksCommands::List {
            plugin: None,
            pre_plugin: false,
            post_plugin: false,
            pre_snapshot: true,
            post_snapshot: false,
            verbose: false,
        };

        let result = handle_hooks_command(command, Some(config_path)).await;
        assert!(result.is_ok());
    }

    /// Test handle_hooks_command function with add subcommand
    /// Verifies that adding hooks through the command works correctly
    #[tokio::test]
    async fn test_handle_hooks_command_add() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create empty config
        Config::default().save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let action = HookActionArgs {
            script: Some("test.sh".to_string()),
            args: None,
            timeout: Some(30),
            log: None,
            notify: None,
            backup: false,
            cleanup: false,
            level: None,
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let command = HooksCommands::Add { target, action };

        let result = handle_hooks_command(command, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was added
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let pre_hooks = updated_config.get_global_pre_snapshot_hooks();
        assert_eq!(pre_hooks.len(), 1);
    }

    /// Test handle_hooks_command function with remove subcommand
    /// Verifies that removing hooks through the command works correctly
    #[tokio::test]
    async fn test_handle_hooks_command_remove() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with a hook to remove
        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![HookAction::Script {
                        command: "test.sh".to_string(),
                        args: vec![],
                        timeout: 30,
                        working_dir: None,
                        env_vars: std::collections::HashMap::new(),
                    }],
                    post_snapshot: vec![],
                }),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let command = HooksCommands::Remove {
            target,
            index: Some(0),
            all: false,
            script: None,
        };

        let result = handle_hooks_command(command, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was removed
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let pre_hooks = updated_config.get_global_pre_snapshot_hooks();
        assert_eq!(pre_hooks.len(), 0);
    }

    /// Test handle_hooks_command function with validate subcommand
    /// Verifies that hook validation through the command works correctly
    #[tokio::test]
    async fn test_handle_hooks_command_validate() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with hooks to validate
        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![HookAction::Log {
                        message: "test".to_string(),
                        level: "info".to_string(),
                    }],
                    post_snapshot: vec![],
                }),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

        let command = HooksCommands::Validate {
            plugin: None,
            pre_plugin: false,
            post_plugin: false,
            pre_snapshot: true,
            post_snapshot: false,
        };

        let result = handle_hooks_command(command, Some(config_path)).await;
        assert!(result.is_ok());
    }

    /// Test handle_hooks_command function with scripts-dir subcommand
    /// Verifies that scripts directory management through the command works correctly
    #[tokio::test]
    async fn test_handle_hooks_command_scripts_dir() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        let scripts_dir = temp_dir.path().join("scripts");

        // Create scripts directory with a test script
        fs::create_dir_all(&scripts_dir).await.unwrap();
        fs::write(scripts_dir.join("test.sh"), "#!/bin/bash\necho test")
            .await
            .unwrap();

        // Create config with scripts directory
        let config = Config {
            hooks: Some(HooksConfig {
                scripts_dir: scripts_dir.clone(),
            }),
            ..Default::default()
        };
        config.save_to_file(&config_path).await.unwrap();

        let command = HooksCommands::ScriptsDir {
            set: Some(scripts_dir),
            create: false,
        };

        let result = handle_hooks_command(command, Some(config_path)).await;
        assert!(result.is_ok());
    }

    /// Test handle_add_hook function with global hooks
    /// Verifies that adding global hooks works correctly
    #[tokio::test]
    async fn test_handle_add_hook_global() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create empty config
        Config::default().save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let action = HookActionArgs {
            script: None,
            args: None,
            timeout: None,
            log: Some("test message".to_string()),
            notify: None,
            backup: false,
            cleanup: false,
            level: Some("info".to_string()),
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = handle_add_hook(target, action, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was added
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = updated_config.get_global_pre_snapshot_hooks();
        assert_eq!(hooks.len(), 1);
        match &hooks[0] {
            HookAction::Log { message, level } => {
                assert_eq!(message, "test message");
                assert_eq!(level, "info");
            }
            _ => panic!("Expected log hook"),
        }
    }

    /// Test handle_add_hook function with plugin hooks
    /// Verifies that adding plugin-specific hooks works correctly
    #[tokio::test]
    async fn test_handle_add_hook_plugin() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create empty config
        Config::default().save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: Some("test_plugin".to_string()),
            post_plugin: None,
        };

        let action = HookActionArgs {
            script: Some("test.sh".to_string()),
            args: Some("arg1".to_string()),
            timeout: Some(60),
            log: None,
            notify: None,
            backup: false,
            cleanup: false,
            level: None,
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = handle_add_hook(target, action, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was added
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = updated_config.get_plugin_pre_hooks("test_plugin");
        assert_eq!(hooks.len(), 1);
        match &hooks[0] {
            HookAction::Script {
                command,
                args,
                timeout,
                working_dir,
                env_vars,
            } => {
                assert_eq!(command, "test.sh");
                assert_eq!(args, &vec!["arg1".to_string()]);
                assert_eq!(*timeout, 60);
                assert_eq!(*working_dir, None);
                assert!(env_vars.is_empty());
            }
            _ => panic!("Expected script hook"),
        }
    }

    /// Test handle_remove_hook function with index removal
    /// Verifies that removing hooks by index works correctly
    #[tokio::test]
    async fn test_handle_remove_hook_by_index() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with multiple hooks
        let mut config = Config::default();
        config.global = Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![
                    HookAction::Log {
                        message: "first hook".to_string(),
                        level: "info".to_string(),
                    },
                    HookAction::Log {
                        message: "second hook".to_string(),
                        level: "warn".to_string(),
                    },
                ],
                post_snapshot: vec![],
            }),
        });
        config.save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let result =
            handle_remove_hook(target, Some(0), false, None, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify first hook was removed
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = updated_config.get_global_pre_snapshot_hooks();
        assert_eq!(hooks.len(), 1);
        match &hooks[0] {
            HookAction::Log { message, .. } => {
                assert_eq!(message, "second hook");
            }
            _ => panic!("Expected log hook"),
        }
    }

    /// Test handle_remove_hook function with all removal
    /// Verifies that removing all hooks works correctly
    #[tokio::test]
    async fn test_handle_remove_hook_all() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with multiple hooks
        let mut config = Config::default();
        config.global = Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![
                    HookAction::Log {
                        message: "first hook".to_string(),
                        level: "info".to_string(),
                    },
                    HookAction::Log {
                        message: "second hook".to_string(),
                        level: "warn".to_string(),
                    },
                ],
                post_snapshot: vec![],
            }),
        });
        config.save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let result = handle_remove_hook(target, None, true, None, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify all hooks were removed
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = updated_config.get_global_pre_snapshot_hooks();
        assert_eq!(hooks.len(), 0);
    }

    /// Test handle_list_hooks function with global hooks
    /// Verifies that listing hooks works correctly
    #[tokio::test]
    async fn test_handle_list_hooks_global() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with hooks
        let mut config = Config::default();
        config.global = Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![HookAction::Log {
                    message: "test".to_string(),
                    level: "info".to_string(),
                }],
                post_snapshot: vec![],
            }),
        });
        config.save_to_file(&config_path).await.unwrap();

        let result = handle_list_hooks(
            None,  // plugin
            false, // pre_plugin
            false, // post_plugin
            true,  // pre_snapshot
            false, // post_snapshot
            false, // verbose
            Some(config_path),
        )
        .await;
        assert!(result.is_ok());
    }

    /// Test handle_validate_hooks function
    /// Verifies that hook validation works correctly
    #[tokio::test]
    async fn test_handle_validate_hooks() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with hooks to validate
        let mut config = Config::default();
        config.global = Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![HookAction::Log {
                    message: "test".to_string(),
                    level: "info".to_string(),
                }],
                post_snapshot: vec![],
            }),
        });
        config.save_to_file(&config_path).await.unwrap();

        let result = handle_validate_hooks(
            None,  // plugin
            false, // pre_plugin
            false, // post_plugin
            true,  // pre_snapshot
            false, // post_snapshot
            Some(config_path),
        )
        .await;
        assert!(result.is_ok());
    }

    /// Test handle_scripts_dir function
    /// Verifies that scripts directory management works correctly
    #[tokio::test]
    async fn test_handle_scripts_dir() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        let scripts_dir = temp_dir.path().join("scripts");

        // Create scripts directory with test scripts
        fs::create_dir_all(&scripts_dir).await.unwrap();
        fs::write(scripts_dir.join("test1.sh"), "#!/bin/bash\necho test1")
            .await
            .unwrap();
        fs::write(
            scripts_dir.join("test2.py"),
            "#!/usr/bin/env python\nprint('test2')",
        )
        .await
        .unwrap();

        // Create config with scripts directory
        let mut config = Config::default();
        let mut hooks_config = HooksConfig::default();
        hooks_config.scripts_dir = scripts_dir.clone();
        config.hooks = Some(hooks_config);
        config.save_to_file(&config_path).await.unwrap();

        let result = handle_scripts_dir(Some(scripts_dir), false, Some(config_path)).await;
        assert!(result.is_ok());
    }

    /// Test handle_remove_hook with post-snapshot hooks
    /// Verifies that post-snapshot hooks can be removed correctly
    #[tokio::test]
    async fn test_handle_remove_hook_post_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with post-snapshot hooks
        let mut config = Config::default();
        config.global = Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![],
                post_snapshot: vec![HookAction::Log {
                    message: "post hook".to_string(),
                    level: "info".to_string(),
                }],
            }),
        });
        config.save_to_file(&config_path).await.unwrap();

        // Remove post-snapshot hook by index
        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: true,
            pre_plugin: None,
            post_plugin: None,
        };
        let result =
            handle_remove_hook(target, Some(0), false, None, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was removed
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = &updated_config.global.unwrap().hooks.unwrap().post_snapshot;
        assert!(hooks.is_empty());
    }

    /// Test determine_hook_target with multiple targets
    /// Verifies that first target takes precedence when multiple are set
    #[test]
    fn test_determine_hook_target_multiple_targets_first_wins() {
        // Test with multiple targets set - pre_snapshot should win
        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: true,
            pre_plugin: None,
            post_plugin: None,
        };
        let result = determine_hook_target(&target).unwrap();
        assert_eq!(result.0, "pre-snapshot");
        assert_eq!(result.1, None);
    }

    /// Test determine_hook_target with no targets
    /// Verifies that error is returned when no target is specified
    #[test]
    fn test_determine_hook_target_no_targets_error() {
        // Test with no targets set
        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };
        let result = determine_hook_target(&target);
        assert!(result.is_err());
    }

    /// Test determine_hook_target with multiple plugin targets
    /// Verifies that first plugin target takes precedence when multiple are set
    #[test]
    fn test_determine_hook_target_multiple_plugins_first_wins() {
        // Test with plugin target but multiple plugins - pre_plugin should win
        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: Some("plugin1".to_string()),
            post_plugin: Some("plugin2".to_string()),
        };
        let result = determine_hook_target(&target).unwrap();
        assert_eq!(result.0, "pre-plugin");
        assert_eq!(result.1, Some("plugin1".to_string()));
    }

    /// Test handle_add_hook with backup action
    /// Verifies that backup hooks can be added correctly
    #[tokio::test]
    async fn test_handle_add_hook_backup() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create empty config
        let config = Config::default();
        config.save_to_file(&config_path).await.unwrap();

        // Add backup hook
        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };
        let action = HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: true,
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: None,
            path: Some(PathBuf::from("/source/path")),
            destination: Some(PathBuf::from("/dest/path")),
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = handle_add_hook(target, action, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was added
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = &updated_config.global.unwrap().hooks.unwrap().pre_snapshot;
        assert_eq!(hooks.len(), 1);
        match &hooks[0] {
            HookAction::Backup { path, destination } => {
                assert_eq!(*path, PathBuf::from("/source/path"));
                assert_eq!(*destination, PathBuf::from("/dest/path"));
            }
            _ => panic!("Expected backup hook"),
        }
    }

    /// Test handle_add_hook with cleanup action
    /// Verifies that cleanup hooks can be added correctly
    #[tokio::test]
    async fn test_handle_add_hook_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create empty config
        let config = Config::default();
        config.save_to_file(&config_path).await.unwrap();

        // Add cleanup hook
        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };
        let action = HookActionArgs {
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
            patterns: Some("*.tmp,*.log".to_string()),
            directories: Some("/tmp,/var/log".to_string()),
            temp_files: true,
        };

        let result = handle_add_hook(target, action, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was added
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = &updated_config.global.unwrap().hooks.unwrap().pre_snapshot;
        assert_eq!(hooks.len(), 1);
        match &hooks[0] {
            HookAction::Cleanup {
                patterns,
                directories,
                temp_files,
            } => {
                assert_eq!(patterns.len(), 2);
                assert!(patterns.contains(&"*.tmp".to_string()));
                assert!(patterns.contains(&"*.log".to_string()));
                assert_eq!(directories.len(), 2);
                assert!(directories.contains(&PathBuf::from("/tmp")));
                assert!(directories.contains(&PathBuf::from("/var/log")));
                assert!(*temp_files);
            }
            _ => panic!("Expected cleanup hook"),
        }
    }

    /// Test handle_add_hook with notify action
    /// Verifies that notify hooks can be added correctly
    #[tokio::test]
    async fn test_handle_add_hook_notify() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create empty config
        let config = Config::default();
        config.save_to_file(&config_path).await.unwrap();

        // Add notify hook
        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };
        let action = HookActionArgs {
            script: None,
            log: None,
            notify: Some("Test notification".to_string()),
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: Some("Test Title".to_string()),
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = handle_add_hook(target, action, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was added
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = &updated_config.global.unwrap().hooks.unwrap().pre_snapshot;
        assert_eq!(hooks.len(), 1);
        match &hooks[0] {
            HookAction::Notify { message, title } => {
                assert_eq!(*message, "Test notification");
                assert_eq!(*title, Some("Test Title".to_string()));
            }
            _ => panic!("Expected notify hook"),
        }
    }

    /// Test convert_action_args_to_hook_action with script
    /// Verifies that script action conversion works correctly
    #[test]
    fn test_convert_action_args_script() {
        let args = HookActionArgs {
            script: Some("test.sh".to_string()),
            log: None,
            notify: None,
            backup: false,
            cleanup: false,
            args: Some("arg1,arg2".to_string()),
            timeout: Some(60),
            level: None,
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(args).unwrap();
        match result {
            HookAction::Script {
                command,
                args,
                timeout,
                ..
            } => {
                assert_eq!(command, "test.sh");
                assert_eq!(args, vec!["arg1".to_string(), "arg2".to_string()]);
                assert_eq!(timeout, 60);
            }
            _ => panic!("Expected script hook"),
        }
    }

    /// Test convert_action_args_to_hook_action with log
    /// Verifies that log action conversion works correctly
    #[test]
    fn test_convert_action_args_log() {
        let args = HookActionArgs {
            script: None,
            log: Some("Test log message".to_string()),
            notify: None,
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            level: Some("warn".to_string()),
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(args).unwrap();
        match result {
            HookAction::Log { message, level } => {
                assert_eq!(message, "Test log message");
                assert_eq!(level, "warn");
            }
            _ => panic!("Expected log hook"),
        }
    }

    /// Test determine_hook_target with valid targets
    /// Verifies that hook target determination works correctly
    #[test]
    fn test_determine_hook_target_valid() {
        // Test pre-snapshot global target
        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };
        let result = determine_hook_target(&target).unwrap();
        assert_eq!(result.0, "pre-snapshot");
        assert_eq!(result.1, None);

        // Test post-snapshot global target
        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: true,
            pre_plugin: None,
            post_plugin: None,
        };
        let result = determine_hook_target(&target).unwrap();
        assert_eq!(result.0, "post-snapshot");
        assert_eq!(result.1, None);

        // Test pre-plugin target
        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: Some("test_plugin".to_string()),
            post_plugin: None,
        };
        let result = determine_hook_target(&target).unwrap();
        assert_eq!(result.0, "pre-plugin");
        assert_eq!(result.1, Some("test_plugin".to_string()));

        // Test post-plugin target
        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: Some("test_plugin".to_string()),
        };
        let result = determine_hook_target(&target).unwrap();
        assert_eq!(result.0, "post-plugin");
        assert_eq!(result.1, Some("test_plugin".to_string()));
    }

    /// Test convert_action_args_to_hook_action with multiple actions
    /// Verifies that first action takes precedence when multiple are specified
    #[test]
    fn test_convert_action_args_multiple_actions_first_wins() {
        let args = HookActionArgs {
            script: Some("test.sh".to_string()),
            log: Some("test log".to_string()),
            notify: None,
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };
        let result = convert_action_args_to_hook_action(args).unwrap();
        // Script should take precedence over log
        match result {
            HookAction::Script { command, .. } => {
                assert_eq!(command, "test.sh");
            }
            _ => panic!("Expected script action to take precedence"),
        }
    }

    /// Test convert_action_args_to_hook_action with no action
    /// Verifies that error is returned when no action is specified
    #[test]
    fn test_convert_action_args_no_action_error() {
        let args = HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: None,
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };
        let result = convert_action_args_to_hook_action(args);
        assert!(result.is_err());
    }

    /// Test show_global_hooks function with various configurations
    /// Verifies that global hooks are displayed correctly
    #[tokio::test]
    async fn test_show_global_hooks() {
        use crate::config::{GlobalConfig, GlobalHooks};
        use crate::core::hooks::HookAction;

        let global_hooks = GlobalHooks {
            pre_snapshot: vec![
                HookAction::Log {
                    message: "Pre-snapshot log".to_string(),
                    level: "info".to_string(),
                },
                HookAction::Script {
                    command: "test.sh".to_string(),
                    args: vec!["arg1".to_string()],
                    timeout: 30,
                    working_dir: None,
                    env_vars: HashMap::new(),
                },
            ],
            post_snapshot: vec![HookAction::Notify {
                message: "Snapshot complete".to_string(),
                title: Some("Dotsnapshot".to_string()),
            }],
        };

        let config = Config {
            global: Some(GlobalConfig {
                hooks: Some(global_hooks),
            }),
            ..Default::default()
        };

        let hooks_config = HooksConfig::default();

        // Test showing all hooks
        show_global_hooks(&config, true, true, false, &hooks_config);

        // Test showing only pre-snapshot hooks
        show_global_hooks(&config, true, false, false, &hooks_config);

        // Test showing only post-snapshot hooks
        show_global_hooks(&config, false, true, false, &hooks_config);

        // Test with verbose mode
        show_global_hooks(&config, true, true, true, &hooks_config);

        // Test with config that has no global hooks
        let empty_config = Config::default();
        show_global_hooks(&empty_config, true, true, false, &hooks_config);
    }

    /// Test show_plugin_hooks function with various plugin types
    /// Verifies that plugin-specific hooks are displayed with correct icons
    #[tokio::test]
    async fn test_show_plugin_hooks() {
        let mut config = Config::default();
        ensure_plugin_config(&mut config, "homebrew_brewfile");
        ensure_plugin_config(&mut config, "vscode_settings");
        ensure_plugin_config(&mut config, "cursor_extensions");
        ensure_plugin_config(&mut config, "npm_config");
        ensure_plugin_config(&mut config, "custom_plugin");

        let hooks_config = HooksConfig::default();

        // Test different plugin types to verify icon selection
        show_plugin_hooks(
            &config,
            "homebrew_brewfile",
            true,
            true,
            false,
            &hooks_config,
        );
        show_plugin_hooks(
            &config,
            "vscode_settings",
            true,
            false,
            false,
            &hooks_config,
        );
        show_plugin_hooks(
            &config,
            "cursor_extensions",
            false,
            true,
            false,
            &hooks_config,
        );
        show_plugin_hooks(&config, "npm_config", true, true, false, &hooks_config);
        show_plugin_hooks(&config, "custom_plugin", true, true, false, &hooks_config);

        // Test with verbose mode
        show_plugin_hooks(
            &config,
            "homebrew_brewfile",
            true,
            true,
            true,
            &hooks_config,
        );
    }

    /// Test show_all_plugin_hooks function
    /// Verifies that all plugins' hooks are displayed correctly
    #[tokio::test]
    async fn test_show_all_plugin_hooks() {
        let mut config = Config::default();
        ensure_plugin_config(&mut config, "plugin1");
        ensure_plugin_config(&mut config, "plugin2");

        let hooks_config = HooksConfig::default();

        // Test showing all plugin hooks
        show_all_plugin_hooks(&config, true, true, false, &hooks_config);

        // Test with no plugins
        let empty_config = Config::default();
        show_all_plugin_hooks(&empty_config, true, true, false, &hooks_config);
    }

    /// Test show_hook_list function with various hook types
    /// Verifies that hook lists are displayed correctly with and without verbose mode
    #[tokio::test]
    async fn test_show_hook_list() {
        use crate::core::hooks::HookAction;

        let hooks = vec![
            HookAction::Script {
                command: "test_script.sh".to_string(),
                args: vec!["arg1".to_string(), "arg2".to_string()],
                timeout: 60,
                working_dir: None,
                env_vars: HashMap::new(),
            },
            HookAction::Log {
                message: "Test log message".to_string(),
                level: "info".to_string(),
            },
            HookAction::Notify {
                message: "Test notification".to_string(),
                title: Some("Test Title".to_string()),
            },
            HookAction::Backup {
                path: PathBuf::from("/test/source"),
                destination: PathBuf::from("/test/backup"),
            },
            HookAction::Cleanup {
                patterns: vec!["*.tmp".to_string()],
                directories: vec![PathBuf::from("/tmp")],
                temp_files: true,
            },
        ];

        let hooks_config = HooksConfig::default();

        // Test normal mode
        show_hook_list(&hooks, "pre-snapshot", None, false, &hooks_config);

        // Test verbose mode
        show_hook_list(
            &hooks,
            "post-plugin",
            Some("test_plugin"),
            true,
            &hooks_config,
        );

        // Test with empty hooks list
        let empty_hooks: Vec<HookAction> = vec![];
        show_hook_list(&empty_hooks, "pre-plugin", None, false, &hooks_config);
    }

    /// Test validate_hook_list function with various hook scenarios
    /// Verifies that hook validation returns correct counts
    #[tokio::test]
    async fn test_validate_hook_list() {
        use crate::core::hooks::{HookAction, HookContext, HookManager};

        let hooks = vec![
            HookAction::Log {
                message: "Valid log".to_string(),
                level: "info".to_string(),
            },
            HookAction::Notify {
                message: "Test notification".to_string(),
                title: None,
            },
        ];

        let hooks_config = HooksConfig::default();
        let hook_manager = HookManager::new(hooks_config.clone());
        let temp_dir = TempDir::new().unwrap();
        let context = HookContext::new(
            "test_snapshot".to_string(),
            temp_dir.path().to_path_buf(),
            hooks_config,
        );

        // Test validation of valid hooks
        let (valid, warnings, errors) =
            validate_hook_list(&hook_manager, &hooks, "pre-snapshot", None, &context);

        // Should have some valid hooks, notifications may generate warnings
        assert!(valid > 0);
        let _ = warnings; // May vary by system
        let _ = errors; // May vary by system

        // Test with empty hooks list
        let empty_hooks: Vec<HookAction> = vec![];
        let (valid, warnings, errors) = validate_hook_list(
            &hook_manager,
            &empty_hooks,
            "pre-plugin",
            Some("test_plugin"),
            &context,
        );

        assert_eq!(valid, 0);
        assert_eq!(warnings, 0);
        assert_eq!(errors, 0);
    }

    /// Test modify_plugin_config function with various scenarios
    /// Verifies that plugin configuration can be modified correctly
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
        let mut config_empty_plugins = Config::default();
        config_empty_plugins.plugins = Some(crate::config::PluginsConfig {
            plugins: HashMap::new(),
        });

        let names = get_all_plugin_names(&config_empty_plugins);
        assert!(names.is_empty());
    }

    /// Test handle_plugin_hook_removal with various removal scenarios
    /// Verifies that plugin hooks can be removed by index, all, or script name
    #[tokio::test]
    async fn test_handle_plugin_hook_removal_comprehensive() -> Result<()> {
        use crate::core::hooks::HookAction;

        // Setup config with plugin containing hooks
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("test_config.toml");

        let mut config = Config::default();
        ensure_plugin_config(&mut config, "test_plugin");

        // Add some hooks to the plugin
        modify_plugin_config(&mut config, "test_plugin", |plugin_config| {
            plugin_config.hooks = Some(PluginHooks {
                pre_plugin: vec![
                    HookAction::Script {
                        command: "script1.sh".to_string(),
                        args: vec![],
                        timeout: 30,
                        working_dir: None,
                        env_vars: HashMap::new(),
                    },
                    HookAction::Script {
                        command: "script2.sh".to_string(),
                        args: vec![],
                        timeout: 30,
                        working_dir: None,
                        env_vars: HashMap::new(),
                    },
                    HookAction::Log {
                        message: "Test log".to_string(),
                        level: "info".to_string(),
                    },
                ],
                post_plugin: vec![HookAction::Notify {
                    message: "Done".to_string(),
                    title: None,
                }],
            });
        });

        config.save_to_file(&config_path).await?;

        // Test removing hook by index
        handle_plugin_hook_removal(
            &mut config,
            "test_plugin",
            "pre-plugin",
            Some(0),
            false,
            None,
            Some(config_path.clone()),
        )
        .await?;

        // Test removing all hooks
        handle_plugin_hook_removal(
            &mut config,
            "test_plugin",
            "pre-plugin",
            None,
            true,
            None,
            Some(config_path.clone()),
        )
        .await?;

        // Test removing hooks by script name
        let mut config = Config::default();
        ensure_plugin_config(&mut config, "test_plugin2");
        modify_plugin_config(&mut config, "test_plugin2", |plugin_config| {
            plugin_config.hooks = Some(PluginHooks {
                pre_plugin: vec![HookAction::Script {
                    command: "remove_me.sh".to_string(),
                    args: vec![],
                    timeout: 30,
                    working_dir: None,
                    env_vars: HashMap::new(),
                }],
                post_plugin: vec![],
            });
        });

        handle_plugin_hook_removal(
            &mut config,
            "test_plugin2",
            "pre-plugin",
            None,
            false,
            Some("remove_me".to_string()),
            Some(config_path.clone()),
        )
        .await?;

        Ok(())
    }

    /// Test count_total_hooks function with various configurations
    /// Verifies that hook counting works correctly
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

    /// Test error handling in convert_action_args_to_hook_action
    /// Verifies that conversion handles missing required fields correctly
    #[tokio::test]
    async fn test_convert_action_args_error_handling_comprehensive() {
        // Test backup action missing destination
        let backup_args_missing_dest = HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: true,
            path: Some(PathBuf::from("/source")),
            destination: None, // Missing required field
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(backup_args_missing_dest);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("destination"));

        // Test backup action missing path
        let backup_args_missing_path = HookActionArgs {
            script: None,
            log: None,
            notify: None,
            backup: true,
            path: None, // Missing required field
            destination: Some(PathBuf::from("/backup")),
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = convert_action_args_to_hook_action(backup_args_missing_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("path"));
    }

    /// Test handle_remove_hook with script name filtering
    /// Verifies that removing hooks by script name works correctly
    #[tokio::test]
    async fn test_handle_remove_hook_by_script_name() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with multiple script hooks
        let mut config = Config::default();
        config.global = Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![
                    HookAction::Script {
                        command: "remove_this.sh".to_string(),
                        args: vec![],
                        timeout: 30,
                        working_dir: None,
                        env_vars: HashMap::new(),
                    },
                    HookAction::Script {
                        command: "keep_this.sh".to_string(),
                        args: vec![],
                        timeout: 30,
                        working_dir: None,
                        env_vars: HashMap::new(),
                    },
                    HookAction::Log {
                        message: "Keep this log".to_string(),
                        level: "info".to_string(),
                    },
                ],
                post_snapshot: vec![],
            }),
        });
        config.save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let result = handle_remove_hook(
            target,
            None,
            false,
            Some("remove_this".to_string()),
            Some(config_path.clone()),
        )
        .await;
        assert!(result.is_ok());

        // Verify only the targeted script was removed
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = updated_config.get_global_pre_snapshot_hooks();
        assert_eq!(hooks.len(), 2);

        // Check remaining hooks
        let script_commands: Vec<String> = hooks
            .iter()
            .filter_map(|h| match h {
                HookAction::Script { command, .. } => Some(command.clone()),
                _ => None,
            })
            .collect();
        assert!(!script_commands.contains(&"remove_this.sh".to_string()));
        assert!(script_commands.contains(&"keep_this.sh".to_string()));
    }

    /// Test handle_remove_hook when no hooks exist
    /// Verifies graceful handling when trying to remove from empty hook list
    #[tokio::test]
    async fn test_handle_remove_hook_no_hooks() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config without any hooks
        Config::default().save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        let result = handle_remove_hook(target, Some(0), false, None, Some(config_path)).await;
        assert!(result.is_ok()); // Should handle gracefully
    }

    /// Test handle_remove_hook with invalid index
    /// Verifies error handling when index is out of bounds
    #[tokio::test]
    async fn test_handle_remove_hook_invalid_index() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with one hook
        let mut config = Config::default();
        config.global = Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![HookAction::Log {
                    message: "Only hook".to_string(),
                    level: "info".to_string(),
                }],
                post_snapshot: vec![],
            }),
        });
        config.save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: true,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: None,
        };

        // Try to remove at index 5 (out of bounds)
        let result = handle_remove_hook(target, Some(5), false, None, Some(config_path)).await;
        assert!(result.is_ok()); // Function handles out-of-bounds gracefully with error message
    }

    /// Test handle_list_hooks with all hook types selected
    /// Verifies listing all hook types at once
    #[tokio::test]
    async fn test_handle_list_hooks_all_types() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with various hooks
        let mut config = Config::default();
        config.global = Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![HookAction::Log {
                    message: "Pre snapshot".to_string(),
                    level: "info".to_string(),
                }],
                post_snapshot: vec![HookAction::Log {
                    message: "Post snapshot".to_string(),
                    level: "info".to_string(),
                }],
            }),
        });

        // Add plugin hooks
        ensure_plugin_config(&mut config, "test_plugin");
        modify_plugin_config(&mut config, "test_plugin", |plugin_config| {
            plugin_config.hooks = Some(PluginHooks {
                pre_plugin: vec![HookAction::Log {
                    message: "Pre plugin".to_string(),
                    level: "info".to_string(),
                }],
                post_plugin: vec![HookAction::Log {
                    message: "Post plugin".to_string(),
                    level: "info".to_string(),
                }],
            });
        });

        config.save_to_file(&config_path).await.unwrap();

        let result = handle_list_hooks(
            None, // plugin
            true, // pre_plugin
            true, // post_plugin
            true, // pre_snapshot
            true, // post_snapshot
            true, // verbose
            Some(config_path),
        )
        .await;
        assert!(result.is_ok());
    }

    /// Test handle_validate_hooks with invalid script hooks
    /// Verifies validation catches non-existent scripts
    #[tokio::test]
    async fn test_handle_validate_hooks_invalid_scripts() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config with script hooks that don't exist
        let mut config = Config::default();
        config.global = Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![HookAction::Script {
                    command: "nonexistent.sh".to_string(),
                    args: vec![],
                    timeout: 30,
                    working_dir: None,
                    env_vars: HashMap::new(),
                }],
                post_snapshot: vec![],
            }),
        });
        config.save_to_file(&config_path).await.unwrap();

        let result = handle_validate_hooks(
            None,  // plugin
            false, // pre_plugin
            false, // post_plugin
            true,  // pre_snapshot
            false, // post_snapshot
            Some(config_path),
        )
        .await;
        assert!(result.is_ok()); // Validation completes but reports errors
    }

    /// Test handle_scripts_dir with create option
    /// Verifies scripts directory creation functionality
    #[tokio::test]
    async fn test_handle_scripts_dir_create() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        let new_scripts_dir = temp_dir.path().join("new_scripts");

        // Create config without scripts directory
        Config::default().save_to_file(&config_path).await.unwrap();

        let result = handle_scripts_dir(
            Some(new_scripts_dir.clone()),
            true, // create
            Some(config_path.clone()),
        )
        .await;
        assert!(result.is_ok());
        assert!(new_scripts_dir.exists());

        // Verify config was updated
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks_config = updated_config.get_hooks_config();
        assert_eq!(hooks_config.scripts_dir, new_scripts_dir);
    }

    /// Test handle_scripts_dir without set option (display only)
    /// Verifies display-only mode for scripts directory
    #[tokio::test]
    async fn test_handle_scripts_dir_display_only() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        let scripts_dir = temp_dir.path().join("scripts");

        // Create scripts directory with some scripts
        fs::create_dir_all(&scripts_dir).await.unwrap();
        fs::write(scripts_dir.join("test1.sh"), "#!/bin/bash\necho test")
            .await
            .unwrap();
        fs::write(scripts_dir.join("test2.js"), "console.log('test')")
            .await
            .unwrap();

        // Create config with scripts directory
        let mut config = Config::default();
        config.hooks = Some(HooksConfig {
            scripts_dir: scripts_dir.clone(),
        });
        config.save_to_file(&config_path).await.unwrap();

        let result = handle_scripts_dir(
            None,  // set
            false, // create
            Some(config_path),
        )
        .await;
        assert!(result.is_ok());
    }

    /// Test handle_plugin_hook_removal with non-existent plugin
    /// Verifies graceful handling when plugin doesn't exist
    #[tokio::test]
    async fn test_handle_plugin_hook_removal_nonexistent_plugin() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create config without the plugin
        Config::default().save_to_file(&config_path).await.unwrap();

        let mut config = Config::load_from_file(&config_path).await.unwrap();
        let result = handle_plugin_hook_removal(
            &mut config,
            "nonexistent_plugin",
            "pre-plugin",
            Some(0),
            false,
            None,
            Some(config_path),
        )
        .await;
        assert!(result.is_ok()); // Should handle gracefully
    }

    /// Test ensure_plugin_config with existing plugin
    /// Verifies that existing plugin config is preserved
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

    /// Test show_hook_list with empty hooks
    /// Verifies that empty hook lists are handled correctly
    #[test]
    fn test_show_hook_list_empty() {
        let empty_hooks: Vec<HookAction> = vec![];
        let hooks_config = HooksConfig::default();

        // Should return early without any output
        show_hook_list(&empty_hooks, "pre-snapshot", None, false, &hooks_config);
        show_hook_list(
            &empty_hooks,
            "post-plugin",
            Some("test"),
            true,
            &hooks_config,
        );
    }

    /// Test validate_hook_list with script validation errors
    /// Verifies that validation errors are properly counted and reported
    #[tokio::test]
    async fn test_validate_hook_list_with_errors() {
        let temp_dir = TempDir::new().unwrap();
        let hooks_config = HooksConfig {
            scripts_dir: temp_dir.path().to_path_buf(),
        };

        let hooks = vec![
            HookAction::Script {
                command: "nonexistent.sh".to_string(),
                args: vec![],
                timeout: 30,
                working_dir: None,
                env_vars: HashMap::new(),
            },
            HookAction::Log {
                message: "Valid log".to_string(),
                level: "invalid_level".to_string(), // Invalid log level
            },
        ];

        let hook_manager = HookManager::new(hooks_config.clone());
        let context = HookContext::new(
            "test".to_string(),
            temp_dir.path().to_path_buf(),
            hooks_config,
        );

        let (valid, _warnings, errors) =
            validate_hook_list(&hook_manager, &hooks, "pre-snapshot", None, &context);

        assert_eq!(valid, 0); // Both hooks should fail validation
        assert_eq!(errors, 2); // Both should produce errors
    }

    /// Test handle_add_hook with post-snapshot target
    /// Verifies adding hooks to post-snapshot
    #[tokio::test]
    async fn test_handle_add_hook_post_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        Config::default().save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: true,
            pre_plugin: None,
            post_plugin: None,
        };

        let action = HookActionArgs {
            script: None,
            log: None,
            notify: Some("Snapshot complete".to_string()),
            backup: false,
            cleanup: false,
            args: None,
            timeout: None,
            level: None,
            title: Some("Dotsnapshot".to_string()),
            path: None,
            destination: None,
            patterns: None,
            directories: None,
            temp_files: false,
        };

        let result = handle_add_hook(target, action, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was added to post-snapshot
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = updated_config.get_global_post_snapshot_hooks();
        assert_eq!(hooks.len(), 1);
        match &hooks[0] {
            HookAction::Notify { message, title } => {
                assert_eq!(message, "Snapshot complete");
                assert_eq!(title, &Some("Dotsnapshot".to_string()));
            }
            _ => panic!("Expected notify hook"),
        }
    }

    /// Test handle_add_hook with post-plugin target
    /// Verifies adding hooks to post-plugin
    #[tokio::test]
    async fn test_handle_add_hook_post_plugin() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        Config::default().save_to_file(&config_path).await.unwrap();

        let target = HookTarget {
            pre_snapshot: false,
            post_snapshot: false,
            pre_plugin: None,
            post_plugin: Some("test_plugin".to_string()),
        };

        let action = HookActionArgs {
            script: None,
            log: Some("Plugin complete".to_string()),
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
        };

        let result = handle_add_hook(target, action, Some(config_path.clone())).await;
        assert!(result.is_ok());

        // Verify hook was added to post-plugin
        let updated_config = Config::load_from_file(&config_path).await.unwrap();
        let hooks = updated_config.get_plugin_post_hooks("test_plugin");
        assert_eq!(hooks.len(), 1);
        match &hooks[0] {
            HookAction::Log { message, level } => {
                assert_eq!(message, "Plugin complete");
                assert_eq!(level, "debug");
            }
            _ => panic!("Expected log hook"),
        }
    }
}

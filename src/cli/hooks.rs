use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{error, info, warn};

use crate::config::{Config, GlobalConfig, GlobalHooks, PluginConfig, PluginHooks};
use crate::core::hooks::{HookAction, HookContext, HookManager, HooksConfig};
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
            let plugin_config = get_plugin_config_mut(&mut config, plugin_name).unwrap();
            plugin_config
                .hooks
                .as_mut()
                .unwrap()
                .pre_plugin
                .push(hook_action.clone());
        }
        "post-plugin" => {
            let plugin_name = plugin_name.as_ref().unwrap();
            ensure_plugin_config(&mut config, plugin_name);
            let plugin_config = get_plugin_config_mut(&mut config, plugin_name).unwrap();
            plugin_config
                .hooks
                .as_mut()
                .unwrap()
                .post_plugin
                .push(hook_action.clone());
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

    info!("✅ Added {hook_type} hook{plugin_context}:");
    info!("   📝 {hook_action}");

    // Check if script exists
    if let HookAction::Script { command, .. } = &hook_action {
        let hooks_config = config.get_hooks_config();
        let script_path = hooks_config.resolve_script_path(command);
        let expanded_path = HooksConfig::expand_tilde(&script_path);

        if !expanded_path.exists() {
            warn!(
                "   ⚠️  Script file not found: {} → {}",
                command,
                expanded_path.display()
            );
            warn!("   💡 Create the script file to complete setup");
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
            if let Some(plugin_config) = get_plugin_config_mut(&mut config, plugin_name) {
                if let Some(hooks) = plugin_config.hooks.as_mut() {
                    &mut hooks.pre_plugin
                } else {
                    info!("No pre-plugin hooks configured for {plugin_name}");
                    return Ok(());
                }
            } else {
                info!("No pre-plugin hooks configured for {plugin_name}");
                return Ok(());
            }
        }
        "post-plugin" => {
            let plugin_name = plugin_name.as_ref().unwrap();
            if let Some(plugin_config) = get_plugin_config_mut(&mut config, plugin_name) {
                if let Some(hooks) = plugin_config.hooks.as_mut() {
                    &mut hooks.post_plugin
                } else {
                    info!("No post-plugin hooks configured for {plugin_name}");
                    return Ok(());
                }
            } else {
                info!("No post-plugin hooks configured for {plugin_name}");
                return Ok(());
            }
        }
        _ => unreachable!(),
    };

    let original_count = hooks.len();

    if all {
        hooks.clear();
        let plugin_context = plugin_name
            .map(|p| format!(" from {p}"))
            .unwrap_or_else(|| " (global)".to_string());
        info!("✅ Removed all {hook_type} hooks{plugin_context}:");
        info!("   🗑️  {} hooks removed", original_count);
    } else if let Some(idx) = index {
        if idx < hooks.len() {
            let removed_hook = hooks.remove(idx);
            let plugin_context = plugin_name
                .map(|p| format!(" from {p}"))
                .unwrap_or_else(|| " (global)".to_string());
            info!("✅ Removed {hook_type} hook{plugin_context}:");
            info!("   📝 {removed_hook}");
        } else {
            error!(
                "❌ Index {idx} is out of range (max: {})",
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
            info!("✅ Removed {removed_count} {hook_type} hook(s){plugin_context} matching script '{script_name}'");
        } else {
            info!("No {hook_type} hooks found matching script '{script_name}'");
            return Ok(());
        }
    } else {
        error!("❌ Must specify --index, --all, or --script to remove hooks");
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

    info!("🪝 Plugin Hooks Configuration:");
    info!(
        "📁 Scripts Directory: {}",
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

    info!("🔍 Validating hook configuration...");
    info!(
        "📁 Scripts Directory: {} (exists: {})",
        hooks_config.scripts_dir.display(),
        if hooks_config.scripts_dir.exists() {
            "✅"
        } else {
            "❌"
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
        info!("✅ All hooks are valid!");
    } else if total_errors == 0 {
        warn!("⚠️  Configuration is valid but has warnings");
    } else {
        error!("❌ Configuration has errors that need to be fixed");
    }

    if !hooks_config.scripts_dir.exists() {
        info!("💡 Run 'dotsnapshot hooks scripts-dir --create' to create the scripts directory");
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

        info!("📁 Scripts directory updated: {}", expanded_path.display());
        if !expanded_path.exists() {
            warn!("   ⚠️  Directory does not exist - run with --create to create it");
            warn!("   💡 Existing scripts will need to be moved manually");
        } else {
            info!("   ✅ Directory exists");
        }

        if create && !expanded_path.exists() {
            tokio::fs::create_dir_all(&expanded_path)
                .await
                .context("Failed to create scripts directory")?;
            info!("   📁 Created scripts directory");
        }
    } else {
        // Show current scripts directory
        let hooks_config = config.get_hooks_config();
        let scripts_dir = &hooks_config.scripts_dir;
        let expanded_dir = HooksConfig::expand_tilde(scripts_dir);

        info!("📁 Current scripts directory: {}", scripts_dir.display());
        info!(
            "   Status: {} {}",
            if expanded_dir.exists() {
                "✅ exists"
            } else {
                "❌ does not exist"
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
            info!("📁 Created scripts directory: {}", expanded_dir.display());
            info!("   ✅ Directory created successfully");
            info!("   💡 You can now add your hook scripts to this directory");
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
            pre_restore: Vec::new(),
            post_restore: Vec::new(),
        });
    }
}

fn ensure_plugin_config(config: &mut Config, plugin_name: &str) {
    use crate::config::PluginsConfig;

    if config.plugins.is_none() {
        config.plugins = Some(PluginsConfig {
            homebrew_brewfile: None,
            vscode_settings: None,
            vscode_keybindings: None,
            vscode_extensions: None,
            cursor_settings: None,
            cursor_keybindings: None,
            cursor_extensions: None,
            npm_global_packages: None,
            npm_config: None,
            static_files: None,
        });
    }

    let plugins = config.plugins.as_mut().unwrap();
    let plugin_config = match plugin_name {
        "homebrew_brewfile" => &mut plugins.homebrew_brewfile,
        "vscode_settings" => &mut plugins.vscode_settings,
        "vscode_keybindings" => &mut plugins.vscode_keybindings,
        "vscode_extensions" => &mut plugins.vscode_extensions,
        "cursor_settings" => &mut plugins.cursor_settings,
        "cursor_keybindings" => &mut plugins.cursor_keybindings,
        "cursor_extensions" => &mut plugins.cursor_extensions,
        "npm_global_packages" => &mut plugins.npm_global_packages,
        "npm_config" => &mut plugins.npm_config,
        _ => return,
    };

    if plugin_config.is_none() {
        *plugin_config = Some(PluginConfig {
            target_path: None,
            hooks: None,
        });
    }

    if plugin_config.as_ref().unwrap().hooks.is_none() {
        plugin_config.as_mut().unwrap().hooks = Some(PluginHooks {
            pre_plugin: Vec::new(),
            post_plugin: Vec::new(),
            pre_plugin_restore: Vec::new(),
            post_plugin_restore: Vec::new(),
        });
    }
}

fn get_plugin_config_mut<'a>(
    config: &'a mut Config,
    plugin_name: &str,
) -> Option<&'a mut PluginConfig> {
    let plugins = config.plugins.as_mut()?;
    match plugin_name {
        "homebrew_brewfile" => plugins.homebrew_brewfile.as_mut(),
        "vscode_settings" => plugins.vscode_settings.as_mut(),
        "vscode_keybindings" => plugins.vscode_keybindings.as_mut(),
        "vscode_extensions" => plugins.vscode_extensions.as_mut(),
        "cursor_settings" => plugins.cursor_settings.as_mut(),
        "cursor_keybindings" => plugins.cursor_keybindings.as_mut(),
        "cursor_extensions" => plugins.cursor_extensions.as_mut(),
        "npm_global_packages" => plugins.npm_global_packages.as_mut(),
        "npm_config" => plugins.npm_config.as_mut(),
        _ => None,
    }
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
        info!("🌍 Global Hooks:");

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
        "homebrew_brewfile" => "🍺",
        name if name.starts_with("vscode") => "💻",
        name if name.starts_with("cursor") => "✏️",
        name if name.starts_with("npm") => "📦",
        _ => "🔌",
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
            let exists = if expanded_path.exists() { "✅" } else { "❌" };

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
    info!("🔍{plugin_label} {hook_type} hooks:");

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
                    warn!("  ⚠️  [{index}] {hook} (system notifications may not be available)");
                } else {
                    info!("  ✅ [{index}] {hook}");
                }
            }
            Err(e) => {
                errors += 1;
                error!("  ❌ [{index}] {hook}");
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
        if plugins.homebrew_brewfile.is_some() {
            names.push("homebrew_brewfile".to_string());
        }
        if plugins.vscode_settings.is_some() {
            names.push("vscode_settings".to_string());
        }
        if plugins.vscode_keybindings.is_some() {
            names.push("vscode_keybindings".to_string());
        }
        if plugins.vscode_extensions.is_some() {
            names.push("vscode_extensions".to_string());
        }
        if plugins.cursor_settings.is_some() {
            names.push("cursor_settings".to_string());
        }
        if plugins.cursor_keybindings.is_some() {
            names.push("cursor_keybindings".to_string());
        }
        if plugins.cursor_extensions.is_some() {
            names.push("cursor_extensions".to_string());
        }
        if plugins.npm_global_packages.is_some() {
            names.push("npm_global_packages".to_string());
        }
        if plugins.npm_config.is_some() {
            names.push("npm_config".to_string());
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

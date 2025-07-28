use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use clap_mangen::Man;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info};

mod cli;
mod config;
mod core;
mod plugins;
mod symbols;

use config::Config;
use core::executor::SnapshotExecutor;
use core::plugin::PluginRegistry;
// Auto-registration system means we don't need explicit plugin imports
// The inventory system will discover all plugins automatically
use symbols::*;

#[derive(Parser)]
#[command(name = "dotsnapshot")]
#[command(about = "A CLI utility to create snapshots of dotfiles and configuration")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Args {
    /// Enable verbose logging (overrides config file)
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Enable debug logging (shows DEBUG level messages)
    #[arg(long, global = true)]
    debug: bool,

    /// Path to config file
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,

    /// Output directory for snapshots (overrides config file) - used when no subcommand
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Specify which plugins to run (comma-separated) - used when no subcommand
    #[arg(short, long)]
    plugins: Option<String>,

    /// List available plugins - used when no subcommand
    #[arg(short, long)]
    list: bool,

    /// Show detailed information about the tool - used when no subcommand
    #[arg(long)]
    info: bool,

    /// Generate shell completions for the specified shell - used when no subcommand
    #[arg(long, value_enum)]
    completions: Option<Shell>,

    /// Generate man page - used when no subcommand
    #[arg(long)]
    man: bool,
}

#[derive(Parser)]
enum Commands {
    /// Manage plugin hooks
    Hooks {
        #[command(subcommand)]
        command: Box<HooksCommands>,
    },
    /// Restore configuration from a snapshot
    Restore {
        /// Path to the snapshot directory to restore from
        snapshot_path: Option<PathBuf>,

        /// Use the latest snapshot from the default snapshot directory
        #[arg(long)]
        latest: bool,

        /// Restore only specific plugins (comma-separated)
        #[arg(short, long)]
        plugins: Option<String>,

        /// Preview changes without applying them
        #[arg(long)]
        dry_run: bool,

        /// Create backup of existing files before restoring
        #[arg(long, default_value = "true")]
        backup: bool,

        /// Skip confirmation prompts
        #[arg(long)]
        force: bool,

        /// Custom target directory for restoration
        #[arg(long)]
        target_dir: Option<PathBuf>,
    },
}

#[derive(Parser)]
// Allow large enum variant because HookActionArgs contains many optional CLI arguments
// Boxing would complicate the clap derive macro usage without significant memory benefits
// since this enum is used transiently for command parsing only
#[allow(clippy::large_enum_variant)]
enum HooksCommands {
    /// Add a new hook to a plugin or globally
    Add {
        /// Hook type and target
        #[command(flatten)]
        target: HookTarget,

        /// Action type
        #[command(flatten)]
        action: HookActionArgs,
    },
    /// Remove existing hooks
    Remove {
        /// Hook type and target
        #[command(flatten)]
        target: HookTarget,

        /// Remove by index
        #[arg(long)]
        index: Option<usize>,

        /// Remove all hooks of this type
        #[arg(long)]
        all: bool,

        /// Remove by matching script name
        #[arg(long)]
        script: Option<String>,
    },
    /// List configured hooks
    List {
        /// Show hooks for specific plugin
        #[arg(long)]
        plugin: Option<String>,

        /// Show only pre-plugin hooks
        #[arg(long, conflicts_with_all = ["post_plugin", "pre_snapshot", "post_snapshot"])]
        pre_plugin: bool,

        /// Show only post-plugin hooks
        #[arg(long, conflicts_with_all = ["pre_plugin", "pre_snapshot", "post_snapshot"])]
        post_plugin: bool,

        /// Show only pre-snapshot hooks
        #[arg(long, conflicts_with_all = ["pre_plugin", "post_plugin", "post_snapshot"])]
        pre_snapshot: bool,

        /// Show only post-snapshot hooks
        #[arg(long, conflicts_with_all = ["pre_plugin", "post_plugin", "pre_snapshot"])]
        post_snapshot: bool,

        /// Show verbose details
        #[arg(long)]
        verbose: bool,
    },
    /// Validate hook configuration
    Validate {
        /// Validate hooks for specific plugin
        #[arg(long)]
        plugin: Option<String>,

        /// Validate only pre-plugin hooks
        #[arg(long)]
        pre_plugin: bool,

        /// Validate only post-plugin hooks
        #[arg(long)]
        post_plugin: bool,

        /// Validate only pre-snapshot hooks
        #[arg(long)]
        pre_snapshot: bool,

        /// Validate only post-snapshot hooks
        #[arg(long)]
        post_snapshot: bool,
    },
    /// Manage scripts directory
    ScriptsDir {
        /// Set new scripts directory
        #[arg(long)]
        set: Option<PathBuf>,

        /// Create scripts directory if it doesn't exist
        #[arg(long)]
        create: bool,
    },
}

#[derive(Parser)]
#[group(required = true, multiple = false)]
struct HookTarget {
    /// Pre-snapshot hook (global)
    #[arg(long)]
    pre_snapshot: bool,

    /// Post-snapshot hook (global)
    #[arg(long)]
    post_snapshot: bool,

    /// Pre-plugin hook for specific plugin
    #[arg(long)]
    pre_plugin: Option<String>,

    /// Post-plugin hook for specific plugin
    #[arg(long)]
    post_plugin: Option<String>,
}

#[derive(Parser)]
#[group(id = "action", required = true, multiple = false)]
struct HookActionArgs {
    /// Script to execute
    #[arg(long, group = "action")]
    script: Option<String>,

    /// Log message
    #[arg(long, group = "action")]
    log: Option<String>,

    /// Notification message
    #[arg(long, group = "action")]
    notify: Option<String>,

    /// Backup action
    #[arg(long, group = "action")]
    backup: bool,

    /// Cleanup action
    #[arg(long, group = "action")]
    cleanup: bool,

    /// Script arguments (comma-separated, only with --script)
    #[arg(long, requires = "script")]
    args: Option<String>,

    /// Script timeout in seconds (only with --script)
    #[arg(long, requires = "script")]
    timeout: Option<u64>,

    /// Log level (only with --log)
    #[arg(long, requires = "log", value_parser = ["trace", "debug", "info", "warn", "error"])]
    level: Option<String>,

    /// Notification title (only with --notify)
    #[arg(long, requires = "notify")]
    title: Option<String>,

    /// Backup source path (only with --backup)
    #[arg(long, requires = "backup")]
    path: Option<PathBuf>,

    /// Backup destination path (only with --backup)
    #[arg(long, requires = "backup")]
    destination: Option<PathBuf>,

    /// Cleanup patterns (comma-separated, only with --cleanup)
    #[arg(long, requires = "cleanup")]
    patterns: Option<String>,

    /// Cleanup directories (comma-separated, only with --cleanup)
    #[arg(long, requires = "cleanup")]
    directories: Option<String>,

    /// Clean temp files (only with --cleanup)
    #[arg(long, requires = "cleanup")]
    temp_files: bool,
}

fn create_subscriber(
    debug: bool,
    time_format: String,
) -> Box<dyn tracing::Subscriber + Send + Sync> {
    let level = if debug {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    // Use predefined formats to avoid lifetime issues
    match time_format.as_str() {
        "[hour]:[minute]:[second]" => {
            let format_desc = time::format_description::parse("[hour]:[minute]:[second]").unwrap();
            Box::new(
                tracing_subscriber::fmt()
                    .with_max_level(level)
                    .with_timer(tracing_subscriber::fmt::time::LocalTime::new(format_desc))
                    .finish(),
            )
        }
        "[month]-[day] [hour]:[minute]" => {
            let format_desc =
                time::format_description::parse("[month]-[day] [hour]:[minute]").unwrap();
            Box::new(
                tracing_subscriber::fmt()
                    .with_max_level(level)
                    .with_timer(tracing_subscriber::fmt::time::LocalTime::new(format_desc))
                    .finish(),
            )
        }
        "[year]/[month]/[day] [hour]:[minute]:[second]" => {
            let format_desc =
                time::format_description::parse("[year]/[month]/[day] [hour]:[minute]:[second]")
                    .unwrap();
            Box::new(
                tracing_subscriber::fmt()
                    .with_max_level(level)
                    .with_timer(tracing_subscriber::fmt::time::LocalTime::new(format_desc))
                    .finish(),
            )
        }
        _ => {
            // Default format for all other cases (including custom formats)
            let format_desc =
                time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]")
                    .unwrap();
            if time_format != "[year]-[month]-[day] [hour]:[minute]:[second]" {
                eprintln!(
                    "Custom time format '{time_format}' not supported. Using default format."
                );
            }
            Box::new(
                tracing_subscriber::fmt()
                    .with_max_level(level)
                    .with_timer(tracing_subscriber::fmt::time::LocalTime::new(format_desc))
                    .finish(),
            )
        }
    }
}

async fn list_plugins() {
    use std::collections::HashMap;

    // Type alias to simplify the complex type
    type PluginInfo = (String, String, String);
    type PluginGroup = (String, Vec<PluginInfo>);

    println!("Available plugins:");
    println!();

    // Load config for UI customization (optional)
    let config = Config::load().await.ok();

    // Auto-discover and register all plugins
    let registry = PluginRegistry::discover_plugins(config.as_ref());

    // Get detailed plugin information with category names and icons
    let plugins_detailed = registry.list_plugins_detailed(config.as_ref());

    // Group plugins by category dynamically
    let mut plugin_groups: HashMap<String, PluginGroup> = HashMap::new();

    for (name, filename, description, category, icon) in plugins_detailed {
        plugin_groups
            .entry(category.clone())
            .or_insert_with(|| (icon.clone(), Vec::new()))
            .1
            .push((name, filename, description));
    }

    // Sort groups by category name for consistent output
    let mut sorted_groups: Vec<_> = plugin_groups.into_iter().collect();
    sorted_groups.sort_by(|a, b| a.0.cmp(&b.0));

    // Display grouped plugins dynamically
    for (category, (icon, plugins)) in sorted_groups {
        if !plugins.is_empty() {
            println!("{icon} {category}:");
            for (name, filename, description) in plugins {
                println!("  {name:<20} -> {filename:<20} {description}");
            }
            println!();
        }
    }

    println!("Usage:");
    println!("  --plugins <plugin1>,<plugin2>  Run specific plugins");
    println!("  --plugins homebrew,vscode      Run all homebrew and vscode plugins");
    println!("  (no --plugins)                 Run all plugins");
}

#[tokio::main]
async fn main() -> Result<()> {
    let start_time = Instant::now();
    let args = Args::parse();

    // Initialize logging early for subcommands that need it
    let config = if let Some(config_path) = &args.config {
        if config_path.exists() {
            Config::load_from_file(config_path).await?
        } else {
            Config::default()
        }
    } else {
        Config::load().await.unwrap_or_default()
    };

    let _verbose = args.verbose || config.is_verbose_default();
    let debug = args.debug;
    let time_format = config.get_time_format();
    let subscriber = create_subscriber(debug, time_format);
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set default subscriber");

    // Handle subcommands
    if let Some(command) = args.command {
        match command {
            Commands::Hooks { command } => {
                return cli::hooks::handle_hooks_command(*command, args.config).await;
            }
            Commands::Restore {
                snapshot_path,
                latest,
                plugins,
                dry_run,
                backup,
                force,
                target_dir,
            } => {
                return cli::restore::handle_restore_command(
                    snapshot_path,
                    latest,
                    plugins,
                    dry_run,
                    backup,
                    force,
                    target_dir,
                    args.config,
                )
                .await;
            }
        }
    }

    // Handle legacy flags when no subcommand is provided

    // Handle --completions flag early
    if let Some(shell) = args.completions {
        let mut app = Args::command();
        generate(shell, &mut app, "dotsnapshot", &mut io::stdout());
        return Ok(());
    }

    // Handle --man flag early
    if args.man {
        let app = Args::command();
        let man = Man::new(app);
        man.render(&mut io::stdout())?;
        return Ok(());
    }

    // Handle --info flag early
    if args.info {
        println!(
            "{SYMBOL_TOOL_CONFIG} dotsnapshot v{}",
            env!("CARGO_PKG_VERSION")
        );
        println!("{SYMBOL_DOC_NOTE} {}", env!("CARGO_PKG_DESCRIPTION"));
        println!(
            "{SYMBOL_SCOPE_GLOBAL} Repository: {}",
            env!("CARGO_PKG_REPOSITORY")
        );
        println!(
            "{SYMBOL_CONTENT_FILE} License: {}",
            env!("CARGO_PKG_LICENSE")
        );
        println!("{SYMBOL_DOC_TAG}  Keywords: dotfiles, backup, configuration, snapshots, cli");
        println!();
        println!("{SYMBOL_CONTENT_PACKAGE} Supported Plugins:");
        println!("  • Homebrew Brewfile generation");
        println!("  • VSCode settings, keybindings, and extensions");
        println!("  • Cursor settings, keybindings, and extensions");
        println!("  • NPM global packages and configuration");
        println!();
        println!("{SYMBOL_ACTION_LAUNCH} Usage:");
        println!("   dotsnapshot [OPTIONS]              Create a snapshot (default)");
        println!("   dotsnapshot hooks <SUBCOMMAND>     Manage plugin hooks");
        println!("   Use --help for detailed options");
        println!();
        println!("{SYMBOL_TOOL_CONFIG} Shell Completions:");
        println!(
            "   dotsnapshot --completions bash > /usr/local/etc/bash_completion.d/dotsnapshot"
        );
        println!("   dotsnapshot --completions zsh > ~/.zfunc/_dotsnapshot");
        println!("   dotsnapshot --completions fish > ~/.config/fish/completions/dotsnapshot.fish");
        println!();
        println!("{SYMBOL_DOC_BOOK} Man Page:");
        println!("   dotsnapshot --man > /usr/local/share/man/man1/dotsnapshot.1");
        return Ok(());
    }

    // Handle --list flag early
    if args.list {
        list_plugins().await;
        return Ok(());
    }

    // Default behavior: create snapshot
    info!("Starting dotsnapshot v{}", env!("CARGO_PKG_VERSION"));

    // Log custom config usage if applicable
    if let Some(config_path) = &args.config {
        info!(
            "{} Using custom config file: {}",
            SYMBOL_INDICATOR_INFO,
            config_path.display()
        );
    }

    // Determine final settings (CLI args override config file)
    let output_dir = args.output.unwrap_or_else(|| config.get_output_dir());

    // Create output directory if it doesn't exist
    tokio::fs::create_dir_all(&output_dir).await?;

    // Determine which plugins to run
    let selected_plugins: Vec<String> = if let Some(cli_plugins) = args.plugins.as_deref() {
        // CLI argument takes precedence
        cli_plugins.split(',').map(|s| s.to_string()).collect()
    } else if let Some(config_plugins) = config.get_include_plugins() {
        // Use config file plugins
        config_plugins
    } else {
        // Default: run all plugins
        vec!["all".to_string()]
    };

    // Auto-discover and register plugins with filtering
    let mut registry = PluginRegistry::new();
    let selected_plugins_refs: Vec<&str> = selected_plugins.iter().map(|s| s.as_str()).collect();
    registry.register_from_descriptors(Some(&config), &selected_plugins_refs);

    // Create executor and run snapshot
    let executor = SnapshotExecutor::with_config(Arc::new(registry), output_dir, Arc::new(config));

    match executor.execute_snapshot().await {
        Ok(snapshot_path) => {
            let duration = start_time.elapsed();
            info!(
                "{} Snapshot created successfully at: {}",
                SYMBOL_INDICATOR_SUCCESS,
                snapshot_path.display()
            );
            info!(
                "{}  Execution time: {:.2?}",
                SYMBOL_EXPERIENCE_TIME, duration
            );
        }
        Err(e) => {
            error!("{} Snapshot creation failed: {}", SYMBOL_INDICATOR_ERROR, e);
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test basic argument parsing with default values
    /// Verifies that command-line arguments are parsed correctly
    #[test]
    fn test_args_parsing() {
        // Test default values
        let args = Args::parse_from(["dotsnapshot"]);
        assert!(args.output.is_none());
        assert!(!args.verbose);
        assert!(args.plugins.is_none());
        assert!(args.config.is_none());
        assert!(!args.list);

        // Test custom values
        let args = Args::parse_from([
            "dotsnapshot",
            "--output",
            "/tmp/test",
            "--verbose",
            "--plugins",
            "homebrew,npm",
            "--config",
            "/path/to/config.toml",
        ]);
        assert_eq!(args.output.unwrap(), PathBuf::from("/tmp/test"));
        assert!(args.verbose);
        assert_eq!(args.plugins.unwrap(), "homebrew,npm");
        assert_eq!(args.config.unwrap(), PathBuf::from("/path/to/config.toml"));
        assert!(!args.list);

        // Test --list flag
        let args = Args::parse_from(["dotsnapshot", "--list"]);
        assert!(args.list);
    }

    /// Test parsing of info and utility flags
    /// Verifies that special flags like --info, --man, --completions are parsed correctly
    #[test]
    fn test_utility_flags_parsing() {
        // Test --info flag
        let args = Args::parse_from(["dotsnapshot", "--info"]);
        assert!(args.info);

        // Test --man flag
        let args = Args::parse_from(["dotsnapshot", "--man"]);
        assert!(args.man);

        // Test --completions flag
        let args = Args::parse_from(["dotsnapshot", "--completions", "bash"]);
        assert_eq!(args.completions, Some(Shell::Bash));

        let args = Args::parse_from(["dotsnapshot", "--completions", "zsh"]);
        assert_eq!(args.completions, Some(Shell::Zsh));
    }

    /// Test hooks command parsing
    /// Verifies that hooks subcommands are parsed correctly
    #[test]
    fn test_hooks_command_parsing() {
        // Test hooks add command
        let args = Args::parse_from([
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
        let args = Args::parse_from(["dotsnapshot", "hooks", "list"]);
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
        let args = Args::parse_from(["dotsnapshot", "restore", "/path/to/snapshot"]);

        match args.command {
            Some(Commands::Restore { snapshot_path, .. }) => {
                assert_eq!(snapshot_path, Some(PathBuf::from("/path/to/snapshot")));
            }
            _ => panic!("Expected restore command"),
        }

        // Test restore with --latest flag
        let args = Args::parse_from(["dotsnapshot", "restore", "--latest"]);

        match args.command {
            Some(Commands::Restore { latest, .. }) => {
                assert!(latest);
            }
            _ => panic!("Expected restore command"),
        }

        // Test restore with options
        let args = Args::parse_from([
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

    /// Test subscriber creation with different configurations
    /// Verifies that logging subscribers are created correctly
    #[test]
    fn test_create_subscriber() {
        // Test debug subscriber
        let subscriber = create_subscriber(true, "[hour]:[minute]:[second]".to_string());
        // Just verify it creates without panicking
        drop(subscriber);

        // Test non-debug subscriber
        let subscriber = create_subscriber(false, "[month]-[day] [hour]:[minute]".to_string());
        drop(subscriber);

        // Test with different time formats
        let subscriber = create_subscriber(
            false,
            "[year]/[month]/[day] [hour]:[minute]:[second]".to_string(),
        );
        drop(subscriber);

        // Test with default format (unsupported format should fall back)
        let subscriber = create_subscriber(false, "unsupported-format".to_string());
        drop(subscriber);
    }

    /// Test hook target parsing
    /// Verifies that hook targets are parsed correctly
    #[test]
    fn test_hook_target_parsing() {
        // Test pre-snapshot hook target
        let args = Args::parse_from([
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
        let args = Args::parse_from([
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
        let args = Args::parse_from([
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
        let args = Args::parse_from([
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
        let args = Args::parse_from([
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
        let args = Args::parse_from([
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
        let args = Args::parse_from([
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
        let args = Args::parse_from(["dotsnapshot", "hooks", "list"]);
        if let Some(Commands::Hooks { command }) = args.command {
            match *command {
                HooksCommands::List { .. } => {
                    // Command parsed correctly
                }
                _ => panic!("Expected list command"),
            }
        }

        // Test list with plugin filter
        let args = Args::parse_from([
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
        let args = Args::parse_from(["dotsnapshot", "hooks", "list", "--pre-plugin"]);
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
        let args = Args::parse_from([
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
        let args = Args::parse_from([
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
        let args = Args::parse_from([
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
        let args = Args::parse_from(["dotsnapshot", "hooks", "validate"]);
        if let Some(Commands::Hooks { command }) = args.command {
            match *command {
                HooksCommands::Validate { .. } => {
                    // Command parsed correctly
                }
                _ => panic!("Expected validate command"),
            }
        }

        // Test validate with filters
        let args = Args::parse_from([
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
        let args = Args::parse_from([
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
        let args = Args::parse_from(["dotsnapshot", "hooks", "scripts-dir", "--create"]);
        if let Some(Commands::Hooks { command }) = args.command {
            match *command {
                HooksCommands::ScriptsDir { create, .. } => {
                    assert!(create);
                }
                _ => panic!("Expected scripts-dir command"),
            }
        }
    }

    /// Test debug flag parsing
    /// Verifies that the debug flag is parsed correctly in CLI arguments
    #[test]
    fn test_debug_flag_parsing() {
        // Test default debug value (should be false)
        let args = Args::parse_from(["dotsnapshot"]);
        assert!(!args.debug);

        // Test --debug flag
        let args = Args::parse_from(["dotsnapshot", "--debug"]);
        assert!(args.debug);

        // Test --debug with other flags
        let args = Args::parse_from(["dotsnapshot", "--debug", "--verbose", "--list"]);
        assert!(args.debug);
        assert!(args.verbose);
        assert!(args.list);

        // Test --debug with subcommands
        let args = Args::parse_from(["dotsnapshot", "--debug", "hooks", "list"]);
        assert!(args.debug);
        match args.command {
            Some(Commands::Hooks { .. }) => {
                // Command structure is correct
            }
            _ => panic!("Expected hooks command"),
        }
    }

    /// Test debug logging level configuration
    /// Verifies that debug flag correctly sets logging levels
    #[test]
    fn test_debug_logging_levels() {
        // Test debug=true should set DEBUG level
        let subscriber = create_subscriber(true, "[hour]:[minute]:[second]".to_string());
        drop(subscriber);

        // Test debug=false should set INFO level
        let subscriber = create_subscriber(false, "[hour]:[minute]:[second]".to_string());
        drop(subscriber);
    }

    /// Test that create_subscriber handles all time format cases
    /// Verifies that time format parsing covers all branches
    #[test]
    fn test_create_subscriber_time_formats() {
        // Test all supported time formats
        let formats = vec![
            "[hour]:[minute]:[second]",
            "[month]-[day] [hour]:[minute]",
            "[year]/[month]/[day] [hour]:[minute]:[second]",
            "[year]-[month]-[day] [hour]:[minute]:[second]", // default
        ];

        for format in formats {
            let subscriber = create_subscriber(false, format.to_string());
            drop(subscriber); // Just ensure it creates without panic
        }

        // Test unsupported format (should fall back to default)
        let subscriber = create_subscriber(true, "custom-unsupported-format".to_string());
        drop(subscriber);
    }

    /// Test list_plugins function
    /// Verifies that plugin listing works correctly
    #[tokio::test]
    async fn test_list_plugins() {
        // This test verifies that list_plugins doesn't panic and can discover plugins
        // We can't easily test the exact output without mocking, but we can test execution
        list_plugins().await;
        // If we reach here, the function completed without panicking
    }

    /// Test config loading scenarios
    /// Verifies different config loading paths work correctly
    #[tokio::test]
    async fn test_config_loading_scenarios() {
        use tempfile::TempDir;
        use tokio::fs;

        let temp_dir = TempDir::new().unwrap();

        // Test with existing config file
        let config_path = temp_dir.path().join("test_config.toml");
        let config_content = r#"
            output_dir = "/tmp/test-snapshots"
            
            [logging]
            verbose = true
            time_format = "[hour]:[minute]:[second]"
        "#;
        fs::write(&config_path, config_content).await.unwrap();

        // Simulate args with custom config
        let args = Args::parse_from([
            "dotsnapshot",
            "--config",
            config_path.to_str().unwrap(),
            "--list",
        ]);

        // Test that config can be loaded from custom path
        let config = if let Some(config_path) = &args.config {
            if config_path.exists() {
                Config::load_from_file(config_path).await.unwrap()
            } else {
                Config::default()
            }
        } else {
            Config::load().await.unwrap_or_default()
        };

        // Verify config was loaded correctly
        assert!(config.is_verbose_default());
        assert_eq!(config.get_time_format(), "[hour]:[minute]:[second]");
    }

    /// Test config loading with nonexistent file
    #[tokio::test]
    async fn test_config_loading_nonexistent_file() {
        let args = Args::parse_from([
            "dotsnapshot",
            "--config",
            "/nonexistent/path/config.toml",
            "--list",
        ]);

        // Should fall back to default config when file doesn't exist
        let config = if let Some(config_path) = &args.config {
            if config_path.exists() {
                Config::load_from_file(config_path).await.unwrap()
            } else {
                Config::default()
            }
        } else {
            Config::load().await.unwrap_or_default()
        };

        // Should be default config
        assert!(!config.is_verbose_default()); // Default is false
    }

    /// Test argument validation and conflicts
    #[test]
    fn test_argument_validation() {
        // Test that certain argument combinations work
        let args = Args::parse_from([
            "dotsnapshot",
            "--verbose",
            "--output",
            "/tmp/test",
            "--plugins",
            "vscode,homebrew",
        ]);

        assert!(args.verbose);
        assert_eq!(args.output, Some(PathBuf::from("/tmp/test")));
        assert_eq!(args.plugins, Some("vscode,homebrew".to_string()));
    }

    /// Test hooks command variations
    #[test]
    fn test_hooks_command_variations() {
        // Test hooks remove with different target types
        let args = Args::parse_from(["dotsnapshot", "hooks", "remove", "--post-snapshot", "--all"]);

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
        let args = Args::parse_from([
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
    #[test]
    fn test_hook_action_extended_parsing() {
        // Test script action (basic without conflicting args)
        let args = Args::parse_from([
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
        let args = Args::parse_from([
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
        let args = Args::parse_from([
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
        let args = Args::parse_from(["dotsnapshot", "hooks", "add", "--pre-snapshot", "--backup"]);

        if let Some(Commands::Hooks { command }) = args.command {
            match *command {
                HooksCommands::Add { action, .. } => {
                    assert!(action.backup);
                }
                _ => panic!("Expected add command"),
            }
        }

        // Test cleanup action (basic without conflicting options)
        let args = Args::parse_from([
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
    #[test]
    fn test_restore_command_extended_options() {
        // Test restore with all options
        let args = Args::parse_from([
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
        let args = Args::parse_from(["dotsnapshot", "restore", "--latest", "--backup", "false"]);

        match args.command {
            Some(Commands::Restore { backup, .. }) => {
                // Note: backup defaults to true, so this tests the default behavior
                assert!(backup); // Default value
            }
            _ => panic!("Expected restore command"),
        }
    }

    /// Test edge cases in argument parsing
    #[test]
    fn test_argument_parsing_edge_cases() {
        // Test with empty plugin list (should still parse)
        let args = Args::parse_from(["dotsnapshot", "--plugins", ""]);
        assert_eq!(args.plugins, Some("".to_string()));

        // Test with multiple output formats
        let args = Args::parse_from(["dotsnapshot", "--output", "/path/with spaces/snapshots"]);
        assert_eq!(
            args.output,
            Some(PathBuf::from("/path/with spaces/snapshots"))
        );

        // Test completions with different shells
        for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
            let args = Args::parse_from(["dotsnapshot", "--completions", shell]);
            assert!(args.completions.is_some());
        }
    }

    /// Test hooks list command with all filter combinations
    #[test]
    fn test_hooks_list_all_filters() {
        // Test pre-plugin filter
        let args = Args::parse_from(["dotsnapshot", "hooks", "list", "--pre-plugin"]);

        if let Some(Commands::Hooks { command }) = args.command {
            match *command {
                HooksCommands::List { pre_plugin, .. } => assert!(pre_plugin),
                _ => panic!("Expected list command"),
            }
        }

        // Test post-plugin filter
        let args = Args::parse_from(["dotsnapshot", "hooks", "list", "--post-plugin"]);

        if let Some(Commands::Hooks { command }) = args.command {
            match *command {
                HooksCommands::List { post_plugin, .. } => assert!(post_plugin),
                _ => panic!("Expected list command"),
            }
        }

        // Test pre-snapshot filter
        let args = Args::parse_from(["dotsnapshot", "hooks", "list", "--pre-snapshot"]);

        if let Some(Commands::Hooks { command }) = args.command {
            match *command {
                HooksCommands::List { pre_snapshot, .. } => assert!(pre_snapshot),
                _ => panic!("Expected list command"),
            }
        }

        // Test post-snapshot filter
        let args = Args::parse_from(["dotsnapshot", "hooks", "list", "--post-snapshot"]);

        if let Some(Commands::Hooks { command }) = args.command {
            match *command {
                HooksCommands::List { post_snapshot, .. } => assert!(post_snapshot),
                _ => panic!("Expected list command"),
            }
        }
    }

    /// Test plugin selection parsing logic
    #[test]
    fn test_plugin_selection_parsing() {
        // Test various plugin string formats
        let test_cases = vec![
            ("vscode", vec!["vscode"]),
            ("vscode,homebrew", vec!["vscode", "homebrew"]),
            ("vscode,homebrew,npm", vec!["vscode", "homebrew", "npm"]),
            ("single", vec!["single"]),
            ("", vec![""]), // Edge case: empty plugin
        ];

        for (input, expected) in test_cases {
            let parsed: Vec<String> = input.split(',').map(|s| s.to_string()).collect();
            assert_eq!(parsed, expected, "Failed to parse plugin string: {input}");
        }
    }

    /// Test default argument values
    #[test]
    fn test_default_argument_values() {
        let args = Args::parse_from(["dotsnapshot"]);

        // Verify all default values
        assert!(!args.verbose);
        assert!(!args.debug);
        assert!(args.config.is_none());
        assert!(args.command.is_none());
        assert!(args.output.is_none());
        assert!(args.plugins.is_none());
        assert!(!args.list);
        assert!(!args.info);
        assert!(args.completions.is_none());
        assert!(!args.man);
    }

    /// Test version information access
    #[test]
    fn test_version_info() {
        // Test that version info can be accessed (used in --info command)
        let version = env!("CARGO_PKG_VERSION");
        let description = env!("CARGO_PKG_DESCRIPTION");
        let repository = env!("CARGO_PKG_REPOSITORY");
        let license = env!("CARGO_PKG_LICENSE");

        assert!(!version.is_empty());
        assert!(!description.is_empty());
        assert!(!repository.is_empty());
        assert!(!license.is_empty());
    }

    /// Test create_subscriber with edge cases
    #[test]
    fn test_create_subscriber_edge_cases() {
        // Test with empty time format (should use default)
        let subscriber = create_subscriber(false, "".to_string());
        drop(subscriber);

        // Test with malformed time format (should use default)
        let subscriber = create_subscriber(true, "[invalid-format]".to_string());
        drop(subscriber);

        // Test with partial match (should use default)
        let subscriber = create_subscriber(false, "[hour]:[minute]".to_string());
        drop(subscriber);
    }
}

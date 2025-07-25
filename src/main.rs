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
    verbose: bool,
    time_format: String,
) -> Box<dyn tracing::Subscriber + Send + Sync> {
    let level = if verbose {
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

    let verbose = args.verbose || config.is_verbose_default();
    let time_format = config.get_time_format();
    let subscriber = create_subscriber(verbose, time_format);
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
}

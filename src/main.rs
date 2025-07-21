use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use clap_mangen::Man;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info};

mod config;
mod core;
mod plugins;

use config::Config;
use core::cleaner::SnapshotCleaner;
use core::executor::SnapshotExecutor;
use core::plugin::PluginRegistry;
use plugins::{
    cursor::{CursorExtensionsPlugin, CursorKeybindingsPlugin, CursorSettingsPlugin},
    homebrew::HomebrewBrewfilePlugin,
    npm::{NpmConfigPlugin, NpmGlobalPackagesPlugin},
    static_files::StaticFilesPlugin,
    vscode::{VSCodeExtensionsPlugin, VSCodeKeybindingsPlugin, VSCodeSettingsPlugin},
};

#[derive(Parser)]
#[command(name = "dotsnapshot")]
#[command(about = "A CLI utility to create snapshots of dotfiles and configuration")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Args {
    /// Path to config file
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    /// Enable verbose logging (overrides config file)
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Generate shell completions for the specified shell
    #[arg(long, value_enum)]
    completions: Option<Shell>,

    /// Generate man page
    #[arg(long)]
    man: bool,

    /// Show detailed information about the tool
    #[arg(long)]
    info: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Parser)]
enum Commands {
    /// Create a snapshot of dotfiles and configuration
    Snapshot {
        /// Output directory for snapshots (overrides config file)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Specify which plugins to run (comma-separated)
        #[arg(short, long)]
        plugins: Option<String>,

        /// List available plugins
        #[arg(short, long)]
        list: bool,
    },
    /// Clean snapshots from the snapshots directory
    Clean {
        /// Output directory containing snapshots (uses config if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// List all snapshots without cleaning
        #[arg(short, long)]
        list: bool,

        /// Clean specific snapshot by name
        #[arg(short, long)]
        name: Option<String>,

        /// Clean snapshots older than specified days
        #[arg(short, long)]
        days: Option<u32>,

        /// Show what would be cleaned without actually deleting
        #[arg(long)]
        dry_run: bool,

        /// Ask for confirmation before deleting
        #[arg(short, long)]
        interactive: bool,
    },
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
    println!("Available plugins:");
    println!();

    // Create a registry and register all plugins
    let mut registry = PluginRegistry::new();

    // Register all plugins
    registry.register(Arc::new(HomebrewBrewfilePlugin::new()));
    registry.register(Arc::new(VSCodeSettingsPlugin::new()));
    registry.register(Arc::new(VSCodeKeybindingsPlugin::new()));
    registry.register(Arc::new(VSCodeExtensionsPlugin::new()));
    registry.register(Arc::new(CursorSettingsPlugin::new()));
    registry.register(Arc::new(CursorKeybindingsPlugin::new()));
    registry.register(Arc::new(CursorExtensionsPlugin::new()));
    registry.register(Arc::new(NpmGlobalPackagesPlugin::new()));
    registry.register(Arc::new(NpmConfigPlugin::new()));
    registry.register(Arc::new(StaticFilesPlugin::new()));

    // Get plugin information
    let plugins = registry.list_plugins();

    // Group plugins by vendor
    let mut homebrew_plugins = Vec::new();
    let mut vscode_plugins = Vec::new();
    let mut cursor_plugins = Vec::new();
    let mut npm_plugins = Vec::new();
    let mut static_plugins = Vec::new();

    for (name, filename, description) in plugins {
        if name.starts_with("homebrew_") {
            homebrew_plugins.push((name, filename, description));
        } else if name.starts_with("vscode_") {
            vscode_plugins.push((name, filename, description));
        } else if name.starts_with("cursor_") {
            cursor_plugins.push((name, filename, description));
        } else if name.starts_with("npm_") {
            npm_plugins.push((name, filename, description));
        } else if name == "static" {
            static_plugins.push((name, filename, description));
        }
    }

    // Display grouped plugins
    if !homebrew_plugins.is_empty() {
        println!("🍺 Homebrew:");
        for (name, filename, description) in homebrew_plugins {
            println!("  {name:<20} -> {filename:<20} {description}");
        }
        println!();
    }

    if !vscode_plugins.is_empty() {
        println!("💻 VSCode:");
        for (name, filename, description) in vscode_plugins {
            println!("  {name:<20} -> {filename:<20} {description}");
        }
        println!();
    }

    if !cursor_plugins.is_empty() {
        println!("✏️  Cursor:");
        for (name, filename, description) in cursor_plugins {
            println!("  {name:<20} -> {filename:<20} {description}");
        }
        println!();
    }

    if !npm_plugins.is_empty() {
        println!("📦 NPM:");
        for (name, filename, description) in npm_plugins {
            println!("  {name:<20} -> {filename:<20} {description}");
        }
        println!();
    }

    if !static_plugins.is_empty() {
        println!("📄 Static:");
        for (name, filename, description) in static_plugins {
            println!("  {name:<20} -> {filename:<20} {description}");
        }
        println!();
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
        println!("🔧 dotsnapshot v{}", env!("CARGO_PKG_VERSION"));
        println!("📝 {}", env!("CARGO_PKG_DESCRIPTION"));
        println!("🌐 Repository: {}", env!("CARGO_PKG_REPOSITORY"));
        println!("📄 License: {}", env!("CARGO_PKG_LICENSE"));
        println!("🏷️  Keywords: dotfiles, backup, configuration, snapshots, cli");
        println!();
        println!("📦 Supported Plugins:");
        println!("  • Homebrew Brewfile generation");
        println!("  • VSCode settings, keybindings, and extensions");
        println!("  • Cursor settings, keybindings, and extensions");
        println!("  • NPM global packages and configuration");
        println!();
        println!("🚀 Usage:");
        println!("   dotsnapshot snapshot [OPTIONS]  # Create snapshots");
        println!("   dotsnapshot clean [OPTIONS]     # Clean snapshots");
        println!("   Use --help for detailed options");
        println!();
        println!("🔧 Shell Completions:");
        println!(
            "   dotsnapshot --completions bash > /usr/local/etc/bash_completion.d/dotsnapshot"
        );
        println!("   dotsnapshot --completions zsh > ~/.zfunc/_dotsnapshot");
        println!("   dotsnapshot --completions fish > ~/.config/fish/completions/dotsnapshot.fish");
        println!();
        println!("📖 Man Page:");
        println!("   dotsnapshot --man > /usr/local/share/man/man1/dotsnapshot.1");
        return Ok(());
    }

    // Load configuration
    let config = if let Some(config_path) = &args.config {
        Config::load_from_file(config_path).await?
    } else {
        Config::load().await?
    };

    // Store config path for later logging (after logging is initialized)
    let custom_config_path = args.config.clone();

    // Determine verbose setting
    let verbose = args.verbose || config.is_verbose_default();
    let time_format = config.get_time_format();

    // Initialize logging
    let subscriber = create_subscriber(verbose, time_format);
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set default subscriber");

    // Log custom config usage if applicable
    if let Some(config_path) = custom_config_path {
        info!("📋 Using custom config file: {}", config_path.display());
    }

    // Handle commands
    match args.command {
        Some(Commands::Snapshot {
            output,
            plugins,
            list,
        }) => handle_snapshot_command(output, plugins, list, config, start_time).await,
        Some(Commands::Clean {
            output,
            list,
            name,
            days,
            dry_run,
            interactive,
        }) => handle_clean_command(output, list, name, days, dry_run, interactive, config).await,
        None => {
            // Default behavior: create snapshot for backward compatibility
            handle_snapshot_command(None, None, false, config, start_time).await
        }
    }
}

async fn handle_snapshot_command(
    output: Option<PathBuf>,
    plugins: Option<String>,
    list: bool,
    config: Config,
    start_time: Instant,
) -> Result<()> {
    info!("Starting dotsnapshot v{}", env!("CARGO_PKG_VERSION"));

    // Handle --list flag
    if list {
        list_plugins().await;
        return Ok(());
    }

    // Determine final settings (CLI args override config file)
    let output_dir = output.unwrap_or_else(|| config.get_output_dir());

    // Create output directory if it doesn't exist
    tokio::fs::create_dir_all(&output_dir).await?;

    // Initialize plugin registry
    let mut registry = PluginRegistry::new();

    // Determine which plugins to run
    let selected_plugins = if let Some(cli_plugins) = plugins.as_deref() {
        // CLI argument takes precedence
        cli_plugins
    } else if let Some(config_plugins) = config.get_include_plugins() {
        // Use config file plugins (convert to comma-separated string)
        let plugins_str = config_plugins.join(",");
        // We need to store this in a variable to extend its lifetime
        let plugins_str = Box::leak(plugins_str.into_boxed_str());
        plugins_str
    } else {
        // Default: run all plugins
        "all"
    };

    // Homebrew plugins
    if selected_plugins == "all" || selected_plugins.contains("homebrew") {
        registry.register(Arc::new(HomebrewBrewfilePlugin::new()));
    }

    // VSCode plugins
    if selected_plugins == "all" || selected_plugins.contains("vscode") {
        registry.register(Arc::new(VSCodeSettingsPlugin::new()));
        registry.register(Arc::new(VSCodeKeybindingsPlugin::new()));
        registry.register(Arc::new(VSCodeExtensionsPlugin::new()));
    }

    // Cursor plugins
    if selected_plugins == "all" || selected_plugins.contains("cursor") {
        registry.register(Arc::new(CursorSettingsPlugin::new()));
        registry.register(Arc::new(CursorKeybindingsPlugin::new()));
        registry.register(Arc::new(CursorExtensionsPlugin::new()));
    }

    // NPM plugins
    if selected_plugins == "all" || selected_plugins.contains("npm") {
        registry.register(Arc::new(NpmGlobalPackagesPlugin::new()));
        registry.register(Arc::new(NpmConfigPlugin::new()));
    }

    // Static files plugin
    if selected_plugins == "all" || selected_plugins.contains("static") {
        registry.register(Arc::new(StaticFilesPlugin::with_config(Arc::new(
            config.clone(),
        ))));
    }

    // Create executor and run snapshot
    let executor = SnapshotExecutor::with_config(Arc::new(registry), output_dir, Arc::new(config));

    match executor.execute_snapshot().await {
        Ok(snapshot_path) => {
            let duration = start_time.elapsed();
            info!(
                "✅ Snapshot created successfully at: {}",
                snapshot_path.display()
            );
            info!("⏱️  Execution time: {:.2?}", duration);
        }
        Err(e) => {
            error!("❌ Snapshot creation failed: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn handle_clean_command(
    output: Option<PathBuf>,
    list: bool,
    name: Option<String>,
    days: Option<u32>,
    dry_run: bool,
    interactive: bool,
    config: Config,
) -> Result<()> {
    info!("Starting dotsnapshot clean v{}", env!("CARGO_PKG_VERSION"));

    // Determine snapshots directory
    let snapshots_dir = output.unwrap_or_else(|| config.get_output_dir());
    let cleaner = SnapshotCleaner::new(snapshots_dir.clone());

    if list {
        // List snapshots
        let snapshots = cleaner.list_snapshots().await?;
        if snapshots.is_empty() {
            println!("No snapshots found in: {}", snapshots_dir.display());
            return Ok(());
        }

        println!("📸 Snapshots in: {}", snapshots_dir.display());
        println!();
        for snapshot in snapshots {
            println!(
                "  {} | {} | {} | {} plugins",
                snapshot.name,
                snapshot.created_at.format("%Y-%m-%d %H:%M:%S"),
                SnapshotCleaner::format_size(snapshot.size_bytes),
                snapshot.plugin_count
            );
        }
        return Ok(());
    }

    if let Some(snapshot_name) = name {
        // Clean specific snapshot by name
        if interactive && !dry_run {
            println!("Are you sure you want to delete snapshot '{snapshot_name}'? (y/N)");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
                println!("Operation cancelled.");
                return Ok(());
            }
        }

        let success = cleaner.clean_by_name(&snapshot_name, dry_run).await?;
        if success {
            if dry_run {
                println!("✅ Would delete snapshot: {snapshot_name}");
            } else {
                println!("✅ Deleted snapshot: {snapshot_name}");
            }
        } else {
            println!("❌ Snapshot '{snapshot_name}' not found");
        }
    } else if let Some(retention_days) = days {
        // Clean by retention period
        if interactive && !dry_run {
            println!(
                "Are you sure you want to delete snapshots older than {retention_days} days? (y/N)"
            );
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
                println!("Operation cancelled.");
                return Ok(());
            }
        }

        let cleaned = cleaner.clean_by_retention(retention_days, dry_run).await?;
        if cleaned.is_empty() {
            println!("No snapshots found older than {retention_days} days");
        } else {
            if dry_run {
                println!(
                    "✅ Would delete {} snapshots older than {} days:",
                    cleaned.len(),
                    retention_days
                );
            } else {
                println!(
                    "✅ Deleted {} snapshots older than {} days:",
                    cleaned.len(),
                    retention_days
                );
            }
            for snapshot_name in cleaned {
                println!("  • {snapshot_name}");
            }
        }
    } else {
        println!(
            "❌ Please specify either --name <snapshot> or --days <number> to clean snapshots"
        );
        println!("Use --list to see available snapshots");
        std::process::exit(1);
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
        assert!(args.config.is_none());
        assert!(!args.verbose);
        assert!(args.command.is_none());

        // Test snapshot subcommand
        let args = Args::parse_from([
            "dotsnapshot",
            "snapshot",
            "--output",
            "/tmp/test",
            "--plugins",
            "homebrew,npm",
        ]);
        assert!(!args.verbose);
        match args.command {
            Some(Commands::Snapshot {
                output,
                plugins,
                list,
            }) => {
                assert_eq!(output.unwrap(), PathBuf::from("/tmp/test"));
                assert_eq!(plugins.unwrap(), "homebrew,npm");
                assert!(!list);
            }
            _ => panic!("Expected Snapshot command"),
        }

        // Test clean subcommand
        let args = Args::parse_from(["dotsnapshot", "clean", "--days", "30", "--dry-run"]);
        match args.command {
            Some(Commands::Clean { days, dry_run, .. }) => {
                assert_eq!(days.unwrap(), 30);
                assert!(dry_run);
            }
            _ => panic!("Expected Clean command"),
        }

        // Test global verbose flag
        let args = Args::parse_from(["dotsnapshot", "--verbose", "snapshot", "--list"]);
        assert!(args.verbose);
        match args.command {
            Some(Commands::Snapshot { list, .. }) => {
                assert!(list);
            }
            _ => panic!("Expected Snapshot command"),
        }
    }
}

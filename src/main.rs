use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info};

mod config;
mod core;
mod plugins;

use config::Config;
use core::executor::SnapshotExecutor;
use core::plugin::PluginRegistry;
use plugins::{
    cursor::{CursorExtensionsPlugin, CursorKeybindingsPlugin, CursorSettingsPlugin},
    homebrew::HomebrewBrewfilePlugin,
    npm::{NpmConfigPlugin, NpmGlobalPackagesPlugin},
    vscode::{VSCodeExtensionsPlugin, VSCodeKeybindingsPlugin, VSCodeSettingsPlugin},
};

#[derive(Parser)]
#[command(name = "dotsnapshot")]
#[command(about = "A CLI utility to create snapshots of dotfiles and configuration")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Args {
    /// Output directory for snapshots (overrides config file)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Enable verbose logging (overrides config file)
    #[arg(short, long)]
    verbose: bool,

    /// Specify which plugins to run (comma-separated)
    #[arg(short, long)]
    plugins: Option<String>,

    /// Path to config file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// List available plugins
    #[arg(short, long)]
    list: bool,
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

    // Get plugin information
    let plugins = registry.list_plugins();

    // Group plugins by vendor
    let mut homebrew_plugins = Vec::new();
    let mut vscode_plugins = Vec::new();
    let mut cursor_plugins = Vec::new();
    let mut npm_plugins = Vec::new();

    for (name, filename, description) in plugins {
        if name.starts_with("homebrew_") {
            homebrew_plugins.push((name, filename, description));
        } else if name.starts_with("vscode_") {
            vscode_plugins.push((name, filename, description));
        } else if name.starts_with("cursor_") {
            cursor_plugins.push((name, filename, description));
        } else if name.starts_with("npm_") {
            npm_plugins.push((name, filename, description));
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

    println!("Usage:");
    println!("  --plugins <plugin1>,<plugin2>  Run specific plugins");
    println!("  --plugins homebrew,vscode      Run all homebrew and vscode plugins");
    println!("  (no --plugins)                 Run all plugins");
}

#[tokio::main]
async fn main() -> Result<()> {
    let start_time = Instant::now();
    let args = Args::parse();

    // Handle --list flag early
    if args.list {
        list_plugins().await;
        return Ok(());
    }

    // Load configuration
    let config = if let Some(config_path) = &args.config {
        Config::load_from_file(config_path).await?
    } else {
        Config::load().await?
    };

    // Determine final settings (CLI args override config file)
    let output_dir = args.output.unwrap_or_else(|| config.get_output_dir());
    let verbose = args.verbose || config.is_verbose_default();
    let time_format = config.get_time_format();

    // Initialize logging
    let subscriber = create_subscriber(verbose, time_format);

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set default subscriber");

    info!("Starting dotsnapshot v{}", env!("CARGO_PKG_VERSION"));

    // Create output directory if it doesn't exist
    tokio::fs::create_dir_all(&output_dir).await?;

    // Initialize plugin registry
    let mut registry = PluginRegistry::new();

    // Determine which plugins to run
    let selected_plugins = if let Some(cli_plugins) = args.plugins.as_deref() {
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

    // Create executor and run snapshot
    let executor = SnapshotExecutor::new(Arc::new(registry), output_dir);

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

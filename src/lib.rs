// Library interface for dotsnapshot
pub mod cli;
pub mod config;
pub mod core;
pub mod plugins;
pub mod symbols;

// Re-export commonly used types
pub use crate::config::Config;
pub use crate::core::hooks::{HookAction, HooksConfig};

// Re-export CLI types for testing
#[derive(clap::Parser)]
#[group(required = true, multiple = false)]
pub struct HookTarget {
    /// Pre-snapshot hook (global)
    #[arg(long)]
    pub pre_snapshot: bool,

    /// Post-snapshot hook (global)
    #[arg(long)]
    pub post_snapshot: bool,

    /// Pre-plugin hook for specific plugin
    #[arg(long)]
    pub pre_plugin: Option<String>,

    /// Post-plugin hook for specific plugin
    #[arg(long)]
    pub post_plugin: Option<String>,
}

#[derive(clap::Parser, Clone)]
#[group(id = "action", required = true, multiple = false)]
pub struct HookActionArgs {
    /// Script to execute
    #[arg(long, group = "action")]
    pub script: Option<String>,

    /// Log message
    #[arg(long, group = "action")]
    pub log: Option<String>,

    /// Notification message
    #[arg(long, group = "action")]
    pub notify: Option<String>,

    /// Backup action
    #[arg(long, group = "action")]
    pub backup: bool,

    /// Cleanup action
    #[arg(long, group = "action")]
    pub cleanup: bool,

    /// Script arguments (comma-separated, only with --script)
    #[arg(long, requires = "script")]
    pub args: Option<String>,

    /// Script timeout in seconds (only with --script)
    #[arg(long, requires = "script")]
    pub timeout: Option<u64>,

    /// Log level (only with --log)
    #[arg(long, requires = "log", value_parser = ["trace", "debug", "info", "warn", "error"])]
    pub level: Option<String>,

    /// Notification title (only with --notify)
    #[arg(long, requires = "notify")]
    pub title: Option<String>,

    /// Backup source path (only with --backup)
    #[arg(long, requires = "backup")]
    pub path: Option<std::path::PathBuf>,

    /// Backup destination path (only with --backup)
    #[arg(long, requires = "backup")]
    pub destination: Option<std::path::PathBuf>,

    /// Cleanup patterns (comma-separated, only with --cleanup)
    #[arg(long, requires = "cleanup")]
    pub patterns: Option<String>,

    /// Cleanup directories (comma-separated, only with --cleanup)
    #[arg(long, requires = "cleanup")]
    pub directories: Option<String>,

    /// Clean temp files (only with --cleanup)
    #[arg(long, requires = "cleanup")]
    pub temp_files: bool,
}

#[derive(clap::Parser)]
// Allow large enum variant because HookActionArgs contains many optional CLI arguments
// Boxing would complicate the clap derive macro usage without significant memory benefits
// since this enum is used transiently for command parsing only
#[allow(clippy::large_enum_variant)]
pub enum HooksCommands {
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
        set: Option<std::path::PathBuf>,

        /// Create scripts directory if it doesn't exist
        #[arg(long)]
        create: bool,
    },
}

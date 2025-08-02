use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::core::hooks::{HookAction, HooksConfig};

/// Configuration for file-snapshots
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Output directory for snapshots
    pub output_dir: Option<PathBuf>,

    /// Specific plugins to include (if not specified, all plugins run)
    pub include_plugins: Option<Vec<String>>,

    /// Logging configuration
    pub logging: Option<LoggingConfig>,

    /// Hooks configuration
    pub hooks: Option<HooksConfig>,

    /// Global hooks configuration
    pub global: Option<GlobalConfig>,

    /// Static plugin configuration (legacy)
    #[serde(rename = "static")]
    pub static_files: Option<StaticFilesConfig>,

    /// Plugin-specific configurations
    pub plugins: Option<PluginsConfig>,

    /// UI configuration
    pub ui: Option<UiConfig>,

    /// Validation configuration
    pub validation: Option<ValidationConfig>,
}

/// Logging configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// Enable verbose logging by default
    pub verbose: Option<bool>,

    /// Time format for log timestamps (uses time crate format syntax)
    pub time_format: Option<String>,
}

/// Global hooks configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalConfig {
    /// Global hooks that apply to all plugins
    pub hooks: Option<GlobalHooks>,
}

/// Global hooks that apply to all snapshots
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalHooks {
    /// Hooks to run before any plugins execute
    #[serde(rename = "pre-snapshot", default)]
    pub pre_snapshot: Vec<HookAction>,

    /// Hooks to run after all plugins complete
    #[serde(rename = "post-snapshot", default)]
    pub post_snapshot: Vec<HookAction>,
}

/// Plugin-specific configurations - raw TOML values for plugin self-discovery
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginsConfig {
    #[serde(flatten)]
    pub plugins: std::collections::HashMap<String, toml::Value>,
}

/// Generic plugin configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginConfig {
    /// Custom target path in snapshot (relative to snapshot root)
    pub target_path: Option<String>,

    /// Custom output file for the plugin (overrides auto-derived filename)
    pub output_file: Option<String>,

    /// Plugin-specific hooks
    pub hooks: Option<PluginHooks>,
}

/// Plugin-specific hooks
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginHooks {
    /// Hooks to run before this plugin executes
    #[serde(rename = "pre-plugin", default)]
    pub pre_plugin: Vec<HookAction>,

    /// Hooks to run after this plugin completes
    #[serde(rename = "post-plugin", default)]
    pub post_plugin: Vec<HookAction>,
}

/// Static files plugin configuration with additional options
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StaticPluginConfig {
    /// Custom target path in snapshot (relative to snapshot root)
    pub target_path: Option<String>,
    /// Custom output file for the plugin (overrides auto-derived filename)
    pub output_file: Option<String>,
    /// List of file paths to include in snapshots
    pub files: Option<Vec<String>>,
    /// Glob patterns to ignore when copying files/directories
    pub ignore: Option<Vec<String>>,
}

/// Static files plugin configuration (legacy)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StaticFilesConfig {
    /// List of file paths to include in snapshots
    pub files: Option<Vec<String>>,
    /// Glob patterns to ignore when copying files/directories
    pub ignore: Option<Vec<String>>,
}

/// UI configuration for display customization
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UiConfig {
    /// Custom names for plugin categories/groups
    pub plugin_categories: Option<std::collections::HashMap<String, String>>,
}

/// Validation configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ValidationConfig {
    /// Configuration files that are allowed to have no file extension
    /// These are well-known configuration files that traditionally don't use extensions
    pub well_known_no_extension: Option<Vec<String>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            output_dir: Some(PathBuf::from("./snapshots")),
            include_plugins: None,
            logging: None,
            hooks: None,
            global: None,
            static_files: None,
            plugins: None,
            ui: None,
            validation: None,
        }
    }
}

impl Config {
    /// Load configuration from file
    pub async fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .await
            .context("Failed to read config file")?;

        let config: Config =
            toml::from_str(&content).context("Failed to parse config file as TOML")?;

        Ok(config)
    }

    /// Try to load configuration from standard locations
    pub async fn load() -> Result<Self> {
        let config_paths = Self::get_config_paths();

        for path in config_paths {
            if path.exists() {
                return Self::load_from_file(&path).await;
            }
        }

        // Return default config if no config file found
        Ok(Self::default())
    }

    /// Get potential configuration file paths in order of preference
    pub fn get_config_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // 1. Current directory
        paths.push(PathBuf::from("dotsnapshot.toml"));
        paths.push(PathBuf::from(".dotsnapshot.toml"));

        // 2. User config directory
        if let Some(config_dir) = dirs::config_dir() {
            paths.push(config_dir.join("dotsnapshot").join("config.toml"));
            paths.push(config_dir.join("dotsnapshot.toml"));
        }

        // 3. User home directory
        if let Some(home_dir) = dirs::home_dir() {
            paths.push(
                home_dir
                    .join(".config")
                    .join("dotsnapshot")
                    .join("config.toml"),
            );
            paths.push(home_dir.join(".dotsnapshot.toml"));
        }

        paths
    }

    /// Get the output directory, using the configured value or default
    pub fn get_output_dir(&self) -> PathBuf {
        let path = self
            .output_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("./snapshots"));
        Self::expand_tilde(&path)
    }

    /// Expand tilde (~) to home directory if present
    fn expand_tilde(path: &Path) -> PathBuf {
        if let Some(path_str) = path.to_str() {
            if let Some(stripped) = path_str.strip_prefix("~/") {
                if let Some(home_dir) = dirs::home_dir() {
                    return home_dir.join(stripped);
                }
            } else if path_str == "~" {
                if let Some(home_dir) = dirs::home_dir() {
                    return home_dir;
                }
            }
        }
        path.to_path_buf()
    }

    /// Get the included plugins list (None means all plugins)
    pub fn get_include_plugins(&self) -> Option<Vec<String>> {
        self.include_plugins.clone()
    }

    /// Check if verbose logging is enabled by default
    pub fn is_verbose_default(&self) -> bool {
        self.logging
            .as_ref()
            .and_then(|l| l.verbose)
            .unwrap_or(false)
    }

    /// Get the time format for log timestamps
    pub fn get_time_format(&self) -> String {
        self.logging
            .as_ref()
            .and_then(|l| l.time_format.clone())
            .unwrap_or_else(|| "[year]-[month]-[day] [hour]:[minute]:[second]".to_string())
    }

    /// Get raw plugin configuration for a specific plugin (plugin self-discovery)
    pub fn get_raw_plugin_config(&self, plugin_name: &str) -> Option<&toml::Value> {
        let plugins = self.plugins.as_ref()?;

        // Handle special case: static_files plugin config is stored under "static" key
        let config_key = match plugin_name {
            "static_files" => "static",
            _ => plugin_name,
        };

        plugins.plugins.get(config_key)
    }

    /// Get hooks configuration
    pub fn get_hooks_config(&self) -> HooksConfig {
        self.hooks.clone().unwrap_or_default()
    }

    /// Get global pre-snapshot hooks
    pub fn get_global_pre_snapshot_hooks(&self) -> Vec<HookAction> {
        self.global
            .as_ref()
            .and_then(|g| g.hooks.as_ref())
            .map(|h| h.pre_snapshot.clone())
            .unwrap_or_default()
    }

    /// Get global post-snapshot hooks
    pub fn get_global_post_snapshot_hooks(&self) -> Vec<HookAction> {
        self.global
            .as_ref()
            .and_then(|g| g.hooks.as_ref())
            .map(|h| h.post_snapshot.clone())
            .unwrap_or_default()
    }

    /// Get plugin-specific pre-plugin hooks
    pub fn get_plugin_pre_hooks(&self, plugin_name: &str) -> Vec<HookAction> {
        self.get_plugin_hooks(plugin_name)
            .map(|h| h.pre_plugin.clone())
            .unwrap_or_default()
    }

    /// Get plugin-specific post-plugin hooks
    pub fn get_plugin_post_hooks(&self, plugin_name: &str) -> Vec<HookAction> {
        self.get_plugin_hooks(plugin_name)
            .map(|h| h.post_plugin.clone())
            .unwrap_or_default()
    }

    /// Get plugin hooks configuration
    fn get_plugin_hooks(&self, plugin_name: &str) -> Option<PluginHooks> {
        let raw_config = self.get_raw_plugin_config(plugin_name)?;
        if let Some(hooks_value) = raw_config.get("hooks") {
            hooks_value.clone().try_into().ok()
        } else {
            None
        }
    }

    /// Save configuration to file
    pub async fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self).context("Failed to serialize config to TOML")?;

        // Create parent directories if they don't exist
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create config directory")?;
        }

        fs::write(path.as_ref(), content)
            .await
            .context("Failed to write config file")?;

        Ok(())
    }
}

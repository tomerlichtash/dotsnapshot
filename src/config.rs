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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.get_output_dir(), PathBuf::from("./snapshots"));
        assert_eq!(config.get_include_plugins(), None);
        assert!(!config.is_verbose_default());
    }

    #[tokio::test]
    async fn test_config_load_and_save() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("config.toml");

        let config = Config {
            output_dir: Some(PathBuf::from("/tmp/snapshots")),
            include_plugins: Some(vec!["homebrew".to_string(), "vscode".to_string()]),
            logging: Some(LoggingConfig {
                verbose: Some(true),
                time_format: Some("[year]-[month]-[day] [hour]:[minute]:[second]".to_string()),
            }),
            hooks: None,
            global: None,
            static_files: Some(StaticFilesConfig {
                files: Some(vec!["~/.gitconfig".to_string(), "/etc/hosts".to_string()]),
                ignore: None,
            }),
            plugins: None,
            ui: None,
        };

        // Save config
        config.save_to_file(&config_path).await?;

        // Load config
        let loaded_config = Config::load_from_file(&config_path).await?;

        assert_eq!(
            loaded_config.get_output_dir(),
            PathBuf::from("/tmp/snapshots")
        );
        assert_eq!(
            loaded_config.get_include_plugins(),
            Some(vec!["homebrew".to_string(), "vscode".to_string()])
        );
        assert!(loaded_config.is_verbose_default());

        Ok(())
    }

    #[tokio::test]
    async fn test_config_paths() {
        let paths = Config::get_config_paths();
        assert!(!paths.is_empty());
        assert!(paths
            .iter()
            .any(|p| p.file_name().unwrap() == "dotsnapshot.toml"));
    }

    /// Test comprehensive hook configuration functionality
    /// Verifies all hook-related configuration methods work correctly
    #[tokio::test]
    async fn test_config_hooks_comprehensive() {
        use std::collections::HashMap;

        let config = Config {
            output_dir: Some(PathBuf::from("/test/output")),
            include_plugins: None,
            logging: None,
            hooks: Some(HooksConfig {
                scripts_dir: PathBuf::from("/test/scripts"),
            }),
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![HookAction::Notify {
                        message: "Starting global snapshot".to_string(),
                        title: Some("Dotsnapshot".to_string()),
                    }],
                    post_snapshot: vec![HookAction::Script {
                        command: "echo".to_string(),
                        args: vec!["Global post-snapshot hook".to_string()],
                        timeout: 60,
                        working_dir: None,
                        env_vars: HashMap::new(),
                    }],
                }),
            }),
            static_files: None,
            plugins: Some(PluginsConfig {
                plugins: {
                    let mut plugins_map = std::collections::HashMap::new();

                    let mut vscode_config = toml::value::Table::new();
                    vscode_config.insert(
                        "target_path".to_string(),
                        toml::Value::String("~/vscode".to_string()),
                    );
                    plugins_map.insert("vscode".to_string(), toml::Value::Table(vscode_config));

                    let mut homebrew_config = toml::value::Table::new();
                    homebrew_config.insert(
                        "output_file".to_string(),
                        toml::Value::String("brewfile.txt".to_string()),
                    );
                    plugins_map.insert("homebrew".to_string(), toml::Value::Table(homebrew_config));

                    plugins_map
                },
            }),
            ui: Some(UiConfig {
                plugin_categories: Some({
                    let mut categories = HashMap::new();
                    categories.insert("vscode".to_string(), "VS Code Editor".to_string());
                    categories.insert("homebrew".to_string(), "Package Manager".to_string());
                    categories
                }),
            }),
        };

        // Test hook configuration methods
        let hooks_config = config.get_hooks_config();
        assert_eq!(hooks_config.scripts_dir, PathBuf::from("/test/scripts"));

        // Test global hooks
        let global_pre = config.get_global_pre_snapshot_hooks();
        assert_eq!(global_pre.len(), 1);

        let global_post = config.get_global_post_snapshot_hooks();
        assert_eq!(global_post.len(), 1);

        // Test plugin-specific hooks (should be empty for plugins without hooks config)
        let plugin_pre = config.get_plugin_pre_hooks("vscode");
        assert_eq!(plugin_pre.len(), 0);

        let plugin_post = config.get_plugin_post_hooks("vscode");
        assert_eq!(plugin_post.len(), 0);

        // Test plugin-specific hooks for non-existent plugin
        let no_plugin_pre = config.get_plugin_pre_hooks("nonexistent");
        assert_eq!(no_plugin_pre.len(), 0);

        let no_plugin_post = config.get_plugin_post_hooks("nonexistent");
        assert_eq!(no_plugin_post.len(), 0);

        // Test plugin configuration retrieval
        let vscode_config = config.get_raw_plugin_config("vscode");
        assert!(vscode_config.is_some());

        let homebrew_config = config.get_raw_plugin_config("homebrew");
        assert!(homebrew_config.is_some());

        let nonexistent_config = config.get_raw_plugin_config("nonexistent");
        assert!(nonexistent_config.is_none());

        // Test time format
        let time_format = config.get_time_format();
        assert!(!time_format.is_empty());

        // Test verbose setting (should be false by default)
        assert!(!config.is_verbose_default());
    }

    /// Test config with minimal settings and edge cases
    /// Verifies that missing configurations are handled correctly
    #[tokio::test]
    async fn test_config_minimal_and_edge_cases() {
        // Test with completely minimal config
        let minimal_config = Config {
            output_dir: None,
            include_plugins: None,
            logging: None,
            hooks: None,
            global: None,
            static_files: None,
            plugins: None,
            ui: None,
        };

        // Test default behaviors
        assert_eq!(
            minimal_config.get_output_dir(),
            PathBuf::from("./snapshots")
        );
        assert_eq!(minimal_config.get_include_plugins(), None);
        assert!(!minimal_config.is_verbose_default());

        // Test default time format
        let default_time_format = minimal_config.get_time_format();
        assert_eq!(
            default_time_format,
            "[year]-[month]-[day] [hour]:[minute]:[second]"
        );

        // Test empty hooks (HooksConfig has scripts_dir field, which gets default value)
        let hooks_config = minimal_config.get_hooks_config();
        assert!(hooks_config
            .scripts_dir
            .to_string_lossy()
            .contains("dotsnapshot"));

        let global_pre = minimal_config.get_global_pre_snapshot_hooks();
        assert_eq!(global_pre.len(), 0);

        let global_post = minimal_config.get_global_post_snapshot_hooks();
        assert_eq!(global_post.len(), 0);

        // Test plugin hooks with no configuration
        let plugin_pre = minimal_config.get_plugin_pre_hooks("any_plugin");
        assert_eq!(plugin_pre.len(), 0);

        let plugin_post = minimal_config.get_plugin_post_hooks("any_plugin");
        assert_eq!(plugin_post.len(), 0);

        // Test plugin config retrieval with no plugins configured
        let no_config = minimal_config.get_raw_plugin_config("any_plugin");
        assert!(no_config.is_none());

        // Test with logging config but no verbose setting
        let config_with_logging = Config {
            output_dir: None,
            include_plugins: None,
            logging: Some(LoggingConfig {
                verbose: None, // No explicit verbose setting
                time_format: Some("[hour]:[minute]".to_string()),
            }),
            hooks: None,
            global: None,
            static_files: None,
            plugins: None,
            ui: None,
        };

        // Should still return false for verbose when not set
        assert!(!config_with_logging.is_verbose_default());

        // Should use custom time format
        assert_eq!(config_with_logging.get_time_format(), "[hour]:[minute]");
    }

    /// Test config serialization and deserialization edge cases
    /// Verifies that complex configurations can be properly saved and loaded
    #[tokio::test]
    async fn test_config_serialization_edge_cases() -> Result<()> {
        use std::collections::HashMap;
        use tempfile::TempDir;

        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("complex_config.toml");

        // Create a complex configuration with all features
        let complex_config = Config {
            output_dir: Some(PathBuf::from("/complex/output")),
            include_plugins: Some(vec![
                "vscode".to_string(),
                "homebrew".to_string(),
                "npm".to_string(),
            ]),
            logging: Some(LoggingConfig {
                verbose: Some(true),
                time_format: Some(
                    "[year]-[month padding:zero]-[day padding:zero]T[hour]:[minute]:[second]Z"
                        .to_string(),
                ),
            }),
            hooks: Some(HooksConfig {
                scripts_dir: PathBuf::from("/usr/local/bin/scripts"),
            }),
            global: Some(GlobalConfig {
                hooks: Some(GlobalHooks {
                    pre_snapshot: vec![HookAction::Script {
                        command: "systemctl".to_string(),
                        args: vec!["is-active".to_string(), "docker".to_string()],
                        timeout: 10,
                        working_dir: None,
                        env_vars: HashMap::new(),
                    }],
                    post_snapshot: vec![HookAction::Notify {
                        message: "All backups completed".to_string(),
                        title: Some("Dotsnapshot".to_string()),
                    }],
                }),
            }),
            static_files: Some(StaticFilesConfig {
                files: Some(vec![
                    "~/.vimrc".to_string(),
                    "~/.zshrc".to_string(),
                    "/etc/hosts".to_string(),
                ]),
                ignore: Some(vec![
                    "*.log".to_string(),
                    "*.tmp".to_string(),
                    ".DS_Store".to_string(),
                ]),
            }),
            plugins: Some(PluginsConfig {
                plugins: {
                    let mut plugins_map = std::collections::HashMap::new();

                    let mut vscode_config = toml::value::Table::new();
                    vscode_config.insert(
                        "target_path".to_string(),
                        toml::Value::String("~/vscode-settings".to_string()),
                    );
                    vscode_config.insert(
                        "output_file".to_string(),
                        toml::Value::String("vscode_config.json".to_string()),
                    );
                    plugins_map.insert("vscode".to_string(), toml::Value::Table(vscode_config));

                    let mut homebrew_config = toml::value::Table::new();
                    homebrew_config.insert(
                        "output_file".to_string(),
                        toml::Value::String("Brewfile".to_string()),
                    );
                    homebrew_config.insert("include_casks".to_string(), toml::Value::Boolean(true));
                    plugins_map.insert("homebrew".to_string(), toml::Value::Table(homebrew_config));

                    let mut npm_config = toml::value::Table::new();
                    npm_config.insert("global_only".to_string(), toml::Value::Boolean(false));
                    plugins_map.insert("npm".to_string(), toml::Value::Table(npm_config));

                    plugins_map
                },
            }),
            ui: Some(UiConfig {
                plugin_categories: Some({
                    let mut categories = HashMap::new();
                    categories.insert("vscode".to_string(), "Editors".to_string());
                    categories.insert("homebrew".to_string(), "Package Managers".to_string());
                    categories.insert("npm".to_string(), "Development Tools".to_string());
                    categories
                }),
            }),
        };

        // Save the complex configuration
        complex_config.save_to_file(&config_path).await?;

        // Load and verify the configuration
        let loaded_config = Config::load_from_file(&config_path).await?;

        // Verify all aspects were preserved
        assert_eq!(
            loaded_config.get_output_dir(),
            PathBuf::from("/complex/output")
        );
        assert!(loaded_config.is_verbose_default());
        assert_eq!(loaded_config.get_include_plugins().unwrap().len(), 3);

        // Verify hooks were preserved
        let hooks = loaded_config.get_hooks_config();
        assert_eq!(hooks.scripts_dir, PathBuf::from("/usr/local/bin/scripts"));

        let global_pre = loaded_config.get_global_pre_snapshot_hooks();
        assert_eq!(global_pre.len(), 1);

        let vscode_pre = loaded_config.get_plugin_pre_hooks("vscode");
        assert_eq!(vscode_pre.len(), 0); // No plugin-level hooks configured

        // Verify plugin configurations were preserved
        let vscode_config = loaded_config.get_raw_plugin_config("vscode");
        assert!(vscode_config.is_some());

        let homebrew_config = loaded_config.get_raw_plugin_config("homebrew");
        assert!(homebrew_config.is_some());

        let npm_config = loaded_config.get_raw_plugin_config("npm");
        assert!(npm_config.is_some());

        Ok(())
    }
}

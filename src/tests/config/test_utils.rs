use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

use crate::config::{
    Config, GlobalConfig, GlobalHooks, LoggingConfig, PluginsConfig, StaticFilesConfig, UiConfig,
};
use crate::core::hooks::{HookAction, HooksConfig};

/// Creates a minimal config for testing
/// Returns config with all None values
pub fn create_minimal_config() -> Config {
    Config {
        output_dir: None,
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

/// Creates a basic config with some standard values for testing
/// Returns config with common settings
pub fn create_basic_config() -> Config {
    Config {
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
        validation: None,
    }
}

/// Creates a config with logging but no verbose setting for edge case testing
/// Returns config with logging configuration but verbose = None
pub fn create_config_with_logging_no_verbose() -> Config {
    Config {
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
        validation: None,
    }
}

/// Creates a complex config with all features for testing
/// Returns config with comprehensive configuration including hooks and plugins
pub fn create_complex_config() -> Config {
    Config {
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
        validation: None,
    }
}

/// Creates a config with hooks for testing hook functionality
/// Returns config with comprehensive hook configuration
pub fn create_config_with_hooks() -> Config {
    Config {
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
        validation: None,
    }
}

/// Creates a temporary directory for testing
/// Returns TempDir instance for cleanup
pub fn create_temp_dir() -> TempDir {
    TempDir::new().unwrap()
}

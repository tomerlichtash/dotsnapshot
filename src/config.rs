use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Configuration for file-snapshots
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Output directory for snapshots
    pub output_dir: Option<PathBuf>,

    /// Specific plugins to include (if not specified, all plugins run)
    pub include_plugins: Option<Vec<String>>,

    /// Logging configuration
    pub logging: Option<LoggingConfig>,
}

/// Logging configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// Enable verbose logging by default
    pub verbose: Option<bool>,

    /// Time format for log timestamps (uses time crate format syntax)
    pub time_format: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            output_dir: Some(PathBuf::from("./snapshots")),
            include_plugins: None,
            logging: None,
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
    fn get_config_paths() -> Vec<PathBuf> {
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

    /// Save configuration to file
    #[allow(dead_code)]
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
}

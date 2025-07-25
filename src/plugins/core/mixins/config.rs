use anyhow::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::core::config_schema::{ConfigSchema, ValidationHelpers};
use crate::core::hooks::HookAction;

/// Standard configuration structure shared across most plugins
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
pub struct StandardConfig {
    #[schemars(description = "Custom directory path within the snapshot for this plugin's output")]
    pub target_path: Option<String>,

    #[schemars(description = "Custom filename for the plugin output")]
    pub output_file: Option<String>,

    #[schemars(description = "Custom target directory for restoration")]
    pub restore_target_dir: Option<String>,

    #[schemars(description = "Plugin-specific hooks configuration")]
    pub hooks: Option<StandardHooks>,
}

/// Standard hooks structure shared across most plugins
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct StandardHooks {
    #[serde(rename = "pre-plugin", default)]
    #[schemars(description = "Hooks to run before plugin execution")]
    pub pre_plugin: Vec<HookAction>,

    #[serde(rename = "post-plugin", default)]
    #[schemars(description = "Hooks to run after plugin execution")]
    pub post_plugin: Vec<HookAction>,
}

impl ConfigSchema for StandardConfig {
    fn schema_name() -> &'static str {
        "StandardConfig"
    }

    fn validate(&self) -> Result<()> {
        // Default validation - can be overridden by specific plugins
        Ok(())
    }
}

/// Mixin trait for common configuration handling
pub trait ConfigMixin {
    type Config: ConfigSchema + Default;

    /// Get the parsed configuration for this plugin
    fn config(&self) -> Option<&Self::Config>;

    /// Get target path from plugin's configuration
    fn get_target_path(&self) -> Option<String> {
        None // Default implementation - override if needed
    }

    /// Get output file from plugin's configuration
    fn get_output_file(&self) -> Option<String> {
        None // Default implementation - override if needed
    }

    /// Get restore target directory from plugin's configuration
    fn get_restore_target_dir(&self) -> Option<String> {
        None // Default implementation - override if needed
    }

    /// Create plugin with configuration and validation
    fn with_config_validation(
        config: toml::Value,
        plugin_name: &str,
        config_section: &str,
        expected_fields: &str,
        example_config: &str,
    ) -> (Self::Config, bool)
    where
        Self: Sized,
    {
        match Self::Config::from_toml_value(&config) {
            Ok(parsed_config) => (parsed_config, true),
            Err(e) => {
                // Use shared error formatting
                let error_msg = ValidationHelpers::format_validation_error(
                    plugin_name,
                    config_section,
                    expected_fields,
                    example_config,
                    &e,
                );

                tracing::warn!("{error_msg}");

                // Return default config and indicate validation failed
                (Self::Config::default(), false)
            }
        }
    }
}

/// Standard implementation of ConfigMixin for plugins using StandardConfig
pub trait StandardConfigMixin: ConfigMixin<Config = StandardConfig> {
    fn get_standard_target_path(&self) -> Option<String> {
        self.config()?.target_path.clone()
    }

    fn get_standard_output_file(&self) -> Option<String> {
        self.config()?.output_file.clone()
    }

    fn get_standard_restore_target_dir(&self) -> Option<String> {
        self.config()?.restore_target_dir.clone()
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config_schema::ConfigSchema;
    use crate::core::hooks::HookAction;

    /// Mock plugin for testing ConfigMixin
    #[derive(Default)]
    struct MockPlugin {
        config: Option<StandardConfig>,
    }

    impl ConfigMixin for MockPlugin {
        type Config = StandardConfig;

        fn config(&self) -> Option<&Self::Config> {
            self.config.as_ref()
        }

        fn get_target_path(&self) -> Option<String> {
            self.get_standard_target_path()
        }

        fn get_output_file(&self) -> Option<String> {
            self.get_standard_output_file()
        }

        fn get_restore_target_dir(&self) -> Option<String> {
            self.get_standard_restore_target_dir()
        }
    }

    impl StandardConfigMixin for MockPlugin {}

    /// Test StandardConfig default creation
    #[test]
    fn test_standard_config_default() {
        let config = StandardConfig::default();
        assert!(config.target_path.is_none());
        assert!(config.output_file.is_none());
        assert!(config.restore_target_dir.is_none());
        assert!(config.hooks.is_none());
    }

    /// Test StandardConfig schema name
    #[test]
    fn test_standard_config_schema_name() {
        assert_eq!(
            <StandardConfig as ConfigSchema>::schema_name(),
            "StandardConfig"
        );
    }

    /// Test StandardConfig validation (default implementation)
    #[test]
    fn test_standard_config_validation() {
        let config = StandardConfig::default();
        assert!(config.validate().is_ok());

        let config_with_values = StandardConfig {
            target_path: Some("custom/path".to_string()),
            output_file: Some("custom.json".to_string()),
            restore_target_dir: Some("/custom/restore".to_string()),
            hooks: Some(StandardHooks {
                pre_plugin: vec![],
                post_plugin: vec![],
            }),
        };
        assert!(config_with_values.validate().is_ok());
    }

    /// Test StandardHooks creation
    #[test]
    fn test_standard_hooks() {
        let hooks = StandardHooks {
            pre_plugin: vec![HookAction::Script {
                command: "echo pre".to_string(),
                args: vec![],
                timeout: 30,
                working_dir: None,
                env_vars: std::collections::HashMap::new(),
            }],
            post_plugin: vec![HookAction::Script {
                command: "echo post".to_string(),
                args: vec![],
                timeout: 30,
                working_dir: None,
                env_vars: std::collections::HashMap::new(),
            }],
        };

        assert_eq!(hooks.pre_plugin.len(), 1);
        assert_eq!(hooks.post_plugin.len(), 1);
    }

    /// Test ConfigMixin default implementations
    #[test]
    fn test_config_mixin_defaults() {
        let plugin = MockPlugin::default();
        assert!(plugin.config().is_none());
        assert!(plugin.get_target_path().is_none());
        assert!(plugin.get_output_file().is_none());
        assert!(plugin.get_restore_target_dir().is_none());
    }

    /// Test ConfigMixin with valid configuration
    #[test]
    fn test_config_mixin_with_config() {
        let config = StandardConfig {
            target_path: Some("test/path".to_string()),
            output_file: Some("test.json".to_string()),
            restore_target_dir: Some("/test/restore".to_string()),
            hooks: None,
        };

        let plugin = MockPlugin {
            config: Some(config),
        };

        assert!(plugin.config().is_some());
        assert_eq!(plugin.get_target_path(), Some("test/path".to_string()));
        assert_eq!(plugin.get_output_file(), Some("test.json".to_string()));
        assert_eq!(
            plugin.get_restore_target_dir(),
            Some("/test/restore".to_string())
        );
    }

    /// Test with_config_validation with valid TOML
    #[test]
    fn test_with_config_validation_valid() {
        let toml_str = r#"
            target_path = "custom/path"
            output_file = "custom.json"
            restore_target_dir = "/custom/restore"
        "#;
        let toml_value: toml::Value = toml::from_str(toml_str).unwrap();

        let (config, is_valid) = MockPlugin::with_config_validation(
            toml_value,
            "test_plugin",
            "[plugins.test_plugin]",
            "target_path, output_file, restore_target_dir",
            "target_path = 'path'\\noutput_file = 'file.json'",
        );

        assert!(is_valid);
        assert_eq!(config.target_path, Some("custom/path".to_string()));
        assert_eq!(config.output_file, Some("custom.json".to_string()));
        assert_eq!(
            config.restore_target_dir,
            Some("/custom/restore".to_string())
        );
    }

    /// Test with_config_validation with hooks
    #[test]
    fn test_with_config_validation_with_hooks() {
        let toml_str = r#"
            target_path = "custom/path"
            
            [hooks]
            pre-plugin = [
                { action = "script", command = "echo", args = ["pre"], timeout = 30 }
            ]
            post-plugin = [
                { action = "script", command = "echo", args = ["post"], timeout = 30 }
            ]
        "#;
        let toml_value: toml::Value = toml::from_str(toml_str).unwrap();

        let (config, is_valid) = MockPlugin::with_config_validation(
            toml_value,
            "test_plugin",
            "[plugins.test_plugin]",
            "target_path, hooks",
            "target_path = 'path'",
        );

        assert!(is_valid);
        assert_eq!(config.target_path, Some("custom/path".to_string()));
        assert!(config.hooks.is_some());

        let hooks = config.hooks.as_ref().unwrap();
        assert_eq!(hooks.pre_plugin.len(), 1);
        assert_eq!(hooks.post_plugin.len(), 1);
    }

    /// Test with_config_validation with invalid TOML
    #[test]
    fn test_with_config_validation_invalid() {
        // Create invalid TOML value (wrong type for target_path)
        let mut invalid_config = toml::Table::new();
        invalid_config.insert("target_path".to_string(), toml::Value::Integer(123));
        let toml_value = toml::Value::Table(invalid_config);

        let (config, is_valid) = MockPlugin::with_config_validation(
            toml_value,
            "test_plugin",
            "[plugins.test_plugin]",
            "target_path",
            "target_path = 'path'",
        );

        assert!(!is_valid);
        // Should return default config when validation fails
        assert!(config.target_path.is_none());
        assert!(config.output_file.is_none());
        assert!(config.restore_target_dir.is_none());
    }

    /// Test with_config_validation with empty TOML
    #[test]
    fn test_with_config_validation_empty() {
        let toml_value = toml::Value::Table(toml::Table::new());

        let (config, is_valid) = MockPlugin::with_config_validation(
            toml_value,
            "test_plugin",
            "[plugins.test_plugin]",
            "target_path",
            "target_path = 'path'",
        );

        assert!(is_valid); // Empty config should be valid (uses defaults)
        assert!(config.target_path.is_none());
        assert!(config.output_file.is_none());
        assert!(config.restore_target_dir.is_none());
    }

    /// Test StandardConfigMixin methods
    #[test]
    fn test_standard_config_mixin_methods() {
        let config = StandardConfig {
            target_path: Some("mixin/path".to_string()),
            output_file: Some("mixin.json".to_string()),
            restore_target_dir: Some("/mixin/restore".to_string()),
            hooks: None,
        };

        let plugin = MockPlugin {
            config: Some(config),
        };

        assert_eq!(
            plugin.get_standard_target_path(),
            Some("mixin/path".to_string())
        );
        assert_eq!(
            plugin.get_standard_output_file(),
            Some("mixin.json".to_string())
        );
        assert_eq!(
            plugin.get_standard_restore_target_dir(),
            Some("/mixin/restore".to_string())
        );
    }

    /// Test StandardConfigMixin methods with no config
    #[test]
    fn test_standard_config_mixin_no_config() {
        let plugin = MockPlugin::default();

        assert!(plugin.get_standard_target_path().is_none());
        assert!(plugin.get_standard_output_file().is_none());
        assert!(plugin.get_standard_restore_target_dir().is_none());
    }

    /// Test serialize/deserialize of StandardConfig
    #[test]
    fn test_standard_config_serde() {
        let config = StandardConfig {
            target_path: Some("serde/path".to_string()),
            output_file: Some("serde.json".to_string()),
            restore_target_dir: Some("/serde/restore".to_string()),
            hooks: Some(StandardHooks {
                pre_plugin: vec![HookAction::Script {
                    command: "echo".to_string(),
                    args: vec!["test".to_string()],
                    timeout: 30,
                    working_dir: None,
                    env_vars: std::collections::HashMap::new(),
                }],
                post_plugin: vec![],
            }),
        };

        // Test serialization to TOML
        let serialized = toml::to_string(&config).unwrap();
        assert!(serialized.contains("target_path"));
        assert!(serialized.contains("serde/path"));
        assert!(serialized.contains("pre-plugin"));

        // Test deserialization from TOML
        let deserialized: StandardConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.target_path, config.target_path);
        assert_eq!(deserialized.output_file, config.output_file);
        assert_eq!(deserialized.restore_target_dir, config.restore_target_dir);
        assert!(deserialized.hooks.is_some());
    }

    /// Test complex hooks configuration
    #[test]
    fn test_complex_hooks_config() {
        let toml_str = r#"
            target_path = "complex/path"
            
            [hooks]
            pre-plugin = [
                { action = "script", command = "pre1", args = ["arg1", "arg2"], timeout = 30 },
                { action = "script", command = "pre2", args = [], timeout = 30 }
            ]
            post-plugin = [
                { action = "script", command = "post1", args = ["post_arg"], timeout = 30, working_dir = "/tmp" }
            ]
        "#;
        let toml_value: toml::Value = toml::from_str(toml_str).unwrap();

        let (config, is_valid) = MockPlugin::with_config_validation(
            toml_value,
            "complex_plugin",
            "[plugins.complex_plugin]",
            "target_path, hooks",
            "target_path = 'path'",
        );

        assert!(is_valid);
        assert_eq!(config.target_path, Some("complex/path".to_string()));

        let hooks = config.hooks.as_ref().unwrap();
        assert_eq!(hooks.pre_plugin.len(), 2);
        assert_eq!(hooks.post_plugin.len(), 1);

        // Verify hook details
        if let HookAction::Script {
            command,
            args,
            working_dir,
            ..
        } = &hooks.pre_plugin[0]
        {
            assert_eq!(command, "pre1");
            assert_eq!(args, &vec!["arg1".to_string(), "arg2".to_string()]);
            assert_eq!(working_dir, &None);
        } else {
            panic!("Expected script hook");
        }

        if let HookAction::Script {
            command,
            args,
            working_dir,
            ..
        } = &hooks.post_plugin[0]
        {
            assert_eq!(command, "post1");
            assert_eq!(args, &vec!["post_arg".to_string()]);
            assert_eq!(working_dir, &Some(std::path::PathBuf::from("/tmp")));
        } else {
            panic!("Expected script hook");
        }
    }

    /// Test StandardConfig from_toml_value method
    /// Verifies the ConfigSchema trait implementation on StandardConfig
    #[test]
    fn test_standard_config_from_toml_value() {
        let toml_str = r#"
            target_path = "from_toml/path"
            output_file = "from_toml.json"
            restore_target_dir = "/from_toml/restore"
        "#;
        let toml_value: toml::Value = toml::from_str(toml_str).unwrap();

        let result = StandardConfig::from_toml_value(&toml_value);
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.target_path, Some("from_toml/path".to_string()));
        assert_eq!(config.output_file, Some("from_toml.json".to_string()));
        assert_eq!(
            config.restore_target_dir,
            Some("/from_toml/restore".to_string())
        );
    }

    /// Test StandardConfig from_toml_value with invalid data
    /// Verifies error handling in ConfigSchema trait implementation
    #[test]
    fn test_standard_config_from_toml_value_invalid() {
        // Create invalid TOML with wrong type
        let mut invalid_config = toml::Table::new();
        invalid_config.insert("target_path".to_string(), toml::Value::Integer(456));
        let toml_value = toml::Value::Table(invalid_config);

        let result = StandardConfig::from_toml_value(&toml_value);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error
            .to_string()
            .contains("Failed to parse StandardConfig configuration"));
    }

    /// Test with_config_validation with malformed hooks
    /// Verifies error handling when hook configuration is invalid
    #[test]
    fn test_with_config_validation_malformed_hooks() {
        let toml_str = r#"
            target_path = "malformed/path"
            
            [hooks]
            pre-plugin = [
                { action = "invalid_action", command = "test" }
            ]
        "#;
        let toml_value: toml::Value = toml::from_str(toml_str).unwrap();

        let (config, is_valid) = MockPlugin::with_config_validation(
            toml_value,
            "malformed_plugin",
            "[plugins.malformed_plugin]",
            "target_path, hooks",
            "target_path = 'path'",
        );

        assert!(!is_valid); // Should fail validation due to invalid hook action
                            // Should return default config when validation fails
        assert!(config.target_path.is_none());
        assert!(config.hooks.is_none());
    }

    /// Test StandardHooks with empty vectors
    /// Verifies proper handling of empty hook configurations
    #[test]
    fn test_standard_hooks_empty() {
        let hooks = StandardHooks {
            pre_plugin: vec![],
            post_plugin: vec![],
        };

        assert_eq!(hooks.pre_plugin.len(), 0);
        assert_eq!(hooks.post_plugin.len(), 0);
    }

    /// Test StandardHooks serialization/deserialization
    /// Verifies proper serde handling of hooks structure
    #[test]
    fn test_standard_hooks_serde() {
        let hooks = StandardHooks {
            pre_plugin: vec![HookAction::Script {
                command: "pre_hook".to_string(),
                args: vec!["arg1".to_string()],
                timeout: 60,
                working_dir: Some(std::path::PathBuf::from("/working")),
                env_vars: {
                    let mut env = std::collections::HashMap::new();
                    env.insert("VAR1".to_string(), "value1".to_string());
                    env
                },
            }],
            post_plugin: vec![HookAction::Script {
                command: "post_hook".to_string(),
                args: vec![],
                timeout: 30,
                working_dir: None,
                env_vars: std::collections::HashMap::new(),
            }],
        };

        // Test serialization
        let serialized = toml::to_string(&hooks).unwrap();
        assert!(serialized.contains("pre-plugin"));
        assert!(serialized.contains("post-plugin"));
        assert!(serialized.contains("pre_hook"));
        assert!(serialized.contains("post_hook"));

        // Test deserialization
        let deserialized: StandardHooks = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.pre_plugin.len(), 1);
        assert_eq!(deserialized.post_plugin.len(), 1);
    }

    /// Test with_config_validation with partially invalid TOML
    /// Verifies behavior when some fields are valid and others are not
    #[test]
    fn test_with_config_validation_partially_invalid() {
        let mut mixed_config = toml::Table::new();
        mixed_config.insert(
            "target_path".to_string(),
            toml::Value::String("valid/path".to_string()),
        );
        mixed_config.insert("output_file".to_string(), toml::Value::Integer(789)); // Invalid type
        let toml_value = toml::Value::Table(mixed_config);

        let (config, is_valid) = MockPlugin::with_config_validation(
            toml_value,
            "mixed_plugin",
            "[plugins.mixed_plugin]",
            "target_path, output_file",
            "target_path = 'path'\\noutput_file = 'file.json'",
        );

        assert!(!is_valid); // Should fail due to invalid output_file type
        assert!(config.target_path.is_none()); // Should get default config
        assert!(config.output_file.is_none());
    }

    /// Test ConfigMixin trait default implementations extensively  
    /// Verifies all default method behaviors work correctly
    #[test]
    fn test_config_mixin_trait_defaults() {
        // Create a plugin that uses default ConfigMixin implementations
        #[derive(Default)]
        struct DefaultPlugin {
            config: Option<StandardConfig>,
        }

        impl ConfigMixin for DefaultPlugin {
            type Config = StandardConfig;

            fn config(&self) -> Option<&Self::Config> {
                self.config.as_ref()
            }
            // Uses all default implementations for get_* methods
        }

        let plugin = DefaultPlugin::default();

        // All methods should return None due to default implementations
        assert!(plugin.get_target_path().is_none());
        assert!(plugin.get_output_file().is_none());
        assert!(plugin.get_restore_target_dir().is_none());
        assert!(plugin.config().is_none());
    }
}

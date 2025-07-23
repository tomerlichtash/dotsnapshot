use crate::symbols::*;
use anyhow::{Context, Result};
use schemars::{schema_for, JsonSchema};
use serde::de::DeserializeOwned;
use std::collections::HashMap;

/// Trait for plugin configuration validation with schema support
pub trait ConfigSchema: DeserializeOwned + JsonSchema {
    /// The name of this configuration schema (for error reporting)
    fn schema_name() -> &'static str;

    /// Validate the configuration and provide detailed error messages
    fn validate(&self) -> Result<()> {
        // Default implementation - can be overridden for custom validation
        Ok(())
    }

    /// Get the JSON schema for this configuration
    #[allow(dead_code)]
    fn get_schema() -> schemars::schema::RootSchema {
        schema_for!(Self)
    }

    /// Parse and validate configuration from TOML value
    fn from_toml_value(value: &toml::Value) -> Result<Self> {
        // First, deserialize the configuration
        let config: Self = value.clone().try_into().with_context(|| {
            format!(
                "Failed to parse {} configuration",
                <Self as ConfigSchema>::schema_name()
            )
        })?;

        // Then validate it
        config.validate().with_context(|| {
            format!(
                "Invalid {} configuration",
                <Self as ConfigSchema>::schema_name()
            )
        })?;

        Ok(config)
    }
}

/// Configuration validation error with detailed context
#[allow(dead_code)]
#[derive(Debug, thiserror::Error)]
pub enum ConfigValidationError {
    #[error("Missing required field '{field}' in {plugin} configuration")]
    MissingField { plugin: String, field: String },

    #[error("Invalid value for field '{field}' in {plugin} configuration: {reason}")]
    InvalidValue {
        plugin: String,
        field: String,
        reason: String,
    },

    #[error("Configuration schema mismatch for {plugin}: {details}")]
    SchemaMismatch { plugin: String, details: String },

    #[error("Custom validation failed for {plugin}: {message}")]
    CustomValidation { plugin: String, message: String },
}

/// Helper functions for common validation patterns
pub struct ValidationHelpers;

impl ValidationHelpers {
    /// Validate that a path exists (for file/directory fields)
    #[allow(dead_code)]
    pub fn validate_path_exists(path: &str) -> Result<()> {
        let expanded_path = shellexpand::tilde(path);
        if !std::path::Path::new(expanded_path.as_ref()).exists() {
            return Err(anyhow::anyhow!("Path does not exist: {}", path));
        }
        Ok(())
    }

    /// Validate that a command exists in PATH
    pub fn validate_command_exists(command: &str) -> Result<()> {
        which::which(command).with_context(|| format!("Command '{command}' not found in PATH"))?;
        Ok(())
    }

    /// Validate file extension
    pub fn validate_file_extension(filename: &str, allowed_extensions: &[&str]) -> Result<()> {
        if let Some(extension) = std::path::Path::new(filename).extension() {
            let ext_str = extension.to_string_lossy();
            if !allowed_extensions.contains(&ext_str.as_ref()) {
                return Err(anyhow::anyhow!(
                    "Invalid file extension '{}'. Allowed extensions: {:?}",
                    ext_str,
                    allowed_extensions
                ));
            }
        } else if !allowed_extensions.is_empty() {
            return Err(anyhow::anyhow!(
                "File must have an extension. Allowed extensions: {:?}",
                allowed_extensions
            ));
        }
        Ok(())
    }

    /// Validate timeout value (must be positive and reasonable)
    #[allow(dead_code)]
    pub fn validate_timeout(timeout: u64) -> Result<()> {
        if timeout == 0 {
            return Err(anyhow::anyhow!("Timeout must be greater than 0"));
        }
        if timeout > 3600 {
            return Err(anyhow::anyhow!(
                "Timeout must be less than 3600 seconds (1 hour)"
            ));
        }
        Ok(())
    }

    /// Format a comprehensive validation error message for plugins
    pub fn format_validation_error(
        plugin_display_name: &str,
        plugin_config_key: &str,
        valid_fields: &str,
        example: &str,
        error: &anyhow::Error,
    ) -> String {
        format!(
            "{INDICATOR_WARNING}  Configuration validation failed for {plugin_display_name}:\n\
             {INDICATOR_INFO} Error details: {error}\n\
             {TOOL_CONFIG} Please check your configuration in 'dotsnapshot.toml' under [plugins.{plugin_config_key}]\n\
             {DOC_BOOK} Valid fields: {valid_fields}\n\
             {EXPERIENCE_IDEA} Example: {example}\n\
             {EXPERIENCE_SPEED} Plugin will continue with default configuration."
        )
    }
}

/// Configuration documentation generator
#[allow(dead_code)]
pub struct ConfigDocGenerator;

impl ConfigDocGenerator {
    /// Generate markdown documentation for a configuration schema
    #[allow(dead_code)]
    pub fn generate_markdown_docs<T: ConfigSchema>() -> String {
        let schema = T::get_schema();
        let mut docs = String::new();

        docs.push_str(&format!(
            "# {} Configuration\n\n",
            <T as ConfigSchema>::schema_name()
        ));

        if let Some(description) = &schema
            .schema
            .metadata
            .as_ref()
            .and_then(|m| m.description.as_ref())
        {
            docs.push_str(&format!("{description}\n\n"));
        }

        // Add schema information
        docs.push_str("## Configuration Schema\n\n");
        docs.push_str("```json\n");
        docs.push_str(&serde_json::to_string_pretty(&schema).unwrap_or_default());
        docs.push_str("\n```\n\n");

        docs
    }

    /// Generate example configuration
    #[allow(dead_code)]
    pub fn generate_example_config<T: ConfigSchema>() -> HashMap<String, toml::Value> {
        // This would generate example values based on the schema
        // For now, return empty map - can be enhanced later
        HashMap::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    struct TestConfig {
        #[schemars(description = "The target path for output")]
        target_path: Option<String>,

        #[schemars(description = "The output filename")]
        output_file: Option<String>,

        #[schemars(description = "Timeout in seconds")]
        timeout: Option<u64>,
    }

    impl ConfigSchema for TestConfig {
        fn schema_name() -> &'static str {
            "TestConfig"
        }

        fn validate(&self) -> Result<()> {
            if let Some(timeout) = self.timeout {
                ValidationHelpers::validate_timeout(timeout)?;
            }

            if let Some(output_file) = &self.output_file {
                ValidationHelpers::validate_file_extension(output_file, &["txt", "json", "toml"])?;
            }

            Ok(())
        }
    }

    #[test]
    fn test_config_schema_validation() {
        let valid_config = TestConfig {
            target_path: Some("test".to_string()),
            output_file: Some("test.json".to_string()),
            timeout: Some(30),
        };

        assert!(valid_config.validate().is_ok());
    }

    #[test]
    fn test_config_schema_invalid_timeout() {
        let invalid_config = TestConfig {
            target_path: Some("test".to_string()),
            output_file: Some("test.json".to_string()),
            timeout: Some(0), // Invalid: zero timeout
        };

        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_config_schema_invalid_extension() {
        let invalid_config = TestConfig {
            target_path: Some("test".to_string()),
            output_file: Some("test.invalid".to_string()), // Invalid extension
            timeout: Some(30),
        };

        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_config_from_toml_value() {
        let toml_str = r#"
            target_path = "test"
            output_file = "test.json"
            timeout = 30
        "#;

        let toml_value: toml::Value = toml::from_str(toml_str).unwrap();
        let config = TestConfig::from_toml_value(&toml_value);

        assert!(config.is_ok());
    }

    #[test]
    fn test_schema_generation() {
        let _schema = TestConfig::get_schema();
        // Just verify we can generate a schema without panicking
    }

    #[test]
    fn test_documentation_generation() {
        let docs = ConfigDocGenerator::generate_markdown_docs::<TestConfig>();
        assert!(docs.contains("TestConfig Configuration"));
        assert!(docs.contains("Configuration Schema"));
    }

    #[test]
    fn test_validation_helpers_file_extension() {
        // Valid extensions
        assert!(
            ValidationHelpers::validate_file_extension("config.json", &["json", "yaml"]).is_ok()
        );
        assert!(ValidationHelpers::validate_file_extension("data.yaml", &["json", "yaml"]).is_ok());

        // Invalid extension
        assert!(ValidationHelpers::validate_file_extension("file.txt", &["json", "yaml"]).is_err());

        // No extension when extensions required
        assert!(ValidationHelpers::validate_file_extension("noext", &["json"]).is_err());

        // No extension when no specific extensions required
        assert!(ValidationHelpers::validate_file_extension("noext", &[]).is_ok());
    }

    #[test]
    fn test_validation_helpers_timeout() {
        // Valid timeouts
        assert!(ValidationHelpers::validate_timeout(1).is_ok());
        assert!(ValidationHelpers::validate_timeout(30).is_ok());
        assert!(ValidationHelpers::validate_timeout(3600).is_ok());

        // Invalid timeouts
        assert!(ValidationHelpers::validate_timeout(0).is_err());
        assert!(ValidationHelpers::validate_timeout(3601).is_err());
        assert!(ValidationHelpers::validate_timeout(10000).is_err());
    }

    #[test]
    fn test_config_schema_error_context() {
        let invalid_toml_str = r#"
            target_path = "test"
            output_file = "test.exe"  # Invalid extension
            timeout = 0  # Invalid timeout
        "#;

        let toml_value: toml::Value = toml::from_str(invalid_toml_str).unwrap();
        let result = TestConfig::from_toml_value(&toml_value);

        assert!(result.is_err());
        let error = result.unwrap_err();
        let error_string = error.to_string();

        // Check that error contains context
        assert!(error_string.contains("Invalid TestConfig configuration"));
    }

    #[test]
    fn test_multiple_validation_errors() {
        // Test config with multiple validation issues
        let config = TestConfig {
            target_path: Some("test".to_string()),
            output_file: Some("test.exe".to_string()), // Invalid extension
            timeout: Some(0),                          // Invalid timeout
        };

        let result = config.validate();
        assert!(result.is_err());

        // The error should be about the first validation that fails
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Timeout must be greater than 0"));
    }
}

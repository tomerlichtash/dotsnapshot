use crate::symbols::*;
use anyhow::{Context, Result};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

/// Trait for plugin configuration validation with schema support
pub trait ConfigSchema: DeserializeOwned + JsonSchema {
    /// The name of this configuration schema (for error reporting)
    fn schema_name() -> &'static str;

    /// Validate the configuration and provide detailed error messages
    fn validate(&self) -> Result<()> {
        // Default implementation - can be overridden for custom validation
        Ok(())
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

/// Helper functions for common validation patterns
pub struct ValidationHelpers;

impl ValidationHelpers {
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

    /// Format a comprehensive validation error message for plugins
    pub fn format_validation_error(
        plugin_display_name: &str,
        plugin_config_key: &str,
        valid_fields: &str,
        example: &str,
        error: &anyhow::Error,
    ) -> String {
        // Extract the most specific error from the chain
        let root_error = error
            .chain()
            .last()
            .map(|e| e.to_string())
            .unwrap_or_else(|| error.to_string());

        format!(
            "{SYMBOL_INDICATOR_WARNING} Configuration validation failed for {plugin_display_name}\n\
             Error: {root_error}\n\
             Check: [plugins.{plugin_config_key}] in dotsnapshot.toml\n\
             Valid fields: {valid_fields}\n\
             Example: {example}\n\
             Note: Plugin will continue with default configuration"
        )
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
    }

    impl ConfigSchema for TestConfig {
        fn schema_name() -> &'static str {
            "TestConfig"
        }

        fn validate(&self) -> Result<()> {
            if let Some(output_file) = &self.output_file {
                ValidationHelpers::validate_file_extension(output_file, &["txt", "json", "toml"])?;
            }

            Ok(())
        }
    }

    /// Test that a valid configuration passes validation
    /// This ensures the basic validation flow works correctly for valid inputs
    #[test]
    fn test_config_schema_validation() {
        let valid_config = TestConfig {
            target_path: Some("test".to_string()),
            output_file: Some("test.json".to_string()), // Valid extension
        };

        assert!(valid_config.validate().is_ok());
    }

    /// Test that configuration with invalid file extension fails validation
    /// This ensures file extension validation is properly enforced
    #[test]
    fn test_config_schema_invalid_extension() {
        let invalid_config = TestConfig {
            target_path: Some("test".to_string()),
            output_file: Some("test.invalid".to_string()), // Extension not in allowed list
        };

        assert!(invalid_config.validate().is_err());
    }

    /// Test deserializing configuration from TOML with validation
    /// This verifies the complete flow: TOML → struct → validation
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

    /// Test file extension validation helper with various scenarios
    /// This ensures the helper correctly validates extensions in all cases
    #[test]
    fn test_validation_helpers_file_extension() {
        // Case 1: Valid extensions should pass
        assert!(
            ValidationHelpers::validate_file_extension("config.json", &["json", "yaml"]).is_ok()
        );
        assert!(ValidationHelpers::validate_file_extension("data.yaml", &["json", "yaml"]).is_ok());

        // Case 2: Invalid extension should fail
        assert!(ValidationHelpers::validate_file_extension("file.txt", &["json", "yaml"]).is_err());

        // Case 3: No extension when extensions are required should fail
        assert!(ValidationHelpers::validate_file_extension("noext", &["json"]).is_err());

        // Case 4: No extension is OK when no specific extensions are required
        assert!(ValidationHelpers::validate_file_extension("noext", &[]).is_ok());
    }

    /// Test that validation errors include proper context information
    /// This ensures users get helpful error messages with context about what went wrong
    #[test]
    fn test_config_schema_error_context() {
        let invalid_toml_str = r#"
            target_path = "test"
            output_file = "test.exe"  # Invalid extension
        "#;

        let toml_value: toml::Value = toml::from_str(invalid_toml_str).unwrap();
        let result = TestConfig::from_toml_value(&toml_value);

        assert!(result.is_err());
        let error = result.unwrap_err();
        let error_string = error.to_string();

        // Verify error includes the configuration type name for context
        assert!(error_string.contains("Invalid TestConfig configuration"));
    }

    /// Test handling of multiple validation errors in a single configuration
    /// This verifies that validation stops at the first error and reports it clearly
    #[test]
    fn test_multiple_validation_errors() {
        // Configuration with an invalid file extension
        let config = TestConfig {
            target_path: Some("test".to_string()),
            output_file: Some("test.exe".to_string()), // Not in allowed extensions
        };

        let result = config.validate();
        assert!(result.is_err());

        // Verify the error message is specific about what failed
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid file extension"));
    }
}

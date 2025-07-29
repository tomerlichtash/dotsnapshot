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
    /// Get well-known configuration files that don't need extensions
    fn get_well_known_no_extension() -> Vec<String> {
        // Try to load from config file, fall back to defaults
        // Note: This is called during validation, so we need to be careful about errors
        let config_paths = crate::config::Config::get_config_paths();
        for config_path in config_paths {
            if config_path.exists() {
                if let Ok(config_content) = std::fs::read_to_string(&config_path) {
                    if let Ok(parsed_config) =
                        toml::from_str::<crate::config::Config>(&config_content)
                    {
                        if let Some(validation_config) = parsed_config.validation {
                            if let Some(custom_list) = validation_config.well_known_no_extension {
                                return custom_list;
                            }
                        }
                    }
                }
            }
        }

        // Default well-known configuration files that don't traditionally have extensions
        vec![
            "Brewfile".to_string(),    // Homebrew dependency file
            "Dockerfile".to_string(),  // Docker container definition
            "Makefile".to_string(),    // Make build file
            "Vagrantfile".to_string(), // Vagrant configuration
            "Gemfile".to_string(),     // Ruby gem dependencies
            "Podfile".to_string(),     // CocoaPods dependencies
        ]
    }

    /// Validate file extension
    pub fn validate_file_extension(filename: &str, allowed_extensions: &[&str]) -> Result<()> {
        let well_known_files = Self::get_well_known_no_extension();
        let well_known_refs: Vec<&str> = well_known_files.iter().map(|s| s.as_str()).collect();

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
            // Allow well-known configuration files without extensions
            if !well_known_refs.contains(&filename) {
                // For dot files (like .npmrc), check if the filename without the leading dot
                // matches any allowed extensions
                let filename_without_dot = filename.strip_prefix('.').unwrap_or(filename);
                if !allowed_extensions.contains(&filename_without_dot) {
                    return Err(anyhow::anyhow!(
                        "File must have an extension. Allowed extensions: {:?}",
                        allowed_extensions
                    ));
                }
            }
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

    /// Test format_validation_error helper function
    /// This ensures comprehensive error formatting for plugin configuration issues
    #[test]
    fn test_format_validation_error() {
        let test_error = anyhow::anyhow!("Test validation error");

        let formatted_error = ValidationHelpers::format_validation_error(
            "Test Plugin",
            "test_plugin",
            "target_path, output_file",
            "target_path = \"~/test\"",
            &test_error,
        );

        // Verify all components are included in the formatted error
        assert!(formatted_error.contains("Test Plugin"));
        assert!(formatted_error.contains("test_plugin"));
        assert!(formatted_error.contains("target_path, output_file"));
        assert!(formatted_error.contains("target_path = \"~/test\""));
        assert!(formatted_error.contains("Test validation error"));
        assert!(formatted_error.contains("Plugin will continue with default configuration"));
    }

    /// Test format_validation_error with error chain
    /// This verifies that complex error chains are properly handled
    #[test]
    fn test_format_validation_error_with_chain() {
        // Create a chain of errors
        let root_error = anyhow::anyhow!("Root cause error");
        let wrapped_error = root_error.context("Wrapper error").context("Outer error");

        let formatted_error = ValidationHelpers::format_validation_error(
            "Complex Plugin",
            "complex",
            "complex_field",
            "complex_field = \"value\"",
            &wrapped_error,
        );

        // Should extract the root cause (last error in chain)
        assert!(formatted_error.contains("Root cause error"));
    }

    /// Test file extension validation with empty allowed extensions
    /// This ensures proper behavior when no specific extensions are required
    #[test]
    fn test_validation_helpers_empty_extensions() {
        // When empty extensions are allowed, only files WITHOUT extensions should pass
        assert!(ValidationHelpers::validate_file_extension("any.file", &[]).is_err());
        assert!(ValidationHelpers::validate_file_extension("no_extension", &[]).is_ok());
        assert!(ValidationHelpers::validate_file_extension("complex.ext.name", &[]).is_err());

        // Files with just names (no extension) should pass
        assert!(ValidationHelpers::validate_file_extension("filename", &[]).is_ok());
        assert!(ValidationHelpers::validate_file_extension("simple", &[]).is_ok());
    }

    /// Test file extension validation with edge cases
    /// This ensures robust handling of unusual file names
    #[test]
    fn test_validation_helpers_extension_edge_cases() {
        // Files with multiple dots
        assert!(ValidationHelpers::validate_file_extension("file.tar.gz", &["gz"]).is_ok());
        assert!(
            ValidationHelpers::validate_file_extension("config.backup.json", &["json"]).is_ok()
        );

        // Hidden files with extensions
        assert!(ValidationHelpers::validate_file_extension(".hidden.json", &["json"]).is_ok());
        assert!(ValidationHelpers::validate_file_extension(".hidden", &["json"]).is_err());

        // Files with just dots
        assert!(ValidationHelpers::validate_file_extension(".", &["json"]).is_err());
        assert!(ValidationHelpers::validate_file_extension("..", &["json"]).is_err());

        // Well-known configuration files without extensions should pass
        assert!(
            ValidationHelpers::validate_file_extension("Brewfile", &["txt", "brewfile"]).is_ok()
        );
        assert!(ValidationHelpers::validate_file_extension("Dockerfile", &["txt"]).is_ok());
        assert!(ValidationHelpers::validate_file_extension("Makefile", &["txt"]).is_ok());

        // Dot files should work if filename matches allowed extension
        assert!(ValidationHelpers::validate_file_extension(".npmrc", &["npmrc", "config"]).is_ok());
        assert!(ValidationHelpers::validate_file_extension(".gitignore", &["gitignore"]).is_ok());
    }

    /// Test ConfigSchema trait default validate method
    /// This ensures the default implementation works as expected
    #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    struct MinimalTestConfig {
        name: Option<String>,
    }

    impl ConfigSchema for MinimalTestConfig {
        fn schema_name() -> &'static str {
            "MinimalTestConfig"
        }
        // Uses default validate() implementation
    }

    #[test]
    fn test_config_schema_default_validation() {
        let config = MinimalTestConfig {
            name: Some("test".to_string()),
        };

        // Default implementation should always return Ok(())
        assert!(config.validate().is_ok());
    }

    /// Test ConfigSchema from_toml_value with serialization failure
    /// This verifies proper error handling when TOML cannot be deserialized
    #[test]
    fn test_config_from_toml_value_serialization_failure() {
        // TOML with wrong type for expected field
        let invalid_toml_str = r#"
            target_path = 123  # Should be string, not number
            output_file = "test.json"
        "#;

        let toml_value: toml::Value = toml::from_str(invalid_toml_str).unwrap();
        let result = TestConfig::from_toml_value(&toml_value);

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error
            .to_string()
            .contains("Failed to parse TestConfig configuration"));
    }

    /// Test ConfigSchema from_toml_value with validation failure
    /// This verifies that validation errors are properly contextized
    #[test]
    fn test_config_from_toml_value_validation_failure() {
        // Valid TOML but fails validation
        let invalid_toml_str = r#"
            target_path = "test"
            output_file = "test.invalid"  # Invalid extension
        "#;

        let toml_value: toml::Value = toml::from_str(invalid_toml_str).unwrap();
        let result = TestConfig::from_toml_value(&toml_value);

        assert!(result.is_err());
        let error = result.unwrap_err();
        let error_string = error.to_string();
        assert!(error_string.contains("Invalid TestConfig configuration"));
    }

    /// Test schema_name method access
    /// This ensures the trait method works correctly
    #[test]
    fn test_config_schema_name() {
        assert_eq!(<TestConfig as ConfigSchema>::schema_name(), "TestConfig");
        assert_eq!(
            <MinimalTestConfig as ConfigSchema>::schema_name(),
            "MinimalTestConfig"
        );
    }

    /// Test file extension validation error messages
    /// This ensures error messages are helpful and specific
    #[test]
    fn test_validation_helpers_error_messages() {
        // Test error for invalid extension
        let result = ValidationHelpers::validate_file_extension("file.exe", &["json", "yaml"]);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid file extension 'exe'"));
        assert!(error
            .to_string()
            .contains("Allowed extensions: [\"json\", \"yaml\"]"));

        // Test error for missing extension
        let result = ValidationHelpers::validate_file_extension("noext", &["json"]);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("File must have an extension"));
        assert!(error.to_string().contains("Allowed extensions: [\"json\"]"));
    }
}

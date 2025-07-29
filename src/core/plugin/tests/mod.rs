//! Tests for plugin system functionality

use crate::config::{Config, UiConfig};
use crate::core::plugin::{Plugin, PluginDescriptor, PluginRegistry, PluginResult};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

/// Mock plugin for testing basic functionality
/// Simple implementation for testing core plugin features
pub struct MockPlugin;

#[async_trait::async_trait]
impl Plugin for MockPlugin {
    fn description(&self) -> &str {
        "Mock plugin"
    }

    fn icon(&self) -> &str {
        "🔧"
    }

    async fn execute(&self) -> Result<String> {
        Ok("test".to_string())
    }

    async fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn get_target_path(&self) -> Option<String> {
        None
    }

    fn get_output_file(&self) -> Option<String> {
        None
    }
}

/// Custom output plugin for testing custom output file behavior
/// Plugin that returns a custom output file name
pub struct CustomOutputPlugin;

#[async_trait::async_trait]
impl Plugin for CustomOutputPlugin {
    fn description(&self) -> &str {
        "Custom output plugin"
    }

    fn icon(&self) -> &str {
        "📝"
    }

    async fn execute(&self) -> Result<String> {
        Ok("test".to_string())
    }

    async fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn get_target_path(&self) -> Option<String> {
        None
    }

    fn get_output_file(&self) -> Option<String> {
        Some("custom-output.json".to_string())
    }
}

mod plugin_trait;
mod registry;
mod result;
mod utilities;

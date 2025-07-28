//! Modular test suite for StaticFilesPlugin
//!
//! This module organizes tests by functionality to improve maintainability
//! and make it easier to locate specific test scenarios.

pub mod test_utils;

// Test modules organized by functionality
pub mod configuration;
pub mod error_handling;
pub mod execution;
pub mod integration;
pub mod mock_cores;
pub mod plugin_core;
pub mod restoration;
pub mod validation;

//! Core plugin infrastructure
//!
//! This module provides the foundational components for the plugin system:
//! - Mixins for reusable functionality
//! - Base plugin types for common patterns
//!
//! The goal is to eliminate code duplication and provide a clean, maintainable
//! architecture for plugin development.

pub mod base;
pub mod mixins;

// Re-export commonly used items

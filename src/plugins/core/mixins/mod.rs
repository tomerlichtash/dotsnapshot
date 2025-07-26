//! Core mixin traits for plugin functionality
//!
//! This module provides reusable mixin traits that eliminate code duplication
//! across plugin implementations. Each mixin focuses on a specific aspect of
//! plugin functionality.

pub mod command;
pub mod config;
pub mod files;

// Re-export the main traits for easier access
pub use command::CommandMixin;
pub use config::{ConfigMixin, StandardConfig, StandardConfigMixin};
pub use files::FilesMixin;

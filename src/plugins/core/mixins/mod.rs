//! Core mixin traits for plugin functionality
//!
//! This module provides reusable mixin traits that eliminate code duplication
//! across plugin implementations. Each mixin focuses on a specific aspect of
//! plugin functionality.

pub mod command;
pub mod config;
pub mod files;
pub mod hooks;

// Re-export the main traits for easier access
pub use command::CommandMixin;
pub use config::{ConfigMixin, StandardConfig, StandardConfigMixin};
pub use files::FilesMixin;
pub use hooks::HooksMixin;

/// A convenient trait that combines all mixins for plugins that need everything
#[allow(dead_code)]
pub trait AllMixins: ConfigMixin + HooksMixin + CommandMixin + FilesMixin {}

// Note: Removed blanket implementation to avoid conflicts with specific implementations
// Each type that needs AllMixins should implement it explicitly

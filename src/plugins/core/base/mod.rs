//! Base plugin types that implement common patterns
//!
//! This module provides generic plugin implementations that can be specialized
//! for specific applications or use cases. Each base type handles a common
//! plugin pattern and uses the mixin system for shared functionality.

pub mod extensions;
pub mod keybindings;
pub mod package;
pub mod settings;
pub mod static_files;

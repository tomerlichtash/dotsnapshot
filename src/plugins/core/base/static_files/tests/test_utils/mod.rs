//! Test utilities for static_files plugin tests
//!
//! This module provides categorized mock implementations and helper functions
//! for testing the StaticFilesPlugin functionality.

pub mod advanced_mocks;
pub mod basic_mocks;
pub mod error_mocks;
pub mod helpers;

// Re-export all mocks and helpers for easy access
pub use advanced_mocks::AdvancedMockCore;
pub use basic_mocks::{MinimalStaticFilesCore, MockStaticFilesCore};
pub use error_mocks::{ErrorMockCore, ErrorProneMockCore, JsonErrorMockCore};
pub use helpers::*;

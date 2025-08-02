//! Include all hook tests from the centralized test directory.
//! This allows the inline `mod tests;` statement to find the tests
//! while keeping tests in the external modular structure.

#[path = "../../tests/core_hooks/actions.rs"]
mod actions;

#[path = "../../tests/core_hooks/config.rs"]
mod config;

#[path = "../../tests/core_hooks/context.rs"]
mod context;

#[path = "../../tests/core_hooks/execution.rs"]
mod execution;

#[path = "../../tests/core_hooks/patterns.rs"]
mod patterns;

#[path = "../../tests/core_hooks/test_utils.rs"]
mod test_utils;

# Clippy Configuration Notes

This document explains why certain clippy warnings are suppressed in this project.

## `large_enum_variant` Warnings

The `HooksCommands` enum contains a large variant (`Add` with `HookActionArgs`) that triggers clippy's `large_enum_variant` warning. This is suppressed because:

1. **CLI Argument Parsing**: The `HookActionArgs` struct contains many optional CLI arguments needed for the comprehensive hooks functionality
2. **Transient Usage**: This enum is only used during command parsing and is short-lived
3. **Clap Integration**: Boxing the large variant would complicate the clap derive macro usage
4. **Memory Impact**: The memory impact is minimal since these structs are not stored long-term

## `new_without_default` Warnings

Plugin structs use `new()` methods without implementing `Default` trait. This is intentional because:

1. **Consistency**: All plugins use `new()` for consistent instantiation patterns
2. **Future Extensibility**: Plugin constructors may need parameters in future versions
3. **Explicit Intent**: Using `new()` makes plugin instantiation more explicit than relying on `Default`

The warnings are suppressed with `#[allow(clippy::new_without_default)]` to maintain clean clippy output while preserving the intended design patterns.
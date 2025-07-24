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

## `uninlined_format_args` Warnings

Clippy requires using inline format arguments instead of positional arguments in format strings. This is enforced by our pre-commit hooks.

**Always use the inline syntax:**
```rust
// ✅ Correct - inline format arguments
println!("Restored paths: {restored_paths:?}");
format!("config for {plugin_name}");
warn!("DRY RUN: Would restore VSCode settings to {}", target_settings_file.display());

// ❌ Wrong - positional arguments (will fail clippy)
println!("Restored paths: {:?}", restored_paths);
format!("config for {}", plugin_name);
warn!("DRY RUN: Would restore VSCode settings to {}", target_settings_file.display());
```

**Key rules:**
1. Use `{variable_name}` instead of `{}` when the variable is available in scope
2. Use `{variable_name:?}` instead of `{:?}` for debug formatting
3. Use `{variable_name:display}` when calling `.display()` on paths
4. This applies to all format macros: `println!`, `format!`, `warn!`, `info!`, `error!`, etc.

This is enforced to improve code readability and reduce the chance of mismatched arguments.
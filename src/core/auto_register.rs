/// Convenience macro for auto-registering plugins
///
/// Usage:
/// ```rust,ignore
/// register_plugin!(HomebrewBrewfilePlugin, "homebrew_brewfile", "homebrew");
/// ```
#[macro_export]
macro_rules! register_plugin {
    ($plugin_type:ty, $name:literal, $category:literal) => {
        inventory::submit! {
            $crate::core::plugin::PluginDescriptor {
                name: $name,
                category: $category,
                factory: |config| {
                    std::sync::Arc::new(if let Some(config) = config {
                        <$plugin_type>::with_config(config)
                    } else {
                        <$plugin_type>::new()
                    })
                },
            }
        }
    };
}

/// Auto-register plugins that don't need configuration
#[macro_export]
macro_rules! register_simple_plugin {
    ($plugin_type:ty, $name:literal, $category:literal) => {
        inventory::submit! {
            $crate::core::plugin::PluginDescriptor {
                name: $name,
                category: $category,
                factory: |_config| {
                    std::sync::Arc::new(<$plugin_type>::new())
                },
            }
        }
    };
}

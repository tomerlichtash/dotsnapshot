//! Tests for plugin utility functions

use super::*;

#[test]
fn test_extract_category_from_plugin_name() {
    // Basic folder extraction
    assert_eq!(
        PluginRegistry::extract_category_from_plugin_name("vscode_settings", None),
        "Vscode"
    );
    assert_eq!(
        PluginRegistry::extract_category_from_plugin_name("cursor_extensions", None),
        "Cursor"
    );
    assert_eq!(
        PluginRegistry::extract_category_from_plugin_name("homebrew_brewfile", None),
        "Homebrew"
    );
    assert_eq!(
        PluginRegistry::extract_category_from_plugin_name("npm_global_packages", None),
        "Npm"
    );
    assert_eq!(
        PluginRegistry::extract_category_from_plugin_name("static_files", None),
        "Static"
    );

    // Single word plugins
    assert_eq!(
        PluginRegistry::extract_category_from_plugin_name("static", None),
        "Static"
    );

    // With config overrides
    let mut categories = HashMap::new();
    categories.insert("vscode".to_string(), "VSCode".to_string());
    categories.insert("npm".to_string(), "NPM".to_string());

    let config = Config {
        output_dir: None,
        include_plugins: None,
        logging: None,
        hooks: None,
        global: None,
        static_files: None,
        plugins: None,
        ui: Some(UiConfig {
            plugin_categories: Some(categories),
        }),
    };

    assert_eq!(
        PluginRegistry::extract_category_from_plugin_name("vscode_settings", Some(&config)),
        "VSCode"
    );
    assert_eq!(
        PluginRegistry::extract_category_from_plugin_name("npm_config", Some(&config)),
        "NPM"
    );
    assert_eq!(
        PluginRegistry::extract_category_from_plugin_name("cursor_settings", Some(&config)),
        "Cursor"
    );
}

#[test]
fn test_folder_name_to_category() {
    assert_eq!(PluginRegistry::folder_name_to_category("vscode"), "Vscode");
    assert_eq!(
        PluginRegistry::folder_name_to_category("static_files"),
        "Static Files"
    );
    assert_eq!(PluginRegistry::folder_name_to_category("npm"), "Npm");
    assert_eq!(
        PluginRegistry::folder_name_to_category("homebrew"),
        "Homebrew"
    );
    assert_eq!(PluginRegistry::folder_name_to_category("cursor"), "Cursor");
}

#[test]
fn test_derive_plugin_filename() {
    // Test auto-derived filenames always use .txt extension
    assert_eq!(
        PluginRegistry::derive_plugin_filename("vscode_settings"),
        "vscode_settings.txt"
    );
    assert_eq!(
        PluginRegistry::derive_plugin_filename("homebrew_brewfile"),
        "homebrew_brewfile.txt"
    );
    assert_eq!(
        PluginRegistry::derive_plugin_filename("npm_global_packages"),
        "npm_global_packages.txt"
    );
    assert_eq!(
        PluginRegistry::derive_plugin_filename("test_plugin"),
        "test_plugin.txt"
    );
}

#[test]
fn test_get_plugin_output_file_from_plugin() {
    use crate::plugins::core::base::settings::SettingsPlugin;
    use crate::plugins::core::base::static_files::StaticFilesPlugin;
    use crate::plugins::r#static::files::StaticFilesAppCore;
    use crate::plugins::vscode::settings::VSCodeCore;

    // Test plugin that doesn't specify a custom output file - should use auto-derived
    let vscode_plugin = SettingsPlugin::new(VSCodeCore);
    assert_eq!(
        PluginRegistry::get_plugin_output_file_from_plugin(&vscode_plugin, "vscode_settings"),
        "vscode_settings.txt"
    );

    // Test static files plugin special handling
    let static_plugin = StaticFilesPlugin::new(StaticFilesAppCore);
    assert_eq!(
        PluginRegistry::get_plugin_output_file_from_plugin(&static_plugin, "static_files"),
        "N/A (copies files directly)"
    );

    // Test that the method would work with any plugin that returns None from get_output_file()
    // This verifies the fallback behavior to auto-derived filenames
    let mock_plugin = MockPlugin;
    assert_eq!(
        PluginRegistry::get_plugin_output_file_from_plugin(&mock_plugin, "mock_plugin"),
        "mock_plugin.txt"
    );

    // Test plugin that specifies a custom output file
    let custom_plugin = CustomOutputPlugin;
    assert_eq!(
        PluginRegistry::get_plugin_output_file_from_plugin(&custom_plugin, "custom_plugin"),
        "custom-output.json"
    );
}

/// Test folder_name_to_category with edge cases
/// Verifies that folder name transformation handles various cases correctly
#[test]
fn test_folder_name_to_category_edge_cases() {
    // Test empty string
    assert_eq!(PluginRegistry::folder_name_to_category(""), "");

    // Test single character
    assert_eq!(PluginRegistry::folder_name_to_category("a"), "A");

    // Test multiple underscores
    assert_eq!(
        PluginRegistry::folder_name_to_category("static_files_manager"),
        "Static Files Manager"
    );

    // Test mixed case input
    assert_eq!(
        PluginRegistry::folder_name_to_category("VSCode_Settings"),
        "Vscode Settings"
    );

    // Test numbers
    assert_eq!(PluginRegistry::folder_name_to_category("app_v2"), "App V2");
}

/// Test extract_category_from_plugin_name edge cases
/// Verifies that category extraction handles various plugin name formats
#[test]
fn test_extract_category_from_plugin_name_edge_cases() {
    // Test empty string
    assert_eq!(
        PluginRegistry::extract_category_from_plugin_name("", None),
        ""
    );

    // Test single character
    assert_eq!(
        PluginRegistry::extract_category_from_plugin_name("a", None),
        "A"
    );

    // Test no underscore (single word)
    assert_eq!(
        PluginRegistry::extract_category_from_plugin_name("homebrew", None),
        "Homebrew"
    );

    // Test plugin with multiple underscores
    assert_eq!(
        PluginRegistry::extract_category_from_plugin_name("vscode_user_settings_backup", None),
        "Vscode"
    );
}

/// Test PluginDescriptor creation
/// Verifies that PluginDescriptor can be created with factory function
#[test]
fn test_plugin_descriptor_creation() {
    fn mock_factory(_config: Option<toml::Value>) -> Arc<dyn Plugin> {
        Arc::new(MockPlugin)
    }

    let descriptor = PluginDescriptor {
        name: "test_plugin",
        category: "test",
        factory: mock_factory,
    };

    assert_eq!(descriptor.name, "test_plugin");
    assert_eq!(descriptor.category, "test");

    // Test factory function
    let plugin = (descriptor.factory)(None);
    assert_eq!(plugin.description(), "Mock plugin");
}

/// Test get_plugin_output_file_from_plugin with static_files special case
/// Verifies that static_files plugin gets special handling for output file
#[test]
fn test_get_plugin_output_file_static_files_special_case() {
    let mock_plugin = MockPlugin;

    // Test with static_files name - should get special handling
    let result = PluginRegistry::get_plugin_output_file_from_plugin(&mock_plugin, "static_files");
    assert_eq!(result, "N/A (copies files directly)");

    // Test with other names - should use normal logic
    let result = PluginRegistry::get_plugin_output_file_from_plugin(&mock_plugin, "other_plugin");
    assert_eq!(result, "other_plugin.txt");
}

/// Test folder_name_to_category with complex names
/// Verifies edge cases in category name conversion
#[test]
fn test_folder_name_to_category_complex_cases() {
    // Test with multiple underscores
    assert_eq!(
        PluginRegistry::folder_name_to_category("one_two_three_four"),
        "One Two Three Four"
    );

    // Test with numbers
    assert_eq!(
        PluginRegistry::folder_name_to_category("plugin_v2_final"),
        "Plugin V2 Final"
    );

    // Test with single character words
    assert_eq!(PluginRegistry::folder_name_to_category("a_b_c"), "A B C");

    // Test with mixed case input (should be normalized)
    assert_eq!(
        PluginRegistry::folder_name_to_category("MyPlugin_TestCase"),
        "Myplugin Testcase"
    );

    // Test with numbers and special characters in reasonable scenarios
    assert_eq!(
        PluginRegistry::folder_name_to_category("npm_config_v1"),
        "Npm Config V1"
    );
}

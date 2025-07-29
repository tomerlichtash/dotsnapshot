//! Test fixtures and configuration builders for CLI hooks tests

use crate::config::{Config, GlobalConfig, GlobalHooks};
use crate::core::hooks::HookAction;
use std::collections::HashMap;
use std::path::PathBuf;

/// Creates a basic empty configuration
pub fn create_empty_config() -> Config {
    Config::default()
}

/// Creates a configuration with pre-snapshot hooks
pub fn create_config_with_pre_snapshot_hooks() -> Config {
    Config {
        global: Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![HookAction::Log {
                    message: "test pre".to_string(),
                    level: "info".to_string(),
                }],
                post_snapshot: vec![],
            }),
        }),
        ..Default::default()
    }
}

/// Creates a configuration with post-snapshot hooks
pub fn create_config_with_post_snapshot_hooks() -> Config {
    Config {
        global: Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![],
                post_snapshot: vec![HookAction::Log {
                    message: "post hook".to_string(),
                    level: "info".to_string(),
                }],
            }),
        }),
        ..Default::default()
    }
}

/// Creates a configuration with multiple hooks for testing removal
pub fn create_config_with_multiple_hooks() -> Config {
    Config {
        global: Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![
                    HookAction::Log {
                        message: "first hook".to_string(),
                        level: "info".to_string(),
                    },
                    HookAction::Log {
                        message: "second hook".to_string(),
                        level: "warn".to_string(),
                    },
                ],
                post_snapshot: vec![],
            }),
        }),
        ..Default::default()
    }
}

/// Creates a configuration with script hooks for testing removal by name
pub fn create_config_with_script_hooks() -> Config {
    Config {
        global: Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![
                    HookAction::Script {
                        command: "remove_this.sh".to_string(),
                        args: vec![],
                        timeout: 30,
                        working_dir: None,
                        env_vars: HashMap::new(),
                    },
                    HookAction::Script {
                        command: "keep_this.sh".to_string(),
                        args: vec![],
                        timeout: 30,
                        working_dir: None,
                        env_vars: HashMap::new(),
                    },
                    HookAction::Log {
                        message: "Keep this log".to_string(),
                        level: "info".to_string(),
                    },
                ],
                post_snapshot: vec![],
            }),
        }),
        ..Default::default()
    }
}

/// Creates a configuration with invalid script hooks for validation testing
pub fn create_config_with_invalid_script_hooks() -> Config {
    Config {
        global: Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![HookAction::Script {
                    command: "nonexistent.sh".to_string(),
                    args: vec![],
                    timeout: 30,
                    working_dir: None,
                    env_vars: HashMap::new(),
                }],
                post_snapshot: vec![],
            }),
        }),
        ..Default::default()
    }
}

/// Creates a configuration with comprehensive hook types for listing tests
pub fn create_config_with_all_hook_types() -> Config {
    Config {
        global: Some(GlobalConfig {
            hooks: Some(GlobalHooks {
                pre_snapshot: vec![HookAction::Log {
                    message: "Pre snapshot".to_string(),
                    level: "info".to_string(),
                }],
                post_snapshot: vec![HookAction::Log {
                    message: "Post snapshot".to_string(),
                    level: "info".to_string(),
                }],
            }),
        }),
        ..Default::default()
    }
}

/// Creates a configuration for script directory testing
pub fn create_config_with_scripts_dir(scripts_dir: PathBuf) -> Config {
    use crate::core::hooks::HooksConfig;

    Config {
        hooks: Some(HooksConfig { scripts_dir }),
        ..Default::default()
    }
}

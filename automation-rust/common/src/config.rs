//! # Configuration Management
//!
//! This module provides configuration management for the automation-rust project.
//! Configuration follows this priority: CLI Args → TOML Config (.automation-rust.toml) → Defaults.
//!
//! ## CLI Arguments (Runtime Overrides Only)
//!
//! The following CLI arguments are supported for runtime overrides and deployment flexibility:
//!
//! | Argument | Environment Variable | Description | Default |
//! |----------|----------------------|-------------|---------|
//! | `--config` | `AUTOMATION_CONFIG` | Path to TOML config file | `.automation-rust.toml` |
//! | `--log-level` | `AUTOMATION_LOG_LEVEL` | Override log level for runtime | Uses TOML setting |
//! | `--verbose` | - | Enable verbose logging (shortcut for debug level) | false |
//! | `--debug` | - | Enable debug mode (shortcut for debug level) | false |
//!
//! ## Configuration Priority
//!
//! 1. CLI arguments take highest priority (for runtime overrides)
//! 2. TOML config file (.automation-rust.toml) provides default configuration
//! 3. Code defaults are used as fallback
//!
//! All other configuration (workspace paths, agent settings, etc.) should be specified
//! in the TOML config file.

use std::path::{Path, PathBuf};
use clap::Parser;

// Import workspace_config module
use crate::workspace_config;

/// Configuration for the automation system.
///
/// This struct contains runtime-only CLI arguments for deployment flexibility.
/// Default configuration should be specified in .automation-rust.toml.
#[derive(Debug, Clone, Parser)]
#[command(name = "automation")]
#[command(about = "Automation system for parallel task execution", long_about = None)]
pub struct Config {
    /// Path to the TOML configuration file
    #[arg(long, env = "AUTOMATION_CONFIG", default_value = ".automation-rust.toml")]
    pub config: PathBuf,

    /// Override log level for this run (overrides TOML setting)
    #[arg(long, env = "AUTOMATION_LOG_LEVEL", value_parser = parse_log_level)]
    pub log_level: Option<String>,

    /// Enable verbose logging (equivalent to --log-level debug)
    #[arg(long, short, conflicts_with = "log_level")]
    pub verbose: bool,

    /// Enable debug mode (equivalent to --log-level debug)
    #[arg(long, conflicts_with = "log_level")]
    pub debug: bool,
}

/// Parse log level value to ensure it's valid
fn parse_log_level(s: &str) -> Result<String, String> {
    let valid_levels = ["trace", "debug", "info", "warn", "error"];
    if valid_levels.contains(&s.to_lowercase().as_str()) {
        Ok(s.to_lowercase())
    } else {
        Err(format!("Invalid log level '{}'. Valid options: trace, debug, info, warn, error", s))
    }
}

impl Config {
    /// Get the effective log level, considering CLI overrides
    pub fn get_log_level(&self) -> Option<&str> {
        if self.verbose || self.debug {
            Some("debug")
        } else {
            self.log_level.as_deref()
        }
    }
}

// Implement Parser trait for Config (via derive macro)
// The clap::Parser trait provides parse() and try_parse_from() methods.

impl Default for Config {
    fn default() -> Self {
        Config {
            config: PathBuf::from(".automation-rust.toml"),
            log_level: None,
            verbose: false,
            debug: false,
        }
    }
}

/// Resolve a path relative to a base directory.
///
/// If the path is absolute, it is returned as-is. Otherwise, it is resolved
/// relative to the base directory.
///
/// # Arguments
///
/// * `path` - The path string to resolve
/// * `base` - The base directory to resolve relative paths from
///
/// # Returns
///
/// Returns a `PathBuf` representing the resolved path.
///
/// # Example
///
/// ```
/// use automation_common::config::resolve_path;
/// use std::path::{Path, PathBuf};
///
/// // Absolute path is returned as-is
/// let resolved = resolve_path("/absolute/path", Path::new("/base"));
/// assert_eq!(resolved, PathBuf::from("/absolute/path"));
///
/// // Relative path is resolved against base
/// let resolved = resolve_path("relative/path", Path::new("/base"));
/// assert_eq!(resolved, PathBuf::from("/base/relative/path"));
/// ```
pub fn resolve_path(path: &str, base: &Path) -> PathBuf {
    let path_buf = PathBuf::from(path);
    if path_buf.is_absolute() {
        path_buf
    } else {
        base.join(path)
    }
}

/// Load workspace configuration from a TOML config file.
///
/// This function reads a TOML configuration file and returns a Config struct
/// for runtime use. Note: The Config struct now only contains CLI arguments
/// for runtime overrides. For full workspace configuration, use the
/// workspace_config module directly.
///
/// # Arguments
///
/// * `config_path` - Path to the TOML configuration file
///
/// # Returns
///
/// Returns a `Result<Config, ConfigError>` containing runtime Config
/// or an error if the file cannot be read or parsed.
///
/// # Example
///
/// ```no_run
/// use automation_common::config::load_from_workspace_config;
///
/// let config = load_from_workspace_config(".automation-rust.toml").unwrap();
/// println!("Config file path: {:?}", config.config);
/// ```
///
/// # Notes
///
/// - This function validates that the TOML config file exists and can be parsed.
/// - The returned Config struct contains only runtime CLI arguments.
/// - For accessing workspace paths, agent settings, and other TOML configuration,
///   use `workspace_config::parse_config_file()` directly.
/// - The configuration loading flow is: CLI Args → TOML Config → Defaults
pub fn load_from_workspace_config<P: AsRef<Path>>(
    config_path: P,
) -> Result<Config, workspace_config::ConfigError> {
    use workspace_config::parse_config_file;

    // Parse the TOML config file to validate it exists and is valid
    let _toml_config = parse_config_file(&config_path)?;

    // Create a Config struct with the provided config file path
    // This is used for runtime configuration; TOML values are loaded separately
    let config = Config {
        config: config_path.as_ref().to_path_buf(),
        ..Default::default()
    };

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_path_absolute() {
        let resolved = resolve_path("/absolute/path", Path::new("/base"));
        assert_eq!(resolved, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_resolve_path_relative() {
        let resolved = resolve_path("relative/path", Path::new("/base"));
        assert_eq!(resolved, PathBuf::from("/base/relative/path"));
    }

    #[test]
    fn test_resolve_path_dotslash() {
        let resolved = resolve_path("./path", Path::new("/base"));
        assert_eq!(resolved, PathBuf::from("/base/./path"));
    }

    #[test]
    fn test_resolve_path_dotdot() {
        let resolved = resolve_path("../path", Path::new("/base/dir"));
        assert_eq!(resolved, PathBuf::from("/base/dir/../path"));
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.config, PathBuf::from(".automation-rust.toml"));
        assert!(config.log_level.is_none());
        assert_eq!(config.verbose, false);
        assert_eq!(config.debug, false);
    }

    #[test]
    fn test_parse_empty_args() {
        let args = vec!["automation"];
        let config = Config::try_parse_from(args).unwrap();
        assert_eq!(config.config, PathBuf::from(".automation-rust.toml"));
        assert!(config.log_level.is_none());
        assert_eq!(config.verbose, false);
        assert_eq!(config.debug, false);
    }

    #[test]
    fn test_parse_with_config() {
        let args = vec!["automation", "--config", "/custom/config.toml"];
        let config = Config::try_parse_from(args).unwrap();
        assert_eq!(config.config, PathBuf::from("/custom/config.toml"));
    }

    #[test]
    fn test_parse_with_log_level() {
        let args = vec!["automation", "--log-level", "debug"];
        let config = Config::try_parse_from(args).unwrap();
        assert_eq!(config.log_level, Some("debug".to_string()));
        assert_eq!(config.verbose, false);
        assert_eq!(config.debug, false);
    }

    #[test]
    fn test_parse_with_verbose() {
        let args = vec!["automation", "--verbose"];
        let config = Config::try_parse_from(args).unwrap();
        assert_eq!(config.log_level, None);
        assert_eq!(config.verbose, true);
        assert_eq!(config.debug, false);
        assert_eq!(config.get_log_level(), Some("debug"));
    }

    #[test]
    fn test_parse_with_debug() {
        let args = vec!["automation", "--debug"];
        let config = Config::try_parse_from(args).unwrap();
        assert_eq!(config.log_level, None);
        assert_eq!(config.verbose, false);
        assert_eq!(config.debug, true);
        assert_eq!(config.get_log_level(), Some("debug"));
    }

    #[test]
    fn test_parse_with_all_options() {
        let args = vec![
            "automation",
            "--config", "/custom/config.toml",
            "--log-level", "trace",
        ];
        let config = Config::try_parse_from(args).unwrap();
        assert_eq!(config.config, PathBuf::from("/custom/config.toml"));
        assert_eq!(config.log_level, Some("trace".to_string()));
        assert_eq!(config.verbose, false);
        assert_eq!(config.debug, false);
        assert_eq!(config.get_log_level(), Some("trace"));
    }

    #[test]
    fn test_get_log_level() {
        let config = Config {
            log_level: Some("warn".to_string()),
            ..Default::default()
        };
        assert_eq!(config.get_log_level(), Some("warn"));

        let config = Config {
            verbose: true,
            ..Default::default()
        };
        assert_eq!(config.get_log_level(), Some("debug"));

        let config = Config {
            debug: true,
            ..Default::default()
        };
        assert_eq!(config.get_log_level(), Some("debug"));

        let config = Config::default();
        assert_eq!(config.get_log_level(), None);
    }

    #[test]
    fn test_load_from_workspace_config_default_state() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary config file with default state path
        let mut config_file = NamedTempFile::new().unwrap();
        let toml_content = r#"
[workspace]
path = "./workspace"
state_path = ".state"

[logging]
level = "info"
format = "pretty"
colors = true

[agents.architect]
interval_minutes = 40
timeout_minutes = 30
immediate = false
prompt_template = "prompts-architect/ARCHITECT.md"

[agents.architect.lock]
max_retries = 3
timeout_seconds = 30

[agents.janitor]
interval_minutes = 20
timeout_minutes = 15
immediate = false
prompt_template = "prompts-janitor/JANITOR.md"

[agents.janitor.lock]
max_retries = 3
timeout_seconds = 30

[agents.prompt]
interval_minutes = 5
timeout_minutes = 3
immediate = false
prompt_template = "prompts-prompt/PROMPT.md"

[agents.prompt.lock]
max_retries = 3
timeout_seconds = 30
"#;
        config_file.write_all(toml_content.as_bytes()).unwrap();
        config_file.flush().unwrap();

        let config = load_from_workspace_config(config_file.path()).unwrap();

        // Config file path should be set to the provided path
        assert_eq!(config.config, config_file.path());
        // Runtime fields should use defaults
        assert!(config.log_level.is_none());
        assert_eq!(config.verbose, false);
        assert_eq!(config.debug, false);
    }

    #[test]
    fn test_load_from_workspace_config_custom_state() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary config file with custom state path
        let mut config_file = NamedTempFile::new().unwrap();
        let toml_content = r#"
[workspace]
path = "./workspace"
state_path = "./custom_state"

[logging]
level = "info"
format = "pretty"
colors = true

[agents.architect]
interval_minutes = 40
timeout_minutes = 30
immediate = false
prompt_template = "prompts-architect/ARCHITECT.md"

[agents.architect.lock]
max_retries = 3
timeout_seconds = 30

[agents.janitor]
interval_minutes = 20
timeout_minutes = 15
immediate = false
prompt_template = "prompts-janitor/JANITOR.md"

[agents.janitor.lock]
max_retries = 3
timeout_seconds = 30

[agents.prompt]
interval_minutes = 5
timeout_minutes = 3
immediate = false
prompt_template = "prompts-prompt/PROMPT.md"

[agents.prompt.lock]
max_retries = 3
timeout_seconds = 30
"#;
        config_file.write_all(toml_content.as_bytes()).unwrap();
        config_file.flush().unwrap();

        let config = load_from_workspace_config(config_file.path()).unwrap();

        // Config file path should be set to the provided path
        assert_eq!(config.config, config_file.path());
        // Runtime fields should use defaults
        assert!(config.log_level.is_none());
        assert_eq!(config.verbose, false);
        assert_eq!(config.debug, false);
    }

    #[test]
    fn test_load_from_workspace_config_absolute_workspace() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary config file with absolute workspace path
        let mut config_file = NamedTempFile::new().unwrap();
        let toml_content = r#"
[workspace]
path = "/absolute/workspace"
state_path = ".state"

[logging]
level = "info"
format = "pretty"
colors = true

[agents.architect]
interval_minutes = 40
timeout_minutes = 30
immediate = false
prompt_template = "prompts-architect/ARCHITECT.md"

[agents.architect.lock]
max_retries = 3
timeout_seconds = 30

[agents.janitor]
interval_minutes = 20
timeout_minutes = 15
immediate = false
prompt_template = "prompts-janitor/JANITOR.md"

[agents.janitor.lock]
max_retries = 3
timeout_seconds = 30

[agents.prompt]
interval_minutes = 5
timeout_minutes = 3
immediate = false
prompt_template = "prompts-prompt/PROMPT.md"

[agents.prompt.lock]
max_retries = 3
timeout_seconds = 30
"#;
        config_file.write_all(toml_content.as_bytes()).unwrap();
        config_file.flush().unwrap();

        let config = load_from_workspace_config(config_file.path()).unwrap();

        // Config file path should be set to the provided path
        assert_eq!(config.config, config_file.path());
        // Runtime fields should use defaults
        assert!(config.log_level.is_none());
        assert_eq!(config.verbose, false);
        assert_eq!(config.debug, false);
    }
}

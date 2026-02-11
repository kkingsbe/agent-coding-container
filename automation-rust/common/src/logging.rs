//! # Logging Infrastructure
//!
//! This module provides structured logging infrastructure using the `tracing` crate.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use automation_common::init_simple;
//!
//! fn main() -> anyhow::Result<()> {
//!     init_simple("info")?;
//!     // Your code here
//!     Ok(())
//! }
//! ```
//!
//! ## Initialization Options
//!
//! - `init_simple(level)` - Initialize with log level (uses pretty format in debug, JSON in release)
//! - `init_config(&LoggingConfig)` - Full control over format, file info, target info
//!
//! ## Environment Variables
//!
//! - `RUST_LOG` - Takes precedence for fine-grained control (e.g., "automation=debug")
//!
//! ## Log Levels
//!
//! - **ERROR**: Errors that prevent task completion
//! - **WARN**: Unexpected situations that don't prevent execution
//! - **INFO**: High-level information about significant events (default)
//! - **DEBUG**: Detailed information useful for debugging
//! - **TRACE**: Extremely detailed information

use crate::Result;
use std::env;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

// Import the workspace configuration types
pub use crate::workspace_config::LoggingConfig as WorkspaceLoggingConfig;

/// The output format for log messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// JSON format (production)
    Json,
    /// Pretty-printed format (development)
    Pretty,
}

impl Default for LogFormat {
    fn default() -> Self {
        #[cfg(debug_assertions)]
        { Self::Pretty }
        #[cfg(not(debug_assertions))]
        { Self::Json }
    }
}

impl LogFormat {
    /// Parse a LogFormat from a string.
    pub fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s.to_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "pretty" => Ok(Self::Pretty),
            _ => Err(format!("Invalid log format '{}'. Valid: 'json', 'pretty'", s)),
        }
    }
}

/// Configuration for logging initialization.
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// The log level (e.g., "info", "debug", "trace")
    pub level: String,
    /// The output format
    pub format: LogFormat,
    /// Whether to include file:line information
    pub with_file: bool,
    /// Whether to include module path
    pub with_target: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: LogFormat::default(),
            with_file: false,
            with_target: true,
        }
    }
}

/// Initialize logging with the specified configuration.
///
/// This function sets up a global tracing subscriber. Can only be called once.
///
/// # Arguments
///
/// * `config` - The logging configuration to use
///
/// # Returns
///
/// * `Ok(())` - If logging was initialized successfully
/// * `Err(AutomationError)` - If initialization failed
pub fn init_config(config: &LoggingConfig) -> Result<()> {
    // Build env filter (RUST_LOG takes precedence)
    let filter = if let Ok(rust_log) = env::var("RUST_LOG") {
        EnvFilter::try_new(&rust_log)
            .map_err(|e| crate::AutomationError::Config(format!("RUST_LOG: {}", e)))?
    } else {
        EnvFilter::try_new(&config.level)
            .map_err(|e| crate::AutomationError::Config(format!("LOG_LEVEL: {}", e)))?
    };

    // Build subscriber
    let registry = tracing_subscriber::registry().with(filter);

    match config.format {
        LogFormat::Json => {
            let layer = fmt::layer()
                .json()
                .with_file(config.with_file)
                .with_target(config.with_target)
                .with_thread_ids(true)
                .with_line_number(config.with_file);
            registry.with(layer).init();
        }
        LogFormat::Pretty => {
            let layer = fmt::layer()
                .pretty()
                .with_file(config.with_file)
                .with_target(config.with_target)
                .with_thread_ids(true)
                .with_line_number(config.with_file);
            registry.with(layer).init();
        }
    }

    Ok(())
}

/// Initialize logging with a simple log level.
///
/// This is the recommended way to initialize logging for most use cases.
/// Uses pretty format in debug builds, JSON in release builds.
///
/// # Arguments
///
/// * `level` - The log level (e.g., "error", "warn", "info", "debug", "trace")
///
/// # Returns
///
/// * `Ok(())` - If logging was initialized successfully
/// * `Err(AutomationError)` - If initialization failed
///
/// # Examples
///
/// ```rust,ignore
/// use automation_common::init_simple;
///
/// init_simple("debug")?;
/// ```
pub fn init_simple(level: &str) -> Result<()> {
    let format = if cfg!(debug_assertions) {
        LogFormat::Pretty
    } else {
        LogFormat::Json
    };

    init_config(&LoggingConfig {
        level: level.to_string(),
        format,
        with_file: cfg!(debug_assertions),
        with_target: true,
    })
}

/// Initialize logging from environment variables.
///
/// Reads `LOG_LEVEL` (default: "info") and `LOG_FORMAT` (default: based on build).
/// RUST_LOG takes precedence over LOG_LEVEL for fine-grained control.
///
/// # Returns
///
/// * `Ok(())` - If logging was initialized successfully
/// * `Err(AutomationError)` - If initialization failed
pub fn init_from_env() -> Result<()> {
    let level = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let format = env::var("LOG_FORMAT")
        .ok()
        .and_then(|s| LogFormat::from_str(&s).ok())
        .unwrap_or_default();

    init_config(&LoggingConfig {
        level,
        format,
        with_file: cfg!(debug_assertions),
        with_target: true,
    })
}

/// Initialize logging with sensible defaults.
///
/// This is the simplest way to initialize logging. Uses log level "info",
/// pretty format in debug builds, JSON in release builds.
///
/// # Returns
///
/// * `Ok(())` - If logging was initialized successfully
/// * `Err(AutomationError)` - If initialization failed
pub fn init_default() -> Result<()> {
    init_config(&LoggingConfig::default())
}

/// Initialize logging from workspace TOML configuration.
///
/// # Arguments
///
/// * `config` - The workspace logging configuration from TOML
///
/// # Returns
///
/// * `Ok(())` - If logging was initialized successfully
/// * `Err(AutomationError)` - If initialization failed
pub fn init_from_workspace_config(config: &WorkspaceLoggingConfig) -> Result<()> {
    let format = LogFormat::from_str(&config.format)
        .map_err(|e| crate::AutomationError::Config(format!("LOG_FORMAT: {}", e)))?;

    init_config(&LoggingConfig {
        level: config.level.clone(),
        format,
        with_file: true,
        with_target: true,
    })
}

/// Create a tracing span for task execution.
pub fn task_span(task_name: &str) -> tracing::Span {
    tracing::info_span!("task", task = %task_name)
}

/// Create a tracing span for scheduler operations.
pub fn scheduler_span(agent_type: &str) -> tracing::Span {
    tracing::info_span!("scheduler", agent = %agent_type)
}

/// Create a tracing span for executor operations.
pub fn executor_span(command: &str) -> tracing::Span {
    tracing::info_span!("executor", command = %command)
}

// Backward compatibility: provide the old init_logging as an alias
#[doc(hidden)]
pub fn init_logging(config: &LoggingConfig) -> Result<()> {
    init_config(config)
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic;

    #[test]
    fn test_logging_config_default() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, "info");
        assert_eq!(config.with_file, false);
        assert_eq!(config.with_target, true);
        #[cfg(debug_assertions)]
        assert_eq!(config.format, LogFormat::Pretty);
        #[cfg(not(debug_assertions))]
        assert_eq!(config.format, LogFormat::Json);
    }

    #[test]
    fn test_log_format_from_str() {
        assert_eq!(LogFormat::from_str("json"), Ok(LogFormat::Json));
        assert_eq!(LogFormat::from_str("JSON"), Ok(LogFormat::Json));
        assert_eq!(LogFormat::from_str("pretty"), Ok(LogFormat::Pretty));
        assert_eq!(LogFormat::from_str("PRETTY"), Ok(LogFormat::Pretty));
        assert!(LogFormat::from_str("invalid").is_err());
    }

    #[test]
    fn test_span_creation() {
        let _task_span = task_span("test-task");
        let _scheduler_span = scheduler_span("architect");
        let _executor_span = executor_span("npm install");
    }

    #[test]
    fn test_init_logging_with_config() {
        let config = LoggingConfig {
            level: "debug".to_string(),
            format: LogFormat::Pretty,
            with_file: false,
            with_target: true,
        };

        let result = panic::catch_unwind(|| init_config(&config));

        match result {
            Ok(Ok(())) | Err(_) => (), // Success or double-init panic is okay
            _ => panic!("Unexpected result"),
        }
    }

    #[test]
    fn test_init_from_env() {
        env::set_var("LOG_LEVEL", "debug");
        env::set_var("LOG_FORMAT", "pretty");

        let result = panic::catch_unwind(|| init_from_env());

        env::remove_var("LOG_LEVEL");
        env::remove_var("LOG_FORMAT");

        match result {
            Ok(Ok(())) | Err(_) => (),
            _ => panic!("Unexpected result"),
        }
    }

    #[test]
    fn test_init_default() {
        let result = panic::catch_unwind(|| init_default());

        match result {
            Ok(Ok(())) | Err(_) => (),
            _ => panic!("Unexpected result"),
        }
    }

    #[test]
    fn test_init_simple() {
        let result = panic::catch_unwind(|| init_simple("debug"));

        match result {
            Ok(Ok(())) | Err(_) => (),
            _ => panic!("Unexpected result"),
        }
    }

    #[test]
    fn test_invalid_rust_log() {
        let config = LoggingConfig {
            level: "info".to_string(),
            format: LogFormat::Pretty,
            with_file: false,
            with_target: true,
        };

        env::set_var("RUST_LOG", "invalid=invalid=invalid");

        let result = panic::catch_unwind(|| init_config(&config));

        env::remove_var("RUST_LOG");

        match result {
            Ok(Err(e)) => {
                assert!(matches!(e, crate::AutomationError::Config(_)));
            }
            Err(_) => (), // Double-init panic is okay
            _ => panic!("Unexpected result"),
        }
    }

    #[test]
    fn test_invalid_log_format_from_env() {
        env::set_var("LOG_FORMAT", "invalid");

        let result = panic::catch_unwind(|| init_from_env());

        env::remove_var("LOG_FORMAT");

        match result {
            Ok(Err(e)) => {
                assert!(matches!(e, crate::AutomationError::Config(_)));
            }
            Err(_) => (),
            _ => panic!("Unexpected result"),
        }
    }

    #[test]
    fn test_log_format_properties() {
        let json = LogFormat::Json;
        let pretty = LogFormat::Pretty;

        assert_eq!(json, LogFormat::Json);
        assert_eq!(pretty, LogFormat::Pretty);
        assert_ne!(json, pretty);

        let debug_str = format!("{:?}", json);
        assert!(debug_str.contains("Json"));
    }

    #[test]
    fn test_logging_config_clone() {
        let config = LoggingConfig {
            level: "trace".to_string(),
            format: LogFormat::Json,
            with_file: true,
            with_target: false,
        };

        let cloned = config.clone();
        assert_eq!(config.level, cloned.level);
        assert_eq!(config.format, cloned.format);
        assert_eq!(config.with_file, cloned.with_file);
        assert_eq!(config.with_target, cloned.with_target);
    }

    #[test]
    fn test_init_from_workspace_config() {
        let config = WorkspaceLoggingConfig {
            level: "debug".to_string(),
            format: "pretty".to_string(),
            colors: true,
        };

        let result = panic::catch_unwind(|| init_from_workspace_config(&config));

        match result {
            Ok(Ok(())) | Err(_) => (),
            _ => panic!("Unexpected result"),
        }
    }

    #[test]
    fn test_init_from_workspace_config_invalid_format() {
        let config = WorkspaceLoggingConfig {
            level: "info".to_string(),
            format: "invalid".to_string(),
            colors: true,
        };

        let result = panic::catch_unwind(|| init_from_workspace_config(&config));

        match result {
            Ok(Err(e)) => {
                assert!(matches!(e, crate::AutomationError::Config(_)));
            }
            Err(_) => (),
            _ => panic!("Unexpected result"),
        }
    }

    #[test]
    fn test_init_logging_alias() {
        // Test that init_logging still works as an alias for init_config
        let config = LoggingConfig {
            level: "info".to_string(),
            format: LogFormat::Pretty,
            with_file: false,
            with_target: true,
        };

        let result = panic::catch_unwind(|| init_logging(&config));

        match result {
            Ok(Ok(())) | Err(_) => (),
            _ => panic!("Unexpected result"),
        }
    }
}

//! # automation-common
//!
//! Shared types and utilities for the automation-rust project.
//!
//! This crate provides common functionality used across all automation-rust modules,
//! including error types, configuration structures, and shared utilities.

pub mod config;
pub mod error;
pub mod logging;
pub mod result;
pub mod workspace_config;

// Re-export for convenience
pub use config::Config;
pub use error::{AutomationError, Result};
pub use logging::{init_config, init_default, init_from_env, init_from_workspace_config, init_simple, init_logging, LoggingConfig, LogFormat};
pub use workspace_config::{
    parse_config_file, AgentConfig, AgentsConfig, Config as WorkspaceConfig, ConfigError,
    ConfigResult, LockConfig, LoggingConfig as WorkspaceLoggingConfig, ParallelConfig,
    WorkspaceConfig as WorkspaceConfigStruct,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }

    #[test]
    fn test_error_module_exists() {
        // Verify error types are accessible
        let _err = AutomationError::FileSystem(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "test error",
        ));
    }

    #[test]
    fn test_result_module_exists() {
        // Verify Result type is accessible
        let _result: Result<i32> = Ok(42);
    }

    #[test]
    fn test_logging_module_exists() {
        // Verify logging types are accessible
        let _config = LoggingConfig {
            level: "info".to_string(),
            format: LogFormat::Pretty,
            with_file: false,
            with_target: true,
        };
    }

    #[test]
    fn test_config_module_exists() {
        // Verify Config type is accessible
        let config = Config::default();
        assert_eq!(config.config, std::path::PathBuf::from(".automation-rust.toml"));
        assert!(config.log_level.is_none());
        assert_eq!(config.verbose, false);
        assert_eq!(config.debug, false);
    }
}

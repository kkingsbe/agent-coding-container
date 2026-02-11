//! # Error Handling
//!
//! This module defines the error types used throughout the automation-rust project.
//!
//! ## Error Handling Philosophy
//!
//! The automation-rust project uses structured error handling with `thiserror` to provide
//! descriptive, actionable error messages with proper error chaining. All errors implement
//! `std::error::Error` and `std::fmt::Display` for maximum compatibility.
//!
//! ## When to Use `Result<T>` vs `anyhow::Result<T>`
//!
//! - **Use `automation_common::Result<T>`** for:
//!   - Library code where consumers need to handle specific error cases
//!   - Error variants that represent domain-specific conditions (e.g., lock timeout)
//!   - Situations where you want to provide specific, actionable error messages
//!
//! - **Use `anyhow::Result<T>`** for:
//!   - Application entry points (main, tests)
//!   - Quick prototyping where you don't need specific error handling
//!   - Situations where you want to preserve full context with minimal boilerplate

use thiserror::Error;

/// Main error type for the automation system
#[derive(Error, Debug)]
pub enum AutomationError {
    /// File system errors
    #[error("File system error: {0}")]
    FileSystem(#[from] std::io::Error),

    /// JSON serialization/deserialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Lock acquisition errors
    #[error("Lock error for task '{task}': {message}")]
    LockError { task: String, message: String },

    /// State management errors
    #[error("State error for task '{task}': {message}")]
    StateError { task: String, message: String },

    /// Template rendering errors
    #[error("Template error: {0}")]
    Template(String),

    /// Process execution errors
    #[error("Process execution error: {0}")]
    Process(String),

    /// Lock timeout
    #[error("Lock acquisition timeout for task '{task}' after {timeout}ms")]
    LockTimeout { task: String, timeout: u64 },

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Invalid schedule configuration
    #[error("Invalid schedule configuration: {reason}")]
    ScheduleConfigInvalid { reason: String },

    /// Failed to spawn a process
    #[error("Failed to spawn process '{command}': {source}")]
    ProcessSpawnFailed { command: String, source: std::io::Error },
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, AutomationError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filesystem_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: AutomationError = io_err.into();
        assert!(matches!(err, AutomationError::FileSystem(_)));
        assert!(err.to_string().contains("File system error"));
    }

    #[test]
    fn test_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let err: AutomationError = json_err.into();
        assert!(matches!(err, AutomationError::Json(_)));
        assert!(err.to_string().contains("JSON error"));
    }

    #[test]
    fn test_lock_error() {
        let err = AutomationError::LockError {
            task: "test-task".to_string(),
            message: "Could not acquire lock".to_string(),
        };
        assert!(matches!(err, AutomationError::LockError { .. }));
        assert!(err.to_string().contains("test-task"));
        assert!(err.to_string().contains("Could not acquire lock"));
    }

    #[test]
    fn test_state_error() {
        let err = AutomationError::StateError {
            task: "test-task".to_string(),
            message: "State corrupted".to_string(),
        };
        assert!(matches!(err, AutomationError::StateError { .. }));
        assert!(err.to_string().contains("test-task"));
        assert!(err.to_string().contains("State corrupted"));
    }

    #[test]
    fn test_template_error() {
        let err = AutomationError::Template("Invalid template syntax".to_string());
        assert!(matches!(err, AutomationError::Template(_)));
        assert!(err.to_string().contains("Invalid template syntax"));
    }

    #[test]
    fn test_process_error() {
        let err = AutomationError::Process("Command failed with exit code 1".to_string());
        assert!(matches!(err, AutomationError::Process(_)));
        assert!(err.to_string().contains("Command failed with exit code 1"));
    }

    #[test]
    fn test_lock_timeout() {
        let err = AutomationError::LockTimeout {
            task: "test-task".to_string(),
            timeout: 5000,
        };
        assert!(matches!(err, AutomationError::LockTimeout { .. }));
        assert!(err.to_string().contains("test-task"));
        assert!(err.to_string().contains("5000"));
    }

    #[test]
    fn test_config_error() {
        let err = AutomationError::Config("Missing required field".to_string());
        assert!(matches!(err, AutomationError::Config(_)));
        assert!(err.to_string().contains("Missing required field"));
    }

    #[test]
    fn test_result_ok() {
        let result: Result<i32> = Ok(42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_result_err() {
        let err = AutomationError::LockError {
            task: "test-task".to_string(),
            message: "Error".to_string(),
        };
        let result: Result<i32> = Err(err);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AutomationError::LockError { .. }));
    }

    #[test]
    fn test_schedule_config_invalid() {
        let err = AutomationError::ScheduleConfigInvalid {
            reason: "Invalid cron expression".to_string(),
        };
        assert!(matches!(err, AutomationError::ScheduleConfigInvalid { .. }));
        assert!(err.to_string().contains("Invalid schedule configuration"));
        assert!(err.to_string().contains("Invalid cron expression"));
    }

    #[test]
    fn test_process_spawn_failed() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "executable not found");
        let err = AutomationError::ProcessSpawnFailed {
            command: "npm run build".to_string(),
            source: io_err,
        };
        assert!(matches!(err, AutomationError::ProcessSpawnFailed { .. }));
        assert!(err.to_string().contains("Failed to spawn process"));
        assert!(err.to_string().contains("npm run build"));
        assert!(err.to_string().contains("executable not found"));
    }
}

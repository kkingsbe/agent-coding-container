//! # Result Type
//!
//! This module provides a type alias for the standard `Result` type with
//! the project's error type.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use automation_common::{Result, AutomationError};
//!
//! fn read_config() -> Result<Config> {
//!     // ... implementation ...
//! }
//! ```
//!
//! This type alias makes it convenient to use the project's error type
//! throughout the codebase without having to repeat the full type signature.

use crate::error::AutomationError;

/// A type alias for `Result<T, AutomationError>`.
///
/// This is the standard result type used throughout the automation-rust project.
/// It wraps the successful value `T` in `Ok`, or provides an `AutomationError`
/// on failure.
///
/// # Examples
///
/// Basic usage:
///
/// ```rust,ignore
/// use automation_common::Result;
///
/// fn read_file(path: &str) -> Result<String> {
///     std::fs::read_to_string(path)
///         .map_err(AutomationError::from)
/// }
/// ```
///
/// Propagating errors:
///
/// ```rust,ignore
/// use automation_common::Result;
///
/// fn process_files(paths: &[&str]) -> Result<Vec<String>> {
///     let mut results = Vec::new();
///     for path in paths {
///         let content = read_file(path)?; // Uses the ? operator
///         results.push(content);
///     }
///     Ok(results)
/// }
/// ```
///
/// # Comparison with `anyhow::Result`
///
/// Use `automation_common::Result<T>` when:
/// - You want consumers to be able to match on specific error variants
/// - You're writing library code where specific error handling is important
/// - You need to provide domain-specific error context
///
/// Use `anyhow::Result<T>` when:
/// - You're at the application boundary (main function, CLI entry point)
/// - You want to preserve full error context with minimal boilerplate
/// - Specific error handling is not required
pub type Result<T> = std::result::Result<T, AutomationError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_result_ok() {
        let result: Result<String> = Ok("success".to_string());
        assert!(result.is_ok());
        assert!(!result.is_err());
        assert_eq!(result.unwrap(), "success");
    }

    #[test]
    fn test_result_err() {
        let err = AutomationError::FileSystem(io::Error::new(io::ErrorKind::NotFound, "not found"));
        let result: Result<String> = Err(err);
        assert!(result.is_err());
        assert!(!result.is_ok());
        assert!(matches!(result.unwrap_err(), AutomationError::FileSystem(_)));
    }

    #[test]
    fn test_result_map() {
        let result: Result<i32> = Ok(5);
        let mapped = result.map(|x| x * 2);
        assert_eq!(mapped.unwrap(), 10);
    }

    #[test]
    fn test_result_map_err() {
        let result: Result<i32> = Err(AutomationError::FileSystem(
            io::Error::new(io::ErrorKind::NotFound, "not found"),
        ));

        let mapped = result.map_err(|e| format!("Error: {}", e));
        assert!(mapped.is_err());
        assert!(mapped.unwrap_err().contains("File system error"));
    }

    #[test]
    fn test_result_and_then() {
        let result: Result<i32> = Ok(5);
        let chained = result.and_then(|x| if x > 0 {
            Ok(x * 2)
        } else {
            Err(AutomationError::Config("must be positive".to_string()))
        });
        assert_eq!(chained.unwrap(), 10);
    }

    #[test]
    fn test_result_or_else() {
        let result: Result<i32> = Err(AutomationError::Config("test error".to_string()));

        let value: Result<i32> = result.or_else(|_| Ok(42));
        assert_eq!(value.unwrap(), 42);
    }
}

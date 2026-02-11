//! # automation-executor
//!
//! CLI executor module for automation-rust project.
//!
//! This module provides functionality for executing Kilo Code CLI with:
//! - Process spawning with stdin/stdout/stderr pipes
//! - Real-time output monitoring
//! - Pattern-based early termination (Mistake Limit detection)
//! - Output buffering with configurable max size
//! - Graceful + forceful termination strategy
//! - Timeout enforcement
//!
//! # Example
//!
//! ```no_run
//! use automation_executor::CLIExecutor;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let executor = CLIExecutor::new();
//!     let result = executor.execute().await?;
//!     Ok(())
//! }
//! ```

pub mod cli;

// Re-export for convenience
pub use cli::{CLIExecutor, ExecutorConfig, ErrorPatterns, OutputBuffer, ProcessResult};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}

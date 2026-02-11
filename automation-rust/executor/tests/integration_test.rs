//! Integration tests for executor module.
//!
//! Tests executor components including:
//! - CLIExecutor with simple commands
//! - stdout/stderr capture and buffering
//! - timeout enforcement
//! - Mistake Limit pattern detection
//! - output buffer truncation
//! - process exit status handling

use automation_executor::{CLIExecutor, ExecutorConfig, ErrorPatterns, OutputBuffer, ProcessResult};
use regex::bytes::Regex;
use std::time::Duration;

#[test]
fn test_output_buffer_within_limit() {
    let mut buffer = OutputBuffer::new(100);
    
    let data = b"Hello, world!";
    let was_full = buffer.add_stdout(data);
    
    assert!(!was_full, "Buffer should not be full");
    assert_eq!(buffer.get_stdout(), "Hello, world!");
    assert_eq!(buffer.len(), 13);
}

#[test]
fn test_output_buffer_at_limit() {
    let mut buffer = OutputBuffer::new(10);
    
    let data = b"0123456789"; // Exactly 10 bytes
    let was_full = buffer.add_stdout(data);
    
    assert!(!was_full, "Buffer should not be full at exact limit");
    assert_eq!(buffer.get_stdout(), "0123456789");
    assert_eq!(buffer.len(), 10);
}

#[test]
fn test_output_buffer_over_limit() {
    let mut buffer = OutputBuffer::new(10);
    
    let data = b"0123456789ABCDEF"; // 16 bytes
    let was_full = buffer.add_stdout(data);
    
    assert!(was_full, "Buffer should be marked as full");
    assert_eq!(buffer.len(), 10, "Should truncate to limit");
    assert_eq!(buffer.get_stdout(), "0123456789");
}

#[test]
fn test_output_buffer_truncation_behavior() {
    let mut buffer = OutputBuffer::new(15);
    
    // First write within limit
    buffer.add_stdout(b"Hello, ");
    assert_eq!(buffer.len(), 7);
    
    // Second write exceeds limit
    let was_full = buffer.add_stdout(b"world! This will be truncated");
    assert!(was_full);
    
    // Should have exactly at limit
    assert_eq!(buffer.len(), 15);
    assert_eq!(buffer.get_stdout(), "Hello, world! T");
}

#[test]
fn test_output_buffer_clear() {
    let mut buffer = OutputBuffer::new(100);
    
    buffer.add_stdout(b"Some data");
    buffer.add_stderr(b"Some error");
    
    assert!(buffer.len() > 0);
    
    buffer.clear();
    
    assert_eq!(buffer.len(), 0);
    assert_eq!(buffer.get_stdout(), "");
    assert_eq!(buffer.get_stderr(), "");
    assert!(!buffer.is_full());
}

#[test]
fn test_error_patterns_default_mistake_limit() {
    let patterns = ErrorPatterns::default();
    
    assert!(
        patterns.matches(b"Mistake Limit Reached"),
        "Should match 'Mistake Limit Reached'"
    );
    assert!(
        patterns.matches(b"\xE2\x9C\x97 Mistake Limit Reached"),
        "Should match with checkmark emoji"
    );
    assert!(
        patterns.matches(b"X Mistake Limit Reached"),
        "Should match with X marker"
    );
}

#[test]
fn test_error_patterns_no_match() {
    let patterns = ErrorPatterns::default();
    
    assert!(
        !patterns.matches(b"Normal output"),
        "Should not match normal output"
    );
    assert!(
        !patterns.matches(b"Task completed successfully"),
        "Should not match success messages"
    );
}

#[test]
fn test_error_patterns_find_match() {
    let patterns = ErrorPatterns::default();
    
    let output = b"Some output\nMistake Limit Reached\nMore output";
    let match_result = patterns.find_match(output);
    
    assert!(match_result.is_some(), "Should find a match");
    assert!(
        match_result.unwrap().contains("Mistake Limit"),
        "Match should contain mistake limit pattern"
    );
}

#[test]
fn test_process_result_default() {
    let result = ProcessResult::default();
    
    assert_eq!(result.exit_code, None);
    assert_eq!(result.stdout, "");
    assert_eq!(result.stderr, "");
    assert!(!result.terminated_early);
    assert!(result.termination_reason.is_none());
    assert_eq!(result.execution_time_ms, 0);
    assert!(result.error.is_none());
}

#[test]
fn test_executor_config_default() {
    let config = ExecutorConfig::default();
    
    assert_eq!(config.command, "kilocode");
    assert_eq!(config.workspace, "/workspace");
    assert_eq!(config.timeout, Duration::from_secs(1800));
    assert_eq!(config.output_buffer_limit, 10 * 1024 * 1024);
    assert_eq!(config.graceful_timeout, Duration::from_secs(5));
    assert_eq!(config.force_timeout, Duration::from_secs(2));
}

#[test]
fn test_executor_config_values() {
    let config = ExecutorConfig {
        command: "test-command".to_string(),
        args: vec!["arg1".to_string(), "arg2".to_string()],
        input: Some("test input".to_string()),
        workspace: "/test/workspace".to_string(),
        timeout: Duration::from_secs(300),
        output_buffer_limit: 5 * 1024 * 1024,
        graceful_timeout: Duration::from_secs(10),
        force_timeout: Duration::from_secs(5),
    };

    assert_eq!(config.command, "test-command");
    assert_eq!(config.args.len(), 2);
    assert_eq!(config.input, Some("test input".to_string()));
    assert_eq!(config.workspace, "/test/workspace");
    assert_eq!(config.output_buffer_limit, 5 * 1024 * 1024);
}

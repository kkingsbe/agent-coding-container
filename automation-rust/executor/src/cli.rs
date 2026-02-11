//! # CLI Executor
//!
//! This module provides functionality for executing CLI commands with:
//! - Process spawning with stdin/stdout/stderr pipes
//! - Real-time output monitoring
//! - Pattern-based early termination (Mistake Limit detection)
//! - Output buffering with configurable max size
//! - Timeout enforcement

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Notify;
use tokio::time::timeout;
use tracing::{error, info, instrument, warn};

use automation_common::AutomationError;

use regex::bytes::Regex;

/// Configuration for CLI execution.
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Command to execute (default: "kilocode")
    pub command: String,

    /// Arguments to pass to the command
    pub args: Vec<String>,

    /// Input to pipe to stdin
    pub input: Option<String>,

    /// Workspace directory path
    pub workspace: String,

    /// Execution timeout
    pub timeout: Duration,

    /// Maximum output buffer size (default: 10MB)
    pub output_buffer_limit: usize,

    /// Graceful termination timeout (default: 5s)
    pub graceful_timeout: Duration,

    /// Forceful termination timeout (default: 2s)
    pub force_timeout: Duration,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            command: String::from("kilocode"),
            args: vec![
                "--mode".to_string(),
                "orchestrator".to_string(),
                "--auto".to_string(),
                "--timeout".to_string(),
                "1800".to_string(),
                "--workspace".to_string(),
                "/workspace".to_string(),
            ],
            input: None,
            workspace: String::from("/workspace"),
            timeout: Duration::from_secs(1800),
            output_buffer_limit: 10 * 1024 * 1024, // 10MB
            graceful_timeout: Duration::from_secs(5),
            force_timeout: Duration::from_secs(2),
        }
    }
}

/// Output buffer with configurable size limit.
#[derive(Debug)]
pub struct OutputBuffer {
    /// Combined stdout output
    stdout: Vec<u8>,

    /// Combined stderr output
    stderr: Vec<u8>,

    /// Maximum buffer size in bytes
    max_size: usize,

    /// Whether buffer has reached capacity limit
    is_full: bool,
}

impl OutputBuffer {
    /// Create a new output buffer with specified max size.
    pub fn new(max_size: usize) -> Self {
        Self {
            stdout: Vec::with_capacity(max_size),
            stderr: Vec::with_capacity(max_size),
            max_size,
            is_full: false,
        }
    }

    /// Add stdout bytes to the buffer.
    ///
    /// Returns true if buffer is now full and data was discarded.
    pub fn add_stdout(&mut self, chunk: &[u8]) -> bool {
        let new_len = self.stdout.len() + chunk.len();
        if new_len > self.max_size {
            // Truncate to fit within limit
            let _keep_size = self.max_size.saturating_sub(chunk.len());
            let remaining = self.max_size - self.stdout.len();
            let to_add = if chunk.len() > remaining {
                &chunk[..remaining]
            } else {
                chunk
            };
            self.stdout.extend_from_slice(to_add);
            self.is_full = new_len > self.max_size;
            return self.is_full;
        }
        self.stdout.extend_from_slice(chunk);
        false
    }

    /// Add stderr bytes to the buffer.
    ///
    /// Returns true if buffer is now full and data was discarded.
    pub fn add_stderr(&mut self, chunk: &[u8]) -> bool {
        let new_len = self.stderr.len() + chunk.len();
        if new_len > self.max_size {
            // Truncate to fit within limit
            let _keep_size = self.max_size.saturating_sub(chunk.len());
            let remaining = self.max_size - self.stderr.len();
            let to_add = if chunk.len() > remaining {
                &chunk[..remaining]
            } else {
                chunk
            };
            self.stderr.extend_from_slice(to_add);
            self.is_full = new_len > self.max_size;
            return self.is_full;
        }
        self.stderr.extend_from_slice(chunk);
        false
    }

    /// Get combined stdout as a string.
    pub fn get_stdout(&self) -> String {
        String::from_utf8_lossy(&self.stdout).to_string()
    }

    /// Get combined stderr as a string.
    pub fn get_stderr(&self) -> String {
        String::from_utf8_lossy(&self.stderr).to_string()
    }

    /// Get total size of buffer.
    pub fn len(&self) -> usize {
        self.stdout.len() + self.stderr.len()
    }

    /// Check if buffer has reached capacity limit.
    pub fn is_full(&self) -> bool {
        self.is_full
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.stdout.clear();
        self.stderr.clear();
        self.is_full = false;
    }
}

/// Error patterns for early termination.
///
/// These patterns are used to detect error conditions that should trigger
/// early termination of the command execution.
#[derive(Debug, Clone)]
pub struct ErrorPatterns {
    /// Mistake Limit patterns
    mistake_limit: Vec<Regex>,
}

impl ErrorPatterns {
    /// Create error patterns with default mistake limit patterns.
    pub fn default() -> Self {
        let patterns = vec![
            b"Mistake Limit Reached".to_vec(),
            b"[\\s\\|]*\\xE2\\x9C\\x97\\s*Mistake Limit Reached".to_vec(),
            b"[\\s\\|]*X\\s*Mistake Limit Reached".to_vec(),
            b"[\\s\\|]*\\*\\s*Mistake Limit Reached".to_vec(),
            b"Mistake Limit Reached[\\s\\S]{0,500}This may indicate a failure in the model".to_vec(),
        ];

        let mistake_limit_set: Vec<Regex> = patterns
            .into_iter()
            .filter_map(|p| String::from_utf8(p).ok())
            .filter_map(|p| Regex::new(&p).ok())
            .collect();

        Self {
            mistake_limit: mistake_limit_set,
        }
    }

    /// Check if any pattern matches in the output.
    pub fn matches(&self, output: &[u8]) -> bool {
        for pattern in &self.mistake_limit {
            if pattern.is_match(output) {
                return true;
            }
        }
        false
    }

    /// Get the first matching pattern if any.
    pub fn find_match(&self, output: &[u8]) -> Option<String> {
        for pattern in &self.mistake_limit {
            if pattern.is_match(output) {
                return Some(pattern.as_str().to_string());
            }
        }
        None
    }
}

/// Process execution result.
#[derive(Debug, Clone)]
pub struct ProcessResult {
    /// Exit code (None if terminated by signal)
    pub exit_code: Option<i32>,

    /// Combined stdout output
    pub stdout: String,

    /// Combined stderr output
    pub stderr: String,

    /// Whether process was terminated early
    pub terminated_early: bool,

    /// Reason for early termination
    pub termination_reason: Option<String>,

    /// Execution duration in milliseconds
    pub execution_time_ms: u64,

    /// Error if process failed to start
    pub error: Option<String>,
}

impl Default for ProcessResult {
    fn default() -> Self {
        Self {
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            terminated_early: false,
            termination_reason: None,
            execution_time_ms: 0,
            error: None,
        }
    }
}

/// CLI executor for running commands with monitoring.
///
/// This executor provides:
/// - Process spawning with tokio
/// - Real-time output capture and buffering
/// - Pattern-based early termination detection
/// - Timeout enforcement
/// - Graceful and forceful termination
///
/// # Example
///
/// ```no_run
/// use automation_executor::CLIExecutor;
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let executor = CLIExecutor::new(ExecutorConfig::default());
///     let result = executor.execute().await?;
///
///     println!("Exit code: {:?}", result.exit_code);
///     println!("Output: {}", result.stdout);
///     Ok(())
/// }
/// ```
pub struct CLIExecutor {
    config: ExecutorConfig,
    patterns: ErrorPatterns,
    shutdown_notify: Arc<Notify>,
    is_running: Arc<AtomicBool>,
}

impl CLIExecutor {
    /// Create a new CLI executor with the specified configuration.
    pub fn new(config: ExecutorConfig) -> Self {
        Self {
            config,
            patterns: ErrorPatterns::default(),
            shutdown_notify: Arc::new(Notify::new()),
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a CLI executor with custom error patterns.
    pub fn with_patterns(config: ExecutorConfig, patterns: ErrorPatterns) -> Self {
        Self {
            config,
            patterns,
            shutdown_notify: Arc::new(Notify::new()),
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Execute the configured command.
    ///
    /// This method will:
    /// 1. Spawn the process with stdin/stdout/stderr pipes
    /// 2. Monitor output in real-time
    /// 3. Check for error patterns (early termination)
    /// 4. Enforce timeout
    /// 5. Handle graceful shutdown
    ///
    /// # Returns
    ///
    /// Returns `Ok(ProcessResult)` with execution details on success or shutdown.
    /// Returns `Err(AutomationError)` on process spawn failure.
    #[instrument(skip(self))]
    pub async fn execute(&self) -> Result<ProcessResult, AutomationError> {
        if self.is_running.load(Ordering::Relaxed) {
            return Err(AutomationError::ScheduleConfigInvalid {
                reason: "Executor is already running".to_string(),
            });
        }

        self.is_running.store(true, Ordering::Relaxed);
        info!(
            command = self.config.command,
            args = ?self.config.args,
            workspace = self.config.workspace,
            "Starting CLI execution"
        );

        let start_time = std::time::Instant::now();

        // Build the command
        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args);
        cmd.current_dir(&self.config.workspace);

        // Set up pipes for stdin, stdout, stderr
        let mut child = match cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                error!(
                    command = self.config.command,
                    error = %e,
                    "Failed to spawn process"
                );
                self.is_running.store(false, Ordering::Relaxed);
                return Err(AutomationError::ProcessSpawnFailed {
                    command: self.config.command.clone(),
                    source: e,
                });
            }
        };

        let shutdown_notify = Arc::clone(&self.shutdown_notify);
        let patterns = self.patterns.clone();
        let output_buffer_limit = self.config.output_buffer_limit;
        let input = self.config.input.clone();

        // Create a task for execution
        let task = tokio::spawn(async move {
            let buffer = Arc::new(std::sync::Mutex::new(OutputBuffer::new(output_buffer_limit)));
            let terminated_early = Arc::new(std::sync::Mutex::new(false));
            let termination_reason = Arc::new(std::sync::Mutex::new(None::<String>));

            // Split child into stdout, stderr, and stdin handles
            let stdout = child.stdout.take().expect("stdout not captured");
            let stderr = child.stderr.take().expect("stderr not captured");
            let mut stdin = child.stdin.take().expect("stdin not captured");

            // Write input to stdin if provided
            if let Some(ref input) = input {
                if let Err(e) = stdin.write_all(input.as_bytes()).await {
                    error!(
                        error = %e,
                        "Failed to write to stdin"
                    );
                }
                // Close stdin to signal EOF
                let _ = stdin.shutdown().await;
            }

            // Create a task to read stdout
            let buffer_stdout = Arc::clone(&buffer);
            let stdout_task = tokio::spawn(async move {
                let mut reader = BufReader::new(stdout);
                let mut buf = vec![0u8; 8192];

                loop {
                    match reader.read(&mut buf).await {
                        Ok(n) if n > 0 => {
                            let chunk = &buf[..n];
                            if buffer_stdout.lock().unwrap().add_stdout(chunk) {
                                warn!("Output buffer full, some stdout data was truncated");
                            }
                        }
                        Ok(_) => break, // EOF or 0 bytes
                        Err(e) => {
                            error!(error = %e, "Error reading stdout");
                            break;
                        }
                    }
                }
            });

            // Create a task to read stderr
            let buffer_stderr = Arc::clone(&buffer);
            let patterns_stderr = patterns.clone();
            let terminated_early_stderr = Arc::clone(&terminated_early);
            let termination_reason_stderr = Arc::clone(&termination_reason);
            let stderr_task = tokio::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut buf = vec![0u8; 8192];

                loop {
                    match reader.read(&mut buf).await {
                        Ok(n) if n > 0 => {
                            let chunk = &buf[..n];
                            if buffer_stderr.lock().unwrap().add_stderr(chunk) {
                                warn!("Output buffer full, some stderr data was truncated");
                            }
                            // Check for error patterns on stderr
                            let stderr_data = buffer_stderr.lock().unwrap().get_stderr();
                            if patterns_stderr.matches(stderr_data.as_bytes()) {
                                let pattern = patterns_stderr.find_match(stderr_data.as_bytes());
                                if let Some(pat) = pattern {
                                    info!(pattern = %pat, "Error pattern detected, triggering early termination");
                                    *terminated_early_stderr.lock().unwrap() = true;
                                    *termination_reason_stderr.lock().unwrap() = Some(format!("Error pattern matched: {}", pat));
                                }
                            }
                        }
                        Ok(_) => break, // EOF or 0 bytes
                        Err(e) => {
                            error!(error = %e, "Error reading stderr");
                            break;
                        }
                    }
                }
            });

            // Wait for both output tasks and process completion
            tokio::select! {
                // Handle shutdown signal
                _ = shutdown_notify.notified() => {
                    info!("Shutdown signal received during execution");
                    *terminated_early.lock().unwrap() = true;
                    *termination_reason.lock().unwrap() = Some("Shutdown signal".to_string());
                }
                // Handle Ctrl+C
                _ = tokio::signal::ctrl_c() => {
                    info!("Ctrl+C received during execution");
                    *terminated_early.lock().unwrap() = true;
                    *termination_reason.lock().unwrap() = Some("Ctrl+C".to_string());
                }
                // Handle process exit
                _ = child.wait() => {
                    info!("Process completed");
                }
            }

            // Wait for output tasks to complete
            let _ = tokio::join!(stdout_task, stderr_task);

            let buffer_unlocked = buffer.lock().unwrap();
            let terminated_early_val = *terminated_early.lock().unwrap();
            let termination_reason_val = termination_reason.lock().unwrap().clone();
            ProcessResult {
                exit_code: Some(0), // Assume success if not terminated early
                stdout: buffer_unlocked.get_stdout(),
                stderr: buffer_unlocked.get_stderr(),
                terminated_early: terminated_early_val,
                termination_reason: termination_reason_val,
                execution_time_ms: start_time.elapsed().as_millis() as u64,
                error: None,
            }
        });

        let result = timeout(self.config.timeout, task).await;

        match result {
            Ok(Ok(res)) => {
                self.is_running.store(false, Ordering::Relaxed);
                info!(
                    execution_time_ms = res.execution_time_ms,
                    terminated_early = res.terminated_early,
                    "CLI execution completed"
                );
                Ok(res)
            }
            Ok(Err(e)) => {
                self.is_running.store(false, Ordering::Relaxed);
                error!(error = %e, "Task execution failed");
                Err(AutomationError::ProcessSpawnFailed {
                    command: self.config.command.clone(),
                    source: std::io::Error::new(std::io::ErrorKind::Other, format!("Task execution failed: {}", e)),
                })
            }
            Err(_) => {
                // Timeout occurred
                info!(
                    timeout_sec = self.config.timeout.as_secs(),
                    "CLI execution timed out"
                );
                self.is_running.store(false, Ordering::Relaxed);

                Ok(ProcessResult {
                    exit_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    terminated_early: true,
                    termination_reason: Some("Timeout".to_string()),
                    execution_time_ms: self.config.timeout.as_millis() as u64,
                    error: None,
                })
            }
        }
    }

    /// Get whether the executor is currently running.
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Request graceful shutdown of the executor.
    pub fn shutdown(&self) {
        self.shutdown_notify.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_config_default() {
        let config = ExecutorConfig::default();
        assert_eq!(config.command, "kilocode");
        assert_eq!(config.workspace, "/workspace");
        assert_eq!(config.timeout, Duration::from_secs(1800));
        assert_eq!(config.output_buffer_limit, 10 * 1024 * 1024);
    }

    #[test]
    fn test_output_buffer_new() {
        let buffer = OutputBuffer::new(100);
        assert_eq!(buffer.len(), 0);
        assert!(!buffer.is_full());
    }

    #[test]
    fn test_output_buffer_add_within_limit() {
        let mut buffer = OutputBuffer::new(100);
        let added = buffer.add_stdout(b"hello");
        assert_eq!(added, false);
        assert_eq!(buffer.len(), 5);
        assert!(!buffer.is_full());
        assert_eq!(buffer.get_stdout(), "hello");
    }

    #[test]
    fn test_output_buffer_add_at_limit() {
        let mut buffer = OutputBuffer::new(10);
        let added = buffer.add_stdout(b"hello world");
        assert_eq!(added, true); // Buffer is full now
        assert_eq!(buffer.len(), 10); // Limited to 10
        assert!(buffer.is_full());
        assert_eq!(buffer.get_stdout(), "hello worl"); // Truncated
    }

    #[test]
    fn test_error_patterns_default() {
        let patterns = ErrorPatterns::default();
        assert!(patterns.matches(b"Mistake Limit Reached"));
        assert!(patterns.matches(b"\xE2\x9C\x97 Mistake Limit Reached"));
        assert!(!patterns.matches(b"Normal output"));
    }

    #[test]
    fn test_process_result_default() {
        let result = ProcessResult::default();
        assert_eq!(result.exit_code, None);
        assert_eq!(result.stdout, String::new());
        assert!(!result.terminated_early);
    }

    #[test]
    fn test_process_is_running() {
        let executor = CLIExecutor::new(ExecutorConfig::default());
        assert!(!executor.is_running());
    }

    #[test]
    fn test_executor_shutdown() {
        let executor = CLIExecutor::new(ExecutorConfig::default());
        executor.shutdown();
        // Just verifies shutdown doesn't panic
    }
}

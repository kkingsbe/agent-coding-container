# automation-executor API Reference

The `automation-executor` crate provides CLI process execution and monitoring capabilities for the gastown system.

## Table of Contents

- [Overview](#overview)
- [Executor](#executor)
- [Command Execution](#command-execution)
- [Output Capture](#output-capture)
- [Process Management](#process-management)
- [Usage Examples](#usage-examples)

## Overview

`automation-executor` provides:

- **Process spawning** - Execute CLI commands asynchronously
- **Output monitoring** - Capture stdout and stderr in real-time
- **Pattern-based termination** - Stop processes based on output patterns
- **Graceful termination** - Cleanly terminate processes with timeout
- **Exit code handling** - Process and validate exit codes

## Executor

### `Executor`

Executes CLI commands and captures output.

```rust
pub struct Executor {
    timeout: Duration,
    working_dir: Option<PathBuf>,
    env: HashMap<String, String>,
}
```

### `Executor::new()`

Create a new executor with default settings.

```rust
impl Executor {
    pub fn new() -> Self
}
```

**Default Settings:**

| Setting | Default Value |
|---------|---------------|
| Timeout | 30 minutes (1800 seconds) |
| Working Directory | Current directory |
| Environment Variables | Inherited from parent process |

**Example:**

```rust
use automation_executor::Executor;

let executor = Executor::new();
```

### `Executor::with_timeout()`

Create an executor with a specific timeout.

```rust
impl Executor {
    pub fn with_timeout(timeout: Duration) -> Self
}
```

**Parameters:**

- `timeout` - Maximum time to allow the process to run

**Returns:**

- `Executor` - Executor with configured timeout

**Example:**

```rust
use automation_executor::Executor;
use std::time::Duration;

let executor = Executor::with_timeout(Duration::from_secs(60));
```

### `Executor::with_working_dir()`

Set the working directory for command execution.

```rust
impl Executor {
    pub fn with_working_dir(self, dir: impl Into<PathBuf>) -> Self
}
```

**Parameters:**

- `dir` - Working directory path

**Returns:**

- `Executor` - Executor with configured working directory

**Example:**

```rust
use automation_executor::Executor;

let executor = Executor::new()
    .with_working_dir("/workspace");
```

### `Executor::with_env()`

Set environment variables for the command.

```rust
impl Executor {
    pub fn with_env(self, env: HashMap<String, String>) -> Self
}
```

**Parameters:**

- `env` - Environment variables to set

**Returns:**

- `Executor` - Executor with configured environment

**Example:**

```rust
use automation_executor::Executor;
use std::collections::HashMap;

let mut env = HashMap::new();
env.insert("MY_VAR".to_string(), "value".to_string());

let executor = Executor::new()
    .with_env(env);
```

### `Executor::with_env_var()`

Add a single environment variable.

```rust
impl Executor {
    pub fn with_env_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self
}
```

**Parameters:**

- `key` - Environment variable name
- `value` - Environment variable value

**Returns:**

- `Executor` - Executor with added environment variable

**Example:**

```rust
use automation_executor::Executor;

let executor = Executor::new()
    .with_env_var("MY_VAR", "value")
    .with_env_var("ANOTHER_VAR", "another_value");
```

## Command Execution

### `execute()`

Execute a command and wait for completion.

```rust
impl Executor {
    pub async fn execute(&self, command: Vec<&str>) -> Result<ExecutionResult, AutomationError>
}
```

**Parameters:**

- `command` - Command and arguments as a vector of strings

**Returns:**

- `Result<ExecutionResult, AutomationError>` - Execution result or error

**Example:**

```rust
use automation_executor::Executor;

let executor = Executor::new();
let result = executor
    .execute(vec!["echo", "Hello, World!"])
    .await?;
```

### `execute_with_termination()`

Execute a command with pattern-based termination.

```rust
impl Executor {
    pub async fn execute_with_termination(
        &self,
        command: Vec<&str>,
        termination_patterns: Vec<TerminationPattern>
    ) -> Result<ExecutionResult, AutomationError>
}
```

**Parameters:**

- `command` - Command and arguments
- `termination_patterns` - Patterns that trigger early termination

**Returns:**

- `Result<ExecutionResult, AutomationError>` - Execution result or error

**TerminationPattern:**

```rust
pub struct TerminationPattern {
    pub pattern: String,
    pub stream: OutputStream,
    pub action: TerminationAction,
}

pub enum OutputStream {
    Stdout,
    Stderr,
    Both,
}

pub enum TerminationAction {
    Stop,
    Continue,
}
```

**Example:**

```rust
use automation_executor::{Executor, TerminationPattern, OutputStream, TerminationAction};

let executor = Executor::new();
let patterns = vec![
    TerminationPattern {
        pattern: "Error:".to_string(),
        stream: OutputStream::Both,
        action: TerminationAction::Stop,
    },
];

let result = executor
    .execute_with_termination(vec!["some-command"], patterns)
    .await?;
```

## Output Capture

### `ExecutionResult`

Contains the results of command execution.

```rust
pub struct ExecutionResult {
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub duration: Duration,
    pub terminated_by_pattern: bool,
    pub timeout_reached: bool,
}
```

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `exit_code` | `Option<i32>` | Process exit code (None if terminated) |
| `stdout` | `Vec<u8>` | Captured stdout output |
| `stderr` | `Vec<u8>` | Captured stderr output |
| `duration` | `Duration` | Execution duration |
| `terminated_by_pattern` | `bool` | Whether terminated by pattern match |
| `timeout_reached` | `bool` | Whether timeout was reached |

### `stdout_as_str()`

Get stdout as a string.

```rust
impl ExecutionResult {
    pub fn stdout_as_str(&self) -> Result<&str, Utf8Error>
}
```

**Returns:**

- `Result<&str, Utf8Error>` - Stdout as string or UTF-8 error

**Example:**

```rust
use automation_executor::Executor;

let executor = Executor::new();
let result = executor.execute(vec!["echo", "test"]).await?;

if let Ok(stdout) = result.stdout_as_str() {
    println!("Output: {}", stdout);
}
```

### `stderr_as_str()`

Get stderr as a string.

```rust
impl ExecutionResult {
    pub fn stderr_as_str(&self) -> Result<&str, Utf8Error>
}
```

**Returns:**

- `Result<&str, Utf8Error>` - Stderr as string or UTF-8 error

**Example:**

```rust
use automation_executor::Executor;

let executor = Executor::new();
let result = executor.execute(vec!["ls", "/nonexistent"]).await?;

if let Ok(stderr) = result.stderr_as_str() {
    println!("Error: {}", stderr);
}
```

### `stdout_lossy()`

Get stdout as a string, replacing invalid UTF-8 sequences.

```rust
impl ExecutionResult {
    pub fn stdout_lossy(&self) -> String
}
```

**Returns:**

- `String` - Stdout as string with invalid UTF-8 replaced

**Example:**

```rust
use automation_executor::Executor;

let executor = Executor::new();
let result = executor.execute(vec!["some-command"]).await?;

let stdout = result.stdout_lossy();
println!("{}", stdout);
```

### `stderr_lossy()`

Get stderr as a string, replacing invalid UTF-8 sequences.

```rust
impl ExecutionResult {
    pub fn stderr_lossy(&self) -> String
}
```

**Returns:**

- `String` - Stderr as string with invalid UTF-8 replaced

**Example:**

```rust
use automation_executor::Executor;

let executor = Executor::new();
let result = executor.execute(vec!["some-command"]).await?;

let stderr = result.stderr_lossy();
eprintln!("{}", stderr);
```

### `success()`

Check if the command succeeded (exit code 0).

```rust
impl ExecutionResult {
    pub fn success(&self) -> bool
}
```

**Returns:**

- `bool` - true if exit code is 0

**Example:**

```rust
use automation_executor::Executor;

let executor = Executor::new();
let result = executor.execute(vec!["echo", "test"]).await?;

if result.success() {
    println!("Command succeeded");
} else {
    println!("Command failed with exit code: {:?}", result.exit_code);
}
```

## Process Management

### `RunningProcess`

Represents a running process that can be monitored and controlled.

```rust
pub struct RunningProcess {
    child: Child,
    stdout_lines: Receiver<String>,
    stderr_lines: Receiver<String>,
    start_time: Instant,
}
```

### `execute_async()`

Execute a command asynchronously, returning a running process.

```rust
impl Executor {
    pub async fn execute_async(&self, command: Vec<&str>) -> Result<RunningProcess, AutomationError>
}
```

**Parameters:**

- `command` - Command and arguments

**Returns:**

- `Result<RunningProcess, AutomationError>` - Running process or error

**Example:**

```rust
use automation_executor::Executor;

let executor = Executor::new();
let process = executor.execute_async(vec!["long-running-command"]).await?;

// Monitor process while it runs
while let Some(line) = process.stdout_lines.next().await {
    println!("{}", line);
}
```

### `terminate()`

Terminate a running process.

```rust
impl RunningProcess {
    pub async fn terminate(self) -> Result<ExecutionResult, AutomationError>
}
```

**Returns:**

- `Result<ExecutionResult, AutomationError>` - Final execution result

**Example:**

```rust
use automation_executor::Executor;

let executor = Executor::new();
let process = executor.execute_async(vec!["long-running-command"]).await?;

// Do something...

// Terminate the process
let result = process.terminate().await?;
```

### `wait()`

Wait for a running process to complete.

```rust
impl RunningProcess {
    pub async fn wait(self) -> Result<ExecutionResult, AutomationError>
}
```

**Returns:**

- `Result<ExecutionResult, AutomationError>` - Execution result

**Example:**

```rust
use automation_executor::Executor;

let executor = Executor::new();
let process = executor.execute_async(vec!["some-command"]).await?;

// Monitor output
while let Some(line) = process.stdout_lines.next().await {
    println!("{}", line);
}

// Wait for completion
let result = process.wait().await?;
```

### `kill()`

Forcefully kill a running process (SIGKILL).

```rust
impl RunningProcess {
    pub async fn kill(self) -> Result<ExecutionResult, AutomationError>
}
```

**Returns:**

- `Result<ExecutionResult, AutomationError>` - Execution result

**Example:**

```rust
use automation_executor::Executor;

let executor = Executor::new();
let process = executor.execute_async(vec!["long-running-command"]).await?;

// Force kill immediately
let result = process.kill().await?;
```

### `pid()`

Get the process ID of a running process.

```rust
impl RunningProcess {
    pub fn pid(&self) -> u32
}
```

**Returns:**

- `u32` - Process ID

**Example:**

```rust
use automation_executor::Executor;

let executor = Executor::new();
let process = executor.execute_async(vec!["some-command"]).await?;

println!("Process PID: {}", process.pid());
```

### `is_running()`

Check if the process is still running.

```rust
impl RunningProcess {
    pub fn is_running(&self) -> bool
}
```

**Returns:**

- `bool` - true if process is running

**Example:**

```rust
use automation_executor::Executor;

let executor = Executor::new();
let process = executor.execute_async(vec!["some-command"]).await?;

if process.is_running() {
    println!("Process is running");
}
```

## Usage Examples

### Basic Command Execution

```rust
use automation_executor::Executor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executor = Executor::new();
    
    // Execute a simple command
    let result = executor
        .execute(vec!["echo", "Hello, World!"])
        .await?;
    
    println!("Exit code: {:?}", result.exit_code);
    println!("Stdout: {}", result.stdout_lossy());
    
    Ok(())
}
```

### Command with Timeout

```rust
use automation_executor::Executor;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Command must complete within 5 seconds
    let executor = Executor::with_timeout(Duration::from_secs(5));
    
    let result = executor
        .execute(vec!["sleep", "10"])
        .await?;
    
    if result.timeout_reached {
        println!("Command timed out");
    }
    
    Ok(())
}
```

### Working Directory and Environment Variables

```rust
use automation_executor::Executor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executor = Executor::new()
        .with_working_dir("/workspace")
        .with_env_var("MY_VAR", "value")
        .with_env_var("ANOTHER_VAR", "another_value");
    
    let result = executor
        .execute(vec!["printenv", "MY_VAR"])
        .await?;
    
    println!("MY_VAR = {}", result.stdout_lossy().trim());
    
    Ok(())
}
```

### Pattern-Based Termination

```rust
use automation_executor::{Executor, TerminationPattern, OutputStream, TerminationAction};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executor = Executor::new();
    
    // Stop if "Error:" appears in output
    let patterns = vec![
        TerminationPattern {
            pattern: "Error:".to_string(),
            stream: OutputStream::Both,
            action: TerminationAction::Stop,
        },
    ];
    
    let result = executor
        .execute_with_termination(
            vec!["some-command"],
            patterns
        )
        .await?;
    
    if result.terminated_by_pattern {
        println!("Command terminated early due to pattern match");
    }
    
    Ok(())
}
```

### Async Execution with Monitoring

```rust
use automation_executor::Executor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executor = Executor::new();
    
    // Start command asynchronously
    let process = executor
        .execute_async(vec!["long-running-process"])
        .await?;
    
    println!("Process started with PID: {}", process.pid());
    
    // Monitor output
    while let Some(line) = process.stdout_lines.next().await {
        println!("{}", line);
        
        // Check if we should stop
        if line.contains("done") {
            println!("Process complete, terminating");
            break;
        }
    }
    
    // Wait for process to complete
    let result = process.wait().await?;
    println!("Exit code: {:?}", result.exit_code);
    
    Ok(())
}
```

### Streaming Output

```rust
use automation_executor::Executor;
use tokio::select;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executor = Executor::new();
    let process = executor.execute_async(vec!["some-command"]).await?;
    
    loop {
        tokio::select! {
            Some(line) = process.stdout_lines.next() => {
                println!("OUT: {}", line);
            }
            Some(line) = process.stderr_lines.next() => {
                eprintln!("ERR: {}", line);
            }
            else => {
                // Both channels closed
                break;
            }
        }
    }
    
    let result = process.wait().await?;
    println!("Done, exit code: {:?}", result.exit_code);
    
    Ok(())
}
```

### Multiple Commands in Sequence

```rust
use automation_executor::Executor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executor = Executor::new();
    
    // Run multiple commands
    let commands = vec![
        vec!["echo", "Step 1"],
        vec!["echo", "Step 2"],
        vec!["echo", "Step 3"],
    ];
    
    for command in commands {
        println!("Executing: {:?}", command);
        
        let result = executor.execute(command).await?;
        
        if !result.success() {
            eprintln!("Command failed: {:?}", result.exit_code);
            eprintln!("Error: {}", result.stderr_lossy());
            return Err("Command failed".into());
        }
        
        println!("Output: {}", result.stdout_lossy().trim());
    }
    
    println!("All commands completed successfully");
    
    Ok(())
}
```

### Error Handling

```rust
use automation_executor::{Executor, AutomationError};

async fn execute_with_retry(
    executor: &Executor,
    command: Vec<&str>,
    max_retries: usize
) -> Result<(), Box<dyn std::error::Error>> {
    for attempt in 0..=max_retries {
        match executor.execute(command.clone()).await {
            Ok(result) if result.success() => {
                println!("Command succeeded on attempt {}", attempt + 1);
                return Ok(());
            }
            Ok(result) => {
                eprintln!("Command failed (exit code {:?})", result.exit_code);
                eprintln!("Stderr: {}", result.stderr_lossy());
            }
            Err(AutomationError::Execution(msg)) if attempt < max_retries => {
                eprintln!("Execution error (attempt {}): {}", attempt + 1, msg);
            }
            Err(e) => return Err(e.into()),
        }
        
        if attempt < max_retries {
            println!("Retrying in 5 seconds...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }
    
    Err("Max retries exceeded".into())
}
```

## Error Handling

### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `AutomationError::Execution("Command not found")` | Command doesn't exist | Check command name and PATH |
| `AutomationError::Execution("Permission denied")` | Insufficient permissions | Check file permissions |
| `AutomationError::Execution("Timeout")` | Command took too long | Increase timeout or optimize command |
| `AutomationError::Io` | I/O errors | Check disk space and permissions |

### Error Handling Example

```rust
use automation_executor::{Executor, AutomationError};

async fn safe_execute(command: Vec<&str>) -> Option<String> {
    let executor = Executor::new();
    
    match executor.execute(command).await {
        Ok(result) if result.success() => {
            Some(result.stdout_lossy())
        }
        Ok(result) => {
            eprintln!("Command failed: {}", result.stderr_lossy());
            None
        }
        Err(AutomationError::Execution(msg)) => {
            eprintln!("Execution error: {}", msg);
            None
        }
        Err(e) => {
            eprintln!("Error: {:?}", e);
            None
        }
    }
}
```

## See Also

- [Architecture Documentation](../architecture.md) - System architecture overview
- [Common API](common.md) - Common types and utilities
- [Agents API](agents.md) - Agent implementations
- [Deployment Guide](../deployment.md) - Deployment instructions

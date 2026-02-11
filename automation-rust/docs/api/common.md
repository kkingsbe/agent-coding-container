# automation-common API Reference

The `automation-common` crate provides shared types, utilities, and error handling used across all modules in the gastown system.

## Table of Contents

- [Overview](#overview)
- [Types](#types)
- [Error Handling](#error-handling)
- [Configuration](#configuration)
- [Logging](#logging)
- [Usage Examples](#usage-examples)

## Overview

`automation-common` is a foundational crate that provides:

- **Common error types** with `thiserror` for easy error handling
- **Configuration management** for loading settings from environment
- **Logging initialization** using `tracing` and `tracing-subscriber`
- **Result type aliases** for consistent error handling across the codebase

## Types

### `Result<T>`

Type alias for `std::result::Result<T, AutomationError>`.

```rust
use automation_common::Result;

pub type Result<T> = std::result::Result<T, AutomationError>;

// Usage
async fn example() -> Result<()> {
    do_something().await
}
```

### `Config`

Configuration structure loaded from environment variables.

```rust
use automation_common::Config;

pub struct Config {
    pub workspace: PathBuf,
    pub state: PathBuf,
    pub interval: Option<Duration>,
    pub timeout: Option<Duration>,
    pub immediate: bool,
}
```

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `workspace` | `PathBuf` | Path to the workspace directory |
| `state` | `PathBuf` | Path to the state directory |
| `interval` | `Option<Duration>` | Execution interval (if specified) |
| `timeout` | `Option<Duration>` | Execution timeout (if specified) |
| `immediate` | `bool` | Whether to run immediately on start |

### `LogLevel`

Log level enum for filtering log output.

```rust
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}
```

## Error Handling

### `AutomationError`

The main error type for the automation system.

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AutomationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Lock error: {0}")]
    Lock(String),

    #[error("State error: {0}")]
    State(String),

    #[error("Execution error: {0}")]
    Execution(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Workspace error: {0}")]
    Workspace(String),

    #[error("Agent error: {0}")]
    Agent(String),
}
```

**Error Variants:**

| Variant | Description | Source |
|---------|-------------|--------|
| `Io` | Standard I/O errors | `std::io::Error` |
| `Json` | JSON parsing/serialization errors | `serde_json::Error` |
| `Lock` | Lock acquisition/release errors | Lock module |
| `State` | State management errors | State module |
| `Execution` | CLI execution errors | Executor module |
| `Config` | Configuration errors | Config module |
| `Workspace` | Workspace operation errors | Workspace module |
| `Agent` | Agent-specific errors | Agents module |

### Error Creation

```rust
use automation_common::{AutomationError, Result};

// From std::io::Error (automatic)
async fn read_file() -> Result<String> {
    let content = tokio::fs::read_to_string("file.txt").await?;
    Ok(content)
}

// From std::io::Error with context
use anyhow::Context;

async fn read_file_with_context() -> Result<String> {
    let content = tokio::fs::read_to_string("file.txt")
        .await
        .context("Failed to read configuration file")?;
    Ok(content)
}

// Custom error
fn validate(value: i32) -> Result<()> {
    if value < 0 {
        return Err(AutomationError::Config(
            "Value must be non-negative".to_string()
        ));
    }
    Ok(())
}
```

### Error Propagation

```rust
use automation_common::Result;

async fn process() -> Result<()> {
    // The ? operator propagates errors
    let config = load_config()?;
    let state = load_state()?;
    // ...
    Ok(())
}
```

## Configuration

### `Config::from_env()`

Load configuration from environment variables.

```rust
impl Config {
    pub fn from_env() -> Result<Self>
}
```

**Environment Variables:**

| Variable | Description | Default |
|----------|-------------|---------|
| `AUTOMATION_WORKSPACE` | Workspace directory path | `/workspace` |
| `AUTOMATION_STATE` | State directory path | `$WORKSPACE/.state` |
| `AUTOMATION_INTERVAL` | Execution interval in seconds | Agent-specific |
| `AUTOMATION_TIMEOUT` | Execution timeout in seconds | `1800` (30 min) |
| `AUTOMATION_IMMEDIATE` | Run immediately (true/false) | `false` |

**Example:**

```rust
use automation_common::Config;

let config = Config::from_env()?;

println!("Workspace: {}", config.workspace.display());
println!("State: {}", config.state.display());
```

### `Config::new()`

Create configuration with explicit values.

```rust
impl Config {
    pub fn new(
        workspace: impl Into<PathBuf>,
        state: impl Into<PathBuf>,
    ) -> Self
}
```

**Example:**

```rust
use automation_common::Config;
use std::path::PathBuf;

let config = Config::new(
    "/path/to/workspace",
    "/path/to/state"
);
```

### `Config::with_interval()`

Set the execution interval.

```rust
impl Config {
    pub fn with_interval(mut self, interval: Duration) -> Self
}
```

**Example:**

```rust
use automation_common::Config;
use std::time::Duration;

let config = Config::from_env()?
    .with_interval(Duration::from_secs(300));
```

### `Config::with_timeout()`

Set the execution timeout.

```rust
impl Config {
    pub fn with_timeout(mut self, timeout: Duration) -> Self
}
```

**Example:**

```rust
use automation_common::Config;
use std::time::Duration;

let config = Config::from_env()?
    .with_timeout(Duration::from_secs(1800));
```

### `Config::with_immediate()`

Set whether to run immediately on start.

```rust
impl Config {
    pub fn with_immediate(mut self, immediate: bool) -> Self
}
```

**Example:**

```rust
use automation_common::Config;

let config = Config::from_env()?
    .with_immediate(true);
```

## Logging

### `init_logging()`

Initialize logging with the specified log level.

```rust
use tracing::Level;

pub fn init_logging(level: Level)
```

**Parameters:**

- `level` - Minimum log level to emit

**Example:**

```rust
use automation_common::init_logging;
use tracing::Level;

init_logging(Level::INFO);

info!("This will be logged");
debug!("This won't be logged (below INFO level)");
```

### `init_logging_from_env()`

Initialize logging from the `RUST_LOG` environment variable.

```rust
pub fn init_logging_from_env()
```

**Environment Variable:**

- `RUST_LOG` - Log level filter (error, warn, info, debug, trace)

**Example:**

```bash
# Set log level
export RUST_LOG=debug
cargo run
```

```rust
use automation_common::init_logging_from_env;

init_logging_from_env();

// Logs will be filtered by RUST_LOG environment variable
info!("Info message");
debug!("Debug message (only if RUST_LOG=debug)");
```

### Log Macros

The crate re-exports the standard `tracing` log macros:

```rust
use tracing::{error, warn, info, debug, trace};

error!("Error occurred: {}", err);
warn!("Warning: {}", warning);
info!("Info message");
debug!("Debug information");
trace!("Detailed trace information");
```

### Structured Logging

```rust
use tracing::{info, instrument, error};

#[instrument(skip(config))]
async fn process_task(config: &Config) -> Result<()> {
    info!("Processing task");
    
    // Fields are automatically added to logs
    // You can also add custom fields
    info!(task_count = 10, "Processing multiple tasks");
    
    Ok(())
}

// The #[instrument] macro automatically adds:
// - Function name
// - Function arguments (unless skipped)
// - Timing information
```

## Usage Examples

### Basic Configuration

```rust
use automation_common::Config;

// Load from environment
let config = Config::from_env()?;

println!("Workspace: {:?}", config.workspace);
println!("State: {:?}", config.state);
println!("Immediate: {}", config.immediate);
```

### Builder Pattern

```rust
use automation_common::Config;
use std::time::Duration;

let config = Config::new("/workspace", "/workspace/.state")
    .with_interval(Duration::from_secs(300))
    .with_timeout(Duration::from_secs(1800))
    .with_immediate(true);
```

### Error Handling

```rust
use automation_common::{AutomationError, Result};

async fn load_data() -> Result<String> {
    // Automatic error conversion
    let data = tokio::fs::read_to_string("data.txt").await?;
    
    // Custom error
    if data.is_empty() {
        return Err(AutomationError::Config(
            "Data file is empty".to_string()
        ));
    }
    
    Ok(data)
}

async fn process_data() -> Result<()> {
    let data = load_data().await?;
    info!("Loaded {} bytes", data.len());
    Ok(())
}
```

### Logging

```rust
use automation_common::init_logging;
use tracing::{info, error, instrument};

#[tokio::main]
async fn main() {
    // Initialize logging
    init_logging(tracing::Level::INFO);
    
    run().await
}

#[instrument]
async fn run() {
    info!("Starting application");
    
    match do_something().await {
        Ok(result) => info!("Completed successfully: {:?}", result),
        Err(err) => error!("Failed: {}", err),
    }
}

async fn do_something() -> Result<String, Box<dyn std::error::Error>> {
    Ok("success".to_string())
}
```

### Custom Error Types

```rust
use automation_common::AutomationError;

fn validate_input(input: &str) -> Result<(), AutomationError> {
    if input.is_empty() {
        return Err(AutomationError::Config(
            "Input cannot be empty".to_string()
        ));
    }
    
    if input.len() > 1000 {
        return Err(AutomationError::Config(
            "Input too long (max 1000 characters)".to_string()
        ));
    }
    
    Ok(())
}
```

### Combining with Other Modules

```rust
use automation_common::{Config, Result};
use automation_state::State;
use automation_workspace::WorkspaceManager;

async fn run_agent() -> Result<()> {
    // Load configuration
    let config = Config::from_env()?;
    
    // Initialize other modules with config
    let state = State::load(config.state.join("agent.json")).await?;
    let workspace = WorkspaceManager::new(&config.workspace).await?;
    
    // Use the modules
    // ...
    
    Ok(())
}
```

## See Also

- [Architecture Documentation](../architecture.md) - System architecture overview
- [State API](state.md) - State management module
- [Workspace API](workspace.md) - Workspace management module
- [Executor API](executor.md) - CLI executor module
- [Scheduler API](scheduler.md) - Task scheduler module
- [Agents API](agents.md) - Agent implementations

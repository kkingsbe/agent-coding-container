# automation-state API Reference

The `automation-state` crate provides state management and file-based locking for coordinating multiple agents in the gastown system.

## Table of Contents

- [Overview](#overview)
- [State](#state)
- [Lock](#lock)
- [Cleanup](#cleanup)
- [Persistence](#persistence)
- [Usage Examples](#usage-examples)

## Overview

`automation-state` provides:

- **JSON state persistence** - Save and load agent state
- **File-based exclusive locks** - Prevent concurrent execution of the same agent
- **Stale lock cleanup** - Automatically clean up locks from dead processes
- **Process status detection** - Check if lock-holding processes are still alive

## State

### `State`

Represents the execution state of an agent.

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub agent_type: String,
    pub last_run: String,
    pub status: String,
    pub result: String,
    #[serde(default)]
    pub execution_time_seconds: f64,
    #[serde(default)]
    pub tasks_processed: usize,
    #[serde(default)]
    pub errors: Vec<String>,
}
```

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `agent_type` | `String` | Type of agent (e.g., "architect", "janitor", "prompt") |
| `last_run` | `String` | ISO 8601 timestamp of last execution |
| `status` | `String` | Execution status ("completed", "failed", "in_progress") |
| `result` | `String` | Result of execution ("success", "error", "timeout") |
| `execution_time_seconds` | `f64` | Execution time in seconds |
| `tasks_processed` | `usize` | Number of tasks processed |
| `errors` | `Vec<String>` | List of errors encountered |

### `State::new()`

Create a new state with default values.

```rust
impl State {
    pub fn new(agent_type: impl Into<String>) -> Self
}
```

**Example:**

```rust
use automation_state::State;

let state = State::new("prompt");
```

### `State::load()`

Load state from a JSON file.

```rust
impl State {
    pub async fn load(path: impl AsRef<Path>) -> Result<State, AutomationError>
}
```

**Parameters:**

- `path` - Path to the state JSON file

**Returns:**

- `Result<State, AutomationError>` - Loaded state or error

**Example:**

```rust
use automation_state::State;

let state = State::load("/workspace/.state/prompt.json").await?;
```

### `State::save()`

Save state to a JSON file.

```rust
impl State {
    pub async fn save(&self, path: impl AsRef<Path>) -> Result<(), AutomationError>
}
```

**Parameters:**

- `path` - Path to save the state JSON file

**Returns:**

- `Result<(), AutomationError>` - Success or error

**Example:**

```rust
use automation_state::State;

let mut state = State::new("prompt");
state.last_run = chrono::Utc::now().to_rfc3339();
state.save("/workspace/.state/prompt.json").await?;
```

### `State::from_file()`

Alternative method to load state from file (alias for `load()`).

```rust
impl State {
    pub async fn from_file(path: impl AsRef<Path>) -> Result<State, AutomationError>
}
```

### `State::to_file()`

Alternative method to save state to file (alias for `save()`).

```rust
impl State {
    pub async fn to_file(&self, path: impl AsRef<Path>) -> Result<(), AutomationError>
}
```

## Lock

### `Lock`

Represents an exclusive file lock.

```rust
pub struct Lock {
    pub path: PathBuf,
    pub pid: u32,
    pub host: String,
    pub timestamp: String,
}
```

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `path` | `PathBuf` | Path to the lock file |
| `pid` | `u32` | Process ID holding the lock |
| `host` | `String` | Hostname where the lock was acquired |
| `timestamp` | `String` | ISO 8601 timestamp when lock was acquired |

### `Lock::acquire()`

Acquire an exclusive lock with timeout.

```rust
impl Lock {
    pub async fn acquire(
        lock_file: impl AsRef<Path>,
        timeout: Duration
    ) -> Result<Lock, AutomationError>
}
```

**Parameters:**

- `lock_file` - Path to the lock file
- `timeout` - Maximum time to wait for lock acquisition

**Returns:**

- `Result<Lock, AutomationError>` - Acquired lock or error

**Example:**

```rust
use automation_state::Lock;
use std::time::Duration;

let lock = Lock::acquire(
    "/workspace/.state/locks/prompt.lock",
    Duration::from_secs(30)
).await?;

// Lock is now held
// ...

// Release lock when done
lock.release().await?;
```

### `Lock::try_acquire()`

Try to acquire lock without waiting.

```rust
impl Lock {
    pub async fn try_acquire(
        lock_file: impl AsRef<Path>
    ) -> Result<Option<Lock>, AutomationError>
}
```

**Parameters:**

- `lock_file` - Path to the lock file

**Returns:**

- `Result<Option<Lock>, AutomationError>` - Some(lock) if acquired, None if already held

**Example:**

```rust
use automation_state::Lock;

if let Some(lock) = Lock::try_acquire("/workspace/.state/locks/prompt.lock").await? {
    // Lock acquired
    lock.release().await?;
} else {
    // Lock already held
    println!("Another instance is running");
}
```

### `Lock::release()`

Release the lock.

```rust
impl Lock {
    pub async fn release(self) -> Result<(), AutomationError>
}
```

**Returns:**

- `Result<(), AutomationError>` - Success or error

**Example:**

```rust
let lock = Lock::acquire("/workspace/.state/locks/prompt.lock", Duration::from_secs(30)).await?;
// Do work
lock.release().await?;
```

### `Lock::is_stale()`

Check if the lock is stale (process no longer running).

```rust
impl Lock {
    pub async fn is_stale(&self) -> Result<bool, AutomationError>
}
```

**Returns:**

- `Result<bool, AutomationError>` - true if stale, false if active

**Example:**

```rust
let lock = Lock::acquire("/workspace/.state/locks/prompt.lock", Duration::from_secs(30)).await?;

if lock.is_stale().await? {
    println!("Lock is stale, would be cleaned up");
}
```

### `Lock::read_lock_file()`

Read lock file without acquiring it.

```rust
impl Lock {
    pub async fn read_lock_file(
        lock_file: impl AsRef<Path>
    ) -> Result<Option<Lock>, AutomationError>
}
```

**Returns:**

- `Result<Option<Lock>, AutomationError>` - Lock information if file exists

**Example:**

```rust
if let Some(lock_info) = Lock::read_lock_file("/workspace/.state/locks/prompt.lock").await? {
    println!("Lock held by PID {} on {}", lock_info.pid, lock_info.host);
}
```

## Cleanup

### `cleanup_stale_locks()`

Clean up stale locks in a directory.

```rust
pub async fn cleanup_stale_locks(
    locks_dir: impl AsRef<Path>
) -> Result<usize, AutomationError>
```

**Parameters:**

- `locks_dir` - Directory containing lock files

**Returns:**

- `Result<usize, AutomationError>` - Number of locks cleaned up

**Example:**

```rust
use automation_state::cleanup_stale_locks;

let cleaned = cleanup_stale_locks("/workspace/.state/locks").await?;
println!("Cleaned up {} stale locks", cleaned);
```

### `cleanup_stale_lock()`

Clean up a specific stale lock file.

```rust
pub async fn cleanup_stale_lock(
    lock_file: impl AsRef<Path>
) -> Result<bool, AutomationError>
```

**Parameters:**

- `lock_file` - Path to the lock file

**Returns:**

- `Result<bool, AutomationError>` - true if lock was cleaned up, false if lock is still active

**Example:**

```rust
use automation_state::cleanup_stale_lock;

if cleanup_stale_lock("/workspace/.state/locks/prompt.lock").await? {
    println!("Stale lock cleaned up");
} else {
    println!("Lock is still active");
}
```

### `cleanup_locks_for_agent()`

Clean up locks for a specific agent type.

```rust
pub async fn cleanup_locks_for_agent(
    locks_dir: impl AsRef<Path>,
    agent_type: &str
) -> Result<usize, AutomationError>
```

**Parameters:**

- `locks_dir` - Directory containing lock files
- `agent_type` - Agent type (e.g., "prompt", "janitor", "architect")

**Returns:**

- `Result<usize, AutomationError>` - Number of locks cleaned up

**Example:**

```rust
use automation_state::cleanup_locks_for_agent;

let cleaned = cleanup_locks_for_agent("/workspace/.state/locks", "prompt").await?;
```

## Persistence

### `persist_state()`

Persist state with error handling and retries.

```rust
pub async fn persist_state(
    state: &State,
    path: impl AsRef<Path>,
    retries: usize
) -> Result<(), AutomationError>
```

**Parameters:**

- `state` - State to persist
- `path` - Path to save state
- `retries` - Number of retries on failure

**Returns:**

- `Result<(), AutomationError>` - Success or error

**Example:**

```rust
use automation_state::{persist_state, State};

let state = State::new("prompt");
persist_state(&state, "/workspace/.state/prompt.json", 3).await?;
```

### `load_state_with_default()`

Load state with a default value if file doesn't exist.

```rust
pub async fn load_state_with_default(
    path: impl AsRef<Path>,
    default: State
) -> Result<State, AutomationError>
```

**Parameters:**

- `path` - Path to load state from
- `default` - Default state to use if file doesn't exist

**Returns:**

- `Result<State, AutomationError>` - Loaded or default state

**Example:**

```rust
use automation_state::{load_state_with_default, State};

let state = load_state_with_default(
    "/workspace/.state/prompt.json",
    State::new("prompt")
).await?;
```

## Usage Examples

### Basic State Management

```rust
use automation_state::State;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state_path = Path::new("/workspace/.state/prompt.json");
    
    // Load state (or create new if doesn't exist)
    let mut state = match State::load(state_path).await {
        Ok(s) => s,
        Err(_) => State::new("prompt"),
    };
    
    // Update state
    state.last_run = chrono::Utc::now().to_rfc3339();
    state.status = "completed".to_string();
    state.result = "success".to_string();
    state.tasks_processed = 10;
    
    // Save state
    state.save(state_path).await?;
    
    Ok(())
}
```

### Lock Acquisition with Cleanup

```rust
use automation_state::{Lock, cleanup_stale_locks};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lock_path = "/workspace/.state/locks/prompt.lock";
    
    // Clean up stale locks first
    cleanup_stale_locks("/workspace/.state/locks").await?;
    
    // Acquire lock
    let lock = Lock::acquire(lock_path, Duration::from_secs(30)).await?;
    
    // Do work
    println!("Lock acquired, doing work...");
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Release lock
    lock.release().await?;
    
    Ok(())
}
```

### Using Lock with State

```rust
use automation_state::{Lock, State};
use std::time::Duration;

async fn run_agent() -> Result<(), Box<dyn std::error::Error>> {
    let lock_path = "/workspace/.state/locks/prompt.lock";
    let state_path = "/workspace/.state/prompt.json";
    
    // Acquire lock
    let lock = Lock::acquire(lock_path, Duration::from_secs(30)).await?;
    
    // Load state
    let mut state = State::load(state_path).await?;
    
    // Update state
    state.last_run = chrono::Utc::now().to_rfc3339();
    state.status = "in_progress".to_string();
    state.save(state_path).await?;
    
    // Do work...
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Update final state
    state.status = "completed".to_string();
    state.result = "success".to_string();
    state.save(state_path).await?;
    
    // Release lock
    lock.release().await?;
    
    Ok(())
}
```

### Lock Timeout Handling

```rust
use automation_state::{Lock, AutomationError};
use std::time::Duration;

async fn run_with_timeout() -> Result<(), Box<dyn std::error::Error>> {
    let lock_path = "/workspace/.state/locks/prompt.lock";
    
    match Lock::acquire(lock_path, Duration::from_secs(5)).await {
        Ok(lock) => {
            // Lock acquired, do work
            println!("Lock acquired");
            lock.release().await?;
            Ok(())
        }
        Err(AutomationError::Lock(msg)) => {
            // Lock timeout
            eprintln!("Could not acquire lock: {}", msg);
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}
```

### Checking Lock Status

```rust
use automation_state::Lock;

async fn check_lock_status() -> Result<(), Box<dyn std::error::Error>> {
    let lock_path = "/workspace/.state/locks/prompt.lock";
    
    // Try to read lock without acquiring
    if let Some(lock_info) = Lock::read_lock_file(lock_path).await? {
        println!("Lock held by PID {} on {}", lock_info.pid, lock_info.host);
        println!("Acquired at: {}", lock_info.timestamp);
        
        // Check if stale
        if lock_info.is_stale().await? {
            println!("Lock is stale");
        } else {
            println!("Lock is active");
        }
    } else {
        println!("No lock file exists");
    }
    
    Ok(())
}
```

## Error Handling

### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `AutomationError::Lock("Timeout")` | Lock acquisition timeout | Increase timeout or wait |
| `AutomationError::Lock("Permission denied")` | Insufficient permissions | Check file permissions |
| `AutomationError::State("Invalid JSON")` | Corrupted state file | Restore from backup |
| `AutomationError::Io` | File system errors | Check disk space and permissions |

### Error Handling Example

```rust
use automation_state::{Lock, AutomationError};
use std::time::Duration;

async fn run_with_retry() -> Result<(), Box<dyn std::error::Error>> {
    let lock_path = "/workspace/.state/locks/prompt.lock";
    let mut retries = 3;
    
    loop {
        match Lock::acquire(lock_path, Duration::from_secs(10)).await {
            Ok(lock) => {
                // Success
                lock.release().await?;
                return Ok(());
            }
            Err(AutomationError::Lock(_)) if retries > 0 => {
                // Retry
                retries -= 1;
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Err(e) => {
                // Give up
                return Err(e.into());
            }
        }
    }
}
```

## See Also

- [Architecture Documentation](../architecture.md) - System architecture overview
- [Common API](common.md) - Common types and utilities
- [Workspace API](workspace.md) - Workspace management module
- [Agents API](agents.md) - Agent implementations
- [Migration Guide](../migration.md) - Lock file compatibility notes

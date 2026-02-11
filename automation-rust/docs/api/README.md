# API Documentation

This section provides detailed API documentation for each module in the gastown system.

## Modules

- [`automation-common`](common.md) - Shared types and utilities
- [`automation-state`](state.md) - State management and locking
- [`automation-workspace`](workspace.md) - Workspace and file operations
- [`automation-executor`](executor.md) - CLI process execution
- [`automation-scheduler`](scheduler.md) - Task scheduling
- [`automation-agents`](agents.md) - Agent implementations

## Overview

The gastown system is organized into six main crates, each with a specific responsibility:

```
┌─────────────────────────────────────────────────────────────┐
│                     gastown                         │
├─────────────────────────────────────────────────────────────┤
│  automation-common  (shared types and utilities)          │
│         │                                                   │
│         ├── automation-state (state management)           │
│         ├── automation-workspace (file operations)        │
│         ├── automation-executor (process execution)        │
│         ├── automation-scheduler (task scheduling)        │
│         └── automation-agents (agent binaries)            │
└─────────────────────────────────────────────────────────────┘
```

## Quick Reference

### Common Types

| Type | Module | Description |
|------|--------|-------------|
| `Config` | common | Configuration structure |
| `AutomationError` | common | Error type for the system |
| `Result<T>` | common | Result alias for `std::result::Result<T, AutomationError>` |

### Core Modules

| Module | Crate | Main Purpose |
|--------|-------|--------------|
| [`State`](state.md#state) | automation-state | JSON state persistence |
| [`Lock`](state.md#lock) | automation-state | File-based exclusive locking |
| [`WorkspaceManager`](workspace.md#workspacemanager) | automation-workspace | Workspace directory management |
| [`Executor`](executor.md#executor) | automation-executor | CLI process execution |
| [`Scheduler`](scheduler.md#scheduler) | automation-scheduler | Periodic task scheduling |
| [`Agent`](agents.md#agent-trait) | automation-agents | Agent behavior abstraction |

### Agent Binaries

| Binary | Agent Type | Interval | Purpose |
|--------|-----------|----------|---------|
| `architect` | ArchitectAgent | 40 min | Long-running planning |
| `janitor` | JanitorAgent | 20 min | Cleanup and maintenance |
| `prompt` | PromptAgent | 5 min | Quick task execution |

## Usage Patterns

### Basic Agent Execution

```rust
use automation_agents::{Agent, PromptAgent};
use automation_common::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let agent = PromptAgent::new(config);
    
    // Run the agent once
    agent.run_once().await?;
    
    Ok(())
}
```

### State Management

```rust
use automation_state::{State, Lock};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Acquire lock
    let lock = Lock::acquire("agent.lock").await?;
    
    // Load state
    let mut state = State::load("state.json").await?;
    
    // Modify state
    state.last_run = chrono::Utc::now();
    
    // Save state
    state.save("state.json").await?;
    
    // Release lock
    lock.release().await?;
    
    Ok(())
}
```

### Workspace Operations

```rust
use automation_workspace::WorkspaceManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = WorkspaceManager::new("/workspace").await?;
    
    // Read markdown file
    let content = workspace.read_file("TODO.md").await?;
    
    // Write file atomically
    workspace.write_file("TODO.md", content).await?;
    
    // Extract TODO section
    let todo_section = workspace.extract_section(&content, "TODO")?;
    
    Ok(())
}
```

### CLI Execution

```rust
use automation_executor::Executor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executor = Executor::new();
    
    // Execute command
    let output = executor
        .execute(vec!["echo", "Hello, World!"])
        .await?;
    
    println!("Exit code: {:?}", output.exit_code);
    println!("Stdout: {}", String::from_utf8_lossy(&output.stdout));
    
    Ok(())
}
```

### Task Scheduling

```rust
use automation_scheduler::Scheduler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scheduler = Scheduler::new(tokio::time::Duration::from_secs(300));
    
    // Run task periodically
    scheduler.run(|| async {
        // Your task here
        println!("Task executed");
        Ok::<(), Box<dyn std::error::Error>>(())
    }).await?;
    
    Ok(())
}
```

## Error Handling

All modules use a consistent error handling pattern with `Result<T, AutomationError>`:

```rust
use automation_common::{AutomationError, Result};

async fn example() -> Result<()> {
    // Operations that can fail return Result
    let state = State::load("state.json").await?;
    
    // The ? operator propagates errors
    let workspace = WorkspaceManager::new("/workspace").await?;
    
    // Return Ok on success
    Ok(())
}
```

## Logging

All modules use the `tracing` crate for structured logging:

```rust
use tracing::{info, warn, error, debug};

async fn example() {
    info!("Starting operation");
    debug!("Debug information");
    warn!("Warning condition");
    error!("Error occurred");
}
```

Set log level using the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug cargo run
```

## Async/Await

All I/O operations are async using Tokio:

```rust
async fn example() -> Result<()> {
    // Use .await for async operations
    let result = async_operation().await?;
    
    // Use tokio::join! for concurrent operations
    let (a, b) = tokio::join!(
        async_operation_a(),
        async_operation_b()
    );
    
    Ok(())
}
```

## Thread Safety

The system uses Tokio's async runtime for concurrency:

- No `std::thread::spawn` - use Tokio tasks
- Use `tokio::sync::Mutex` for async mutexes
- Use channels (`tokio::sync::mpsc`) for communication

## Configuration

Configuration is loaded from environment variables:

```rust
use automation_common::Config;

let config = Config::from_env()?;
println!("Workspace: {}", config.workspace);
println!("State: {}", config.state);
```

Key environment variables:

- `AUTOMATION_WORKSPACE` - Workspace directory path
- `AUTOMATION_STATE` - State directory path
- `AUTOMATION_INTERVAL` - Execution interval in seconds
- `AUTOMATION_TIMEOUT` - Execution timeout in seconds
- `AUTOMATION_IMMEDIATE` - Run immediately on start
- `RUST_LOG` - Log level

## Testing

All modules include comprehensive tests:

```bash
# Run all tests
cargo test

# Run tests for specific module
cargo test -p automation-state

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_lock_acquisition
```

## See Also

- [Architecture Documentation](../architecture.md) - System architecture and design decisions
- [Deployment Guide](../deployment.md) - Deployment instructions
- [Migration Guide](../migration.md) - Migrating from Node.js to Rust
- [Troubleshooting Guide](../troubleshooting.md) - Common issues and solutions

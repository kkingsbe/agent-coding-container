# Rust Rewrite Plan for @automation-parallel

**Document Version:** 1.0  
**Date:** 2025-02-07  
**Author:** Architect Mode  
**Status:** Planning Phase

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Analysis of Current System](#analysis-of-current-system)
3. [Rust Architecture Design](#rust-architecture-design)
4. [Technology Stack Recommendations](#technology-stack-recommendations)
5. [Migration Strategy](#migration-strategy)
6. [Performance Considerations](#performance-considerations)
7. [Implementation Roadmap](#implementation-roadmap)
8. [Testing Strategy](#testing-strategy)
9. [Documentation Requirements](#documentation-requirements)
10. [Risk Assessment and Mitigation](#risk-assessment-and-mitigation)

---

## Executive Summary

The @automation-parallel system is a Node.js-based parallel execution framework for AI agents that coordinates three independent agent types (prompt, janitor, architect) through shared workspace and state directories. This document provides a comprehensive plan for rewriting the entire system in Rust, leveraging Rust's performance, safety, and concurrency features.

### Key Objectives

1. **Performance Enhancement:** Utilize Rust's zero-cost abstractions and memory safety for improved throughput and reduced resource consumption
2. **Reliability:** Eliminate common Node.js runtime errors through Rust's strong type system and ownership model
3. **Maintainability:** Establish a modular, well-tested codebase with clear separation of concerns
4. **Feature Parity:** Maintain 100% functional compatibility with the existing Node.js implementation

### Scope

- Complete rewrite of all core modules: scheduler, state-manager, workspace-manager, cli-executor
- Rewriting all entry points: run-prompt, run-janitor, run-architect
- Maintaining Docker deployment compatibility
- Preserving configuration and environment variable interfaces

---

## Analysis of Current System

### System Overview

The automation-parallel system operates as a multi-container architecture where each agent type runs independently on a scheduled interval:

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Shared Workspace                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐           │
│  │ TODO.md     │  │ BACKLOG.md  │  │ COMPLETED.md│           │
│  │ BLOCKERS.md │  │ PRD.md      │  │ .state/     │           │
│  └─────────────┘  └─────────────┘  └─────────────┘           │
└─────────────────────────────────────────────────────────────────────┘
           │                        │                        │
           ▼                        ▼                        ▼
    ┌─────────────┐      ┌─────────────┐      ┌─────────────┐
    │   prompt    │      │   janitor   │      │  architect  │
    │ (5 min)     │      │ (20 min)    │      │  (40 min)   │
    └─────────────┘      └─────────────┘      └─────────────┘
```

### Core Components Analysis

#### 1. Scheduler Module ([`lib/scheduler.js`](../lib/scheduler.js:1))

**Purpose:** Manages recurring task execution with retry logic and graceful shutdown

**Key Features:**
- `ScheduledTask` class with configurable intervals, immediate execution, and max retries
- Exponential backoff retry strategy (100ms * 2^attempt)
- Graceful shutdown support with timeout handling
- Statistics tracking (success/failure counts)

**Key Methods:**
- `start()` - Begin scheduling
- `stop()` - Stop scheduling
- `_scheduleNextRun()` - Recurring execution via setTimeout
- `_executeWithRetry()` - Retry logic with exponential backoff
- `createScheduledTask()` - Factory function for task creation
- `gracefulShutdown()` - Clean shutdown for multiple tasks

**JavaScript Limitations:**
- Single-threaded event loop limits parallelism
- setTimeout delays are not precise
- Error handling relies on callbacks/promises

**Rust Advantages:**
- Tokio provides precise async scheduling
- `tokio::time::interval` for accurate timing
- Type-safe error handling with `Result<T, E>`

#### 2. State Manager Module ([`lib/state-manager.js`](../lib/state-manager.js:1))

**Purpose:** Manages state file operations and file-based locking for container coordination

**Key Features:**
- JSON state persistence with backward compatibility
- File-based exclusive locks with timeout
- Lock cleanup for stale locks (dead processes, old locks, cross-host scenarios)
- State tracking: lastRun, lastSuccess, lastFailure, errorCount, consecutiveFailures, status
- Process status detection to identify dead processes

**Key Methods:**
- `getStateFilePath()` / `getLockFilePath()` - Path generation
- `readState()` / `writeState()` / `updateState()` - State persistence
- `resetState()` - State initialization
- `recordEarlyTermination()` / `recordSuccess()` - Event recording
- `deleteState()` - State cleanup
- `acquireLock()` / `releaseLock()` / `isLocked()` - Lock management
- `acquireLockWithRetry()` - Retry-based lock acquisition
- `cleanupStaleLocks()` - Orphaned lock cleanup
- `cleanupAllStates()` - Bulk state cleanup
- `_isProcessRunning()` - Process status detection (Windows/Unix)
- `_getLockCleanupMaxAge()` - Environment-based cleanup threshold

**JavaScript Limitations:**
- File I/O operations block the event loop
- No atomic file operations (though temp file + rename is used)
- Process checking uses platform-specific commands

**Rust Advantages:**
- `std::fs` for synchronous I/O and `tokio::fs` for async I/O
- `fs2` crate for file locking with proper atomic operations
- Cross-platform process detection via `sysinfo` crate
- Strongly typed state structures

#### 3. Workspace Manager Module ([`lib/workspace-manager.js`](../lib/workspace-manager.js:1))

**Purpose:** Handles workspace file operations for markdown files (TODO.md, BACKLOG.md, COMPLETED.md, BLOCKERS.md, PRD.md)

**Key Features:**
- Atomic write operations using temp files + rename
- Directory existence checking and creation
- Markdown section parsing by `##` headings
- Context extraction for template variables

**Key Methods:**
- `getWorkspacePath()` - Workspace directory getter
- `fileExists()` - File existence check
- `ensureDirectory()` - Directory creation
- `readMarkdownFile()` - Generic markdown reader
- `readTodoFile()` / `readBacklogFile()` / etc. - Specific file readers
- `writeMarkdownFile()` - Atomic write with temp file
- `parseMarkdownSections()` - Section extraction
- `extractSection()` - Named section retrieval

**JavaScript Limitations:**
- String manipulation for markdown parsing is error-prone
- No built-in markdown AST parsing

**Rust Advantages:**
- `pulldown-cmark` crate for proper markdown parsing
- `tempfile` crate for safe temporary file handling
- Strong typing for file paths via `std::path::Path`

#### 4. CLI Executor Module ([`lib/cli-executor.js`](../lib/cli-executor.js:1))

**Purpose:** Executes Kilo Code CLI with output monitoring and early termination based on error patterns

**Key Features:**
- Process spawning with stdin/stdout/stderr pipes
- Real-time output monitoring with callbacks
- Pattern-based early termination (Mistake Limit detection)
- Output buffering with configurable max size (10MB default)
- Graceful + forceful termination strategy
- Timeout enforcement

**Key Classes:**
- `OutputBuffer` - Bounded output accumulation (10MB max)
- `ProcessState` enum - Process lifecycle tracking
- `CLIExecutor` - Main execution orchestration

**Key Methods:**
- `execute()` - Main execution method with options
- `_detectPattern()` - Regex-based error pattern detection
- `_terminateProcess()` - Two-phase termination (SIGTERM then SIGKILL)

**Error Patterns Detected:**
- `MISTAKE_LIMIT` patterns: Multiple regex variations for "Mistake Limit Reached"
- `MISTAKE_LIMIT_MULTILINE`: Pattern with context "This may indicate a failure in the model"

**JavaScript Limitations:**
- `spawn` with `shell: true` is less efficient
- Pattern matching on full output can be slow
- No proper process resource limits

**Rust Advantages:**
- `tokio::process::Command` for efficient process spawning
- `regex` crate for compiled pattern matching
- Fine-grained process control via `tokio::process::Child`
- Resource limits via cgroups on Linux

#### 5. Entry Points ([`run-prompt.js`](../run-prompt.js:1), [`run-janitor.js`](../run-janitor.js:1), [`run-architect.js`](../run-architect.js:1))

**Purpose:** Main entry points for each agent type

**Common Pattern:**
1. Parse CLI arguments (`--workspace`, `--state`, `--interval`, `--timeout`, `--immediate`)
2. Initialize managers (WorkspaceManager, StateManager)
3. Perform startup lock cleanup
4. Create scheduled task with execution handler
5. Set up signal handlers (SIGTERM, SIGINT, uncaughtException, unhandledRejection)
6. Start task and log

**Handler Pattern:**
1. Acquire lock with retry
2. Update state (lastRun, status: 'running')
3. Read workspace files in parallel
4. Execute prompt template with context substitution
5. Execute Kilo Code CLI with prompt
6. Handle early termination / success / failure
7. Write output file
8. Release lock
9. Update state

**JavaScript Limitations:**
- Manual argument parsing is error-prone
- No type safety for configuration
- Signal handling differences across platforms

**Rust Advantages:**
- `clap` crate for declarative CLI argument parsing
- `serde` for configuration deserialization
- `tokio::signal` for cross-platform signal handling

### Configuration Files

#### package.json
- Defines scripts for Docker operations
- Specifies Node.js >= 18.0.0 requirement
- Lists files for packaging

#### .env.example
- Environment variables for all agents
- Configuration for intervals, timeouts, immediate execution
- Docker container prefix for multi-workspace support

#### Dockerfile
- Based on `node:22-slim`
- Installs Rust (for potential future use)
- Installs `@kilocode/cli@0.26.0`
- Creates non-root user for security

#### docker-compose.yml
- Three services: agent_prompt, agent_janitor, agent_architect
- Shared volumes: workspace, workspace-state, logs
- Health checks and logging configuration

---

## Rust Architecture Design

### Project Structure

```
automation-parallel-rust/
├── Cargo.toml                          # Workspace manifest
├── Cargo.lock
├── Dockerfile                           # Rust-based Docker image
├── docker-compose.yml                    # Compose configuration
├── .env.example                        # Environment template
├── README.md                           # Project documentation
├── bin/                               # Binary entry points
│   ├── agent-prompt.rs                 # Prompt agent binary
│   ├── agent-janitor.rs                # Janitor agent binary
│   └── agent-architect.rs             # Architect agent binary
├── src/
│   ├── lib.rs                         # Library root (re-export modules)
│   ├── scheduler/                      # Scheduling module
│   │   ├── mod.rs
│   │   ├── task.rs                    # ScheduledTask implementation
│   │   ├── retry.rs                   # Retry strategies
│   │   └── shutdown.rs                # Graceful shutdown
│   ├── state/                         # State management module
│   │   ├── mod.rs
│   │   ├── manager.rs                 # StateManager implementation
│   │   ├── lock.rs                   # File-based locking
│   │   ├── cleanup.rs                 # Stale lock cleanup
│   │   └── models.rs                 # State data structures
│   ├── workspace/                     # Workspace module
│   │   ├── mod.rs
│   │   ├── manager.rs                # WorkspaceManager implementation
│   │   ├── files.rs                 # Workspace file handlers
│   │   └── markdown.rs              # Markdown parsing utilities
│   ├── executor/                     # CLI execution module
│   │   ├── mod.rs
│   │   ├── cli.rs                   # CLIExecutor implementation
│   │   ├── buffer.rs                # Output buffer
│   │   ├── patterns.rs               # Error pattern detection
│   │   └── process.rs               # Process management
│   ├── prompts/                      # Prompt template module
│   │   ├── mod.rs
│   │   ├── template.rs              # Template engine
│   │   └── context.rs               # Context building
│   └── common/                      # Common utilities
│       ├── mod.rs
│       ├── config.rs                 # Configuration structures
│       ├── error.rs                  # Error types
│       ├── logging.rs                # Logging setup
│       └── cli.rs                   # Shared CLI argument parsing
├── prompts/                          # Prompt templates (unchanged)
│   └── development/
│       ├── PROMPT.md
│       ├── JANITOR.md
│       └── ARCHITECT.md
└── tests/                           # Integration tests
    ├── integration/
    │   ├── scheduler_test.rs
    │   ├── state_test.rs
    │   └── executor_test.rs
    └── common/
        └── fixtures/
```

### Cargo.toml Workspace Configuration

```toml
[workspace]
members = ["src/*"]
resolver = "2"

[workspace.package]
version = "1.0.0"
edition = "2021"
rust-version = "1.75"
license = "ISC"
authors = ["Automation System"]

[workspace.dependencies]
# Async runtime
tokio = { version = "1.35", features = ["full"] }
tokio-util = "0.7"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# CLI
clap = { version = "4.4", features = ["derive", "env"] }

# Filesystem
fs2 = "0.4"
tempfile = "3.8"
walkdir = "2.4"

# Process management
sysinfo = "0.30"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-appender = "0.2"

# Error handling
anyhow = "1.0"
thiserror = "1.0"

# Markdown parsing
pulldown-cmark = { version = "0.10", features = ["html"] }

# Regular expressions
regex = "1.10"

# Date/time
chrono = { version = "0.4", features = ["serde"] }

# Testing
proptest = "1.4"
```

### Core Data Structures

#### State Models (state/models.rs)

```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Represents the execution state of a task
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Idle,
    Running,
    Success,
    Failed,
}

/// Task state persisted to JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskState {
    /// Timestamp of last run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run: Option<DateTime<Utc>>,
    
    /// Timestamp of last successful execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_success: Option<DateTime<Utc>>,
    
    /// Timestamp of last failure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure: Option<DateTime<Utc>>,
    
    /// Total error count
    #[serde(default)]
    pub error_count: u32,
    
    /// Count of consecutive failures
    #[serde(default)]
    pub consecutive_failures: u32,
    
    /// Current status
    #[serde(default)]
    pub status: TaskStatus,
    
    /// Reason for last early termination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_termination_reason: Option<String>,
    
    /// Count of early terminations
    #[serde(default)]
    pub early_termination_count: u32,
    
    /// Total execution time in milliseconds
    #[serde(default)]
    pub total_execution_time_ms: u64,
    
    /// Average execution time in milliseconds
    #[serde(default)]
    pub average_execution_time_ms: u64,
    
    /// Count of successful terminations
    #[serde(default)]
    pub successful_terminations: u32,
    
    /// Count of failed terminations
    #[serde(default)]
    pub failed_terminations: u32,
}

impl Default for TaskState {
    fn default() -> Self {
        Self {
            last_run: None,
            last_success: None,
            last_failure: None,
            error_count: 0,
            consecutive_failures: 0,
            status: TaskStatus::Idle,
            last_termination_reason: None,
            early_termination_count: 0,
            total_execution_time_ms: 0,
            average_execution_time_ms: 0,
            successful_terminations: 0,
            failed_terminations: 0,
        }
    }
}

/// Lock file metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockData {
    /// Process ID that acquired the lock
    pub pid: u32,
    
    /// Lock acquisition timestamp
    pub timestamp: i64,
    
    /// Hostname where lock was acquired
    pub host: String,
}
```

#### Scheduler Types (scheduler/task.rs)

```rust
use std::time::Duration;
use tokio::time::Interval;

/// Configuration for a scheduled task
#[derive(Debug, Clone)]
pub struct TaskConfig {
    /// Name of the task for logging
    pub task_name: String,
    
    /// Interval between executions
    pub interval: Duration,
    
    /// Execute immediately on start
    pub immediate: bool,
    
    /// Maximum number of retries
    pub max_retries: u32,
    
    /// Retry delay base for exponential backoff
    pub retry_delay_base: Duration,
}

impl Default for TaskConfig {
    fn default() -> Self {
        Self {
            task_name: String::from("unknown"),
            interval: Duration::from_secs(300), // 5 minutes
            immediate: false,
            max_retries: 3,
            retry_delay_base: Duration::from_millis(100),
        }
    }
}

/// Task execution statistics
#[derive(Debug, Clone, Default)]
pub struct TaskStats {
    /// Number of successful executions
    pub success_count: u32,
    
    /// Number of failed executions
    pub failure_count: u32,
    
    /// Timestamp of last run
    pub last_run_timestamp: Option<DateTime<Utc>>,
}

/// Result of task execution
#[derive(Debug)]
pub enum ExecutionResult {
    Success { duration: Duration },
    Failure { error: anyhow::Error },
    EarlyTermination { reason: String, duration: Duration },
}
```

#### CLI Executor Types (executor/mod.rs)

```rust
use std::time::Duration;

/// Configuration for CLI execution
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

/// Process execution result
#[derive(Debug, Clone)]
pub struct ProcessResult {
    /// Exit code (None if terminated by signal)
    pub exit_code: Option<i32>,
    
    /// Signal that terminated the process (if applicable)
    pub signal: Option<String>,
    
    /// Combined stdout output
    pub stdout: String,
    
    /// Combined stderr output
    pub stderr: String,
    
    /// Whether process was terminated early
    pub terminated_early: bool,
    
    /// Reason for early termination
    pub termination_reason: Option<String>,
    
    /// Execution duration
    pub execution_time_ms: u64,
    
    /// Error if process failed to start
    pub error: Option<String>,
}

impl Default for ProcessResult {
    fn default() -> Self {
        Self {
            exit_code: None,
            signal: None,
            stdout: String::new(),
            stderr: String::new(),
            terminated_early: false,
            termination_reason: None,
            execution_time_ms: 0,
            error: None,
        }
    }
}
```

### Error Handling Strategy

#### Error Types (common/error.rs)

```rust
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
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, AutomationError>;
```

### Concurrency Model

The Rust implementation will use **Tokio** as the async runtime with the following concurrency patterns:

1. **Async/Await:** All I/O operations (file system, process spawning, network) will use async/await
2. **Task Spawning:** Independent agents run as separate Tokio tasks
3. **Select for Timeouts:** Use `tokio::select!` for managing timeouts alongside async operations
4. **Mutex for Shared State:** When multiple tasks need access to shared state, use `tokio::sync::Mutex`

#### Example: Scheduled Task Loop

```rust
use tokio::time::{interval, Duration};

async fn scheduled_task_loop<F, Fut>(handler: F, config: TaskConfig) -> anyhow::Result<()>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
{
    let mut interval = interval(config.interval);
    interval.tick().await; // First tick completes immediately
    
    if config.immediate {
        handler().await?;
    }
    
    loop {
        interval.tick().await;
        handler().await?;
    }
}
```

---

## Technology Stack Recommendations

### Core Runtime

| Component | Recommendation | Rationale |
|-----------|----------------|------------|
| **Async Runtime** | `tokio` 1.35+ | Mature async runtime with extensive ecosystem, supports Windows/Linux/macOS, excellent for I/O-bound workloads |
| **Runtime Features** | `full` feature flag | Includes all needed features: fs, process, signal, time, macros |

### CLI Argument Parsing

| Crate | Version | Rationale |
|--------|----------|-----------|
| **clap** | 4.4+ | Declarative argument parsing, derives from struct, excellent help generation, subcommand support |

```rust
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "agent-prompt")]
#[command(about = "Prompt agent for automation-parallel", long_about = None)]
struct Args {
    /// Workspace directory path
    #[arg(long, default_value = "/workspace")]
    workspace: PathBuf,
    
    /// State directory path
    #[arg(long, default_value = "/workspace/.state")]
    state: PathBuf,
    
    /// Schedule interval in milliseconds
    #[arg(long, default_value = "300000")]
    interval: u64,
    
    /// Execution timeout in milliseconds
    #[arg(long, default_value = "900000")]
    timeout: u64,
    
    /// Execute immediately on startup
    #[arg(long)]
    immediate: bool,
}
```

### Serialization

| Crate | Version | Rationale |
|--------|----------|-----------|
| **serde** | 1.0+ | De/serialization framework, derive macros for structs |
| **serde_json** | 1.0+ | JSON support for state files |

### File System Operations

| Crate | Version | Rationale |
|--------|----------|-----------|
| **fs2** | 0.4+ | File locking with atomic operations, cross-platform |
| **tempfile** | 3.8+ | Safe temporary file handling for atomic writes |
| **walkdir** | 2.4+ | Efficient directory traversal for cleanup operations |

### Process Management

| Crate | Version | Rationale |
|--------|----------|-----------|
| **sysinfo** | 0.30+ | Cross-platform process detection, works on Windows/Linux/macOS |
| **tokio-process** | (via tokio) | Async process spawning and management |

### Logging

| Crate | Version | Rationale |
|--------|----------|-----------|
| **tracing** | 0.1+ | Structured logging, async-aware, span support |
| **tracing-subscriber** | 0.3+ | Configurable log layers (fmt, json, env-filter) |
| **tracing-appender** | 0.2+ | File rotation support |

```rust
use tracing::{info, warn, error, instrument};
use tracing_subscriber::{EnvFilter, fmt};

fn init_logging(level: &str) -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));
    
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_max_level(tracing::Level::TRACE)
        .init();
    
    Ok(())
}
```

### Error Handling

| Crate | Version | Rationale |
|--------|----------|-----------|
| **thiserror** | 1.0+ | Derive error types, automatic Display implementation |
| **anyhow** | 1.0+ | Error context, anyhow::Result for error propagation |

### Regular Expressions

| Crate | Version | Rationale |
|--------|----------|-----------|
| **regex** | 1.10+ | Fast regex engine, Unicode support, compile-time optimization |

### Markdown Parsing

| Crate | Version | Rationale |
|--------|----------|-----------|
| **pulldown-cmark** | 0.10+ | CommonMark compliant, event-driven parser, efficient |

### Date/Time

| Crate | Version | Rationale |
|--------|----------|-----------|
| **chrono** | 0.4+ | Date/time manipulation, ISO 8601 parsing, serde integration |

### Testing

| Crate | Version | Rationale |
|--------|----------|-----------|
| **proptest** | 1.4+ | Property-based testing for robust testing |
| **tokio-test** | 0.4+ | Async testing utilities |

### Configuration Management

Use environment variables with `clap`'s env feature for configuration:

```rust
#[derive(Parser, Debug, Deserialize)]
#[command(env = "AUTOMATION_ENV")]
struct Config {
    #[arg(long, env = "WORKSPACE_PATH")]
    workspace: PathBuf,
    
    #[arg(long, env = "STATE_PATH")]
    state: PathBuf,
    
    #[arg(long, env = "SCHEDULE_INTERVAL_MS")]
    interval: u64,
}
```

---

## Migration Strategy

### Approach Selection

**Recommended: Complete Rewrite**

Given the fundamental architectural differences between Node.js and Rust (event loop vs tokio async runtime), a complete rewrite is more efficient than incremental migration. The benefits include:

1. Clean architecture designed for Rust's ownership model
2. No compatibility layers between JavaScript and Rust
3. Ability to leverage Rust-specific optimizations
4. Simpler deployment (single language stack)

**Alternative Considered: Hybrid Approach**

Running Node.js agents alongside Rust components was considered but rejected due to:
- Increased complexity in coordination
- No clear migration path
- Maintenance burden of two languages

### Migration Phases

#### Phase 1: Foundation (Weeks 1-2)

**Objective:** Set up project infrastructure and implement core utilities

**Deliverables:**
- [ ] Cargo workspace with crate structure
- [ ] Common error types and Result aliases
- [ ] Logging infrastructure
- [ ] Configuration structures and parsing
- [ ] Basic CLI argument parsing

**Validation:**
- Unit tests for error types
- Configuration loading tests
- Logging output verification

#### Phase 2: State Management (Weeks 3-4)

**Objective:** Implement file-based state management and locking

**Deliverables:**
- [ ] StateManager with read/write/update operations
- [ ] File lock implementation with fs2
- [ ] Lock cleanup for stale locks
- [ ] Process detection via sysinfo
- [ ] State persistence and backward compatibility

**Validation:**
- Integration tests for lock acquisition/release
- Tests for stale lock cleanup scenarios
- Cross-platform compatibility tests (Windows/Linux)

#### Phase 3: Workspace Management (Week 5)

**Objective:** Implement workspace file operations and markdown parsing

**Deliverables:**
- [ ] WorkspaceManager for file I/O
- [ ] Atomic file write operations
- [ ] Markdown section parsing with pulldown-cmark
- [ ] Context extraction for templates

**Validation:**
- Tests for file existence checks
- Atomic write tests (verify temp file + rename)
- Markdown parsing accuracy tests

#### Phase 4: CLI Executor (Weeks 6-7)

**Objective:** Implement Kilo Code CLI execution with monitoring

**Deliverables:**
- [ ] Process spawning with tokio
- [ ] Output buffering and streaming
- [ ] Error pattern detection with regex
- [ ] Graceful and forceful termination
- [ ] Timeout enforcement

**Validation:**
- Process execution tests with successful/failed runs
- Pattern detection tests
- Termination behavior tests
- Timeout handling tests

#### Phase 5: Scheduler (Week 8)

**Objective:** Implement task scheduling with retry logic

**Deliverables:**
- [ ] ScheduledTask with interval-based execution
- [ ] Exponential backoff retry strategy
- [ ] Graceful shutdown support
- [ ] Statistics tracking

**Validation:**
- Timing accuracy tests
- Retry logic tests
- Shutdown behavior tests

#### Phase 6: Agent Entry Points (Weeks 9-10)

**Objective:** Implement agent binaries (prompt, janitor, architect)

**Deliverables:**
- [ ] Prompt agent binary
- [ ] Janitor agent binary
- [ ] Architect agent binary
- [ ] Template engine for prompt substitution
- [ ] Signal handling (SIGTERM, SIGINT)

**Validation:**
- End-to-end tests for each agent
- Signal handling tests
- Integration with StateManager and WorkspaceManager

#### Phase 7: Docker Integration (Week 11)

**Objective:** Update Docker configuration for Rust deployment

**Deliverables:**
- [ ] Rust-based Dockerfile (using multi-stage build)
- [ ] Updated docker-compose.yml
- [ ] Environment variable compatibility
- [ ] Build and deployment scripts

**Validation:**
- Successful container build
- Container startup and execution
- Health check passing

#### Phase 8: Testing and Documentation (Weeks 12-13)

**Objective:** Comprehensive testing and documentation

**Deliverables:**
- [ ] Full integration test suite
- [ ] Property-based tests for core functions
- [ ] User-facing documentation
- [ ] Developer documentation
- [ ] Migration guide for existing deployments

**Validation:**
- Test coverage > 80%
- Documentation review
- Migration test (from Node.js to Rust)

### Rollback Strategy

During migration and testing:

1. **Keep Node.js version:** Maintain existing Node.js code in a separate branch
2. **Docker tag versioning:** Use version tags for container images (`v1.0.0-node`, `v1.0.0-rust`)
3. **Feature flags:** Add environment variable to switch between implementations during testing
4. **Blue-green deployment:** Deploy Rust version alongside Node.js, gradually switch traffic

Rollback triggers:
- Critical bugs discovered in Rust version
- Performance regression
- Incompatibility with existing workflows

### Interoperability During Migration

If a gradual migration is needed, implement a compatibility layer:

1. **Shared state format:** Ensure state files remain compatible (JSON format)
2. **Lock file compatibility:** Keep lock file format identical
3. **Output file format:** Maintain identical output file naming

---

## Performance Considerations

### Expected Performance Improvements

#### Memory Efficiency

| Aspect | Node.js | Rust | Expected Improvement |
|--------|----------|-------|-------------------|
| Base memory footprint | ~50-100 MB per container | ~10-30 MB per container | 60-80% reduction |
| Memory leaks | Possible due to event loop quirks | Prevented by ownership model | Eliminated |
| Garbage collection pauses | Frequent pause times | Zero-copy, deterministic | Eliminated |

#### Execution Speed

| Aspect | Node.js | Rust | Expected Improvement |
|--------|----------|-------|-------------------|
| File I/O | Async, event loop | Async with tokio (more efficient) | 10-20% faster |
| Pattern matching | Regex on strings | Compiled regex, zero-copy | 20-40% faster |
| Process spawning | Child process with shell | Direct tokio process | 5-15% faster |
| JSON parsing | v8 JSON parser | serde (Rust-native) | 2-3x faster |

#### Concurrency

| Aspect | Node.js | Rust | Expected Improvement |
|--------|----------|-------|-------------------|
| Parallelism | Single-threaded event loop | Multi-threaded tokio | True parallelism |
| CPU utilization | Limited to single core | Utilizes all cores | 2-4x throughput |

### Identified Bottlenecks in Current Implementation

1. **Event Loop Blocking:** File operations in state-manager.js can block the event loop
2. **Synchronous Lock Acquisition:** Lock acquisition with retry uses blocking `setTimeout`
3. **String Concatenation:** Output buffering uses string concatenation, inefficient for large buffers
4. **Pattern Matching:** Error patterns are tested sequentially on full output

### Rust Solutions to Bottlenecks

1. **Async I/O:** All file operations use `tokio::fs` for non-blocking I/O
2. **Async Lock Retry:** Use `tokio::time::sleep` and `tokio::select!` for async retry
3. **Vec<u8> Buffering:** Use bytes buffers with capacity management
4. **Compiled Regex:** Pre-compile all regex patterns, test in parallel

### Memory Management

#### Zero-Copy Strategies

```rust
// Use bytes instead of strings for output buffering
pub struct OutputBuffer {
    buffer: Vec<u8>,
    max_size: usize,
}

impl OutputBuffer {
    pub fn add(&mut self, chunk: &[u8]) -> bool {
        // Check capacity without allocation
        if self.buffer.len() + chunk.len() > self.max_size {
            // Truncate efficiently without copy
            let keep_size = self.max_size - chunk.len();
            if keep_size > 0 {
                let remaining = self.buffer.split_off(self.buffer.len() - keep_size);
                std::mem::replace(&mut self.buffer, remaining);
            }
            return true;
        }
        
        // Extend without copy
        self.buffer.extend_from_slice(chunk);
        false
    }
}
```

#### Stack Allocation for Small Data

```rust
// Use SmallVec for lock file paths (typically short)
use smallvec::SmallVec;

pub fn get_lock_file_path(task_name: &str) -> SmallVec<[u8; 64]> {
    let mut path = SmallVec::new();
    path.extend_from_slice(b"/workspace/.state/");
    path.extend_from_slice(task_name.as_bytes());
    path.extend_from_slice(b".lock");
    path
}
```

### Resource Limits

Implement resource limits via configuration:

```rust
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum memory usage per agent (MB)
    pub max_memory_mb: usize,
    
    /// Maximum CPU time per execution (seconds)
    pub max_cpu_time: u64,
    
    /// Maximum file size for output buffering (MB)
    pub max_file_size_mb: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 512,
            max_cpu_time: 1800, // 30 minutes
            max_file_size_mb: 100,
        }
    }
}
```

### Performance Monitoring

Add built-in performance metrics:

```rust
#[derive(Debug, Default)]
pub struct PerformanceMetrics {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub total_execution_time_ms: u64,
    pub max_execution_time_ms: u64,
    pub min_execution_time_ms: u64,
    pub avg_execution_time_ms: u64,
    pub memory_peak_mb: usize,
    pub lock_contention_count: u64,
}

impl PerformanceMetrics {
    pub fn record_execution(&mut self, duration_ms: u64, success: bool) {
        self.total_executions += 1;
        self.total_execution_time_ms += duration_ms;
        
        if success {
            self.successful_executions += 1;
        } else {
            self.failed_executions += 1;
        }
        
        self.max_execution_time_ms = self.max_execution_time_ms.max(duration_ms);
        self.min_execution_time_ms = if self.min_execution_time_ms == 0 {
            duration_ms
        } else {
            self.min_execution_time_ms.min(duration_ms)
        };
        
        self.avg_execution_time_ms = self.total_execution_time_ms / self.total_executions;
    }
}
```

---

## Implementation Roadmap

### Phase 1: Foundation (Weeks 1-2)

**Sprint 1.1: Project Setup (Week 1)**

| Task | Owner | Days | Dependencies |
|-------|--------|-------|--------------|
| Create Cargo workspace structure | TBD | 1 | - |
| Define core error types | TBD | 1 | Workspace |
| Set up logging infrastructure | TBD | 1 | Error types |
| Create configuration structures | TBD | 1 | Error types |
| Implement CLI argument parsing | TBD | 2 | Configuration |
| Write unit tests for foundation | TBD | 1 | All above |

**Deliverables:**
- Cargo workspace with 3 crates (lib, bin-architect, bin-janitor, bin-prompt)
- `common/error.rs` with AutomationError enum
- `common/logging.rs` with tracing setup
- `common/config.rs` with Config struct
- `common/cli.rs` with clap derives

**Acceptance Criteria:**
- `cargo build` succeeds without errors
- Unit tests pass (`cargo test`)
- Logging produces structured output
- CLI arguments parse correctly

**Sprint 1.2: Foundation Tests (Week 2)**

| Task | Owner | Days | Dependencies |
|-------|--------|-------|--------------|
| Write property-based tests for config | TBD | 2 | Config struct |
| Test error propagation chains | TBD | 1 | Error types |
| Validate CLI argument parsing | TBD | 1 | CLI module |
| Document foundation APIs | TBD | 2 | All above |

**Deliverables:**
- Property-based tests using proptest
- Error handling examples
- API documentation with rustdoc

### Phase 2: State Management (Weeks 3-4)

**Sprint 2.1: State Persistence (Week 3)**

| Task | Owner | Days | Dependencies |
|-------|--------|-------|--------------|
| Implement StateManager struct | TBD | 2 | Foundation |
| Implement readState/writeState | TBD | 2 | StateManager |
| Implement updateState/resetState | TBD | 1 | Read/write |
| Add backward compatibility merging | TBD | 1 | UpdateState |
| Write state persistence tests | TBD | 2 | All above |

**Deliverables:**
- `state/manager.rs` with StateManager
- `state/models.rs` with TaskState and LockData
- Unit tests for state operations

**Acceptance Criteria:**
- State files read/write correctly
- JSON format matches Node.js version
- Backward compatibility verified

**Sprint 2.2: Lock Management (Week 4)**

| Task | Owner | Days | Dependencies |
|-------|--------|-------|--------------|
| Implement file lock with fs2 | TBD | 2 | Foundation |
| Implement lock acquisition with timeout | TBD | 2 | Lock struct |
| Implement lock release | TBD | 1 | Acquisition |
| Implement lock retry logic | TBD | 2 | Acquisition |
| Add process detection via sysinfo | TBD | 2 | Foundation |
| Implement stale lock cleanup | TBD | 3 | Process detection |
| Write lock management tests | TBD | 2 | All above |

**Deliverables:**
- `state/lock.rs` with lock implementation
- `state/cleanup.rs` with stale lock cleanup
- Cross-platform process detection

**Acceptance Criteria:**
- Locks prevent concurrent execution
- Dead process locks are cleaned up
- Old locks are cleaned up based on age
- Cross-host locks handled correctly

### Phase 3: Workspace Management (Week 5)

| Task | Owner | Days | Dependencies |
|-------|--------|-------|--------------|
| Implement WorkspaceManager struct | TBD | 1 | Foundation |
| Implement file existence checks | TBD | 1 | WorkspaceManager |
| Implement directory creation | TBD | 1 | Foundation |
| Implement atomic file writes | TBD | 2 | tempfile |
| Implement markdown reading | TBD | 1 | File I/O |
| Implement markdown section parsing | TBD | 3 | pulldown-cmark |
| Implement context extraction | TBD | 1 | Section parsing |
| Write workspace tests | TBD | 2 | All above |

**Deliverables:**
- `workspace/manager.rs` with WorkspaceManager
- `workspace/markdown.rs` with markdown utilities
- Unit tests for workspace operations

**Acceptance Criteria:**
- Workspace files read correctly
- Atomic writes prevent corruption
- Markdown sections parsed accurately

### Phase 4: CLI Executor (Weeks 6-7)

**Sprint 4.1: Process Execution (Week 6)**

| Task | Owner | Days | Dependencies |
|-------|--------|-------|--------------|
| Implement OutputBuffer struct | TBD | 1 | Foundation |
| Implement process spawning | TBD | 2 | tokio |
| Implement stdout/stderr capture | TBD | 2 | Spawning |
| Implement stdin piping | TBD | 1 | Spawning |
| Implement timeout enforcement | TBD | 2 | tokio::select |
| Write process execution tests | TBD | 2 | All above |

**Deliverables:**
- `executor/process.rs` with process management
- `executor/buffer.rs` with OutputBuffer
- Unit tests for process operations

**Acceptance Criteria:**
- Processes spawn correctly
- Output captured properly
- Timeouts enforced
- Exit codes captured

**Sprint 4.2: Pattern Detection and Termination (Week 7)**

| Task | Owner | Days | Dependencies |
|-------|--------|-------|--------------|
| Compile error patterns as regex | TBD | 1 | regex crate |
| Implement pattern detection | TBD | 2 | Regex |
| Implement graceful termination | TBD | 2 | tokio |
| Implement forceful termination | TBD | 2 | Graceful |
| Implement CLIExecutor orchestration | TBD | 3 | All above |
| Write executor tests | TBD | 2 | All above |

**Deliverables:**
- `executor/patterns.rs` with error patterns
- `executor/cli.rs` with CLIExecutor
- Integration tests for execution flow

**Acceptance Criteria:**
- Error patterns detected correctly
- Graceful termination works
- Forceful termination works
- Early termination triggers correctly

### Phase 5: Scheduler (Week 8)

| Task | Owner | Days | Dependencies |
|-------|--------|-------|--------------|
| Implement ScheduledTask struct | TBD | 2 | Foundation |
| Implement interval-based scheduling | TBD | 2 | tokio::time::interval |
| Implement exponential backoff retry | TBD | 2 | Foundation |
| Implement graceful shutdown | TBD | 2 | tokio::signal |
| Implement statistics tracking | TBD | 1 | Foundation |
| Write scheduler tests | TBD | 2 | All above |

**Deliverables:**
- `scheduler/task.rs` with ScheduledTask
- `scheduler/retry.rs` with retry strategies
- `scheduler/shutdown.rs` with graceful shutdown
- Unit tests for scheduler

**Acceptance Criteria:**
- Tasks execute at correct intervals
- Retry logic works with exponential backoff
- Graceful shutdown completes tasks
- Statistics tracked accurately

### Phase 6: Agent Entry Points (Weeks 9-10)

**Sprint 6.1: Template Engine (Week 9)**

| Task | Owner | Days | Dependencies |
|-------|--------|-------|--------------|
| Implement template substitution | TBD | 2 | Foundation |
| Implement prompt file loading | TBD | 1 | File I/O |
| Implement context building | TBD | 1 | Workspace |
| Write template tests | TBD | 1 | All above |

**Deliverables:**
- `prompts/template.rs` with template engine
- `prompts/context.rs` with context building
- Unit tests for templates

**Sprint 6.2: Agent Binaries (Week 10)**

| Task | Owner | Days | Dependencies |
|-------|--------|-------|--------------|
| Implement prompt agent binary | TBD | 3 | All modules |
| Implement janitor agent binary | TBD | 3 | All modules |
| Implement architect agent binary | TBD | 3 | All modules |
| Implement signal handling | TBD | 2 | tokio::signal |
| Implement .done file checking | TBD | 1 | Foundation |
| Write agent integration tests | TBD | 3 | All above |

**Deliverables:**
- `bin/agent-prompt.rs`
- `bin/agent-janitor.rs`
- `bin/agent-architect.rs`
- Integration tests for each agent

**Acceptance Criteria:**
- Agents acquire locks correctly
- Agents execute handlers correctly
- Agents release locks correctly
- Signal handlers work
- .done file detection works

### Phase 7: Docker Integration (Week 11)

| Task | Owner | Days | Dependencies |
|-------|--------|-------|--------------|
| Create multi-stage Rust Dockerfile | TBD | 2 | Rust binary |
| Update docker-compose.yml | TBD | 1 | Dockerfile |
| Add environment variable support | TBD | 1 | clap env |
| Test container builds | TBD | 1 | Dockerfile |
| Test container execution | TBD | 2 | Compose |
| Update deployment scripts | TBD | 1 | All above |

**Deliverables:**
- `Dockerfile` (Rust-based)
- `docker-compose.yml` (updated)
- Build/deployment scripts

**Acceptance Criteria:**
- Container builds successfully
- Container starts correctly
- Health checks pass
- Agents execute on schedule

### Phase 8: Testing and Documentation (Weeks 12-13)

**Sprint 8.1: Comprehensive Testing (Week 12)**

| Task | Owner | Days | Dependencies |
|-------|--------|-------|--------------|
| Write property-based tests for state | TBD | 2 | State module |
| Write property-based tests for executor | TBD | 2 | Executor module |
| Write end-to-end integration tests | TBD | 3 | All modules |
| Add performance benchmarks | TBD | 2 | All modules |
| Verify test coverage > 80% | TBD | 1 | All tests |

**Deliverables:**
- Property-based test suite
- Integration test suite
- Benchmark suite
- Coverage report

**Sprint 8.2: Documentation (Week 13)**

| Task | Owner | Days | Dependencies |
|-------|--------|-------|--------------|
| Write README.md | TBD | 2 | - |
| Write API documentation | TBD | 3 | rustdoc comments |
| Write deployment guide | TBD | 1 | Docker |
| Write migration guide | TBD | 2 | - |
| Write troubleshooting guide | TBD | 1 | - |
| Document architecture decisions | TBD | 2 | - |

**Deliverables:**
- `README.md`
- `docs/api/` directory
- `docs/deployment.md`
- `docs/migration.md`
- `docs/troubleshooting.md`
- `docs/architecture.md`

**Acceptance Criteria:**
- Documentation covers all features
- Migration guide tested
- Examples provided

### Milestones and Review Points

| Milestone | Week | Review Criteria |
|-----------|-------|----------------|
| M1: Foundation Complete | 2 | All foundation tests pass, CLI works |
| M2: State Management Complete | 4 | Lock tests pass, state persistence verified |
| M3: Workspace & Executor Complete | 7 | File I/O works, process execution works |
| M4: Scheduler Complete | 8 | Scheduling accurate, retries work |
| M5: Agents Functional | 10 | All agents run independently |
| M6: Docker Integration | 11 | Containers deploy successfully |
| M7: Production Ready | 13 | Tests pass, documentation complete |

---

## Testing Strategy

### Unit Testing

#### Framework

- **Standard:** Built-in `cargo test`
- **Mocking:** Use `mockall` crate for mocking dependencies
- **Async Testing:** `tokio::test` macro for async tests

#### Coverage Areas

| Module | Key Tests | Target Coverage |
|--------|------------|-----------------|
| Error types | All error variants, Display implementation | 100% |
| Config | Parsing, validation, defaults | 90% |
| State | CRUD operations, lock management | 85% |
| Workspace | File operations, markdown parsing | 85% |
| Executor | Process spawning, termination, patterns | 90% |
| Scheduler | Timing, retries, shutdown | 90% |

#### Example Unit Test

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_state_write_and_read() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("test.state.json");
        let manager = StateManager::new(temp_dir.path().to_path_buf());
        
        let state = TaskState {
            status: TaskStatus::Running,
            error_count: 5,
            ..Default::default()
        };
        
        // Write state
        manager.write_state("test", &state).await.unwrap();
        
        // Read state
        let read_state = manager.read_state("test").await.unwrap();
        
        assert_eq!(read_state.status, TaskStatus::Running);
        assert_eq!(read_state.error_count, 5);
    }
}
```

### Integration Testing

#### Scenarios

1. **Agent Lifecycle:**
   - Agent starts
   - Acquires lock
   - Executes handler
   - Releases lock
   - Repeats on interval

2. **Lock Contention:**
   - Two agents try to acquire same lock
   - One succeeds, other waits/retries
   - First releases, second acquires

3. **Stale Lock Cleanup:**
   - Create lock for dead process
   - Run cleanup
   - Verify lock removed

4. **Process Termination:**
   - Start long-running process
   - Detect error pattern
   - Terminate gracefully
   - Force kill if needed

5. **Signal Handling:**
   - Send SIGTERM
   - Verify graceful shutdown
   - Send SIGINT
   - Verify graceful shutdown

#### Integration Test Example

```rust
#[tokio::test]
async fn test_agent_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path();
    let state_path = workspace.join(".state");
    
    // Create workspace files
    fs::create_dir_all(&state_path).await.unwrap();
    fs::write(workspace.join("TODO.md"), "- [ ] Task 1\n").await.unwrap();
    
    // Create managers
    let state_manager = StateManager::new(state_path);
    let workspace_manager = WorkspaceManager::new(workspace.to_path_buf());
    
    // Simulate agent execution
    let lock_acquired = state_manager
        .acquire_lock_with_retry("test", 5000, 3, 1000)
        .await
        .unwrap();
    
    assert!(lock_acquired);
    
    // Execute handler
    let todo_content = workspace_manager.read_todo_file().await.unwrap();
    assert!(todo_content.contains("Task 1"));
    
    // Release lock
    state_manager.release_lock("test").await.unwrap();
    
    // Verify lock released
    let is_locked = state_manager.is_locked("test").await.unwrap();
    assert!(!is_locked);
}
```

### Property-Based Testing

#### Framework

- **Tool:** `proptest` crate
- **Strategy:** Generate random inputs, verify invariants

#### Properties to Test

| Function | Property |
|----------|-----------|
| `updateState()` | Update then read equals original + update |
| `acquireLock()` / `releaseLock()` | Released lock can be re-acquired |
| `atomicWrite()` | Written file content equals input |
| `patternDetection()` | Valid pattern always detected, invalid never |

#### Property-Based Test Example

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_state_update_preserves_existing(
        mut original_state in arb_task_state(),
        updates in arb_state_updates()
    ) {
        let temp_dir = TempDir::new().unwrap();
        let manager = StateManager::new(temp_dir.path().to_path_buf());
        
        // Write original state
        manager.write_state("test", &original_state).await.unwrap();
        
        // Apply updates
        manager.update_state("test", updates).await.unwrap();
        
        // Read and verify
        let read_state = manager.read_state("test").await.unwrap();
        
        // Invariant: Updates should override original values
        for (key, value) in updates {
            // Verify update was applied (implementation-specific check)
            // ...
        }
    }
}
```

### Performance Benchmarking

#### Framework

- **Tool:** `criterion` crate
- **Metrics:** Throughput, latency, memory usage

#### Benchmarks

| Component | Metric | Target |
|-----------|---------|---------|
| State I/O | 1000 state reads/writes/sec | >1000 ops/sec |
| Lock Acquisition | 100 lock acquisitions/sec | >100 ops/sec |
| Pattern Matching | 10MB output scanned | <10ms |
| Process Spawn | Process start time | <50ms |
| Markdown Parsing | 100KB markdown file | <5ms |

#### Benchmark Example

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_state_write(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let manager = StateManager::new(temp_dir.path().to_path_buf());
    let state = TaskState::default();
    
    c.bench_function("state_write", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| manager.write_state("test", black_box(&state)));
    });
}

criterion_group!(benches, benchmark_state_write);
criterion_main!(benches);
```

### Cross-Platform Testing

#### Platforms

- **Primary:** Linux (production environment)
- **Secondary:** Windows, macOS (development environments)

#### Compatibility Tests

| Feature | Linux | Windows | macOS |
|----------|---------|----------|---------|
| File locking | ✅ | ✅ | ✅ |
| Process detection | ✅ | ✅ | ✅ |
| Signal handling | ✅ | ⚠️ | ✅ |
| File permissions | ✅ | ✅ | ✅ |
| Path handling | ✅ | ⚠️ | ✅ |

### Continuous Integration

#### GitHub Actions Workflow

```yaml
name: Rust CI

on: [push, pull_request]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
        rust: [stable, nightly]
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          profile: minimal
          toolchain: ${{ matrix.rust }}
          override: true
          components: rustfmt, clippy
      
      - name: Cache dependencies
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Build
        run: cargo build --all-features
      
      - name: Test
        run: cargo test --all-features
      
      - name: Clippy
        run: cargo clippy --all-features -- -D warnings
      
      - name: Format check
        run: cargo fmt --all -- --check
      
      - name: Coverage
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --out Xml
      - name: Upload coverage
        uses: codecov/codecov-action@v3
```

---

## Documentation Requirements

### Code Documentation

#### Rustdoc Standards

- **Public Items:** All public functions, structs, enums must have documentation
- **Examples:** Complex functions should include example code
- **Warnings:** Document any panic conditions
- **Safety:** For unsafe code, document safety invariants

#### Documentation Template

```rust
/// Acquires an exclusive lock for the specified task.
///
/// This function will retry acquiring the lock up to `max_retries` times
/// with exponential backoff. If the lock cannot be acquired after all retries,
/// an error is returned.
///
/// # Arguments
///
/// * `task_name` - The name of the task to lock
/// * `timeout_ms` - Timeout per lock acquisition attempt in milliseconds
/// * `max_retries` - Maximum number of retry attempts
/// * `retry_delay_ms` - Delay between retry attempts in milliseconds
///
/// # Returns
///
/// Returns `Ok(true)` if the lock was acquired, `Err(AutomationError)` otherwise.
///
/// # Errors
///
/// Returns `AutomationError::LockTimeout` if the lock cannot be acquired
/// within the specified timeout and retry limit.
///
/// # Examples
///
/// ```no_run
/// # use automation_parallel::state::StateManager;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let manager = StateManager::new("/workspace/.state".into());
/// let locked = manager.acquire_lock_with_retry("task", 5000, 3, 1000).await?;
/// # Ok(())
/// # }
/// ```
pub async fn acquire_lock_with_retry(
    &self,
    task_name: &str,
    timeout_ms: u64,
    max_retries: u32,
    retry_delay_ms: u64,
) -> Result<bool> {
    // Implementation...
}
```

### User Documentation

#### README.md Structure

```markdown
# Automation Parallel (Rust)

A high-performance, Rust-based parallel automation system for AI agents.

## Features

- **Performance:** 60-80% lower memory footprint than Node.js version
- **Reliability:** Type-safe implementation eliminates runtime errors
- **Concurrency:** True parallel execution across multiple CPU cores
- **Compatibility:** Drop-in replacement for Node.js version

## Quick Start

### Docker

```bash
cd automation-parallel-rust
docker-compose up -d
```

### Binary

```bash
cargo build --release
./target/release/agent-prompt --workspace /workspace --interval 300000
```

## Configuration

Environment variables (same as Node.js version):
- `WORKSPACE_PATH` - Workspace directory
- `STATE_PATH` - State directory
- `PROMPT_SCHEDULE_INTERVAL_MS` - Schedule interval
- ... (all .env.example variables supported)

## Migration from Node.js

See [MIGRATION.md](docs/migration.md) for detailed migration guide.
```

#### Documentation Files

| File | Purpose | Audience |
|-------|---------|------------|
| `README.md` | Project overview and quick start | Users |
| `docs/migration.md` | Migrating from Node.js to Rust | Existing users |
| `docs/deployment.md` | Docker deployment details | DevOps |
| `docs/architecture.md` | System architecture design | Developers |
| `docs/api/` | API reference | Developers |
| `docs/troubleshooting.md` | Common issues and solutions | Users |
| `CHANGELOG.md` | Version history and changes | All |

### Developer Documentation

#### Architecture Documentation

```markdown
# Architecture

## Module Overview

- **scheduler/**: Task scheduling with Tokio intervals
- **state/**: State persistence and file locking
- **workspace/**: Workspace file operations
- **executor/**: CLI process execution
- **prompts/**: Template rendering
- **common/**: Shared utilities

## Concurrency Model

All I/O operations use async/await with Tokio. Independent agents
run as separate Tokio tasks, allowing true parallelism.

## Error Handling

Errors are represented by the `AutomationError` enum. All functions
return `Result<T, AutomationError>`.

## Locking Strategy

File-based exclusive locks using `fs2`. Locks include:
- Process ID
- Acquisition timestamp
- Hostname

Stale locks are cleaned on startup using process detection.
```

### Migration Guide Structure

```markdown
# Migration Guide: Node.js to Rust

## Overview

The Rust version is a drop-in replacement for the Node.js version
with identical functionality and improved performance.

## Compatibility

### State Files

State files are fully compatible. You can switch between versions
without data loss.

### Configuration

All environment variables from the Node.js version are supported.
No configuration changes required.

### Docker Images

- **Node.js version:** `automation-parallel:latest-node`
- **Rust version:** `automation-parallel:latest-rust`

## Migration Steps

1. **Stop Node.js containers:**
   ```bash
   docker-compose down
   ```

2. **Build Rust version:**
   ```bash
   cd automation-parallel-rust
   docker-compose build
   ```

3. **Start Rust containers:**
   ```bash
   docker-compose up -d
   ```

4. **Verify operation:**
   ```bash
   docker-compose logs -f
   ```

## Rollback

If issues occur, roll back to Node.js version:
```bash
cd automation-parallel
docker-compose up -d
```
```

### API Documentation

Generate Rustdoc with:

```bash
cargo doc --all-features --no-deps --open
```

Publish to: `docs/api/index.html`

---

## Risk Assessment and Mitigation

### Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|-------|-----------|---------|------------|
| **Tokio runtime issues** | Low | High | Comprehensive testing, use stable Tokio version |
| **Cross-platform file locking** | Medium | Medium | Test on Windows/Linux/macOS, use fs2 |
| **Process detection failures** | Low | Medium | Use sysinfo with fallbacks, extensive testing |
| **Memory safety bugs** | Low | High | Rust's ownership model prevents most, extensive unit tests |
| **Performance regression** | Low | Medium | Benchmark before/after, profile with flamegraph |
| **Docker image size** | Medium | Low | Use multi-stage builds, strip binaries |

### Project Risks

| Risk | Likelihood | Impact | Mitigation |
|-------|-----------|---------|------------|
| **Timeline overrun** | Medium | Medium | Agile sprints, regular reviews, flexibility |
| **Feature creep** | Medium | Low | Strict scope definition, change control process |
| **Insufficient testing** | Medium | High | 2-week dedicated testing phase, >80% coverage |
| **Knowledge transfer loss** | Low | Medium | Comprehensive documentation, code reviews |

### Operational Risks

| Risk | Likelihood | Impact | Mitigation |
|-------|-----------|---------|------------|
| **Deployment issues** | Low | High | Blue-green deployment, canary testing |
| **Configuration errors** | Low | Medium | Validation, default values, documentation |
| **Data loss during migration** | Low | High | Backup state files, test migration, rollback plan |
| **Resource exhaustion** | Low | Medium | Resource limits, monitoring, graceful degradation |

### Mitigation Strategies

#### Incremental Deployment

1. **Phase 1:** Deploy Rust version to staging environment
2. **Phase 2:** Run alongside Node.js version in production
3. **Phase 3:** Gradually shift traffic to Rust version
4. **Phase 4:** Monitor metrics, verify stability
5. **Phase 5:** Full cutover to Rust version

#### Monitoring and Alerts

Key metrics to monitor:
- Container resource usage (CPU, memory)
- Agent execution frequency
- Lock contention rates
- Error rates
- Process termination reasons

Alert thresholds:
- Memory > 80% of limit
- CPU > 90% sustained
- Error rate > 5%
- Lock timeout > 10% of attempts

#### Rollback Procedure

If issues detected:

1. **Immediate rollback:**
   ```bash
   cd automation-parallel
   docker-compose up -d
   ```

2. **Data recovery:**
   - State files are compatible, no recovery needed
   - Verify state integrity
   - Check for corrupted files

3. **Analysis:**
   - Review logs for error patterns
   - Identify root cause
   - Document fix

#### Communication Plan

- **Stakeholders:** Weekly progress updates
- **Users:** Pre-launch announcement, migration guide
- **Support:** Incident response procedure documented

---

## Appendix A: Environment Variables

### Complete Environment Variable Reference

| Variable | Required | Default | Description |
|----------|-----------|---------|-------------|
| `CONTAINER_PREFIX` | No | Empty | Prefix for container names |
| `WORKSPACE_PATH` | Yes | `/workspace` | Workspace directory |
| `STATE_PATH` | Yes | `/workspace/.state` | State directory |
| `PROMPT_TASK_NAME` | No | `prompt` | Prompt agent task name |
| `PROMPT_SCHEDULE_INTERVAL_MS` | No | `300000` | Prompt interval (ms) |
| `PROMPT_SCHEDULE_IMMEDIATE` | No | `true` | Execute immediately |
| `JANITOR_TASK_NAME` | No | `janitor` | Janitor agent task name |
| `JANITOR_SCHEDULE_INTERVAL_MS` | No | `1200000` | Janitor interval (ms) |
| `JANITOR_SCHEDULE_IMMEDIATE` | No | `true` | Execute immediately |
| `ARCHITECT_TASK_NAME` | No | `architect` | Architect agent task name |
| `ARCHITECT_SCHEDULE_INTERVAL_MS` | No | `2400000` | Architect interval (ms) |
| `ARCHITECT_SCHEDULE_IMMEDIATE` | No | `true` | Execute immediately |
| `LOCK_TIMEOUT_MS` | No | `30000` | Lock timeout (ms) |
| `LOG_LEVEL` | No | `info` | Logging verbosity |
| `NODE_ENV` | No | `production` | Runtime environment |
| `MOUNT_HOST_DIR` | No | `.` | Host directory to mount |

---

## Appendix B: File Format Specifications

### State File Format (JSON)

```json
{
  "lastRun": "2025-02-07T12:00:00Z",
  "lastSuccess": "2025-02-07T12:00:00Z",
  "lastFailure": null,
  "errorCount": 0,
  "consecutiveFailures": 0,
  "status": "idle",
  "lastTerminationReason": null,
  "earlyTerminationCount": 0,
  "totalExecutionTimeMs": 0,
  "averageExecutionTimeMs": 0,
  "successfulTerminations": 0,
  "failedTerminations": 0
}
```

### Lock File Format (JSON)

```json
{
  "pid": 12345,
  "timestamp": 1707313600000,
  "host": "host.example.com"
}
```

### Workspace File Format (Markdown)

```markdown
# TODO.md

## Active Tasks

- [ ] Implement feature X
- [ ] Fix bug Y
- [ ] Write tests for Z

## In Progress

- [x] Feature A (in progress)

## Completed

- [x] Feature B
```

---

## Appendix C: Error Patterns

### Mistake Limit Detection Patterns

Compiled regex patterns:

| Pattern | Description |
|---------|-------------|
| `Mistake Limit Reached` | Direct text match |
| `[\s\|]*\u{2717}\s*Mistake Limit Reached` | With check mark |
| `[\s\|]*X\s*Mistake Limit Reached` | With X |
| `[\s\|]*\*\s*Mistake Limit Reached` | With asterisk |
| `Mistake Limit Reached[\s\S]{0,500}This may indicate a failure in the model` | Multi-line with context |

---

## Appendix D: System Requirements

### Minimum Requirements

| Component | Requirement |
|-----------|-------------|
| **Rust** | 1.75+ |
| **Docker** | 20.10+ |
| **Docker Compose** | 2.0+ |
| **Disk Space** | 500 MB for images, 100 MB for runtime |
| **Memory** | 512 MB per container |

### Recommended Requirements

| Component | Requirement |
|-----------|-------------|
| **Rust** | 1.75+ (latest stable) |
| **Docker** | Latest version |
| **Docker Compose** | Latest version |
| **Disk Space** | 1 GB for images, 200 MB for runtime |
| **Memory** | 1 GB per container |

### Platform Support

| Platform | Support | Notes |
|----------|----------|-------|
| **Linux** | ✅ Full | Production target |
| **Windows** | ✅ Supported | Development environment |
| **macOS** | ✅ Supported | Development environment |

---

## Conclusion

This comprehensive plan provides a roadmap for rewriting the @automation-parallel system in Rust. The migration is expected to deliver:

- **60-80% reduction** in memory footprint
- **10-40% improvement** in execution speed
- **True parallelism** across CPU cores
- **Enhanced reliability** through Rust's type system
- **Maintained compatibility** with existing deployments

The 13-week implementation roadmap is structured to deliver value incrementally, with clear milestones and review points. Comprehensive testing and documentation ensure a production-ready release.

---

**Document End**

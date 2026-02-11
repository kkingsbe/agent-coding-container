# automation-scheduler API Reference

The `automation-scheduler` crate provides task scheduling capabilities with configurable intervals, retry logic, and graceful shutdown for the gastown system.

## Table of Contents

- [Overview](#overview)
- [Scheduler](#scheduler)
- [Task Scheduling](#task-scheduling)
- [Retry Logic](#retry-logic)
- [Statistics](#statistics)
- [Usage Examples](#usage-examples)

## Overview

`automation-scheduler` provides:

- **Periodic task execution** - Run tasks at configurable intervals
- **Exponential backoff** - Automatic retry with increasing delays
- **Graceful shutdown** - Clean shutdown with in-flight task completion
- **Statistics tracking** - Track execution count, failures, and timings
- **Timeout handling** - Task timeout enforcement

## Scheduler

### `Scheduler`

Manages periodic task execution.

```rust
pub struct Scheduler {
    interval: Duration,
    timeout: Duration,
    max_retries: usize,
    stats: Statistics,
}
```

### `Scheduler::new()`

Create a new scheduler.

```rust
impl Scheduler {
    pub fn new(interval: Duration) -> Self
}
```

**Parameters:**

- `interval` - Time between task executions

**Returns:**

- `Scheduler` - New scheduler instance

**Default Settings:**

| Setting | Default Value |
|---------|---------------|
| Timeout | 30 minutes (1800 seconds) |
| Max Retries | 3 |
| Backoff Factor | 2 |
| Initial Backoff | 1 second |

**Example:**

```rust
use automation_scheduler::Scheduler;
use std::time::Duration;

// Run every 5 minutes
let scheduler = Scheduler::new(Duration::from_secs(300));
```

### `Scheduler::with_timeout()`

Set the maximum task execution timeout.

```rust
impl Scheduler {
    pub fn with_timeout(mut self, timeout: Duration) -> Self
}
```

**Parameters:**

- `timeout` - Maximum time to allow each task execution

**Returns:**

- `Scheduler` - Scheduler with configured timeout

**Example:**

```rust
use automation_scheduler::Scheduler;
use std::time::Duration;

let scheduler = Scheduler::new(Duration::from_secs(300))
    .with_timeout(Duration::from_secs(1800)); // 30 minutes
```

### `Scheduler::with_max_retries()`

Set the maximum number of retry attempts.

```rust
impl Scheduler {
    pub fn with_max_retries(mut self, max_retries: usize) -> Self
}
```

**Parameters:**

- `max_retries` - Maximum number of retry attempts

**Returns:**

- `Scheduler` - Scheduler with configured max retries

**Example:**

```rust
use automation_scheduler::Scheduler;

let scheduler = Scheduler::new(Duration::from_secs(300))
    .with_max_retries(5); // Retry up to 5 times
```

### `Scheduler::with_backoff()`

Set the initial backoff duration and factor.

```rust
impl Scheduler {
    pub fn with_backoff(mut self, initial: Duration, factor: u64) -> Self
}
```

**Parameters:**

- `initial` - Initial backoff duration
- `factor` - Exponential backoff factor

**Returns:**

- `Scheduler` - Scheduler with configured backoff

**Example:**

```rust
use automation_scheduler::Scheduler;
use std::time::Duration;

let scheduler = Scheduler::new(Duration::from_secs(300))
    .with_backoff(Duration::from_secs(1), 2); // 1s, 2s, 4s, 8s, ...
```

### `Scheduler::interval()`

Get the current interval.

```rust
impl Scheduler {
    pub fn interval(&self) -> Duration
}
```

**Returns:**

- `Duration` - Current interval

## Task Scheduling

### `run()`

Run a task periodically with retry logic.

```rust
impl Scheduler {
    pub async fn run<F, Fut, E>(&mut self, task: F) -> Result<(), SchedulerError<E>>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: Future<Output = Result<(), E>> + Send,
        E: std::error::Error + Send + Sync + 'static,
}
```

**Parameters:**

- `task` - Async function to execute periodically

**Returns:**

- `Result<(), SchedulerError<E>>` - Success or scheduler error

**Behavior:**

1. Execute the task
2. Wait for the configured interval
3. Repeat until shutdown is requested

**Example:**

```rust
use automation_scheduler::Scheduler;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scheduler = Scheduler::new(Duration::from_secs(60));
    
    scheduler.run(|| async {
        println!("Task executed at: {:?}", std::time::Instant::now());
        Ok::<(), Box<dyn std::error::Error>>(())
    }).await?;
    
    Ok(())
}
```

### `run_once()`

Execute the task once without scheduling.

```rust
impl Scheduler {
    pub async fn run_once<F, Fut, E>(&self, task: F) -> Result<TaskResult, SchedulerError<E>>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<(), E>>,
        E: std::error::Error + Send + Sync + 'static,
}
```

**Parameters:**

- `task` - Async function to execute

**Returns:**

- `Result<TaskResult, SchedulerError<E>>` - Task result or error

**Example:**

```rust
use automation_scheduler::Scheduler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scheduler = Scheduler::new(std::time::Duration::from_secs(300));
    
    let result = scheduler.run_once(|| async {
        println!("Running once");
        Ok::<(), Box<dyn std::error::Error>>(())
    }).await?;
    
    println!("Execution time: {:?}", result.duration);
    println!("Success: {}", result.success);
    
    Ok(())
}
```

### `run_with_shutdown()`

Run a task with shutdown signal handling.

```rust
impl Scheduler {
    pub async fn run_with_shutdown<F, Fut, E, S, SFut>(
        &mut self,
        task: F,
        shutdown_signal: S
    ) -> Result<(), SchedulerError<E>>
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: Future<Output = Result<(), E>> + Send,
        E: std::error::Error + Send + Sync + 'static,
        S: Fn() -> SFut + Send + Sync,
        SFut: Future<Output = ()> + Send,
}
```

**Parameters:**

- `task` - Async function to execute periodically
- `shutdown_signal` - Async function that completes when shutdown is requested

**Returns:**

- `Result<(), SchedulerError<E>>` - Success or scheduler error

**Example:**

```rust
use automation_scheduler::Scheduler;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scheduler = Scheduler::new(std::time::Duration::from_secs(60));
    
    scheduler.run_with_shutdown(
        || async {
            println!("Task executed");
            Ok::<(), Box<dyn std::error::Error>>(())
        },
        signal::ctrl_c
    ).await?;
    
    println!("Shutdown gracefully");
    Ok(())
}
```

## Retry Logic

### `TaskResult`

Contains the result of a single task execution.

```rust
pub struct TaskResult {
    pub success: bool,
    pub duration: Duration,
    pub retry_count: usize,
    pub error: Option<String>,
}
```

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `success` | `bool` | Whether the task succeeded |
| `duration` | `Duration` | Execution duration |
| `retry_count` | `usize` | Number of retries attempted |
| `error` | `Option<String>` | Error message if failed |

### `Statistics`

Tracks scheduler statistics across multiple executions.

```rust
pub struct Statistics {
    pub total_executions: usize,
    pub successful_executions: usize,
    pub failed_executions: usize,
    pub total_duration: Duration,
    pub last_execution: Option<Instant>,
}
```

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `total_executions` | `usize` | Total number of executions |
| `successful_executions` | `usize` | Number of successful executions |
| `failed_executions` | `usize` | Number of failed executions |
| `total_duration` | `Duration` | Total time spent executing tasks |
| `last_execution` | `Option<Instant>` | Time of last execution |

### `Scheduler::stats()`

Get the current statistics.

```rust
impl Scheduler {
    pub fn stats(&self) -> &Statistics
}
```

**Returns:**

- `&Statistics` - Reference to statistics

**Example:**

```rust
use automation_scheduler::Scheduler;

let scheduler = Scheduler::new(std::time::Duration::from_secs(60));
let stats = scheduler.stats();

println!("Total executions: {}", stats.total_executions);
println!("Success rate: {:.2}%", 
    stats.successful_executions as f64 / stats.total_executions as f64 * 100.0
);
```

### `Scheduler::reset_stats()`

Reset statistics.

```rust
impl Scheduler {
    pub fn reset_stats(&mut self)
}
```

**Example:**

```rust
use automation_scheduler::Scheduler;

let mut scheduler = Scheduler::new(std::time::Duration::from_secs(60));
scheduler.reset_stats();
```

## Statistics

### `Statistics::success_rate()`

Calculate success rate.

```rust
impl Statistics {
    pub fn success_rate(&self) -> f64
}
```

**Returns:**

- `f64` - Success rate as a percentage (0.0 to 100.0)

**Example:**

```rust
use automation_scheduler::Scheduler;

let scheduler = Scheduler::new(std::time::Duration::from_secs(60));
let stats = scheduler.stats();

let rate = stats.success_rate();
println!("Success rate: {:.1}%", rate);
```

### `Statistics::average_duration()`

Calculate average execution duration.

```rust
impl Statistics {
    pub fn average_duration(&self) -> Option<Duration>
}
```

**Returns:**

- `Option<Duration>` - Average duration, or None if no executions

**Example:**

```rust
use automation_scheduler::Scheduler;

let scheduler = Scheduler::new(std::time::Duration::from_secs(60));
let stats = scheduler.stats();

if let Some(avg) = stats.average_duration() {
    println!("Average duration: {:?}", avg);
}
```

## Usage Examples

### Basic Periodic Execution

```rust
use automation_scheduler::Scheduler;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scheduler = Scheduler::new(Duration::from_secs(60));
    
    scheduler.run(|| async {
        println!("Executing task at {:?}", std::time::SystemTime::now());
        Ok::<(), Box<dyn std::error::Error>>(())
    }).await?;
    
    Ok(())
}
```

### Execution with Timeout

```rust
use automation_scheduler::Scheduler;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scheduler = Scheduler::new(Duration::from_secs(300))
        .with_timeout(Duration::from_secs(60)); // 1 minute timeout
    
    scheduler.run(|| async {
        // This task must complete within 1 minute
        tokio::time::sleep(Duration::from_secs(30)).await;
        println!("Task completed");
        Ok::<(), Box<dyn std::error::Error>>(())
    }).await?;
    
    Ok(())
}
```

### Retry with Exponential Backoff

```rust
use automation_scheduler::Scheduler;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scheduler = Scheduler::new(Duration::from_secs(300))
        .with_max_retries(5)
        .with_backoff(Duration::from_secs(1), 2); // 1s, 2s, 4s, 8s, 16s
    
    scheduler.run(|| async {
        // This may fail and will be retried
        static mut COUNTER: usize = 0;
        unsafe {
            COUNTER += 1;
            if COUNTER < 3 {
                eprintln!("Attempt {} failed, will retry", COUNTER);
                return Err::<(), Box<dyn std::error::Error>>("Simulated failure".into());
            }
        }
        println!("Task succeeded!");
        Ok(())
    }).await?;
    
    Ok(())
}
```

### Graceful Shutdown

```rust
use automation_scheduler::Scheduler;
use tokio::signal;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scheduler = Scheduler::new(Duration::from_secs(60));
    
    println!("Press Ctrl+C to shutdown...");
    
    scheduler.run_with_shutdown(
        || async {
            println!("Task executing...");
            tokio::time::sleep(Duration::from_secs(5)).await;
            println!("Task completed");
            Ok::<(), Box<dyn std::error::Error>>(())
        },
        signal::ctrl_c
    ).await?;
    
    println!("Shutting down gracefully");
    Ok(())
}
```

### Tracking Statistics

```rust
use automation_scheduler::Scheduler;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scheduler = Scheduler::new(Duration::from_secs(10));
    
    // Run for a few iterations
    let mut iterations = 0;
    let max_iterations = 5;
    
    scheduler.run(|| async {
        iterations += 1;
        
        // Simulate occasional failure
        if iterations % 3 == 0 {
            println!("Iteration {} failed", iterations);
            return Err::<(), Box<dyn std::error::Error>>("Simulated failure".into());
        }
        
        println!("Iteration {} succeeded", iterations);
        Ok(())
    }).await?;
    
    // Print statistics
    let stats = scheduler.stats();
    println!("\n--- Statistics ---");
    println!("Total executions: {}", stats.total_executions);
    println!("Successful: {}", stats.successful_executions);
    println!("Failed: {}", stats.failed_executions);
    println!("Success rate: {:.1}%", stats.success_rate());
    
    if let Some(avg) = stats.average_duration() {
        println!("Average duration: {:?}", avg);
    }
    
    Ok(())
}
```

### Single Execution

```rust
use automation_scheduler::Scheduler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scheduler = Scheduler::new(std::time::Duration::from_secs(300));
    
    let result = scheduler.run_once(|| async {
        println!("Running task once");
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        Ok::<(), Box<dyn std::error::Error>>(())
    }).await?;
    
    println!("Duration: {:?}", result.duration);
    println!("Success: {}", result.success);
    
    Ok(())
}
```

### Custom Task with State

```rust
use automation_scheduler::Scheduler;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scheduler = Scheduler::new(Duration::from_secs(60));
    
    // Shared state across executions
    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    
    scheduler.run({
        let counter = counter.clone();
        move || async move {
            let count = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            println!("Execution #{}", count + 1);
            Ok::<(), Box<dyn std::error::Error>>(())
        }
    }).await?;
    
    Ok(())
}
```

## Error Handling

### `SchedulerError`

Errors that can occur during scheduling.

```rust
pub enum SchedulerError<E> {
    TaskError(E),
    Timeout,
    Shutdown,
}
```

### Error Handling Example

```rust
use automation_scheduler::{Scheduler, SchedulerError};
use std::time::Duration;

#[tokio::main]
async fn main() {
    let mut scheduler = Scheduler::new(Duration::from_secs(10));
    
    match scheduler.run(|| async {
        // Simulate a task that fails
        Err::<(), Box<dyn std::error::Error>>("Task failed".into())
    }).await {
        Ok(_) => println!("Scheduler completed successfully"),
        Err(SchedulerError::TaskError(e)) => {
            eprintln!("Task error: {:?}", e);
        }
        Err(SchedulerError::Timeout) => {
            eprintln!("Task timed out");
        }
        Err(SchedulerError::Shutdown) => {
            eprintln!("Shutdown requested");
        }
    }
}
```

## See Also

- [Architecture Documentation](../architecture.md) - System architecture overview
- [Common API](common.md) - Common types and utilities
- [Agents API](agents.md) - Agent implementations
- [Deployment Guide](../deployment.md) - Deployment instructions

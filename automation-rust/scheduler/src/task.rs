//! # Scheduled Task
//!
//! This module provides core `ScheduledTask` implementation for managing
//! recurring task execution with retry logic and graceful shutdown support.
//!
//! # Heartbeat Scheduling
//!
//! This module also provides heartbeat and cleanup scheduling for parallel agents mode.
//! These components enable:
//! - Periodic heartbeat updates to signal agent health
//! - Automatic cleanup of stale agents and completed tasks
//! - Graceful shutdown of scheduled background tasks
//!
//! # Example
//!
//! ```no_run
//! use automation_scheduler::{HeartbeatTask, CleanupTask, schedule_heartbeat, schedule_cleanup};
//! use automation_state::{AgentRegistry, TaskRegistry};
//! use automation_common::workspace_config::ParallelConfig;
//! use std::time::Duration;
//! use std::sync::Arc;
//! use tokio::sync::Mutex;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let agent_registry = Arc::new(Mutex::new(AgentRegistry::new()?));
//!     let task_registry = Arc::new(Mutex::new(TaskRegistry::new()?));
//!     let config = ParallelConfig::default();
//!
//!     // Start heartbeat for an agent
//!     let _heartbeat_handle = schedule_heartbeat(
//!         "agent-123".to_string(),
//!         Duration::from_secs(30),
//!         agent_registry.clone()
//!     );
//!
//!     // Start cleanup task
//!     let _cleanup_handle = schedule_cleanup(
//!         &config,
//!         task_registry.clone(),
//!         agent_registry.clone()
//!     );
//!
//!     // Run your application...
//!
//!     Ok(())
//! }
//! ```

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use automation_common::AutomationError;
use automation_common::workspace_config::ParallelConfig;
use automation_state::agent_registry::AgentRegistry;
use automation_state::task_registry::TaskRegistry;
use chrono::Utc;
use tokio::select;
use tokio::signal::ctrl_c;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Instant};

use tracing::{error, info, warn};

/// Task execution statistics.
///
/// Tracks success/failure counts and timestamps for monitoring
/// and debugging purposes.
#[derive(Debug, Clone, Default)]
pub struct TaskStats {
    /// Number of successful executions
    pub success_count: u32,

    /// Number of failed executions
    pub failure_count: u32,

    /// Timestamp of last run
    pub last_run_timestamp: Option<chrono::DateTime<Utc>>,
}

impl TaskStats {
    /// Record a successful execution.
    pub fn record_success(&mut self) {
        self.success_count += 1;
        self.last_run_timestamp = Some(Utc::now());
    }

    /// Record a failed execution.
    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_run_timestamp = Some(Utc::now());
    }
}

/// Result of a single task execution.
#[derive(Debug)]
pub enum ExecutionResult {
    /// Execution completed successfully
    Success { duration: Duration },
    /// Execution failed with error
    Failure { error: anyhow::Error },
    /// Execution was terminated early by shutdown signal
    EarlyTermination { reason: String, duration: Duration },
}

/// Configuration for a scheduled task.
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

    /// Retry delay base for exponential backoff (100ms * 2^attempt)
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

/// A scheduled task that executes a handler function at regular intervals.
///
/// The task supports:
/// - Interval-based scheduling using tokio::time::interval
/// - Immediate execution on startup if configured
/// - Exponential backoff retry strategy (100ms * 2^attempt)
/// - Graceful shutdown on SIGINT/SIGTERM
/// - Statistics tracking for monitoring
///
/// # Example
///
/// ```no_run
/// use automation_scheduler::ScheduledTask;
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let mut task = ScheduledTask::new(
///         "my-task",
///         Duration::from_secs(60),
///         true,
///         3,
///         Duration::from_millis(100),
///     );
///
///     task.start(|| async {
///         println!("Executing task...");
///         Ok(())
///     }).await?;
///     Ok(())
/// }
/// ```
pub struct ScheduledTask {
    config: TaskConfig,
    shutdown_notify: Arc<Notify>,
    is_running: Arc<AtomicBool>,
    stats: Arc<std::sync::Mutex<TaskStats>>,
    task_handle: Option<JoinHandle<()>>,
}

impl ScheduledTask {
    /// Create a new scheduled task.
    ///
    /// # Arguments
    ///
    /// * `task_name` - Name of the task for logging
    /// * `interval` - Interval between executions
    /// * `immediate` - Whether to execute immediately on startup
    /// * `max_retries` - Maximum number of retry attempts
    /// * `retry_delay_base` - Base delay for exponential backoff
    pub fn new(
        task_name: impl Into<String>,
        interval: Duration,
        immediate: bool,
        max_retries: u32,
        retry_delay_base: Duration,
    ) -> Self {
        Self {
            config: TaskConfig {
                task_name: task_name.into(),
                interval,
                immediate,
                max_retries,
                retry_delay_base,
            },
            shutdown_notify: Arc::new(Notify::new()),
            is_running: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(std::sync::Mutex::new(TaskStats::default())),
            task_handle: None,
        }
    }

    /// Start the scheduled task.
    ///
    /// This method begins the scheduling loop. It will:
    /// 1. Execute immediately if configured
    /// 2. Execute on each interval tick
    /// 3. Retry on failure with exponential backoff (100ms * 2^attempt)
    /// 4. Handle graceful shutdown on SIGINT
    ///
    /// # Arguments
    ///
    /// * `handler` - Async function to execute on each run
    pub async fn start<H, Fut>(&mut self, handler: H) -> Result<(), AutomationError>
    where
        H: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = anyhow::Result<()>> + Send,
    {
        if self.is_running.load(Ordering::Relaxed) {
            return Err(AutomationError::ScheduleConfigInvalid {
                reason: format!(
                    "Task '{}' is already running",
                    self.config.task_name
                ),
            });
        }

        self.is_running.store(true, Ordering::Relaxed);
        info!(
            task = %self.config.task_name,
            interval_sec = self.config.interval.as_secs(),
            "Starting scheduled task"
        );

        let shutdown_notify = Arc::clone(&self.shutdown_notify);
        let is_running = Arc::clone(&self.is_running);
        let stats = Arc::clone(&self.stats);
        let schedule_interval = self.config.interval;
        let immediate = self.config.immediate;
        let task_name = self.config.task_name.clone();
        let max_retries = self.config.max_retries;
        let retry_delay_base = self.config.retry_delay_base;

        self.task_handle = Some(tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(schedule_interval);
            interval_timer.tick().await; // First tick completes immediately

            if immediate {
                // Execute immediately on startup
                Self::execute_with_retry(
                    &task_name,
                    &handler,
                    &stats,
                    max_retries,
                    retry_delay_base,
                )
                .await;
            }

            loop {
                select! {
                    // Execute on interval tick
                    _ = interval_timer.tick() => {
                        Self::execute_with_retry(
                            &task_name,
                            &handler,
                            &stats,
                            max_retries,
                            retry_delay_base,
                        )
                        .await;
                    }
                    // Handle shutdown signal
                    _ = shutdown_notify.notified() => {
                        info!(
                            task = %task_name,
                            "Shutdown signal received, stopping task"
                        );
                        is_running.store(false, Ordering::Relaxed);
                        break;
                    }
                    // Handle Ctrl+C
                    _ = ctrl_c() => {
                        info!(
                            task = %task_name,
                            "Ctrl+C received, stopping task"
                        );
                        is_running.store(false, Ordering::Relaxed);
                        break;
                    }
                }
            }
        }));

        Ok(())
    }

    /// Stop the scheduled task gracefully.
    ///
    /// This method will:
    /// 1. Notify the task loop to stop
    /// 2. Wait for the task to complete
    /// 3. Return once fully stopped
    pub async fn stop(&mut self) -> Result<(), AutomationError> {
        info!(
            task = %self.config.task_name,
            "Stopping scheduled task"
        );

        self.shutdown_notify.notify_one();
        self.is_running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.task_handle.take() {
            if let Err(e) = handle.await {
                error!(
                    task = %self.config.task_name,
                    error = %e,
                    "Error while stopping task"
                );
                return Err(AutomationError::Process(format!(
                    "Failed to stop task '{}': {}",
                    self.config.task_name, e
                )));
            }
        }

        info!(
            task = %self.config.task_name,
            "Scheduled task stopped successfully"
        );
        Ok(())
    }

    /// Get task statistics.
    pub fn get_stats(&self) -> TaskStats {
        let stats = self.stats.lock().unwrap();
        stats.clone()
    }

    /// Check if the task is currently running.
    pub fn is_task_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Execute the handler with retry logic and exponential backoff.
    ///
    /// Implements exponential backoff: 100ms * 2^attempt
    /// - First retry: 100ms * 2^1 = 200ms
    /// - Second retry: 100ms * 2^2 = 400ms
    /// - Third retry: 100ms * 2^3 = 800ms
    async fn execute_with_retry<H, Fut>(
        task_name: &str,
        handler: &H,
        stats: &Arc<std::sync::Mutex<TaskStats>>,
        max_retries: u32,
        retry_delay_base: Duration,
    ) where
        H: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = anyhow::Result<()>> + Send,
    {
        let mut attempt = 0;

        loop {
            let start_time = Instant::now();
            let result = handler().await;
            let duration = start_time.elapsed();

            match result {
                Ok(_) => {
                    stats.lock().unwrap().record_success();
                    info!(
                        task = %task_name,
                        duration_ms = duration.as_millis(),
                        "Task execution completed successfully"
                    );
                    break;
                }
                Err(e) => {
                    attempt += 1;
                    stats.lock().unwrap().record_failure();

                    if attempt > max_retries {
                        error!(
                            task = %task_name,
                            attempt = attempt,
                            error = %e,
                            "Task execution failed after all retry attempts"
                        );
                        break;
                    }

                    // Calculate exponential backoff delay: 100ms * 2^attempt
                    let backoff_ms = retry_delay_base.as_millis() as u64 * 2u64.pow(attempt);
                    let backoff_duration = Duration::from_millis(backoff_ms);

                    warn!(
                        task = %task_name,
                        attempt = attempt,
                        max_retries = max_retries,
                        error = %e,
                        retry_delay_ms = backoff_ms,
                        "Task execution failed, retrying with exponential backoff"
                    );

                    sleep(backoff_duration).await;
                }
            }
        }
    }
}

/// Factory function to create a new scheduled task with default configuration.
///
/// # Arguments
///
/// * `task_name` - Name of the task for logging
///
/// # Returns
///
/// Returns a new `ScheduledTask` with default configuration.
pub fn create_scheduled_task(task_name: impl Into<String>) -> ScheduledTask {
    ScheduledTask::new(
        task_name,
        TaskConfig::default().interval,
        TaskConfig::default().immediate,
        TaskConfig::default().max_retries,
        TaskConfig::default().retry_delay_base,
    )
}

/// Gracefully shutdown multiple scheduled tasks.
///
/// This function will:
/// 1. Stop all tasks in parallel
/// 2. Wait for all tasks to complete with a timeout
/// 3. Return once all tasks have stopped or timeout is reached
///
/// # Arguments
///
/// * `tasks` - Vector of mutable references to tasks to stop
/// * `timeout` - Maximum time to wait for all tasks to stop
///
/// # Returns
///
/// Returns `Ok(())` if all tasks stopped successfully, or `Err(AutomationError)` if timeout is reached.
pub async fn graceful_shutdown(
    tasks: &mut [&mut ScheduledTask],
    timeout: Duration,
) -> Result<(), AutomationError> {
    info!(
        task_count = tasks.len(),
        timeout_sec = timeout.as_secs(),
        "Starting graceful shutdown for {} tasks",
        tasks.len()
    );

    // Send shutdown signals to all tasks in parallel
    for task in tasks.iter_mut() {
        task.shutdown_notify.notify_one();
    }

    // Wait for all tasks to stop with timeout
    let stop_future = async {
        for task in tasks.iter_mut() {
            if let Some(handle) = task.task_handle.take() {
                if let Err(e) = handle.await {
                    error!(
                        task = %task.config.task_name,
                        error = %e,
                        "Error while stopping task"
                    );
                }
            }
            task.is_running.store(false, Ordering::Relaxed);
        }
        Ok::<(), AutomationError>(())
    };

    match tokio::time::timeout(timeout, stop_future).await {
        Ok(Ok(())) => {
            info!("All tasks stopped successfully");
            Ok(())
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err(AutomationError::ScheduleConfigInvalid {
            reason: format!("Graceful shutdown timeout after {:?}", timeout),
        }),
    }
}

/// A periodic heartbeat task for agents running in parallel mode.
///
/// The heartbeat task allows agents to signal they are still active and
/// prevents stale agents from holding onto tasks indefinitely. It runs
/// in the background and periodically updates the agent's heartbeat
/// timestamp in the registry.
///
/// # Example
///
/// ```no_run
/// use automation_scheduler::HeartbeatTask;
/// use automation_state::AgentRegistry;
/// use std::sync::Arc;
/// use tokio::sync::Mutex;
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let agent_registry = Arc::new(Mutex::new(AgentRegistry::new()?));
///
///     let heartbeat_task = HeartbeatTask::new(
///         "agent-123".to_string(),
///         Duration::from_secs(30),
///         agent_registry.clone()
///     );
///
///     // Start the heartbeat task in the background
///     let handle = heartbeat_task.start();
///
///     // Do some work...
///
///     // Signal the task to stop
///     heartbeat_task.stop();
///     handle.await?;
///
///     Ok(())
/// }
/// ```
pub struct HeartbeatTask {
    /// Unique identifier for this agent instance
    agent_id: String,
    /// Interval between heartbeat updates
    interval: Duration,
    /// Shared reference to the agent registry
    agent_registry: Arc<Mutex<AgentRegistry>>,
    /// Shutdown signal
    shutdown_notify: Arc<Notify>,
    /// Whether the task is currently running
    is_running: Arc<AtomicBool>,
}

impl HeartbeatTask {
    /// Create a new heartbeat task.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - Unique identifier for the agent instance
    /// * `interval` - Interval between heartbeat updates
    /// * `agent_registry` - Shared reference to the agent registry
    ///
    /// # Example
    ///
    /// ```
    /// use automation_scheduler::HeartbeatTask;
    /// use automation_state::AgentRegistry;
    /// use std::sync::Arc;
    /// use tokio::sync::Mutex;
    /// use std::time::Duration;
    ///
    /// let agent_registry = Arc::new(Mutex::new(
    ///     AgentRegistry::new().unwrap()
    /// ));
    ///
    /// let task = HeartbeatTask::new(
    ///     "agent-123".to_string(),
    ///     Duration::from_secs(30),
    ///     agent_registry
    /// );
    /// ```
    pub fn new(
        agent_id: String,
        interval: Duration,
        agent_registry: Arc<Mutex<AgentRegistry>>,
    ) -> Self {
        Self {
            agent_id,
            interval,
            agent_registry,
            shutdown_notify: Arc::new(Notify::new()),
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Execute a single heartbeat update.
    ///
    /// This method updates the agent's heartbeat timestamp in the registry.
    /// It can be called directly if you want to control the heartbeat timing
    /// manually instead of using the background task.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the heartbeat was updated successfully
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The agent is not found in the registry
    /// - The registry lock cannot be acquired
    /// - The registry cannot be saved
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_scheduler::HeartbeatTask;
    /// use automation_state::{AgentRegistry, generate_agent_id};
    /// use std::sync::Arc;
    /// use tokio::sync::Mutex;
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let agent_registry = Arc::new(Mutex::new(AgentRegistry::new()?));
    ///     let agent_id = generate_agent_id("prompt");
    ///
    ///     {
    ///         let mut registry = agent_registry.lock().await;
    ///         registry.register_agent(agent_id.clone(), "prompt".to_string())?;
    ///     }
    ///
    ///     let heartbeat = HeartbeatTask::new(
    ///         agent_id.clone(),
    ///         Duration::from_secs(30),
    ///         agent_registry.clone()
    ///     );
    ///
    ///     // Send a single heartbeat
    ///     heartbeat.run().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn run(&self) -> Result<(), AutomationError> {
        let mut registry = self.agent_registry.lock().await;
        registry
            .update_heartbeat(&self.agent_id)
            .map_err(|e| AutomationError::StateError {
                task: format!("heartbeat update for agent {}", self.agent_id),
                message: format!("Failed to update heartbeat: {}", e),
            })?;

        info!(
            agent_id = %self.agent_id,
            "Heartbeat updated successfully"
        );

        Ok(())
    }

    /// Start the heartbeat task as a background task.
    ///
    /// This method spawns a background tokio task that periodically sends
    /// heartbeats at the configured interval. The task will continue running
    /// until `stop()` is called.
    ///
    /// # Returns
    ///
    /// A `JoinHandle` that can be used to await the background task
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_scheduler::HeartbeatTask;
    /// use automation_state::{AgentRegistry, generate_agent_id};
    /// use std::sync::Arc;
    /// use tokio::sync::Mutex;
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let agent_registry = Arc::new(Mutex::new(AgentRegistry::new()?));
    ///     let agent_id = generate_agent_id("prompt");
    ///
    ///     {
    ///         let mut registry = agent_registry.lock().await;
    ///         registry.register_agent(agent_id.clone(), "prompt".to_string())?;
    ///     }
    ///
    ///     let heartbeat = HeartbeatTask::new(
    ///         agent_id.clone(),
    ///         Duration::from_secs(30),
    ///         agent_registry.clone()
    ///     );
    ///
    ///     // Start the heartbeat task
    ///     let handle = heartbeat.start();
    ///
    ///     // Run for a while...
    ///     tokio::time::sleep(Duration::from_secs(60)).await;
    ///
    ///     // Stop the task
    ///     heartbeat.stop();
    ///     handle.await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn start(self) -> JoinHandle<Result<(), AutomationError>> {
        let agent_id = self.agent_id.clone();
        let heartbeat_interval = self.interval;
        let shutdown_notify = Arc::clone(&self.shutdown_notify);
        let is_running = Arc::clone(&self.is_running);

        info!(
            agent_id = %agent_id,
            interval_sec = heartbeat_interval.as_secs(),
            "Starting heartbeat task"
        );

        is_running.store(true, Ordering::Relaxed);

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(heartbeat_interval);
            interval_timer.tick().await; // First tick completes immediately

            loop {
                select! {
                    // Send heartbeat on interval tick
                    _ = interval_timer.tick() => {
                        if let Err(e) = self.run().await {
                            error!(
                                agent_id = %agent_id,
                                error = %e,
                                "Heartbeat failed"
                            );
                        }
                    }
                    // Handle shutdown signal
                    _ = shutdown_notify.notified() => {
                        info!(
                            agent_id = %agent_id,
                            "Shutdown signal received, stopping heartbeat task"
                        );
                        is_running.store(false, Ordering::Relaxed);
                        break;
                    }
                    // Handle Ctrl+C
                    _ = ctrl_c() => {
                        info!(
                            agent_id = %agent_id,
                            "Ctrl+C received, stopping heartbeat task"
                        );
                        is_running.store(false, Ordering::Relaxed);
                        break;
                    }
                }
            }

            Ok(())
        })
    }

    /// Signal the heartbeat task to stop.
    ///
    /// This method notifies the background task to stop running. After
    /// calling this method, you should await the `JoinHandle` returned
    /// from `start()` to ensure the task has stopped.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_scheduler::HeartbeatTask;
    /// use automation_state::{AgentRegistry, generate_agent_id};
    /// use std::sync::Arc;
    /// use tokio::sync::Mutex;
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let agent_registry = Arc::new(Mutex::new(AgentRegistry::new()?));
    ///     let agent_id = generate_agent_id("prompt");
    ///
    ///     {
    ///         let mut registry = agent_registry.lock().await;
    ///         registry.register_agent(agent_id.clone(), "prompt".to_string())?;
    ///     }
    ///
    ///     let heartbeat = HeartbeatTask::new(
    ///         agent_id.clone(),
    ///         Duration::from_secs(30),
    ///         agent_registry.clone()
    ///     );
    ///
    ///     let handle = heartbeat.start();
    ///
    ///     // Signal to stop
    ///     heartbeat.stop();
    ///
    ///     // Wait for the task to stop
    ///     handle.await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn stop(&self) {
        info!(
            agent_id = %self.agent_id,
            "Stopping heartbeat task"
        );
        self.shutdown_notify.notify_one();
        self.is_running.store(false, Ordering::Relaxed);
    }

    /// Check if the heartbeat task is currently running.
    ///
    /// # Returns
    ///
    /// `true` if the task is running, `false` otherwise
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }
}

/// A periodic cleanup task for parallel agents mode.
///
/// The cleanup task performs maintenance operations on the registries,
/// including:
/// - Marking stale agents based on heartbeat thresholds
/// - Removing shutdown agents after a retention period
/// - Cleaning up completed tasks after a retention period
/// - Abandoning stale tasks with expired leases
///
/// # Example
///
/// ```no_run
/// use automation_scheduler::CleanupTask;
/// use automation_state::{AgentRegistry, TaskRegistry};
/// use automation_common::workspace_config::ParallelConfig;
/// use std::sync::Arc;
/// use tokio::sync::Mutex;
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let task_registry = Arc::new(Mutex::new(TaskRegistry::new()?));
///     let agent_registry = Arc::new(Mutex::new(AgentRegistry::new()?));
///     let config = ParallelConfig::default();
///
///     let cleanup_task = CleanupTask::new(
///         task_registry.clone(),
///         agent_registry.clone(),
///         Duration::from_secs(60),
///         config.stale_agent_threshold,
///         config.completed_task_retention,
///         config.stale_agent_retention
///     );
///
///     // Start the cleanup task in the background
///     let handle = cleanup_task.start();
///
///     // Do some work...
///
///     // Signal the task to stop
///     cleanup_task.stop();
///     handle.await?;
///
///     Ok(())
/// }
/// ```
pub struct CleanupTask {
    /// Shared reference to the task registry
    task_registry: Arc<Mutex<TaskRegistry>>,
    /// Shared reference to the agent registry
    agent_registry: Arc<Mutex<AgentRegistry>>,
    /// Interval between cleanup operations
    cleanup_interval: Duration,
    /// Threshold for marking agents as stale
    stale_threshold: Duration,
    /// Duration to keep completed tasks before cleanup
    completed_retention: Duration,
    /// Duration to keep stale agents before removal
    stale_agent_retention: Duration,
    /// Shutdown signal
    shutdown_notify: Arc<Notify>,
    /// Whether the task is currently running
    is_running: Arc<AtomicBool>,
}

impl CleanupTask {
    /// Create a new cleanup task.
    ///
    /// # Arguments
    ///
    /// * `task_registry` - Shared reference to the task registry
    /// * `agent_registry` - Shared reference to the agent registry
    /// * `cleanup_interval` - Interval between cleanup operations
    /// * `stale_threshold` - Threshold for marking agents as stale
    /// * `completed_retention` - Duration to keep completed tasks before cleanup
    /// * `stale_agent_retention` - Duration to keep stale agents before removal
    ///
    /// # Example
    ///
    /// ```
    /// use automation_scheduler::CleanupTask;
    /// use automation_state::{TaskRegistry, AgentRegistry};
    /// use std::sync::Arc;
    /// use tokio::sync::Mutex;
    /// use std::time::Duration;
    ///
    /// let task_registry = Arc::new(Mutex::new(
    ///     TaskRegistry::new().unwrap()
    /// ));
    /// let agent_registry = Arc::new(Mutex::new(
    ///     AgentRegistry::new().unwrap()
    /// ));
    ///
    /// let cleanup = CleanupTask::new(
    ///     task_registry,
    ///     agent_registry,
    ///     Duration::from_secs(60),
    ///     Duration::from_secs(90),
    ///     Duration::from_secs(86400),
    ///     Duration::from_secs(3600)
    /// );
    /// ```
    pub fn new(
        task_registry: Arc<Mutex<TaskRegistry>>,
        agent_registry: Arc<Mutex<AgentRegistry>>,
        cleanup_interval: Duration,
        stale_threshold: Duration,
        completed_retention: Duration,
        stale_agent_retention: Duration,
    ) -> Self {
        Self {
            task_registry,
            agent_registry,
            cleanup_interval,
            stale_threshold,
            completed_retention,
            stale_agent_retention,
            shutdown_notify: Arc::new(Notify::new()),
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Execute a single cleanup operation.
    ///
    /// This method performs all cleanup operations:
    /// 1. Mark stale agents
    /// 2. Cleanup shutdown agents
    /// 3. Cleanup completed tasks
    /// 4. Abandon stale tasks
    ///
    /// # Returns
    ///
    /// `Ok(())` if cleanup completed successfully
    ///
    /// # Errors
    ///
    /// Returns an error if any cleanup operation fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_scheduler::CleanupTask;
    /// use automation_state::{TaskRegistry, AgentRegistry};
    /// use std::sync::Arc;
    /// use tokio::sync::Mutex;
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let task_registry = Arc::new(Mutex::new(TaskRegistry::new()?));
    ///     let agent_registry = Arc::new(Mutex::new(AgentRegistry::new()?));
    ///
    ///     let cleanup = CleanupTask::new(
    ///         task_registry,
    ///         agent_registry,
    ///         Duration::from_secs(60),
    ///         Duration::from_secs(90),
    ///         Duration::from_secs(86400),
    ///         Duration::from_secs(3600)
    ///     );
    ///
    ///     // Run a single cleanup operation
    ///     cleanup.run().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn run(&self) -> Result<(), AutomationError> {
        // Mark stale agents
        {
            let mut registry = self.agent_registry.lock().await;
            match registry.mark_stale_agents(self.stale_threshold) {
                Ok(stale_agents) if !stale_agents.is_empty() => {
                    info!(
                        count = stale_agents.len(),
                        "Marked agents as stale"
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    warn!(
                        error = %e,
                        "Failed to mark stale agents"
                    );
                }
            }
        }

        // Cleanup shutdown agents
        {
            let mut registry = self.agent_registry.lock().await;
            match registry.cleanup_shutdown_agents(self.stale_agent_retention) {
                Ok(removed_agents) if !removed_agents.is_empty() => {
                    info!(
                        count = removed_agents.len(),
                        "Removed shutdown agents"
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    warn!(
                        error = %e,
                        "Failed to cleanup shutdown agents"
                    );
                }
            }
        }

        // Cleanup completed tasks
        {
            let mut registry = self.task_registry.lock().await;
            match registry.cleanup_completed_tasks(self.completed_retention) {
                Ok(removed_tasks) if !removed_tasks.is_empty() => {
                    info!(
                        count = removed_tasks.len(),
                        "Removed completed tasks"
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    warn!(
                        error = %e,
                        "Failed to cleanup completed tasks"
                    );
                }
            }
        }

        // Abandon stale tasks (tasks with expired leases)
        {
            let mut registry = self.task_registry.lock().await;
            match registry.abandon_stale_tasks(self.stale_threshold) {
                Ok(abandoned_tasks) if !abandoned_tasks.is_empty() => {
                    info!(
                        count = abandoned_tasks.len(),
                        "Abandoned stale tasks"
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    warn!(
                        error = %e,
                        "Failed to abandon stale tasks"
                    );
                }
            }
        }

        info!("Cleanup operation completed");

        Ok(())
    }

    /// Start the cleanup task as a background task.
    ///
    /// This method spawns a background tokio task that periodically performs
    /// cleanup operations at the configured interval. The task will continue
    /// running until `stop()` is called.
    ///
    /// # Returns
    ///
    /// A `JoinHandle` that can be used to await the background task
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_scheduler::CleanupTask;
    /// use automation_state::{TaskRegistry, AgentRegistry};
    /// use std::sync::Arc;
    /// use tokio::sync::Mutex;
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let task_registry = Arc::new(Mutex::new(TaskRegistry::new()?));
    ///     let agent_registry = Arc::new(Mutex::new(AgentRegistry::new()?));
    ///
    ///     let cleanup = CleanupTask::new(
    ///         task_registry,
    ///         agent_registry,
    ///         Duration::from_secs(60),
    ///         Duration::from_secs(90),
    ///         Duration::from_secs(86400),
    ///         Duration::from_secs(3600)
    ///     );
    ///
    ///     // Start the cleanup task
    ///     let handle = cleanup.start();
    ///
    ///     // Run for a while...
    ///     tokio::time::sleep(Duration::from_secs(300)).await;
    ///
    ///     // Stop the task
    ///     cleanup.stop();
    ///     handle.await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn start(self) -> JoinHandle<Result<(), AutomationError>> {
        let cleanup_interval = self.cleanup_interval;
        let shutdown_notify = Arc::clone(&self.shutdown_notify);
        let is_running = Arc::clone(&self.is_running);

        info!(
            interval_sec = cleanup_interval.as_secs(),
            "Starting cleanup task"
        );

        is_running.store(true, Ordering::Relaxed);

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(cleanup_interval);
            interval_timer.tick().await; // First tick completes immediately

            loop {
                select! {
                    // Run cleanup on interval tick
                    _ = interval_timer.tick() => {
                        if let Err(e) = self.run().await {
                            error!(
                                error = %e,
                                "Cleanup operation failed"
                            );
                        }
                    }
                    // Handle shutdown signal
                    _ = shutdown_notify.notified() => {
                        info!(
                            "Shutdown signal received, stopping cleanup task"
                        );
                        is_running.store(false, Ordering::Relaxed);
                        break;
                    }
                    // Handle Ctrl+C
                    _ = ctrl_c() => {
                        info!(
                            "Ctrl+C received, stopping cleanup task"
                        );
                        is_running.store(false, Ordering::Relaxed);
                        break;
                    }
                }
            }

            Ok(())
        })
    }

    /// Signal the cleanup task to stop.
    ///
    /// This method notifies the background task to stop running. After
    /// calling this method, you should await the `JoinHandle` returned
    /// from `start()` to ensure the task has stopped.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_scheduler::CleanupTask;
    /// use automation_state::{TaskRegistry, AgentRegistry};
    /// use std::sync::Arc;
    /// use tokio::sync::Mutex;
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let task_registry = Arc::new(Mutex::new(TaskRegistry::new()?));
    ///     let agent_registry = Arc::new(Mutex::new(AgentRegistry::new()?));
    ///
    ///     let cleanup = CleanupTask::new(
    ///         task_registry,
    ///         agent_registry,
    ///         Duration::from_secs(60),
    ///         Duration::from_secs(90),
    ///         Duration::from_secs(86400),
    ///         Duration::from_secs(3600)
    ///     );
    ///
    ///     let handle = cleanup.start();
    ///
    ///     // Signal to stop
    ///     cleanup.stop();
    ///
    ///     // Wait for the task to stop
    ///     handle.await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn stop(&self) {
        info!("Stopping cleanup task");
        self.shutdown_notify.notify_one();
        self.is_running.store(false, Ordering::Relaxed);
    }

    /// Check if the cleanup task is currently running.
    ///
    /// # Returns
    ///
    /// `true` if the task is running, `false` otherwise
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }
}

/// Convenience function to start a heartbeat task for an agent.
///
/// This function creates a new `HeartbeatTask` and starts it as a
/// background task, returning the join handle.
///
/// # Arguments
///
/// * `agent_id` - Unique identifier for the agent instance
/// * `interval` - Interval between heartbeat updates
/// * `agent_registry` - Shared reference to the agent registry
///
/// # Returns
///
/// A `JoinHandle` that can be used to await the background task
///
/// # Example
///
/// ```no_run
/// use automation_scheduler::schedule_heartbeat;
/// use automation_state::{AgentRegistry, generate_agent_id};
/// use std::sync::Arc;
/// use tokio::sync::Mutex;
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let agent_registry = Arc::new(Mutex::new(AgentRegistry::new()?));
///     let agent_id = generate_agent_id("prompt");
///
///     {
///         let mut registry = agent_registry.lock().await;
///         registry.register_agent(agent_id.clone(), "prompt".to_string())?;
///     }
///
///     // Start heartbeat
///     let _heartbeat_handle = schedule_heartbeat(
///         agent_id,
///         Duration::from_secs(30),
///         agent_registry
///     );
///
///     // Run your application...
///
///     Ok(())
/// }
/// ```
pub fn schedule_heartbeat(
    agent_id: String,
    interval: Duration,
    agent_registry: Arc<Mutex<AgentRegistry>>,
) -> JoinHandle<Result<(), AutomationError>> {
    let task = HeartbeatTask::new(agent_id, interval, agent_registry);
    task.start()
}

/// Convenience function to start a cleanup task.
///
/// This function creates a new `CleanupTask` with configuration from
/// the `ParallelConfig` and starts it as a background task, returning
/// the join handle.
///
/// # Arguments
///
/// * `config` - Parallel configuration containing cleanup settings
/// * `task_registry` - Shared reference to the task registry
/// * `agent_registry` - Shared reference to the agent registry
///
/// # Returns
///
/// A `JoinHandle` that can be used to await the background task
///
/// # Example
///
/// ```no_run
/// use automation_scheduler::schedule_cleanup;
/// use automation_state::{TaskRegistry, AgentRegistry};
/// use automation_common::workspace_config::ParallelConfig;
/// use std::sync::Arc;
/// use tokio::sync::Mutex;
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let task_registry = Arc::new(Mutex::new(TaskRegistry::new()?));
///     let agent_registry = Arc::new(Mutex::new(AgentRegistry::new()?));
///     let config = ParallelConfig::default();
///
///     // Start cleanup
///     let _cleanup_handle = schedule_cleanup(
///         &config,
///         task_registry,
///         agent_registry
///     );
///
///     // Run your application...
///
///     Ok(())
/// }
/// ```
pub fn schedule_cleanup(
    config: &ParallelConfig,
    task_registry: Arc<Mutex<TaskRegistry>>,
    agent_registry: Arc<Mutex<AgentRegistry>>,
) -> JoinHandle<Result<(), AutomationError>> {
    let task = CleanupTask::new(
        task_registry,
        agent_registry,
        Duration::from_secs(60), // Cleanup interval: 1 minute
        config.stale_agent_threshold,
        config.completed_task_retention,
        config.stale_agent_retention,
    );
    task.start()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_config_default() {
        let config = TaskConfig::default();
        assert_eq!(config.task_name, "unknown");
        assert_eq!(config.interval, Duration::from_secs(300));
        assert_eq!(config.immediate, false);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_base, Duration::from_millis(100));
    }

    #[test]
    fn test_task_stats_record_success() {
        let mut stats = TaskStats::default();
        stats.record_success();
        stats.record_success();

        assert_eq!(stats.success_count, 2);
        assert_eq!(stats.failure_count, 0);
        assert!(stats.last_run_timestamp.is_some());
    }

    #[test]
    fn test_task_stats_record_failure() {
        let mut stats = TaskStats::default();
        stats.record_failure();
        stats.record_failure();

        assert_eq!(stats.success_count, 0);
        assert_eq!(stats.failure_count, 2);
        assert!(stats.last_run_timestamp.is_some());
    }

    #[test]
    fn test_execution_result_variants() {
        // Test Success variant
        let success = ExecutionResult::Success { duration: Duration::from_millis(100) };
        match success {
            ExecutionResult::Success { duration } => {
                assert_eq!(duration.as_millis(), 100);
            }
            _ => panic!("Expected Success variant"),
        }

        // Test Failure variant
        let failure = ExecutionResult::Failure {
            error: anyhow::anyhow!("test error"),
        };
        match failure {
            ExecutionResult::Failure { error } => {
                assert_eq!(error.to_string(), "test error");
            }
            _ => panic!("Expected Failure variant"),
        }

        // Test EarlyTermination variant
        let early = ExecutionResult::EarlyTermination {
            reason: "test reason".to_string(),
            duration: Duration::from_millis(50),
        };
        match early {
            ExecutionResult::EarlyTermination { reason, duration } => {
                assert_eq!(reason, "test reason");
                assert_eq!(duration.as_millis(), 50);
            }
            _ => panic!("Expected EarlyTermination variant"),
        }
    }

    #[test]
    fn test_create_scheduled_task_factory() {
        let task = create_scheduled_task("factory-test");
        assert_eq!(task.get_stats().success_count, 0);
        assert!(!task.is_task_running());
    }

    #[tokio::test]
    async fn test_task_start_stop() {
        let mut task = ScheduledTask::new(
            "test-task",
            Duration::from_millis(100),
            false,
            0,
            Duration::from_millis(10),
        );

        task.start(|| async { Ok(()) }).await.unwrap();
        assert!(task.is_task_running());

        // Give it a moment to start
        tokio::time::sleep(Duration::from_millis(50)).await;

        task.stop().await.unwrap();
        assert!(!task.is_task_running());
    }

    #[tokio::test]
    async fn test_exponential_backoff_calculation() {
        // Test that exponential backoff is calculated correctly
        // base = 100ms
        // attempt 1: 100 * 2^1 = 200ms
        // attempt 2: 100 * 2^2 = 400ms
        // attempt 3: 100 * 2^3 = 800ms

        let base = Duration::from_millis(100);

        // First attempt (attempt=1)
        let backoff_ms = base.as_millis() as u64 * 2u64.pow(1);
        assert_eq!(backoff_ms, 200);

        // Second attempt (attempt=2)
        let backoff_ms = base.as_millis() as u64 * 2u64.pow(2);
        assert_eq!(backoff_ms, 400);

        // Third attempt (attempt=3)
        let backoff_ms = base.as_millis() as u64 * 2u64.pow(3);
        assert_eq!(backoff_ms, 800);
    }

    #[tokio::test]
    async fn test_task_with_immediate_execution() {
        let mut task = ScheduledTask::new(
            "immediate-task",
            Duration::from_secs(60),
            true,
            0,
            Duration::from_millis(10),
        );

        task.start(|| async { Ok(()) }).await.unwrap();

        // Give it time for immediate execution
        tokio::time::sleep(Duration::from_millis(100)).await;

        task.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_graceful_shutdown_single_task() {
        let mut task = ScheduledTask::new(
            "shutdown-task",
            Duration::from_millis(100),
            false,
            0,
            Duration::from_millis(10),
        );

        task.start(|| async { Ok(()) }).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        let result = graceful_shutdown(&mut [&mut task], Duration::from_secs(1)).await;
        assert!(result.is_ok());
        assert!(!task.is_task_running());
    }

    #[tokio::test]
    async fn test_graceful_shutdown_multiple_tasks() {
        let mut task1 = ScheduledTask::new(
            "task1",
            Duration::from_millis(100),
            false,
            0,
            Duration::from_millis(10),
        );
        let mut task2 = ScheduledTask::new(
            "task2",
            Duration::from_millis(100),
            false,
            0,
            Duration::from_millis(10),
        );
        let mut task3 = ScheduledTask::new(
            "task3",
            Duration::from_millis(100),
            false,
            0,
            Duration::from_millis(10),
        );

        task1.start(|| async { Ok(()) }).await.unwrap();
        task2.start(|| async { Ok(()) }).await.unwrap();
        task3.start(|| async { Ok(()) }).await.unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        let result = graceful_shutdown(&mut [&mut task1, &mut task2, &mut task3], Duration::from_secs(1)).await;
        assert!(result.is_ok());
        assert!(!task1.is_task_running());
        assert!(!task2.is_task_running());
        assert!(!task3.is_task_running());
    }

    #[tokio::test]
    async fn test_task_stats_tracking() {
        let mut task = ScheduledTask::new(
            "stats-task",
            Duration::from_millis(50),
            false,
            0,
            Duration::from_millis(10),
        );

        let execution_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let execution_count_clone = execution_count.clone();

        task.start(move || {
            let count = execution_count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
        })
        .await
        .unwrap();

        // Wait for a few executions
        tokio::time::sleep(Duration::from_millis(200)).await;

        let stats = task.get_stats();
        assert!(stats.success_count >= 2, "Expected at least 2 executions");
        assert!(stats.last_run_timestamp.is_some());

        task.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_task_with_retries() {
        let attempt_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));

        let mut task = ScheduledTask::new(
            "retry-task",
            Duration::from_secs(60),
            true,  // Execute immediately to test retry logic
            3,
            Duration::from_millis(10),
        );

        task.start({
            let attempt_count = attempt_count.clone();
            move || {
                let count = attempt_count.clone();
                async move {
                    let attempt = count.fetch_add(1, Ordering::Relaxed);
                    if attempt < 2 {
                        Err(anyhow::anyhow!("Simulated failure"))
                    } else {
                        Ok(())
                    }
                }
            }
        })
        .await
        .unwrap();

        // Wait for retries: 20ms + 40ms = 60ms for 2 failures, plus buffer
        tokio::time::sleep(Duration::from_millis(200)).await;

        let stats = task.get_stats();
        // Should have success after retries
        assert_eq!(stats.success_count, 1);
        assert_eq!(stats.failure_count, 2);

        task.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_heartbeat_task_new() {
        let agent_registry = Arc::new(Mutex::new(AgentRegistry::new().unwrap()));
        let task = HeartbeatTask::new(
            "agent-123".to_string(),
            Duration::from_secs(30),
            agent_registry,
        );
        assert_eq!(task.agent_id, "agent-123");
        assert_eq!(task.interval, Duration::from_secs(30));
        assert!(!task.is_running());
    }

    #[tokio::test]
    async fn test_heartbeat_task_run() {
        use automation_state::generate_agent_id;
        let agent_registry = Arc::new(Mutex::new(AgentRegistry::new().unwrap()));
        let agent_id = generate_agent_id("test-run");

        // Register the agent first
        {
            let mut registry = agent_registry.lock().await;
            registry
                .register_agent(agent_id.clone(), "test".to_string())
                .unwrap();
        }

        let heartbeat_task = HeartbeatTask::new(
            agent_id.clone(),
            Duration::from_secs(30),
            agent_registry.clone(),
        );

        // Run a single heartbeat
        heartbeat_task.run().await.unwrap();

        // Verify heartbeat was updated
        let registry = agent_registry.lock().await;
        let agent = registry.get_agent(&agent_id).unwrap();
        assert!(agent.last_heartbeat_age() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_heartbeat_task_start_stop() {
        use automation_state::generate_agent_id;
        let agent_registry = Arc::new(Mutex::new(AgentRegistry::new().unwrap()));
        let agent_id = generate_agent_id("test-start-stop");

        // Register the agent first
        {
            let mut registry = agent_registry.lock().await;
            registry
                .register_agent(agent_id.clone(), "test".to_string())
                .unwrap();
        }

        let heartbeat_task = HeartbeatTask::new(
            agent_id.clone(),
            Duration::from_millis(50),
            agent_registry.clone(),
        );

        // Clone is_running flag before start (start takes ownership)
        let is_running = Arc::clone(&heartbeat_task.is_running);
        assert!(!is_running.load(Ordering::Relaxed));

        // Start the task
        let handle = heartbeat_task.start();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(is_running.load(Ordering::Relaxed));

        // Wait for the task to finish
        handle.abort();
    }

    #[tokio::test]
    async fn test_cleanup_task_new() {
        let task_registry = Arc::new(Mutex::new(TaskRegistry::new().unwrap()));
        let agent_registry = Arc::new(Mutex::new(AgentRegistry::new().unwrap()));

        let task = CleanupTask::new(
            task_registry,
            agent_registry,
            Duration::from_secs(60),
            Duration::from_secs(90),
            Duration::from_secs(86400),
            Duration::from_secs(3600),
        );

        assert_eq!(task.cleanup_interval, Duration::from_secs(60));
        assert_eq!(task.stale_threshold, Duration::from_secs(90));
        assert_eq!(task.completed_retention, Duration::from_secs(86400));
        assert_eq!(task.stale_agent_retention, Duration::from_secs(3600));
        assert!(!task.is_running());
    }

    #[tokio::test]
    async fn test_cleanup_task_run() {
        let task_registry = Arc::new(Mutex::new(TaskRegistry::new().unwrap()));
        let agent_registry = Arc::new(Mutex::new(AgentRegistry::new().unwrap()));

        let cleanup_task = CleanupTask::new(
            task_registry,
            agent_registry,
            Duration::from_secs(60),
            Duration::from_secs(90),
            Duration::from_secs(86400),
            Duration::from_secs(3600),
        );

        // Run a single cleanup (should succeed even with empty registries)
        cleanup_task.run().await.unwrap();
    }

    #[tokio::test]
    async fn test_cleanup_task_start_stop() {
        let task_registry = Arc::new(Mutex::new(TaskRegistry::new().unwrap()));
        let agent_registry = Arc::new(Mutex::new(AgentRegistry::new().unwrap()));

        let cleanup_task = CleanupTask::new(
            task_registry,
            agent_registry,
            Duration::from_millis(50),
            Duration::from_secs(90),
            Duration::from_secs(86400),
            Duration::from_secs(3600),
        );

        // Clone is_running flag before start (start takes ownership)
        let is_running = Arc::clone(&cleanup_task.is_running);
        assert!(!is_running.load(Ordering::Relaxed));

        // Start the task
        let handle = cleanup_task.start();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(is_running.load(Ordering::Relaxed));

        // Wait for the task to finish
        handle.abort();
    }

    #[tokio::test]
    async fn test_schedule_heartbeat() {
        let agent_registry = Arc::new(Mutex::new(AgentRegistry::new().unwrap()));
        let agent_id = generate_agent_id("test-schedule");
        use automation_state::generate_agent_id;

        // Register the agent first
        {
            let mut registry = agent_registry.lock().await;
            registry
                .register_agent(agent_id.clone(), "test".to_string())
                .unwrap();
        }

        // Schedule heartbeat
        let handle = schedule_heartbeat(
            agent_id.clone(),
            Duration::from_millis(50),
            agent_registry.clone(),
        );

        // Let it run a bit
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Verify heartbeat was sent
        let registry = agent_registry.lock().await;
        let agent = registry.get_agent(&agent_id).unwrap();
        assert!(agent.last_heartbeat_age() < Duration::from_secs(1));
        drop(registry);

        // Cleanup
        handle.abort();
    }

    #[tokio::test]
    async fn test_schedule_cleanup() {
        let task_registry = Arc::new(Mutex::new(TaskRegistry::new().unwrap()));
        let agent_registry = Arc::new(Mutex::new(AgentRegistry::new().unwrap()));
        let config = ParallelConfig::default();

        // Schedule cleanup
        let handle = schedule_cleanup(&config, task_registry, agent_registry);

        // Let it run a bit
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Cleanup
        handle.abort();
    }

    #[tokio::test]
    async fn test_cleanup_with_stale_agents() {
        let task_registry = Arc::new(Mutex::new(TaskRegistry::new().unwrap()));
        let agent_registry = Arc::new(Mutex::new(AgentRegistry::new().unwrap()));
        let agent_id = "stale-agent-1".to_string();

        // Register an agent
        {
            let mut registry = agent_registry.lock().await;
            registry
                .register_agent(agent_id.clone(), "test".to_string())
                .unwrap();
        }

        // Create cleanup task with very short stale threshold
        let cleanup_task = CleanupTask::new(
            task_registry,
            agent_registry.clone(),
            Duration::from_secs(60),
            Duration::from_millis(10), // Very short threshold
            Duration::from_secs(86400),
            Duration::from_secs(3600),
        );

        // Wait for agent to become stale
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Run cleanup - agent should be marked as stale
        cleanup_task.run().await.unwrap();

        let registry = agent_registry.lock().await;
        let agent = registry.get_agent(&agent_id).unwrap();
        assert!(agent.status.is_stale());
    }
}

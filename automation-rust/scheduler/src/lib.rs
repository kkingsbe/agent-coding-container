//! # automation-scheduler
//!
//! Scheduler module for automation-rust project.
//!
//! This module provides functionality for managing recurring task execution with:
//! - Configurable intervals
//! - Immediate execution support
//! - Exponential backoff retry strategy
//! - Graceful shutdown support with timeout handling
//! - Statistics tracking (success/failure counts)
//! - Heartbeat scheduling for parallel agents mode
//! - Periodic cleanup for stale agents and completed tasks
//!
//! # Example - Basic Scheduled Task
//!
//! ```no_run
//! use automation_scheduler::ScheduledTask;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut task = ScheduledTask::new(
//!         "my-task",
//!         Duration::from_secs(60),
//!         true,
//!         3,
//!         Duration::from_millis(100),
//!     );
//!     task.start(|| async {
//!         println!("Executing task...");
//!         Ok(())
//!     }).await?;
//!     Ok(())
//! }
//! ```
//!
//! # Example - Heartbeat and Cleanup for Parallel Agents
//!
//! ```no_run
//! use automation_scheduler::{schedule_heartbeat, schedule_cleanup};
//! use automation_state::{AgentRegistry, TaskRegistry, generate_agent_id};
//! use automation_common::workspace_config::ParallelConfig;
//! use std::sync::Arc;
//! use tokio::sync::Mutex;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let agent_registry = Arc::new(Mutex::new(AgentRegistry::new()?));
//!     let task_registry = Arc::new(Mutex::new(TaskRegistry::new()?));
//!     let config = ParallelConfig::default();
//!     let agent_id = generate_agent_id("prompt");
//!
//!     // Register the agent
//!     {
//!         let mut registry = agent_registry.lock().await;
//!         registry.register_agent(agent_id.clone(), "prompt".to_string())?;
//!     }
//!
//!     // Start heartbeat for the agent
//!     let _heartbeat_handle = schedule_heartbeat(
//!         agent_id,
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
//!     tokio::time::sleep(Duration::from_secs(3600)).await;
//!
//!     Ok(())
//! }
//! ```

pub mod task;

// Re-export for convenience
pub use task::{
    ScheduledTask,
    TaskConfig,
    TaskStats,
    ExecutionResult,
    create_scheduled_task,
    graceful_shutdown,
    HeartbeatTask,
    CleanupTask,
    schedule_heartbeat,
    schedule_cleanup,
};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}

//! # automation-state
//!
//! State management module for the automation-rust project.
//!
//! This module provides data structures and functionality for managing task state files,
//! including:
//! - State data structures with JSON serialization
//! - Mistake/error tracking for system analysis
//! - Lock information structures for file-based locking
//! - Performance metrics tracking
//! - Termination tracking
//! - File-based locking with RAII pattern
//! - State persistence with atomic writes
//! - Backup and restore functionality
//! - Cleanup utilities for stale data
//! - Task registry for parallel agent execution tracking
//!
//! # Modules
//!
//! - [`state`] - Core state data structures (State, Mistake, LockInfo)
//! - [`lock`] - File-based locking using fs2 with RAII pattern
//! - [`persistence`] - State persistence with atomic writes and backup/restore
//! - [`cleanup`] - Cleanup utilities for stale state data, backups, mistakes, and locks
//! - [`task_registry`] - Task registry for tracking tasks with assignments, leases, and state transitions (RegistryTaskState)
//! - [`agent_registry`] - Agent registry for tracking agent instances with heartbeats and status
//!
//! # Example
//!
//! ```no_run
//! use automation_state::State;
//! use chrono::Utc;
//!
//! // Create a new state
//! let mut state = State::new();
//!
//! // Add a mistake record
//! let mistake_id = state.add_mistake(
//!     "Task failed with timeout error".to_string(),
//!     Some("architect".to_string())
//! );
//!
//! // Update last run timestamp
//! state.update_last_run(Utc::now());
//!
//! // Serialize to JSON
//! let json = serde_json::to_string_pretty(&state).unwrap();
//! ```
//!
//! # Persistence Example
//!
//! ```no_run
//! use automation_state::persistence::StateManager;
//! use std::path::PathBuf;
//!
//! // Create a state manager
//! let manager = StateManager::new(
//!     PathBuf::from("/workspace/.state"),
//!     "architect"
//! ).unwrap();
//!
//! // Load state (returns default if file doesn't exist)
//! let state = manager.load_state().unwrap();
//!
//! // Modify and save state
//! let mut state = state;
//! state.update_last_run(chrono::Utc::now());
//! manager.save_state(&state).unwrap();
//! ```

pub mod state;
pub mod lock;
pub mod persistence;
pub mod cleanup;
pub mod task_registry;
pub mod agent_registry;

// Re-export for convenience
pub use state::{
    State,
    Mistake,
    LockInfo,
    TaskStatus,
    TaskState,
    LockData,
    generate_mistake_id,
    create_lock_info,
    Mergeable,
    deep_merge_json,
};

// Re-export lock module items
pub use lock::{
    LockManager,
    LockHandle,
    get_current_process_id,
    get_hostname,
    is_process_alive,
};

// Re-export persistence module items
pub use persistence::{
    StateManager,
    get_state_file_path,
    get_task_state_file_path,
    get_backup_path,
    list_backups,
    atomic_write,
    lock_file_path_for_state,
};

// Re-export cleanup module items
pub use cleanup::{
    CleanupManager,
    CleanupConfig,
    CleanupStats,
    CleanupReport,
    get_default_cleanup_config,
    should_cleanup_state,
    calculate_backup_age,
};

// Re-export task_registry module items
pub use task_registry::{
    TaskRegistry,
    RegistryTaskState,
    TaskAssignment,
};

// Re-export agent_registry module items
pub use agent_registry::{
    AgentRegistry,
    AgentInstance,
    AgentStatus,
    AgentStats,
    generate_agent_id,
};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}

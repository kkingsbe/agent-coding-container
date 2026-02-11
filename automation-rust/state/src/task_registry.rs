//! Task registry for tracking tasks with assignments, leases, and state transitions.
//!
//! This module provides a task registry that enables parallel agent execution by:
//! - Tracking task availability and assignment status
//! - Managing task leases with expiration
//! - Supporting state transitions (Available → InProgress → Completed/Failed)
//! - Cleaning up stale tasks and completed tasks
//!
//! # Thread Safety
//!
//! The registry uses file-based locking via `LockManager` to coordinate access
//! across multiple processes. All operations that modify the registry acquire
//! a lock before making changes.
//!
//! # Example
//!
//! ```no_run
//! use automation_state::task_registry::{TaskRegistry, RegistryTaskState};
//! use std::time::Duration;
//!
//! // Create or load a task registry
//! let mut registry = TaskRegistry::new().unwrap();
//!
//! // Register a new task
//! registry.register_task("task-1".to_string(), "Build the project".to_string()).unwrap();
//!
//! // Claim a task for an agent
//! if registry.claim_task("task-1", "agent-123", Duration::from_secs(60)).unwrap() {
//!     // Task was claimed successfully
//!     // ... do work ...
//!
//!     // Complete the task
//!     registry.complete_task("task-1", "agent-123").unwrap();
//! }
//! ```

use automation_common::{AutomationError, Result};
use crate::lock::LockManager;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Task state enumeration.
///
/// Represents the current status of a task in the registry.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RegistryTaskState {
    /// Task is available for claiming
    Available,
    /// Task is currently being worked on by an agent
    InProgress,
    /// Task completed successfully
    Completed,
    /// Task failed with an error
    Failed,
}

impl RegistryTaskState {
    /// Returns true if the task can be claimed.
    pub fn is_claimable(&self) -> bool {
        matches!(self, RegistryTaskState::Available)
    }

    /// Returns true if the task is currently being worked on.
    pub fn is_in_progress(&self) -> bool {
        matches!(self, RegistryTaskState::InProgress)
    }

    /// Returns true if the task has reached a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, RegistryTaskState::Completed | RegistryTaskState::Failed)
    }
}

impl Default for RegistryTaskState {
    fn default() -> Self {
        RegistryTaskState::Available
    }
}

/// Task assignment information.
///
/// Contains all metadata about a task including its state, assignment details,
/// timestamps, and attempt history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskAssignment {
    /// Unique identifier for the task
    pub task_id: String,
    /// Human-readable description of the task
    pub task_description: String,
    /// Current state of the task
    pub state: RegistryTaskState,
    /// ID of the agent currently assigned to this task (if any)
    pub assigned_to: Option<String>,
    /// Timestamp when the task was claimed
    pub claimed_at: Option<DateTime<Utc>>,
    /// Timestamp when the task lease expires
    pub lease_expires_at: Option<DateTime<Utc>>,
    /// Timestamp when the task was completed
    pub completed_at: Option<DateTime<Utc>>,
    /// Number of times this task has been attempted
    pub attempts: u32,
    /// Timestamp of the last heartbeat from the assigned agent
    pub last_heartbeat_at: Option<DateTime<Utc>>,
}

impl TaskAssignment {
    /// Creates a new task assignment with the given ID and description.
    ///
    /// # Arguments
    ///
    /// * `task_id` - Unique identifier for the task
    /// * `task_description` - Human-readable description
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::task_registry::{TaskAssignment, RegistryTaskState};
    ///
    /// let task = TaskAssignment::new("task-1".to_string(), "Build the project".to_string());
    /// assert_eq!(task.state, RegistryTaskState::Available);
    /// ```
    pub fn new(task_id: String, task_description: String) -> Self {
        TaskAssignment {
            task_id,
            task_description,
            state: RegistryTaskState::Available,
            assigned_to: None,
            claimed_at: None,
            lease_expires_at: None,
            completed_at: None,
            attempts: 0,
            last_heartbeat_at: None,
        }
    }

    /// Checks if the task's lease has expired.
    ///
    /// Returns `true` if the task has a lease expiration time and it's in the past.
    pub fn is_lease_expired(&self) -> bool {
        match self.lease_expires_at {
            Some(expires_at) => Utc::now() > expires_at,
            None => false,
        }
    }

    /// Checks if the task was completed before the given threshold.
    ///
    /// Returns `true` if the task is completed and the completion time is
    /// older than the specified duration.
    pub fn is_completed_older_than(&self, threshold: Duration) -> bool {
        if self.state != RegistryTaskState::Completed {
            return false;
        }
        match self.completed_at {
            Some(completed_at) => {
                let age = Utc::now().signed_duration_since(completed_at);
                age > ChronoDuration::from_std(threshold).unwrap_or_else(|_| ChronoDuration::zero())
            }
            None => false,
        }
    }

    /// Records a heartbeat for this task.
    ///
    /// Updates the last heartbeat timestamp to the current time.
    pub fn record_heartbeat(&mut self) {
        self.last_heartbeat_at = Some(Utc::now());
    }

    /// Checks if the heartbeat is stale based on the threshold.
    ///
    /// Returns `true` if the task has a heartbeat and it's older than the threshold.
    pub fn is_heartbeat_stale(&self, threshold: Duration) -> bool {
        match self.last_heartbeat_at {
            Some(heartbeat_at) => {
                let age = Utc::now().signed_duration_since(heartbeat_at);
                age > ChronoDuration::from_std(threshold).unwrap_or_else(|_| ChronoDuration::zero())
            }
            None => false,
        }
    }
}

/// Registry for tracking tasks with assignments, leases, and state transitions.
///
/// The `TaskRegistry` provides a thread-safe mechanism for managing tasks across
/// multiple parallel agents. It uses file-based locking to coordinate access
/// and persists state to disk for recovery after failures.
///
/// # Fields
///
/// * `tasks` - HashMap mapping task IDs to task assignments
/// * `registry_path` - Path to the registry file
///
/// # Thread Safety
///
/// All operations that modify the registry acquire a file lock before making
/// changes. The lock is automatically released when the operation completes.
#[derive(Debug)]
pub struct TaskRegistry {
    /// Internal task storage
    tasks: HashMap<String, TaskAssignment>,
    /// Path to the registry file on disk
    registry_path: PathBuf,
    /// Lock manager for coordinating access
    lock_manager: LockManager,
}

/// Base path for task registry files
const REGISTRY_BASE_PATH: &str = ".state/tasks";
/// Name of the registry file
const REGISTRY_FILE_NAME: &str = "registry.json";
/// Default timeout for lock acquisition (30 seconds)
const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(30);

impl TaskRegistry {
    /// Creates a new task registry or loads it from disk.
    ///
    /// If the registry file exists, it will be loaded. If it doesn't exist,
    /// a new empty registry will be created. The `.state/tasks/` directory
    /// will be created if it doesn't exist.
    ///
    /// # Returns
    ///
    /// A new `TaskRegistry` instance
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The registry directory cannot be created
    /// - The registry file cannot be read
    /// - The registry file contains corrupted JSON
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::task_registry::TaskRegistry;
    ///
    /// let registry = TaskRegistry::new().unwrap();
    /// ```
    pub fn new() -> Result<Self> {
        let registry_dir = PathBuf::from(REGISTRY_BASE_PATH);
        let registry_path = registry_dir.join(REGISTRY_FILE_NAME);

        // Create directory if it doesn't exist
        fs::create_dir_all(&registry_dir).map_err(|e| AutomationError::StateError {
            task: "TaskRegistry".to_string(),
            message: format!("Failed to create registry directory: {}", e),
        })?;

        // Create lock manager
        let lock_file_path = registry_dir.join("registry.lock");
        let lock_manager = LockManager::new(lock_file_path, DEFAULT_LOCK_TIMEOUT);

        // Try to load existing registry or create new one
        let tasks = if registry_path.exists() {
            let content = fs::read_to_string(&registry_path).map_err(|e| {
                AutomationError::StateError {
                    task: "TaskRegistry".to_string(),
                    message: format!("Failed to read registry file: {}", e),
                }
            })?;
            serde_json::from_str(&content).map_err(|e| {
                AutomationError::StateError {
                    task: "TaskRegistry".to_string(),
                    message: format!("Failed to parse registry file: {}", e),
                }
            })?
        } else {
            HashMap::new()
        };

        Ok(TaskRegistry {
            tasks,
            registry_path,
            lock_manager,
        })
    }

    /// Returns the path to the registry file.
    pub fn registry_path(&self) -> &Path {
        &self.registry_path
    }

    /// Persists the registry to disk.
    ///
    /// The registry is serialized to JSON and written atomically to disk.
    /// A lock is acquired before writing to prevent concurrent modifications.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the registry was saved successfully
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The lock cannot be acquired
    /// - The registry cannot be serialized
    /// - The registry file cannot be written
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::task_registry::TaskRegistry;
    ///
    /// let mut registry = TaskRegistry::new().unwrap();
    /// registry.register_task("task-1".to_string(), "Description".to_string()).unwrap();
    /// registry.save().unwrap();
    /// ```
    pub fn save(&self) -> Result<()> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        // Serialize to JSON
        let content = serde_json::to_string_pretty(&self.tasks).map_err(|e| {
            AutomationError::StateError {
                task: "TaskRegistry".to_string(),
                message: format!("Failed to serialize registry: {}", e),
            }
        })?;

        // Write atomically
        atomic_write(&self.registry_path, content.as_bytes())?;

        Ok(())
    }

    /// Loads the registry from disk.
    ///
    /// This method loads the latest state from disk, replacing the in-memory
    /// state. A lock is acquired during loading to prevent concurrent modifications.
    ///
    /// # Returns
    ///
    /// The loaded `TaskRegistry` instance
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The lock cannot be acquired
    /// - The registry file cannot be read
    /// - The registry file contains corrupted JSON
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::task_registry::TaskRegistry;
    ///
    /// let mut registry = TaskRegistry::new().unwrap();
    /// // After some time, reload from disk
    /// registry = TaskRegistry::load().unwrap();
    /// ```
    pub fn load() -> Result<Self> {
        let registry_dir = PathBuf::from(REGISTRY_BASE_PATH);
        let registry_path = registry_dir.join(REGISTRY_FILE_NAME);
        let lock_file_path = registry_dir.join("registry.lock");
        let lock_manager = LockManager::new(lock_file_path, DEFAULT_LOCK_TIMEOUT);

        // Acquire lock
        let _lock_handle = lock_manager.acquire_lock()?;

        // Load tasks
        let tasks = if registry_path.exists() {
            let content = fs::read_to_string(&registry_path).map_err(|e| {
                AutomationError::StateError {
                    task: "TaskRegistry".to_string(),
                    message: format!("Failed to read registry file: {}", e),
                }
            })?;
            serde_json::from_str(&content).map_err(|e| {
                AutomationError::StateError {
                    task: "TaskRegistry".to_string(),
                    message: format!("Failed to parse registry file: {}", e),
                }
            })?
        } else {
            HashMap::new()
        };

        Ok(TaskRegistry {
            tasks,
            registry_path,
            lock_manager,
        })
    }

    /// Registers a new task in the registry.
    ///
    /// The task will be added with `Available` state, ready to be claimed.
    /// If a task with the same ID already exists, an error will be returned.
    ///
    /// # Arguments
    ///
    /// * `task_id` - Unique identifier for the task
    /// * `description` - Human-readable description of the task
    ///
    /// # Returns
    ///
    /// `Ok(())` if the task was registered successfully
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A task with the same ID already exists
    /// - The lock cannot be acquired
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::task_registry::TaskRegistry;
    ///
    /// let mut registry = TaskRegistry::new().unwrap();
    /// registry.register_task("task-1".to_string(), "Build the project".to_string()).unwrap();
    /// ```
    pub fn register_task(&mut self, task_id: String, description: String) -> Result<()> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        // Check if task already exists
        if self.tasks.contains_key(&task_id) {
            return Err(AutomationError::StateError {
                task: task_id.clone(),
                message: "Task already exists in registry".to_string(),
            });
        }

        // Create new task assignment
        let task = TaskAssignment::new(task_id.clone(), description);
        self.tasks.insert(task_id, task);

        // Persist changes
        self.save_with_lock()?;

        Ok(())
    }

    /// Claims a task for an agent with a lease duration.
    ///
    /// This operation is atomic - if two agents try to claim the same task
    /// simultaneously, only one will succeed. Returns `false` if the task
    /// cannot be claimed (not available, already claimed, or doesn't exist).
    ///
    /// # Arguments
    ///
    /// * `task_id` - ID of the task to claim
    /// * `agent_id` - ID of the agent claiming the task
    /// * `lease_duration` - How long the claim is valid (after which it expires)
    ///
    /// # Returns
    ///
    /// * `true` - Task was claimed successfully
    /// * `false` - Task could not be claimed
    ///
    /// # Errors
    ///
    /// Returns an error if the lock cannot be acquired.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::task_registry::TaskRegistry;
    /// use std::time::Duration;
    ///
    /// let mut registry = TaskRegistry::new().unwrap();
    /// registry.register_task("task-1".to_string(), "Description".to_string()).unwrap();
    ///
    /// if registry.claim_task("task-1", "agent-123", Duration::from_secs(60)).unwrap() {
    ///     println!("Task claimed successfully!");
    /// } else {
    ///     println!("Could not claim task");
    /// }
    /// ```
    pub fn claim_task(&mut self, task_id: &str, agent_id: &str, lease_duration: Duration) -> Result<bool> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        // Get task
        let task = match self.tasks.get_mut(task_id) {
            Some(t) => t,
            None => return Ok(false), // Task doesn't exist
        };

        // Check if task can be claimed
        if !task.state.is_claimable() {
            return Ok(false); // Task is not available
        }

        // Claim the task
        let now = Utc::now();
        task.state = RegistryTaskState::InProgress;
        task.assigned_to = Some(agent_id.to_string());
        task.claimed_at = Some(now);
        task.lease_expires_at = Some(now + ChronoDuration::from_std(lease_duration).unwrap());
        task.last_heartbeat_at = Some(now);
        task.attempts += 1;

        // Persist changes
        self.save_with_lock()?;

        Ok(true)
    }

    /// Renews the lease for a task currently claimed by an agent.
    ///
    /// Extends the lease expiration time by the specified duration.
    /// Returns `false` if the task is not claimed by the specified agent.
    ///
    /// # Arguments
    ///
    /// * `task_id` - ID of the task whose lease to renew
    /// * `agent_id` - ID of the agent that claimed the task
    /// * `lease_duration` - Additional time to add to the lease
    ///
    /// # Returns
    ///
    /// * `true` - Lease was renewed successfully
    /// * `false` - Task is not claimed by the specified agent
    ///
    /// # Errors
    ///
    /// Returns an error if the lock cannot be acquired.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::task_registry::TaskRegistry;
    /// use std::time::Duration;
    ///
    /// let mut registry = TaskRegistry::new().unwrap();
    /// // ... task was claimed ...
    ///
    /// if registry.renew_lease("task-1", "agent-123", Duration::from_secs(60)).unwrap() {
    ///     println!("Lease renewed!");
    /// }
    /// ```
    pub fn renew_lease(&mut self, task_id: &str, agent_id: &str, lease_duration: Duration) -> Result<bool> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        // Get task
        let task = match self.tasks.get_mut(task_id) {
            Some(t) => t,
            None => return Ok(false), // Task doesn't exist
        };

        // Check if task is assigned to the specified agent
        if task.assigned_to.as_ref() != Some(&agent_id.to_string()) {
            return Ok(false); // Not assigned to this agent
        }

        // Renew the lease
        let now = Utc::now();
        task.lease_expires_at = Some(now + ChronoDuration::from_std(lease_duration).unwrap());
        task.last_heartbeat_at = Some(now);

        // Persist changes
        self.save_with_lock()?;

        Ok(true)
    }

    /// Marks a task as completed.
    ///
    /// The task must be in `InProgress` state and assigned to the specified agent.
    /// Once completed, the task cannot be claimed again.
    ///
    /// # Arguments
    ///
    /// * `task_id` - ID of the task to complete
    /// * `agent_id` - ID of the agent that completed the task
    ///
    /// # Returns
    ///
    /// `Ok(())` if the task was marked as completed successfully
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The task doesn't exist
    /// - The task is not assigned to the specified agent
    /// - The lock cannot be acquired
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::task_registry::TaskRegistry;
    ///
    /// let mut registry = TaskRegistry::new().unwrap();
    /// // ... task was claimed and work was done ...
    ///
    /// registry.complete_task("task-1", "agent-123").unwrap();
    /// ```
    pub fn complete_task(&mut self, task_id: &str, agent_id: &str) -> Result<()> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        // Get task
        let task = self.tasks.get_mut(task_id).ok_or_else(|| {
            AutomationError::StateError {
                task: task_id.to_string(),
                message: "Task not found in registry".to_string(),
            }
        })?;

        // Check if task is assigned to the specified agent
        if task.assigned_to.as_ref() != Some(&agent_id.to_string()) {
            return Err(AutomationError::StateError {
                task: task_id.to_string(),
                message: format!("Task is not assigned to agent '{}'", agent_id),
            });
        }

        // Mark as completed
        task.state = RegistryTaskState::Completed;
        task.completed_at = Some(Utc::now());

        // Persist changes
        self.save_with_lock()?;

        Ok(())
    }

    /// Marks a task as failed with a reason.
    ///
    /// The task must be in `InProgress` state and assigned to the specified agent.
    /// Once failed, the task cannot be claimed again.
    ///
    /// # Arguments
    ///
    /// * `task_id` - ID of the task to fail
    /// * `agent_id` - ID of the agent that was working on the task
    /// * `reason` - Human-readable reason for the failure
    ///
    /// # Returns
    ///
    /// `Ok(++)` if the task was marked as failed successfully
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The task doesn't exist
    /// - The task is not assigned to the specified agent
    /// - The lock cannot be acquired
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::task_registry::TaskRegistry;
    ///
    /// let mut registry = TaskRegistry::new().unwrap();
    /// // ... task was claimed but failed ...
    ///
    /// registry.fail_task("task-1", "agent-123", "Timeout error".to_string()).unwrap();
    /// ```
    pub fn fail_task(&mut self, task_id: &str, agent_id: &str, reason: String) -> Result<()> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        // Get task
        let task = self.tasks.get_mut(task_id).ok_or_else(|| {
            AutomationError::StateError {
                task: task_id.to_string(),
                message: "Task not found in registry".to_string(),
            }
        })?;

        // Check if task is assigned to the specified agent
        if task.assigned_to.as_ref() != Some(&agent_id.to_string()) {
            return Err(AutomationError::StateError {
                task: task_id.to_string(),
                message: format!("Task is not assigned to agent '{}'", agent_id),
            });
        }

        // Mark as failed (store reason in description for now)
        task.state = RegistryTaskState::Failed;
        task.completed_at = Some(Utc::now());
        task.task_description = format!("{} [FAILED: {}]", task.task_description, reason);

        // Persist changes
        self.save_with_lock()?;

        Ok(())
    }

    /// Gets a list of available tasks up to the specified limit.
    ///
    /// Available tasks are those in `Available` state that can be claimed.
    /// The returned list is limited to the specified number of tasks.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of tasks to return
    ///
    /// # Returns
    ///
    /// A vector of available `TaskAssignment` instances
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::task_registry::TaskRegistry;
    ///
    /// let registry = TaskRegistry::new().unwrap();
    /// let available = registry.get_available_tasks(10);
    /// println!("Found {} available tasks", available.len());
    /// ```
    pub fn get_available_tasks(&self, limit: usize) -> Vec<TaskAssignment> {
        self.tasks
            .values()
            .filter(|task| task.state.is_claimable())
            .take(limit)
            .cloned()
            .collect()
    }

    /// Gets a list of tasks currently in progress.
    ///
    /// Returns all tasks that are currently being worked on by agents.
    ///
    /// # Returns
    ///
    /// A vector of in-progress `TaskAssignment` instances
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::task_registry::TaskRegistry;
    ///
    /// let registry = TaskRegistry::new().unwrap();
    /// let in_progress = registry.get_tasks_in_progress();
    /// println!("Found {} tasks in progress", in_progress.len());
    /// ```
    pub fn get_tasks_in_progress(&self) -> Vec<TaskAssignment> {
        self.tasks
            .values()
            .filter(|task| task.state.is_in_progress())
            .cloned()
            .collect()
    }

    /// Gets a specific task by ID.
    ///
    /// Returns `None` if the task doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `task_id` - ID of the task to retrieve
    ///
    /// # Returns
    ///
    /// * `Some(&TaskAssignment)` - Task was found
    /// * `None` - Task doesn't exist
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::task_registry::TaskRegistry;
    ///
    /// let registry = TaskRegistry::new().unwrap();
    /// if let Some(task) = registry.get_task("task-1") {
    ///     println!("Task: {}", task.task_description);
    /// }
    /// ```
    pub fn get_task(&self, task_id: &str) -> Option<&TaskAssignment> {
        self.tasks.get(task_id)
    }

    /// Returns tasks with expired leases back to Available state.
    ///
    /// Tasks that are in `InProgress` state but have an expired lease will
    /// be returned to `Available` state so they can be claimed by another agent.
    /// The attempt count is preserved for tracking purposes.
    ///
    /// # Arguments
    ///
    /// * `stale_threshold` - Minimum lease age before considering a task stale
    ///
    /// # Returns
    ///
    /// A list of task IDs that were abandoned
    ///
    /// # Errors
    ///
    /// Returns an error if the lock cannot be acquired.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::task_registry::TaskRegistry;
    /// use std::time::Duration;
    ///
    /// let mut registry = TaskRegistry::new().unwrap();
    ///
    /// // Abandon tasks with leases expired more than 5 minutes ago
    /// let abandoned = registry.abandon_stale_tasks(Duration::from_secs(300)).unwrap();
    /// println!("Abandoned {} tasks", abandoned.len());
    /// ```
    pub fn abandon_stale_tasks(&mut self, stale_threshold: Duration) -> Result<Vec<String>> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        let now = Utc::now();
        let threshold_chrono = ChronoDuration::from_std(stale_threshold).unwrap_or_else(|_| ChronoDuration::zero());
        let mut abandoned_tasks = Vec::new();

        for (task_id, task) in self.tasks.iter_mut() {
            // Check if task is in progress and lease is expired
            if task.state == RegistryTaskState::InProgress && task.is_lease_expired() {
                // Additional check: lease must have expired at least stale_threshold ago
                if let Some(expires_at) = task.lease_expires_at {
                    if now.signed_duration_since(expires_at) >= threshold_chrono {
                        task.state = RegistryTaskState::Available;
                        task.assigned_to = None;
                        task.claimed_at = None;
                        task.lease_expires_at = None;
                        task.last_heartbeat_at = None;
                        abandoned_tasks.push(task_id.clone());
                    }
                }
            }
        }

        // Persist changes if any tasks were abandoned
        if !abandoned_tasks.is_empty() {
            self.save_with_lock()?;
        }

        Ok(abandoned_tasks)
    }

    /// Removes completed tasks that are older than the specified threshold.
    ///
    /// Tasks in `Completed` state that were completed before the threshold
    /// will be permanently removed from the registry. Failed tasks are
    /// preserved for analysis.
    ///
    /// # Arguments
    ///
    /// * `older_than` - Age threshold - tasks completed before this duration ago will be removed
    ///
    /// # Returns
    ///
    /// A list of task IDs that were removed
    ///
    /// # Errors
    ///
    /// Returns an error if the lock cannot be acquired.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::task_registry::TaskRegistry;
    /// use std::time::Duration;
    ///
    /// let mut registry = TaskRegistry::new().unwrap();
    ///
    /// // Remove tasks completed more than 24 hours ago
    /// let removed = registry.cleanup_completed_tasks(Duration::from_secs(86400)).unwrap();
    /// println!("Removed {} old completed tasks", removed.len());
    /// ```
    pub fn cleanup_completed_tasks(&mut self, older_than: Duration) -> Result<Vec<String>> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        let mut removed_tasks = Vec::new();

        self.tasks.retain(|task_id, task| {
            if task.is_completed_older_than(older_than) {
                removed_tasks.push(task_id.clone());
                false // Remove this task
            } else {
                true // Keep this task
            }
        });

        // Persist changes if any tasks were removed
        if !removed_tasks.is_empty() {
            self.save_with_lock()?;
        }

        Ok(removed_tasks)
    }

    /// Records a heartbeat for a task currently claimed by an agent.
    ///
    /// Updates the last heartbeat timestamp. Returns `false` if the task
    /// is not claimed by the specified agent.
    ///
    /// # Arguments
    ///
    /// * `task_id` - ID of the task
    /// * `agent_id` - ID of the agent claiming the task
    ///
    /// # Returns
    ///
    /// * `true` - Heartbeat was recorded
    /// * `false` - Task is not claimed by the specified agent
    ///
    /// # Errors
    ///
    /// Returns an error if the lock cannot be acquired.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::task_registry::TaskRegistry;
    ///
    /// let mut registry = TaskRegistry::new().unwrap();
    ///
    /// if registry.record_heartbeat("task-1", "agent-123").unwrap() {
    ///     println!("Heartbeat recorded");
    /// }
    /// ```
    pub fn record_heartbeat(&mut self, task_id: &str, agent_id: &str) -> Result<bool> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        // Get task
        let task = match self.tasks.get_mut(task_id) {
            Some(t) => t,
            None => return Ok(false), // Task doesn't exist
        };

        // Check if task is assigned to the specified agent
        if task.assigned_to.as_ref() != Some(&agent_id.to_string()) {
            return Ok(false); // Not assigned to this agent
        }

        // Record heartbeat
        task.record_heartbeat();

        // Persist changes
        self.save_with_lock()?;

        Ok(true)
    }

    /// Gets the total number of tasks in the registry.
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Gets a list of all task IDs.
    pub fn all_task_ids(&self) -> Vec<String> {
        self.tasks.keys().cloned().collect()
    }

    /// Saves the registry to disk without acquiring a lock.
    ///
    /// This is an internal method that should only be called when a lock
    /// is already held by the caller.
    fn save_with_lock(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.tasks).map_err(|e| {
            AutomationError::StateError {
                task: "TaskRegistry".to_string(),
                message: format!("Failed to serialize registry: {}", e),
            }
        })?;

        atomic_write(&self.registry_path, content.as_bytes())?;

        Ok(())
    }
}

/// Writes data to a file atomically.
///
/// This function writes data to a temporary file and then renames it
/// to the target path, ensuring atomic writes.
fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    // Create temporary file path
    let temp_path = path.with_extension("tmp");

    // Write to temporary file
    fs::write(&temp_path, data).map_err(|e| AutomationError::FileSystem(e))?;

    // Rename to target path (atomic on most filesystems)
    fs::rename(&temp_path, path).map_err(|e| AutomationError::FileSystem(e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::time::Duration;

    /// Helper to create a temporary registry for testing
    fn create_test_registry() -> (TempDir, TaskRegistry) {
        let temp_dir = TempDir::new().unwrap();
        let registry_dir = temp_dir.path().join("tasks");
        let registry_path = registry_dir.join(REGISTRY_FILE_NAME);
        let lock_file_path = registry_dir.join("registry.lock");

        fs::create_dir_all(&registry_dir).unwrap();

        let lock_manager = LockManager::new(lock_file_path, DEFAULT_LOCK_TIMEOUT);

        let registry = TaskRegistry {
            tasks: HashMap::new(),
            registry_path,
            lock_manager,
        };

        (temp_dir, registry)
    }

    #[test]
    fn test_task_state_is_claimable() {
        assert!(RegistryTaskState::Available.is_claimable());
        assert!(!RegistryTaskState::InProgress.is_claimable());
        assert!(!RegistryTaskState::Completed.is_claimable());
        assert!(!RegistryTaskState::Failed.is_claimable());
    }

    #[test]
    fn test_task_state_is_in_progress() {
        assert!(!RegistryTaskState::Available.is_in_progress());
        assert!(RegistryTaskState::InProgress.is_in_progress());
        assert!(!RegistryTaskState::Completed.is_in_progress());
        assert!(!RegistryTaskState::Failed.is_in_progress());
    }

    #[test]
    fn test_task_state_is_terminal() {
        assert!(!RegistryTaskState::Available.is_terminal());
        assert!(!RegistryTaskState::InProgress.is_terminal());
        assert!(RegistryTaskState::Completed.is_terminal());
        assert!(RegistryTaskState::Failed.is_terminal());
    }

    #[test]
    fn test_task_assignment_new() {
        let task = TaskAssignment::new("task-1".to_string(), "Test task".to_string());

        assert_eq!(task.task_id, "task-1");
        assert_eq!(task.task_description, "Test task");
        assert_eq!(task.state, RegistryTaskState::Available);
        assert!(task.assigned_to.is_none());
        assert!(task.claimed_at.is_none());
        assert!(task.lease_expires_at.is_none());
        assert!(task.completed_at.is_none());
        assert_eq!(task.attempts, 0);
        assert!(task.last_heartbeat_at.is_none());
    }

    #[test]
    fn test_task_assignment_is_lease_expired() {
        let mut task = TaskAssignment::new("task-1".to_string(), "Test task".to_string());

        // No lease set - not expired
        assert!(!task.is_lease_expired());

        // Set expired lease
        task.lease_expires_at = Some(Utc::now() - ChronoDuration::seconds(60));
        assert!(task.is_lease_expired());

        // Set future lease
        task.lease_expires_at = Some(Utc::now() + ChronoDuration::seconds(60));
        assert!(!task.is_lease_expired());
    }

    #[test]
    fn test_task_assignment_is_completed_older_than() {
        let mut task = TaskAssignment::new("task-1".to_string(), "Test task".to_string());

        // Not completed - not old enough
        assert!(!task.is_completed_older_than(Duration::from_secs(60)));

        // Completed recently - not old enough
        task.state = RegistryTaskState::Completed;
        task.completed_at = Some(Utc::now() - ChronoDuration::seconds(30));
        assert!(!task.is_completed_older_than(Duration::from_secs(60)));

        // Completed long ago - old enough
        task.completed_at = Some(Utc::now() - ChronoDuration::seconds(120));
        assert!(task.is_completed_older_than(Duration::from_secs(60)));
    }

    #[test]
    fn test_task_assignment_record_heartbeat() {
        let mut task = TaskAssignment::new("task-1".to_string(), "Test task".to_string());

        assert!(task.last_heartbeat_at.is_none());

        task.record_heartbeat();

        assert!(task.last_heartbeat_at.is_some());
        let heartbeat = task.last_heartbeat_at.unwrap();
        let age = Utc::now().signed_duration_since(heartbeat);
        assert!(age.num_seconds() < 5); // Should be very recent
    }

    #[test]
    fn test_task_assignment_is_heartbeat_stale() {
        let mut task = TaskAssignment::new("task-1".to_string(), "Test task".to_string());

        // No heartbeat - not stale
        assert!(!task.is_heartbeat_stale(Duration::from_secs(60)));

        // Recent heartbeat - not stale
        task.last_heartbeat_at = Some(Utc::now() - ChronoDuration::seconds(30));
        assert!(!task.is_heartbeat_stale(Duration::from_secs(60)));

        // Old heartbeat - stale
        task.last_heartbeat_at = Some(Utc::now() - ChronoDuration::seconds(120));
        assert!(task.is_heartbeat_stale(Duration::from_secs(60)));
    }

    #[test]
    fn test_registry_register_task() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Test task".to_string()).unwrap();

        assert_eq!(registry.task_count(), 1);
        assert_eq!(registry.all_task_ids(), vec!["task-1"]);

        let task = registry.get_task("task-1");
        assert!(task.is_some());
        assert_eq!(task.unwrap().task_description, "Test task");
    }

    #[test]
    fn test_registry_register_duplicate_task() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Test task".to_string()).unwrap();

        let result = registry.register_task("task-1".to_string(), "Duplicate task".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_claim_task() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Test task".to_string()).unwrap();

        // Claim task
        let claimed = registry.claim_task("task-1", "agent-1", Duration::from_secs(60)).unwrap();
        assert!(claimed);

        // Check task state
        let task = registry.get_task("task-1").unwrap();
        assert_eq!(task.state, RegistryTaskState::InProgress);
        assert_eq!(task.assigned_to, Some("agent-1".to_string()));
        assert_eq!(task.attempts, 1);
        assert!(task.claimed_at.is_some());
        assert!(task.lease_expires_at.is_some());
        assert!(task.last_heartbeat_at.is_some());
    }

    #[test]
    fn test_registry_claim_nonexistent_task() {
        let (_temp_dir, mut registry) = create_test_registry();

        let claimed = registry.claim_task("nonexistent", "agent-1", Duration::from_secs(60)).unwrap();
        assert!(!claimed);
    }

    #[test]
    fn test_registry_claim_already_claimed_task() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Test task".to_string()).unwrap();

        // Claim once
        registry.claim_task("task-1", "agent-1", Duration::from_secs(60)).unwrap();

        // Try to claim again - should fail
        let claimed = registry.claim_task("task-1", "agent-2", Duration::from_secs(60)).unwrap();
        assert!(!claimed);
    }

    #[test]
    fn test_registry_renew_lease() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Test task".to_string()).unwrap();
        registry.claim_task("task-1", "agent-1", Duration::from_secs(60)).unwrap();

        // Get original lease expiration
        let original_expires = registry.get_task("task-1").unwrap().lease_expires_at.unwrap();

        // Renew lease
        let renewed = registry.renew_lease("task-1", "agent-1", Duration::from_secs(120)).unwrap();
        assert!(renewed);

        // Check lease was extended
        let new_expires = registry.get_task("task-1").unwrap().lease_expires_at.unwrap();
        assert!(new_expires > original_expires);
    }

    #[test]
    fn test_registry_renew_lease_wrong_agent() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Test task".to_string()).unwrap();
        registry.claim_task("task-1", "agent-1", Duration::from_secs(60)).unwrap();

        // Try to renew with wrong agent
        let renewed = registry.renew_lease("task-1", "agent-2", Duration::from_secs(60)).unwrap();
        assert!(!renewed);
    }

    #[test]
    fn test_registry_complete_task() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Test task".to_string()).unwrap();
        registry.claim_task("task-1", "agent-1", Duration::from_secs(60)).unwrap();

        // Complete task
        registry.complete_task("task-1", "agent-1").unwrap();

        // Check task state
        let task = registry.get_task("task-1").unwrap();
        assert_eq!(task.state, RegistryTaskState::Completed);
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn test_registry_complete_task_wrong_agent() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Test task".to_string()).unwrap();
        registry.claim_task("task-1", "agent-1", Duration::from_secs(60)).unwrap();

        // Try to complete with wrong agent
        let result = registry.complete_task("task-1", "agent-2");
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_fail_task() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Test task".to_string()).unwrap();
        registry.claim_task("task-1", "agent-1", Duration::from_secs(60)).unwrap();

        // Fail task
        registry.fail_task("task-1", "agent-1", "Timeout error".to_string()).unwrap();

        // Check task state
        let task = registry.get_task("task-1").unwrap();
        assert_eq!(task.state, RegistryTaskState::Failed);
        assert!(task.completed_at.is_some());
        assert!(task.task_description.contains("[FAILED: Timeout error]"));
    }

    #[test]
    fn test_registry_fail_task_wrong_agent() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Test task".to_string()).unwrap();
        registry.claim_task("task-1", "agent-1", Duration::from_secs(60)).unwrap();

        // Try to fail with wrong agent
        let result = registry.fail_task("task-1", "agent-2", "Error".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_get_available_tasks() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Task 1".to_string()).unwrap();
        registry.register_task("task-2".to_string(), "Task 2".to_string()).unwrap();
        registry.register_task("task-3".to_string(), "Task 3".to_string()).unwrap();

        let available = registry.get_available_tasks(2);
        assert_eq!(available.len(), 2);
        // Check that we got 2 tasks from the 3 available
        let ids: Vec<_> = available.iter().map(|t| t.task_id.as_str()).collect();
        assert!(ids.len() == 2);
        assert!(ids.contains(&"task-1") || ids.contains(&"task-2") || ids.contains(&"task-3"));
    }

    #[test]
    fn test_registry_get_available_tasks_excludes_claimed() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Task 1".to_string()).unwrap();
        registry.register_task("task-2".to_string(), "Task 2".to_string()).unwrap();
        registry.claim_task("task-1", "agent-1", Duration::from_secs(60)).unwrap();

        let available = registry.get_available_tasks(10);
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].task_id, "task-2");
    }

    #[test]
    fn test_registry_get_tasks_in_progress() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Task 1".to_string()).unwrap();
        registry.register_task("task-2".to_string(), "Task 2".to_string()).unwrap();
        registry.claim_task("task-1", "agent-1", Duration::from_secs(60)).unwrap();

        let in_progress = registry.get_tasks_in_progress();
        assert_eq!(in_progress.len(), 1);
        assert_eq!(in_progress[0].task_id, "task-1");
    }

    #[test]
    fn test_registry_get_task() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Test task".to_string()).unwrap();

        let task = registry.get_task("task-1");
        assert!(task.is_some());
        assert_eq!(task.unwrap().task_description, "Test task");

        let nonexistent = registry.get_task("nonexistent");
        assert!(nonexistent.is_none());
    }

    #[test]
    fn test_registry_abandon_stale_tasks() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Task 1".to_string()).unwrap();
        registry.register_task("task-2".to_string(), "Task 2".to_string()).unwrap();
        registry.claim_task("task-1", "agent-1", Duration::from_secs(1)).unwrap();
        registry.claim_task("task-2", "agent-2", Duration::from_secs(1)).unwrap();

        // Wait for lease to expire
        std::thread::sleep(Duration::from_millis(1100));

        // Abandon stale tasks
        let abandoned = registry.abandon_stale_tasks(Duration::from_secs(0)).unwrap();
        assert_eq!(abandoned.len(), 2);

        // Check tasks are back to available
        let available = registry.get_available_tasks(10);
        assert_eq!(available.len(), 2);
    }

    #[test]
    fn test_registry_abandon_stale_tasks_with_threshold() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Task 1".to_string()).unwrap();
        registry.claim_task("task-1", "agent-1", Duration::from_secs(1)).unwrap();

        // Wait for lease to expire but not long enough for threshold
        std::thread::sleep(Duration::from_millis(1100));

        // Try to abandon with high threshold - should not abandon
        let abandoned = registry.abandon_stale_tasks(Duration::from_secs(60)).unwrap();
        assert_eq!(abandoned.len(), 0);

        // Task should still be in progress
        let in_progress = registry.get_tasks_in_progress();
        assert_eq!(in_progress.len(), 1);
    }

    #[test]
    fn test_registry_cleanup_completed_tasks() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Task 1".to_string()).unwrap();
        registry.register_task("task-2".to_string(), "Task 2".to_string()).unwrap();
        registry.register_task("task-3".to_string(), "Task 3".to_string()).unwrap();

        registry.claim_task("task-1", "agent-1", Duration::from_secs(60)).unwrap();
        registry.complete_task("task-1", "agent-1").unwrap();

        registry.claim_task("task-2", "agent-2", Duration::from_secs(60)).unwrap();
        registry.complete_task("task-2", "agent-2").unwrap();

        // Modify completion time of task-1 to be old
        let task1 = registry.tasks.get_mut("task-1").unwrap();
        task1.completed_at = Some(Utc::now() - ChronoDuration::seconds(120));

        // Cleanup old completed tasks
        let removed = registry.cleanup_completed_tasks(Duration::from_secs(60)).unwrap();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0], "task-1");

        // Only task-2 and task-3 should remain
        assert_eq!(registry.task_count(), 2);
    }

    #[test]
    fn test_registry_cleanup_completed_tasks_preserves_failed() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Task 1".to_string()).unwrap();
        registry.register_task("task-2".to_string(), "Task 2".to_string()).unwrap();

        registry.claim_task("task-1", "agent-1", Duration::from_secs(60)).unwrap();
        registry.complete_task("task-1", "agent-1").unwrap();

        registry.claim_task("task-2", "agent-2", Duration::from_secs(60)).unwrap();
        registry.fail_task("task-2", "agent-2", "Error".to_string()).unwrap();

        // Make both old
        for task in registry.tasks.values_mut() {
            task.completed_at = Some(Utc::now() - ChronoDuration::seconds(120));
        }

        // Cleanup - should only remove completed, not failed
        let removed = registry.cleanup_completed_tasks(Duration::from_secs(60)).unwrap();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0], "task-1");

        // Failed task should still exist
        assert_eq!(registry.task_count(), 1);
        assert_eq!(registry.get_task("task-2").unwrap().state, RegistryTaskState::Failed);
    }

    #[test]
    fn test_registry_record_heartbeat() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Task 1".to_string()).unwrap();
        registry.claim_task("task-1", "agent-1", Duration::from_secs(60)).unwrap();

        // Record heartbeat
        let recorded = registry.record_heartbeat("task-1", "agent-1").unwrap();
        assert!(recorded);

        let task = registry.get_task("task-1").unwrap();
        assert!(task.last_heartbeat_at.is_some());
    }

    #[test]
    fn test_registry_record_heartbeat_wrong_agent() {
        let (_temp_dir, mut registry) = create_test_registry();

        registry.register_task("task-1".to_string(), "Task 1".to_string()).unwrap();
        registry.claim_task("task-1", "agent-1", Duration::from_secs(60)).unwrap();

        let recorded = registry.record_heartbeat("task-1", "agent-2").unwrap();
        assert!(!recorded);
    }

    #[test]
    fn test_registry_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let registry_dir = temp_dir.path().join("tasks");
        let registry_path = registry_dir.join(REGISTRY_FILE_NAME);
        let lock_file_path = registry_dir.join("registry.lock");

        fs::create_dir_all(&registry_dir).unwrap();

        // Create and populate first registry
        {
            let lock_manager = LockManager::new(lock_file_path.clone(), DEFAULT_LOCK_TIMEOUT);
            let mut registry = TaskRegistry {
                tasks: HashMap::new(),
                registry_path: registry_path.clone(),
                lock_manager,
            };

            registry.register_task("task-1".to_string(), "Task 1".to_string()).unwrap();
            registry.register_task("task-2".to_string(), "Task 2".to_string()).unwrap();
            registry.save().unwrap();
        }

        // Load in new registry instance
        {
            let lock_manager = LockManager::new(lock_file_path, DEFAULT_LOCK_TIMEOUT);
            let _registry = TaskRegistry {
                tasks: HashMap::new(),
                registry_path: registry_path.clone(),
                lock_manager,
            };

            // Verify file exists
            assert!(registry_path.exists());

            // Load tasks manually from file
            let content = fs::read_to_string(&registry_path).unwrap();
            let loaded_tasks: HashMap<String, TaskAssignment> = serde_json::from_str(&content).unwrap();

            assert_eq!(loaded_tasks.len(), 2);
            assert!(loaded_tasks.contains_key("task-1"));
            assert!(loaded_tasks.contains_key("task-2"));
        }
    }

    #[test]
    fn test_registry_task_count_and_all_ids() {
        let (_temp_dir, mut registry) = create_test_registry();

        assert_eq!(registry.task_count(), 0);
        assert_eq!(registry.all_task_ids().len(), 0);

        registry.register_task("task-1".to_string(), "Task 1".to_string()).unwrap();
        registry.register_task("task-2".to_string(), "Task 2".to_string()).unwrap();

        assert_eq!(registry.task_count(), 2);
        let ids = registry.all_task_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"task-1".to_string()));
        assert!(ids.contains(&"task-2".to_string()));
    }

    #[test]
    fn test_task_state_default() {
        assert_eq!(RegistryTaskState::default(), RegistryTaskState::Available);
    }

    #[test]
    fn test_task_assignment_serialization() {
        let task = TaskAssignment::new("task-1".to_string(), "Test task".to_string());
        let serialized = serde_json::to_string(&task).unwrap();
        let deserialized: TaskAssignment = serde_json::from_str(&serialized).unwrap();

        assert_eq!(task.task_id, deserialized.task_id);
        assert_eq!(task.task_description, deserialized.task_description);
        assert_eq!(task.state, deserialized.state);
    }

    #[test]
    fn test_task_state_serialization() {
        let states = vec![RegistryTaskState::Available, RegistryTaskState::InProgress, RegistryTaskState::Completed, RegistryTaskState::Failed];

        for state in states {
            let serialized = serde_json::to_string(&state).unwrap();
            let deserialized: RegistryTaskState = serde_json::from_str(&serialized).unwrap();
            assert_eq!(state, deserialized);
        }
    }
}

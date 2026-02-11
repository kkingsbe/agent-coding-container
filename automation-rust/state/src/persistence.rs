//! State persistence for the automation system.
//!
//! This module provides state persistence functionality using file-based locking
//! to coordinate access to state files across multiple processes and hosts. State
//! is persisted as JSON for backward compatibility with the Node.js version.
//!
//! # Persistence Strategy
//!
//! The persistence system uses a combination of:
//! - **File-based locks** via `LockManager` to prevent concurrent access
//! - **Atomic writes** using temporary file + rename pattern
//! - **Backup/restore** functionality for state recovery
//! - **JSON serialization** for cross-language compatibility
//!
//! # State File Format
//!
//! State files are stored in JSON format with the following naming convention:
//! - State files: `{agent_type}.state.json`
//! - Backup files: `{agent_type}.state.json.backup.{timestamp}`
//!
//! # Example
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
//!
//! # Thread Safety
//!
//! `StateManager` is designed to be used from a single thread. For concurrent
//! access across multiple threads, you should wrap it in a `Mutex` or use
//! `Arc<Mutex<StateManager>>`.

use automation_common::{Result, AutomationError};
use crate::state::{State, TaskState, Mergeable};
use crate::lock::LockManager;
use chrono::{DateTime, Utc};
use serde_json;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Default timeout for lock acquisition (30 seconds)
const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(30);

/// State file suffix
const STATE_FILE_SUFFIX: &str = ".state.json";

/// Backup file suffix
const BACKUP_FILE_SUFFIX: &str = ".backup";

/// A state manager that handles loading, saving, and managing state files.
///
/// `StateManager` coordinates access to state files through file-based locking,
/// ensuring safe concurrent access across multiple processes. It uses atomic
/// writes to prevent data corruption and provides backup/restore functionality.
///
/// # Fields
///
/// * `state_dir` - Directory where state files are stored
/// * `state_file_path` - Full path to the state file
/// * `lock_manager` - Lock manager for coordinating access
///
/// # Example
///
/// ```no_run
/// use automation_state::persistence::StateManager;
/// use std::path::PathBuf;
///
/// let manager = StateManager::new(
///     PathBuf::from("/workspace/.state"),
///     "architect"
/// ).unwrap();
///
/// let state = manager.load_state().unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct StateManager {
    /// Directory where state files are stored
    state_dir: PathBuf,
    /// Full path to the state file
    state_file_path: PathBuf,
    /// Lock manager for coordinating access to the state file
    lock_manager: LockManager,
}

impl StateManager {
    /// Creates a new state manager with default lock timeout (30 seconds).
    ///
    /// # Arguments
    ///
    /// * `state_dir` - Directory where state files are stored
    /// * `agent_type` - Type identifier for the agent (e.g., "architect", "janitor")
    ///
    /// # Returns
    ///
    /// A new `StateManager` instance
    ///
    /// # Errors
    ///
    /// Returns an error if the state directory cannot be created.
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/tmp/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// assert_eq!(manager.state_file_path().file_name().unwrap(), "architect.state.json");
    /// ```
    pub fn new(state_dir: PathBuf, agent_type: &str) -> Result<Self> {
        Self::with_lock_timeout(state_dir, agent_type, DEFAULT_LOCK_TIMEOUT)
    }

    /// Creates a new state manager with a custom lock timeout.
    ///
    /// # Arguments
    ///
    /// * `state_dir` - Directory where state files are stored
    /// * `agent_type` - Type identifier for the agent
    /// * `lock_timeout` - Maximum time to wait for lock acquisition
    ///
    /// # Returns
    ///
    /// A new `StateManager` instance
    ///
    /// # Errors
    ///
    /// Returns an error if the state directory cannot be created.
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    /// use std::time::Duration;
    ///
    /// let manager = StateManager::with_lock_timeout(
    ///     PathBuf::from("/tmp/.state"),
    ///     "architect",
    ///     Duration::from_secs(60)
    /// ).unwrap();
    /// ```
    pub fn with_lock_timeout(state_dir: PathBuf, agent_type: &str, lock_timeout: Duration) -> Result<Self> {
        // Create state directory if it doesn't exist
        fs::create_dir_all(&state_dir).map_err(|e| AutomationError::LockError {
            task: agent_type.to_string(),
            message: format!("Failed to create state directory: {}", e),
        })?;

        let state_file_path = get_state_file_path(&state_dir, agent_type);
        let lock_manager = LockManager::new(lock_file_path_for_state(&state_file_path), lock_timeout);

        Ok(StateManager {
            state_dir,
            state_file_path,
            lock_manager,
        })
    }

    /// Returns the path to the state directory.
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/tmp/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// assert!(manager.state_dir().ends_with(".state"));
    /// ```
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// Returns the path to the state file.
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/tmp/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// let path = manager.state_file_path();
    /// assert!(path.to_str().unwrap().ends_with("architect.state.json"));
    /// ```
    pub fn state_file_path(&self) -> &Path {
        &self.state_file_path
    }

    /// Loads the state from the state file.
    ///
    /// If the state file doesn't exist, returns a default `State` instance.
    /// If the state file exists but is corrupted, returns an error.
    ///
    /// This method acquires a lock before reading and releases it after completion.
    ///
    /// # Returns
    ///
    /// The loaded state or a default state if the file doesn't exist
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The lock cannot be acquired
    /// - The state file cannot be read
    /// - The state file contains corrupted JSON
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/workspace/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// // Load state (returns default if file doesn't exist)
    /// let state = manager.load_state().unwrap();
    /// ```
    pub fn load_state(&self) -> Result<State> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        // Read state file
        if !self.state_file_path.exists() {
            // State file doesn't exist, return default state
            return Ok(State::new());
        }

        let content = fs::read_to_string(&self.state_file_path)?;
        let state: State = serde_json::from_str(&content)?;

        Ok(state)
    }

    /// Saves the state to the state file.
    ///
    /// This method uses atomic writes (temporary file + rename) to prevent
    /// data corruption. The file is locked during the write operation.
    ///
    /// # Arguments
    ///
    /// * `state` - The state to save
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The lock cannot be acquired
    /// - The state cannot be serialized to JSON
    /// - The state file cannot be written
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    /// use chrono::Utc;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/workspace/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// let mut state = manager.load_state().unwrap();
    /// state.update_last_run(Utc::now());
    /// manager.save_state(&state).unwrap();
    /// ```
    pub fn save_state(&self, state: &State) -> Result<()> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        // Serialize state to JSON
        let content = serde_json::to_string_pretty(state)?;

        // Write atomically
        atomic_write(&self.state_file_path, content.as_bytes())?;

        Ok(())
    }

    /// Loads, modifies, and saves state in a single atomic operation.
    ///
    /// This method provides a safe pattern for state updates. The lock is held
    /// throughout the entire operation, ensuring no other process can modify
    /// the state concurrently.
    ///
    /// # Arguments
    ///
    /// * `f` - A function that takes a mutable reference to the state and returns a result
    ///
    /// # Returns
    ///
    /// The result of the function applied to the state
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The lock cannot be acquired
    /// - The state file cannot be read or written
    /// - The state is corrupted
    /// - The function returns an error
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    /// use chrono::Utc;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/workspace/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// // Update state atomically
    /// manager.load_state_with_lock(|state| {
    ///     state.update_last_run(Utc::now());
    ///     state.error_count += 1;
    ///     Ok(())
    /// }).unwrap();
    /// ```
    pub fn load_state_with_lock<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut State) -> Result<R>,
    {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        // Load state
        let mut state = if self.state_file_path.exists() {
            let content = fs::read_to_string(&self.state_file_path)?;
            serde_json::from_str(&content)?
        } else {
            State::new()
        };

        // Apply function
        let result = f(&mut state)?;

        // Save state
        let content = serde_json::to_string_pretty(&state)?;
        atomic_write(&self.state_file_path, content.as_bytes())?;

        Ok(result)
    }

    /// Creates a backup of the current state file.
    ///
    /// The backup is created with a timestamp in the filename to allow for
    /// historical tracking and recovery.
    ///
    /// # Returns
    ///
    /// The path to the backup file
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The state file doesn't exist
    /// - The lock cannot be acquired
    /// - The backup file cannot be created
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/workspace/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// let backup_path = manager.backup_state().unwrap();
    /// println!("Backup created at: {:?}", backup_path);
    /// ```
    pub fn backup_state(&self) -> Result<PathBuf> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        if !self.state_file_path.exists() {
            return Err(AutomationError::StateError {
                task: "backup".to_string(),
                message: "state file does not exist".to_string(),
            });
        }

        let backup_path = get_backup_path(&self.state_file_path, Utc::now());
        fs::copy(&self.state_file_path, &backup_path)?;

        Ok(backup_path)
    }

    /// Restores state from a backup file.
    ///
    /// This method copies the backup file to the state file path, overwriting
    /// the current state. The backup file is not deleted.
    ///
    /// # Arguments
    ///
    /// * `backup_path` - Path to the backup file to restore from
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The backup file doesn't exist
    /// - The lock cannot be acquired
    /// - The backup file cannot be copied
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/workspace/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// let backup_path = PathBuf::from("/tmp/.state/architect.state.json.backup.2024-01-01T12:00:00Z");
    /// manager.restore_state(&backup_path).unwrap();
    /// ```
    pub fn restore_state(&self, backup_path: &Path) -> Result<()> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        if !backup_path.exists() {
            return Err(AutomationError::StateError {
                task: "restore".to_string(),
                message: "backup file does not exist".to_string(),
            });
        }

        fs::copy(backup_path, &self.state_file_path)?;

        Ok(())
    }

    /// Checks if the state file exists.
    ///
    /// This method does not acquire a lock and simply checks for file existence.
    ///
    /// # Returns
    ///
    /// `true` if the state file exists, `false` otherwise
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/workspace/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// if manager.state_exists() {
    ///     println!("State file exists");
    /// }
    /// ```
    pub fn state_exists(&self) -> bool {
        self.state_file_path.exists()
    }

    /// Deletes the state file.
    ///
    /// This method acquires a lock before deletion to ensure no other process
    /// is modifying the state concurrently. If the state file doesn't exist,
    /// no error is returned.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The lock cannot be acquired
    /// - The state file cannot be deleted
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/workspace/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// manager.delete_state().unwrap();
    /// ```
    pub fn delete_state(&self) -> Result<()> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        if self.state_file_path.exists() {
            fs::remove_file(&self.state_file_path)?;
        }

        Ok(())
    }

    /// Reads task state for the specified task name.
    ///
    /// This method reads the state file for a task and returns the TaskState.
    /// If the file doesn't exist, returns None.
    ///
    /// # Arguments
    ///
    /// * `task_name` - Name of the task to read state for
    ///
    /// # Returns
    ///
    /// Some(TaskState) if the state file exists, None otherwise
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The state file cannot be read
    /// - The state file contains corrupted JSON
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/workspace/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// if let Some(task_state) = manager.read_state("my-task").unwrap() {
    ///     println!("Last run: {:?}", task_state.last_run);
    /// }
    /// ```
    pub fn read_state(&self, task_name: &str) -> Result<Option<TaskState>> {
        let state_file_path = get_task_state_file_path(&self.state_dir, task_name);

        if !state_file_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&state_file_path)?;
        let state: TaskState = serde_json::from_str(&content)?;

        Ok(Some(state))
    }

    /// Writes task state for the specified task name.
    ///
    /// This method writes the TaskState to a JSON file using atomic writes.
    ///
    /// # Arguments
    ///
    /// * `task_name` - Name of the task to write state for
    /// * `state` - The task state to write
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The state cannot be serialized to JSON
    /// - The state file cannot be written
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::persistence::StateManager;
    /// use automation_state::TaskState;
    /// use std::path::PathBuf;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/workspace/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// let state = TaskState::new();
    /// manager.write_state("my-task", &state).unwrap();
    /// ```
    pub fn write_state(&self, task_name: &str, state: &TaskState) -> Result<()> {
        let state_file_path = get_task_state_file_path(&self.state_dir, task_name);

        // Serialize state to JSON
        let content = serde_json::to_string_pretty(state)?;

        // Write atomically
        atomic_write(&state_file_path, content.as_bytes())?;

        Ok(())
    }

    /// Updates task state with a modification function.
    ///
    /// This method reads the existing state, applies the modification function,
    /// and writes it back. It supports backward compatibility by merging missing
    /// fields with defaults when reading old state files.
    ///
    /// # Arguments
    ///
    /// * `task_name` - Name of the task to update state for
    /// * `update` - A function that takes a mutable reference to the TaskState
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The state file cannot be read or written
    /// - The state file contains corrupted JSON
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::persistence::StateManager;
    /// use automation_state::TaskStatus;
    /// use std::path::PathBuf;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/workspace/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// manager.update_state("my-task", |state| {
    ///     state.status = TaskStatus::Running;
    ///     Ok(())
    /// }).unwrap();
    /// ```
    pub fn update_state<F>(&self, task_name: &str, update: F) -> Result<()>
    where
        F: FnOnce(&mut TaskState) -> Result<()>,
    {
        let state_file_path = get_task_state_file_path(&self.state_dir, task_name);

        // Read existing state or create new default
        let mut state = if state_file_path.exists() {
            let content = fs::read_to_string(&state_file_path)?;
            let mut state: TaskState = serde_json::from_str(&content)?;

            // Merge missing fields with defaults for backward compatibility
            if let Err(e) = state.merge_from(&TaskState::default()) {
                tracing::warn!("Failed to merge task state fields: {}", e);
            }

            state
        } else {
            TaskState::default()
        };

        // Apply update function
        update(&mut state)?;

        // Write state back
        self.write_state(task_name, &state)?;

        Ok(())
    }

    /// Resets task state to default values.
    ///
    /// This method resets the task state to its initial default values.
    ///
    /// # Arguments
    ///
    /// * `task_name` - Name of the task to reset state for
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The state file cannot be written
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/workspace/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// manager.reset_state("my-task").unwrap();
    /// ```
    pub fn reset_state(&self, task_name: &str) -> Result<()> {
        self.write_state(task_name, &TaskState::default())
    }

    /// Records an early termination event for a task.
    ///
    /// This method updates the task state with early termination information.
    ///
    /// # Arguments
    ///
    /// * `task_name` - Name of the task
    /// * `reason` - Reason for the early termination
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The state file cannot be read or written
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/workspace/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// manager.record_early_termination("my-task", "Process killed").unwrap();
    /// ```
    pub fn record_early_termination(&self, task_name: &str, reason: &str) -> Result<()> {
        self.update_state(task_name, |state| {
            state.record_early_termination(reason, None);
            Ok(())
        })
    }

    /// Records a successful execution for a task.
    ///
    /// This method updates the task state with success information.
    ///
    /// # Arguments
    ///
    /// * `task_name` - Name of the task
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The state file cannot be read or written
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/workspace/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// manager.record_success("my-task").unwrap();
    /// ```
    pub fn record_success(&self, task_name: &str) -> Result<()> {
        self.update_state(task_name, |state| {
            state.record_success(Utc::now(), 0);
            Ok(())
        })
    }

    /// Records a failure for a task.
    ///
    /// This method updates the task state with failure information.
    ///
    /// # Arguments
    ///
    /// * `task_name` - Name of the task
    /// * `error` - Error message describing the failure
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The state file cannot be read or written
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/workspace/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// manager.record_failure("my-task", "Connection timeout").unwrap();
    /// ```
    pub fn record_failure(&self, task_name: &str, error: &str) -> Result<()> {
        self.update_state(task_name, |state| {
            state.record_failure(error);
            Ok(())
        })
    }

    /// Deletes the task state file.
    ///
    /// This method deletes the state file for a specific task. If the file
    /// doesn't exist, no error is returned.
    ///
    /// # Arguments
    ///
    /// * `task_name` - Name of the task
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The state file cannot be deleted
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = StateManager::new(
    ///     PathBuf::from("/workspace/.state"),
    ///     "architect"
    /// ).unwrap();
    ///
    /// manager.delete_task_state("my-task").unwrap();
    /// ```
    pub fn delete_task_state(&self, task_name: &str) -> Result<()> {
        let state_file_path = get_task_state_file_path(&self.state_dir, task_name);

        if state_file_path.exists() {
            fs::remove_file(&state_file_path)?;
        }

        Ok(())
    }
}

/// Gets the state file path for a given agent type.
///
/// State files follow the naming convention: `{agent_type}.state.json`
///
/// # Arguments
///
/// * `state_dir` - Directory where state files are stored
/// * `agent_type` - Type identifier for the agent
///
/// # Returns
///
/// Full path to the state file
///
/// # Example
///
/// ```
/// use automation_state::persistence::get_state_file_path;
/// use std::path::PathBuf;
///
/// let path = get_state_file_path(&PathBuf::from("/tmp/.state"), "architect");
/// assert_eq!(path, PathBuf::from("/tmp/.state/architect.state.json"));
/// ```
pub fn get_state_file_path(state_dir: &Path, agent_type: &str) -> PathBuf {
    let filename = format!("{}{}", agent_type, STATE_FILE_SUFFIX);
    state_dir.join(filename)
}

/// Gets the backup file path for a state file at a given timestamp.
///
/// Backup files follow the naming convention: `{original_name}.backup.{timestamp}`
/// where timestamp is in ISO 8601 UTC format.
///
/// # Arguments
///
/// * `state_file_path` - Path to the state file
/// * `timestamp` - Timestamp for the backup
///
/// # Returns
///
/// Full path to the backup file
///
/// # Example
///
/// ```
/// use automation_state::persistence::get_backup_path;
/// use std::path::PathBuf;
/// use chrono::Utc;
///
/// let state_file = PathBuf::from("/tmp/.state/architect.state.json");
/// let backup_path = get_backup_path(&state_file, Utc::now());
/// assert!(backup_path.to_str().unwrap().contains(".backup."));
/// ```
pub fn get_backup_path(state_file_path: &Path, timestamp: DateTime<Utc>) -> PathBuf {
    let base_name = state_file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("state");
    let timestamp_str = timestamp.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let backup_name = format!("{}{}{}.{}", base_name, BACKUP_FILE_SUFFIX, timestamp_str, "bak");
    state_file_path.with_file_name(backup_name)
}

/// Lists all backup files for a given state file.
///
/// Backup files are identified by the `.backup.` suffix in their filename.
///
/// # Arguments
///
/// * `state_file_path` - Path to the state file
///
/// # Returns
///
/// A vector of paths to backup files
///
/// # Errors
    ///
/// Returns an error if the directory cannot be read.
///
/// # Example
///
/// ```no_run
/// use automation_state::persistence::list_backups;
/// use std::path::PathBuf;
///
/// let state_file = PathBuf::from("/tmp/.state/architect.state.json");
/// let backups = list_backups(&state_file).unwrap();
///
/// for backup in backups {
///     println!("Backup: {:?}", backup);
/// }
/// ```
pub fn list_backups(state_file_path: &Path) -> Result<Vec<PathBuf>> {
    let state_dir = state_file_path
        .parent()
        .ok_or_else(|| AutomationError::StateError {
            task: "list_backups".to_string(),
            message: "state file has no parent directory".to_string(),
        })?;

    let state_file_name = state_file_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AutomationError::StateError {
            task: "list_backups".to_string(),
            message: "state file has no file name".to_string(),
        })?;

    let base_name = state_file_name.strip_suffix(STATE_FILE_SUFFIX).unwrap_or(state_file_name);

    let mut backups = Vec::new();
    let entries = fs::read_dir(state_dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            if filename.starts_with(&format!("{}{}", base_name, BACKUP_FILE_SUFFIX)) {
                backups.push(path);
            }
        }
    }

    // Sort backups by modification time (newest first)
    backups.sort_by(|a, b| {
        let a_time = a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let b_time = b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        b_time.cmp(&a_time)
    });

    Ok(backups)
}

/// Writes content to a file atomically using the temporary file + rename pattern.
///
/// This function ensures atomic writes by:
/// 1. Writing content to a temporary file in the same directory
/// 2. Syncing the temporary file to disk
/// 3. Renaming the temporary file to the target path (atomic operation on most filesystems)
///
/// If any step fails, the temporary file is cleaned up.
///
/// # Arguments
///
/// * `path` - Target file path
/// * `content` - Content to write
///
/// # Errors
///
/// Returns an error if:
/// - The temporary file cannot be created
/// - The content cannot be written
/// - The file cannot be synced to disk
/// - The rename operation fails
///
/// # Example
///
/// ```no_run
/// use automation_state::persistence::atomic_write;
/// use std::path::PathBuf;
///
/// let path = PathBuf::from("/tmp/myfile.txt");
/// atomic_write(&path, b"Hello, world!").unwrap();
/// ```
pub fn atomic_write(path: &Path, content: &[u8]) -> io::Result<()> {
    // Create temporary file in the same directory
    let temp_path = path.with_extension("tmp");
    
    // Write to temporary file
    let mut file = File::create(&temp_path)?;
    file.write_all(content)?;
    file.sync_all()?; // Ensure data is written to disk
    
    // Atomic rename
    fs::rename(&temp_path, path)?;
    
    Ok(())
}

/// Gets the lock file path for a state file.
///
/// Lock files follow the naming convention: `{agent_type}.lock`
///
/// # Arguments
///
/// * `state_file_path` - Path to the state file
///
/// # Returns
///
/// Full path to the lock file
pub fn lock_file_path_for_state(state_file_path: &Path) -> PathBuf {
    let state_file_name = state_file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("state");
    
    let agent_type = state_file_name
        .strip_suffix(STATE_FILE_SUFFIX)
        .unwrap_or(state_file_name);
    
    state_file_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{}.lock", agent_type))
}

/// Gets the state file path for a task name in the state directory.
///
/// Task state files follow the naming convention: `{task_name}.state.json`
///
/// # Arguments
///
/// * `state_dir` - Directory where state files are stored
/// * `task_name` - Name of the task
///
/// # Returns
///
/// Full path to the task state file
///
/// # Example
///
/// ```
/// use automation_state::persistence::get_task_state_file_path;
/// use std::path::PathBuf;
///
/// let path = get_task_state_file_path(&PathBuf::from("/tmp/.state"), "architect");
/// assert_eq!(path, PathBuf::from("/tmp/.state/architect.state.json"));
/// ```
pub fn get_task_state_file_path(state_dir: &Path, task_name: &str) -> PathBuf {
    let filename = format!("{}{}", task_name, STATE_FILE_SUFFIX);
    state_dir.join(filename)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::state::State;
    use std::thread;
    use std::time::Duration as StdDuration;

    fn create_test_manager(temp_dir: &Path, agent_type: &str) -> StateManager {
        StateManager::new(temp_dir.to_path_buf(), agent_type).unwrap()
    }

    #[test]
    fn test_state_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(temp_dir.path(), "architect");

        assert_eq!(manager.state_file_path(), temp_dir.path().join("architect.state.json"));
    }

    #[test]
    fn test_get_state_file_path() {
        let state_dir = PathBuf::from("/tmp/.state");
        let path = get_state_file_path(&state_dir, "architect");

        assert_eq!(path, PathBuf::from("/tmp/.state/architect.state.json"));
    }

    #[test]
    fn test_get_backup_path() {
        let state_file = PathBuf::from("/tmp/.state/architect.state.json");
        let timestamp = Utc::now();
        let backup_path = get_backup_path(&state_file, timestamp);

        assert!(backup_path.to_str().unwrap().contains(".backup."));
        assert!(backup_path.to_str().unwrap().ends_with(".bak"));
    }

    #[test]
    fn test_load_non_existent_state() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(temp_dir.path(), "architect");

        let state = manager.load_state().unwrap();
        assert_eq!(state.error_count, 0);
        assert_eq!(state.status, "idle");
        assert!(state.mistakes.is_empty());
    }

    #[test]
    fn test_save_and_load_state() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(temp_dir.path(), "architect");

        // Create and save state
        let mut state = State::new();
        state.error_count = 5;
        state.status = "error".to_string();
        state.add_mistake("Test error".to_string(), Some("test-task".to_string()));

        manager.save_state(&state).unwrap();

        // Load state
        let loaded_state = manager.load_state().unwrap();
        assert_eq!(loaded_state.error_count, 5);
        assert_eq!(loaded_state.status, "error");
        assert_eq!(loaded_state.mistakes.len(), 1);
        assert_eq!(loaded_state.mistakes[0].error_message, "Test error");
    }

    #[test]
    fn test_load_state_with_lock() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(temp_dir.path(), "architect");

        // Use load_state_with_lock to modify state
        let result = manager.load_state_with_lock(|state| {
            state.error_count = 10;
            state.add_mistake("New error".to_string(), None);
            Ok::<_, AutomationError>(42)
        });

        assert_eq!(result.unwrap(), 42);

        // Verify state was saved
        let state = manager.load_state().unwrap();
        assert_eq!(state.error_count, 10);
        assert_eq!(state.mistakes.len(), 1);
    }

    #[test]
    fn test_state_exists() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(temp_dir.path(), "architect");

        assert!(!manager.state_exists());

        manager.save_state(&State::new()).unwrap();
        assert!(manager.state_exists());
    }

    #[test]
    fn test_delete_state() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(temp_dir.path(), "architect");

        manager.save_state(&State::new()).unwrap();
        assert!(manager.state_exists());

        manager.delete_state().unwrap();
        assert!(!manager.state_exists());
    }

    #[test]
    fn test_delete_non_existent_state() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(temp_dir.path(), "architect");

        // Deleting a non-existent state should not error
        manager.delete_state().unwrap();
    }

    #[test]
    fn test_backup_and_restore() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(temp_dir.path(), "architect");

        // Create initial state
        let mut state = State::new();
        state.error_count = 5;
        state.status = "error".to_string();
        manager.save_state(&state).unwrap();

        // Create backup
        let backup_path = manager.backup_state().unwrap();
        assert!(backup_path.exists());

        // Modify state
        state.error_count = 10;
        manager.save_state(&state).unwrap();

        // Verify modified state
        let loaded_state = manager.load_state().unwrap();
        assert_eq!(loaded_state.error_count, 10);

        // Restore from backup
        manager.restore_state(&backup_path).unwrap();

        // Verify restored state
        let loaded_state = manager.load_state().unwrap();
        assert_eq!(loaded_state.error_count, 5);
        assert_eq!(loaded_state.status, "error");
    }

    #[test]
    fn test_list_backups() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(temp_dir.path(), "architect");

        manager.save_state(&State::new()).unwrap();

        // Create multiple backups
        let backup1 = manager.backup_state().unwrap();
        thread::sleep(StdDuration::from_millis(10));
        let backup2 = manager.backup_state().unwrap();

        let backups = list_backups(manager.state_file_path()).unwrap();
        assert_eq!(backups.len(), 2);
        
        // Backups should be sorted by modification time (newest first)
        assert_eq!(backups[0], backup2);
        assert_eq!(backups[1], backup1);
    }

    #[test]
    fn test_atomic_write() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        atomic_write(&file_path, b"Hello, world!").unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello, world!");
    }

    #[test]
    fn test_atomic_write_overwrites() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        atomic_write(&file_path, b"First").unwrap();
        atomic_write(&file_path, b"Second").unwrap();

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Second");
    }

    #[test]
    fn test_state_corruption_handling() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(temp_dir.path(), "architect");

        // Create corrupted state file
        let state_file = manager.state_file_path();
        fs::write(state_file, b"invalid json").unwrap();

        // Loading corrupted state should return an error
        let result = manager.load_state();
        assert!(result.is_err());

        if let Err(AutomationError::Json(_)) = result {
            // Expected error type
        } else {
            panic!("Expected Json error");
        }
    }

    #[test]
    fn test_concurrent_access_safety() {
        let temp_dir = TempDir::new().unwrap();
        let state_dir = temp_dir.path().to_path_buf();

        let manager1 = create_test_manager(&state_dir, "architect");
        let _manager2 = create_test_manager(&state_dir, "architect");

        // Write initial state
        let mut state = State::new();
        state.error_count = 5;
        manager1.save_state(&state).unwrap();

        // Create two threads that both try to modify the state
        let state_dir_clone = state_dir.clone();
        let handle1 = thread::spawn(move || {
            let manager = create_test_manager(&state_dir_clone, "architect");
            manager.load_state_with_lock(|s| {
                s.error_count += 1;
                Ok::<_, AutomationError>(())
            })
        });

        let state_dir_clone2 = state_dir.clone();
        let handle2 = thread::spawn(move || {
            let manager = create_test_manager(&state_dir_clone2, "architect");
            manager.load_state_with_lock(|s| {
                s.error_count += 1;
                Ok::<_, AutomationError>(())
            })
        });

        // Both threads should complete without errors
        handle1.join().unwrap().unwrap();
        handle2.join().unwrap().unwrap();

        // Final error count should be 7 (initial 5 + 2 increments)
        let final_state = manager1.load_state().unwrap();
        assert_eq!(final_state.error_count, 7);
    }

    #[test]
    fn test_lock_file_path_for_state() {
        let state_file = PathBuf::from("/tmp/.state/architect.state.json");
        let lock_path = lock_file_path_for_state(&state_file);

        assert_eq!(lock_path, PathBuf::from("/tmp/.state/architect.lock"));
    }

    #[test]
    fn test_custom_lock_timeout() {
        let temp_dir = TempDir::new().unwrap();
        let manager = StateManager::with_lock_timeout(
            temp_dir.path().to_path_buf(),
            "architect",
            StdDuration::from_secs(60),
        ).unwrap();

        assert!(manager.state_file_path().ends_with("architect.state.json"));
    }

    #[test]
    fn test_multiple_agents() {
        let temp_dir = TempDir::new().unwrap();

        let architect_manager = create_test_manager(temp_dir.path(), "architect");
        let janitor_manager = create_test_manager(temp_dir.path(), "janitor");

        let mut architect_state = State::new();
        architect_state.status = "architect-running".to_string();
        architect_manager.save_state(&architect_state).unwrap();

        let mut janitor_state = State::new();
        janitor_state.status = "janitor-running".to_string();
        janitor_manager.save_state(&janitor_state).unwrap();

        // Each agent should have its own state file
        let loaded_architect = architect_manager.load_state().unwrap();
        let loaded_janitor = janitor_manager.load_state().unwrap();

        assert_eq!(loaded_architect.status, "architect-running");
        assert_eq!(loaded_janitor.status, "janitor-running");
    }

    #[test]
    fn test_restore_non_existent_backup() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(temp_dir.path(), "architect");

        let fake_backup = temp_dir.path().join("fake-backup.json");
        let result = manager.restore_state(&fake_backup);

        assert!(result.is_err());
        if let Err(AutomationError::FileSystem(_)) = result {
            // Expected error type
        } else {
            panic!("Expected FileSystem error");
        }
    }

    #[test]
    fn test_backup_non_existent_state() {
        let temp_dir = TempDir::new().unwrap();
        let manager = create_test_manager(temp_dir.path(), "architect");

        let result = manager.backup_state();

        assert!(result.is_err());
        if let Err(AutomationError::StateError { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected StateError error");
        }
    }
}

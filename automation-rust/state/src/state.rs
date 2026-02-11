//! State data structures for the automation system.
//!
//! This module provides the core data structures used for tracking and managing
//! state across parallel agent executions. The state is persisted as JSON and
//! includes:
//!
//! - **Execution tracking**: Timestamps for last run, success, and failure
//! - **Error tracking**: Error counts and consecutive failures
//! - **Mistake tracking**: Detailed records of errors and failures for analysis
//! - **Lock information**: Process and host tracking for file locking
//! - **Performance metrics**: Execution time tracking and averages
//! - **Termination tracking**: Early termination reasons and counts
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

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration as StdDuration;
use uuid::Uuid;

use automation_common::{AutomationError, Result};

/// Trait for types that can be merged from a newer version.
///
/// This trait provides a mechanism to merge missing fields from one instance
/// into another, enabling backward compatibility when reading old state files
/// that may be missing fields from newer schema versions.
///
/// The merge is deep (recursive) for nested objects, ensuring that all levels
/// of the structure are properly merged.
///
/// # Examples
///
/// ```
/// use automation_state::TaskState;
/// use automation_state::Mergeable;
///
/// let mut old_state = TaskState::new();
/// let new_state = TaskState::new(); // May have additional fields in future
///
/// // Merge missing fields from new_state into old_state
/// old_state.merge_from(&new_state).unwrap();
/// ```
pub trait Mergeable: Serialize + for<'de> Deserialize<'de> {
    /// Merges missing fields from another instance.
    ///
    /// This performs a deep merge that handles nested structures recursively.
    /// Fields in `self` are preserved; only missing or null fields in `self`
    /// are filled from `other`.
    ///
    /// # Merge Rules
    ///
    /// - **Objects**: Keys are merged recursively. If a key exists in both
    ///   values, their values are merged recursively.
    /// - **Arrays**: Arrays are replaced (not merged) with the array from
    ///   `other` if the array in `self` is empty or missing.
    /// - **Primitives**: If `self` has a null or missing value, it's replaced
    ///   with the value from `other`.
    /// - **Null values**: Treated as missing fields.
    ///
    /// # Arguments
    ///
    /// * `other` - The instance to merge fields from (typically a newer version)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the merge was successful
    /// * `Err(AutomationError)` - If serialization or deserialization fails
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization or deserialization fails.
    fn merge_from(&mut self, other: &Self) -> Result<()>;
}

/// Performs a deep merge of two JSON values.
///
/// This function implements a recursive merge strategy for JSON values that preserves
/// existing fields and only adds missing ones. This is designed for backward compatibility
/// when merging state from newer schema versions into older state instances.
///
/// # Merge Rules
///
/// - **Objects**: Keys are merged recursively. For each key in `other`:
///   - If the key doesn't exist in `self`, add it from `other`
///   - If both values are objects, merge them recursively (preserving existing sub-keys)
///   - Otherwise, preserve `self`'s value (don't overwrite existing values)
/// - **Arrays**: Replace `self` array with `other` array only if `self` array is empty
///   - Empty arrays are treated as missing fields
/// - **Primitives**: Preserve `self`'s value if it exists (not null/missing)
/// - **Null**: Treat as missing field
///
/// # Arguments
///
/// * `self_val` - The base JSON value to merge into (existing state)
/// * `other_val` - The JSON value to merge from (newer state with default values)
///
/// # Returns
///
/// The merged JSON value with existing fields preserved.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use automation_state::deep_merge_json;
///
/// let base = json!({"a": 1, "b": {"x": 10}});
/// let patch = json!({"b": {"y": 20}, "c": 3});
/// let merged = deep_merge_json(base, patch);
///
/// assert_eq!(merged, json!({"a": 1, "b": {"x": 10, "y": 20}, "c": 3}));
/// ```
pub fn deep_merge_json(self_val: Value, other_val: Value) -> Value {
    match (self_val.clone(), other_val.clone()) {
        (Value::Object(mut self_map), Value::Object(other_map)) => {
            for (key, other_value) in other_map {
                match self_map.get(&key) {
                    Some(self_value) => {
                        // If self value is null, replace it with other value
                        if self_value.is_null() {
                            self_map.insert(key, other_value);
                        }
                        // Recursively merge if both are objects
                        else if self_value.is_object() && other_value.is_object() {
                            let merged = deep_merge_json(self_value.clone(), other_value);
                            self_map.insert(key, merged);
                        }
                        // Otherwise, preserve self's value (don't overwrite existing values)
                        // This ensures backward compatibility - existing data is preserved
                    }
                    None => {
                        // Key doesn't exist in self, add it from other
                        self_map.insert(key, other_value);
                    }
                }
            }
            Value::Object(self_map)
        }
        // If self is null, use other value
        (Value::Null, other) => other,
        // For arrays: only replace if self array is empty
        (Value::Array(self_arr), Value::Array(_)) if self_arr.is_empty() => {
            other_val
        }
        // For arrays and primitives: preserve self's value (don't overwrite)
        (_, _) => self_val,
    }
}

impl Mergeable for TaskState {
    fn merge_from(&mut self, other: &TaskState) -> Result<()> {
        // Serialize both states to JSON
        let self_json = serde_json::to_value(&*self)
            .map_err(|e| AutomationError::StateError {
                task: "TaskState".to_string(),
                message: format!("Serialization error: {}", e),
            })?;
        let other_json = serde_json::to_value(other)
            .map_err(|e| AutomationError::StateError {
                task: "TaskState".to_string(),
                message: format!("Serialization error: {}", e),
            })?;

        // Perform deep merge
        let merged = deep_merge_json(self_json, other_json);

        // Deserialize back to TaskState
        *self = serde_json::from_value(merged)
            .map_err(|e| AutomationError::StateError {
                task: "TaskState".to_string(),
                message: format!("Deserialization error: {}", e),
            })?;

        Ok(())
    }
}

impl Mergeable for State {
    fn merge_from(&mut self, other: &State) -> Result<()> {
        // Serialize both states to JSON
        let self_json = serde_json::to_value(&*self)
            .map_err(|e| AutomationError::StateError {
                task: "State".to_string(),
                message: format!("Serialization error: {}", e),
            })?;
        let other_json = serde_json::to_value(other)
            .map_err(|e| AutomationError::StateError {
                task: "State".to_string(),
                message: format!("Serialization error: {}", e),
            })?;

        // Perform deep merge
        let merged = deep_merge_json(self_json, other_json);

        // Deserialize back to State
        *self = serde_json::from_value(merged)
            .map_err(|e| AutomationError::StateError {
                task: "State".to_string(),
                message: format!("Deserialization error: {}", e),
            })?;

        Ok(())
    }
}

/// Task status enumeration.
///
/// Represents the current status of a task execution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    /// Task is idle (not running)
    Idle,
    /// Task is currently running
    Running,
    /// Task completed successfully
    Success,
    /// Task failed
    Failed,
}

impl TaskStatus {
    /// Returns true if the task is currently running.
    pub fn is_running(&self) -> bool {
        matches!(self, TaskStatus::Running)
    }

    /// Returns true if the task completed successfully.
    pub fn is_success(&self) -> bool {
        matches!(self, TaskStatus::Success)
    }

    /// Returns true if the task failed.
    pub fn is_failed(&self) -> bool {
        matches!(self, TaskStatus::Failed)
    }

    /// Returns true if the task is idle.
    pub fn is_idle(&self) -> bool {
        matches!(self, TaskStatus::Idle)
    }
}

impl Default for TaskStatus {
    fn default() -> Self {
        TaskStatus::Idle
    }
}

/// Lock data structure for file-based locking.
///
/// Contains information about a lock held by a process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockData {
    /// Process ID of the process holding the lock
    pub pid: Option<u32>,
    /// Timestamp when the lock was acquired
    pub timestamp: Option<DateTime<Utc>>,
    /// Hostname of the machine holding the lock
    pub host: Option<String>,
}

impl LockData {
    /// Creates a new lock data with the current timestamp.
    ///
    /// # Arguments
    ///
    /// * `pid` - Optional process ID
    /// * `host` - Optional hostname
    pub fn new(pid: Option<u32>, host: Option<String>) -> Self {
        LockData {
            pid,
            timestamp: Some(Utc::now()),
            host,
        }
    }
}

impl Default for LockData {
    fn default() -> Self {
        LockData {
            pid: None,
            timestamp: None,
            host: None,
        }
    }
}

/// Task state structure.
///
/// Represents the state of a single task, including execution history,
/// error tracking, performance metrics, and termination information.
///
/// This structure is designed to be serialized to JSON for persistence,
/// with backward compatibility support for missing fields when reading
/// old state files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskState {
    /// Timestamp of the last run
    pub last_run: Option<DateTime<Utc>>,
    /// Timestamp of the last successful completion
    pub last_success: Option<DateTime<Utc>>,
    /// Timestamp of the last failure
    pub last_failure: Option<DateTime<Utc>>,
    /// Total count of errors
    pub error_count: u64,
    /// Count of consecutive failures
    pub consecutive_failures: u64,
    /// Current status
    pub status: TaskStatus,
    /// Reason for the last early termination
    pub last_termination_reason: Option<String>,
    /// Count of early terminations
    pub early_termination_count: u64,
    /// Total execution time in milliseconds
    pub total_execution_time_ms: u64,
    /// Average execution time in milliseconds
    pub average_execution_time_ms: u64,
    /// Count of successful terminations
    pub successful_terminations: u64,
    /// Count of failed terminations
    pub failed_terminations: u64,
}

impl TaskState {
    /// Creates a new task state with default values.
    pub fn new() -> Self {
        TaskState::default()
    }

    /// Updates the last run timestamp.
    pub fn update_last_run(&mut self, timestamp: DateTime<Utc>) {
        self.last_run = Some(timestamp);
    }

    /// Records a successful completion with execution time.
    pub fn record_success(&mut self, timestamp: DateTime<Utc>, execution_time_ms: u64) {
        self.last_success = Some(timestamp);
        self.status = TaskStatus::Success;
        self.consecutive_failures = 0;
        self.successful_terminations += 1;
        self.total_execution_time_ms += execution_time_ms;
        self.recalculate_average_execution_time();
    }

    /// Records a failure.
    pub fn record_failure(&mut self, error: &str) {
        self.last_failure = Some(Utc::now());
        self.error_count += 1;
        self.consecutive_failures += 1;
        self.failed_terminations += 1;
        self.status = TaskStatus::Failed;
        self.last_termination_reason = Some(error.to_string());
    }

    /// Records an early termination event.
    pub fn record_early_termination(&mut self, reason: &str, execution_time_ms: Option<u64>) {
        self.last_termination_reason = Some(reason.to_string());
        self.early_termination_count += 1;

        if let Some(time_ms) = execution_time_ms {
            self.total_execution_time_ms += time_ms;
            self.recalculate_average_execution_time();
        }
    }

    /// Resets the state to default values.
    pub fn reset(&mut self) {
        *self = TaskState::default();
    }

    /// Recalculates the average execution time based on total time and execution count.
    fn recalculate_average_execution_time(&mut self) {
        let total_executions = self.early_termination_count + self.successful_terminations;
        if total_executions > 0 {
            self.average_execution_time_ms = self.total_execution_time_ms / total_executions;
        }
    }

    /// Merges missing fields from another state for backward compatibility.
    ///
    /// When reading old state files, some fields may be missing. This method
    /// merges those missing fields with their default values.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated in favor of [`Mergeable::merge_from`], which
    /// provides proper deep merge for nested structures including objects and arrays.
    /// This method only merges top-level fields and does not handle nested structures.
    ///
    /// Use `self.merge_from(other)` instead for complete recursive merging.
    #[deprecated(
        since = "0.2.0",
        note = "Use merge_from instead for proper deep merge of nested structures including objects and arrays. This method only merges top-level Option<DateTime> fields."
    )]
    pub fn merge_missing_fields(&mut self, other: &TaskState) {
        // For backward compatibility, use the new Mergeable trait
        // which provides proper deep merge for all nested structures
        if let Err(e) = self.merge_from(other) {
            // Fallback to manual merge if JSON merge fails
            // This maintains backward compatibility even on errors
            tracing::warn!("Failed to perform deep merge, falling back to manual merge: {}", e);
            if self.last_run.is_none() {
                self.last_run = other.last_run;
            }
            if self.last_success.is_none() {
                self.last_success = other.last_success;
            }
            if self.last_failure.is_none() {
                self.last_failure = other.last_failure;
            }
            // Error counts and other numeric fields should be preserved
            // Status should be preserved (use more recent status)
            if self.last_termination_reason.is_none() {
                self.last_termination_reason = other.last_termination_reason.clone();
            }
        }
    }
}

impl Default for TaskState {
    fn default() -> Self {
        TaskState {
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

/// A mistake/error record for tracking and analysis.
///
/// Mistakes are recorded when tasks fail or encounter errors. They provide
/// detailed information for debugging and system analysis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Mistake {
    /// Unique identifier for this mistake (UUID v4)
    pub id: String,
    /// Timestamp when the mistake occurred
    pub timestamp: DateTime<Utc>,
    /// Human-readable error message
    pub error_message: String,
    /// Name of the task that caused the mistake (optional)
    pub task: Option<String>,
    /// Additional context or stack trace (optional)
    pub context: Option<String>,
}

impl Mistake {
    /// Creates a new mistake with the given error message.
    ///
    /// # Arguments
    ///
    /// * `error_message` - The error message describing what went wrong
    /// * `task` - Optional task name that caused the mistake
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::Mistake;
    ///
    /// let mistake = Mistake::new(
    ///     "Task failed with timeout".to_string(),
    ///     Some("architect".to_string())
    /// );
    /// ```
    pub fn new(error_message: String, task: Option<String>) -> Self {
        Mistake {
            id: generate_mistake_id(),
            timestamp: Utc::now(),
            error_message,
            task,
            context: None,
        }
    }

    /// Creates a new mistake with additional context.
    ///
    /// # Arguments
    ///
    /// * `error_message` - The error message describing what went wrong
    /// * `task` - Optional task name that caused the mistake
    /// * `context` - Optional additional context or stack trace
    pub fn new_with_context(
        error_message: String,
        task: Option<String>,
        context: String,
    ) -> Self {
        Mistake {
            id: generate_mistake_id(),
            timestamp: Utc::now(),
            error_message,
            task,
            context: Some(context),
        }
    }

    /// Returns the age of the mistake as a Duration.
    pub fn age(&self) -> chrono::TimeDelta {
        Utc::now() - self.timestamp
    }
}

/// Lock information for file-based locking.
///
/// Locks are used to prevent concurrent access to shared resources
/// across different processes and hosts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockInfo {
    /// Timestamp when the lock was acquired
    pub acquired_at: DateTime<Utc>,
    /// Process ID of the process holding the lock
    pub process_id: Option<u32>,
    /// Hostname of the machine holding the lock
    pub hostname: Option<String>,
}

impl LockInfo {
    /// Creates a new lock info with the current timestamp.
    ///
    /// # Arguments
    ///
    /// * `process_id` - Optional process ID
    /// * `hostname` - Optional hostname
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::LockInfo;
    ///
    /// let lock = LockInfo::new(Some(1234), Some("worker-1".to_string()));
    /// ```
    pub fn new(process_id: Option<u32>, hostname: Option<String>) -> Self {
        LockInfo {
            acquired_at: Utc::now(),
            process_id,
            hostname,
        }
    }

    /// Returns the age of the lock as a Duration.
    pub fn age(&self) -> chrono::TimeDelta {
        Utc::now() - self.acquired_at
    }
}

/// The main state structure for tracking task execution.
///
/// This state is persisted as JSON and tracks all aspects of task execution
/// including timestamps, error counts, mistakes, performance metrics, and
/// termination information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct State {
    /// Timestamp of the last run (optional)
    pub last_run: Option<DateTime<Utc>>,
    /// Timestamp of the last successful completion (optional)
    pub last_success: Option<DateTime<Utc>>,
    /// Timestamp of the last failure (optional)
    pub last_failure: Option<DateTime<Utc>>,
    /// Total count of errors
    pub error_count: u64,
    /// Count of consecutive failures
    pub consecutive_failures: u64,
    /// Current status (e.g., "idle", "running", "success", "error")
    pub status: String,
    /// Reason for the last early termination (optional)
    pub last_termination_reason: Option<String>,
    /// Count of early terminations
    pub early_termination_count: u64,
    /// Total execution time in milliseconds
    pub total_execution_time_ms: u64,
    /// Average execution time in milliseconds
    pub average_execution_time_ms: u64,
    /// Count of successful terminations
    pub successful_terminations: u64,
    /// Count of failed terminations
    pub failed_terminations: u64,
    /// Array of mistake records for detailed error tracking
    pub mistakes: Vec<Mistake>,
    /// Lock information (optional)
    pub lock: Option<LockInfo>,
}

impl State {
    /// Creates a new empty state with default values.
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::State;
    ///
    /// let state = State::new();
    /// assert_eq!(state.error_count, 0);
    /// assert_eq!(state.status, "idle");
    /// ```
    pub fn new() -> Self {
        State {
            last_run: None,
            last_success: None,
            last_failure: None,
            error_count: 0,
            consecutive_failures: 0,
            status: "idle".to_string(),
            last_termination_reason: None,
            early_termination_count: 0,
            total_execution_time_ms: 0,
            average_execution_time_ms: 0,
            successful_terminations: 0,
            failed_terminations: 0,
            mistakes: Vec::new(),
            lock: None,
        }
    }

    /// Adds a mistake record to the state.
    ///
    /// This method creates a new mistake record and adds it to the state's
    /// mistakes vector. The mistake ID is returned for reference.
    ///
    /// # Arguments
    ///
    /// * `error_message` - The error message describing what went wrong
    /// * `task` - Optional task name that caused the mistake
    ///
    /// # Returns
    ///
    /// The unique ID of the created mistake
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::State;
    ///
    /// let mut state = State::new();
    /// let mistake_id = state.add_mistake(
    ///     "Task failed with timeout".to_string(),
    ///     Some("architect".to_string())
    /// );
    /// ```
    pub fn add_mistake(&mut self, error_message: String, task: Option<String>) -> String {
        let mistake = Mistake::new(error_message, task);
        let id = mistake.id.clone();
        self.mistakes.push(mistake);
        id
    }

    /// Adds a mistake record with additional context.
    ///
    /// # Arguments
    ///
    /// * `error_message` - The error message describing what went wrong
    /// * `task` - Optional task name that caused the mistake
    /// * `context` - Additional context or stack trace
    ///
    /// # Returns
    ///
    /// The unique ID of the created mistake
    pub fn add_mistake_with_context(
        &mut self,
        error_message: String,
        task: Option<String>,
        context: String,
    ) -> String {
        let mistake = Mistake::new_with_context(error_message, task, context);
        let id = mistake.id.clone();
        self.mistakes.push(mistake);
        id
    }

    /// Clears all mistakes from the state.
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::State;
    ///
    /// let mut state = State::new();
    /// state.add_mistake("Error".to_string(), None);
    /// assert_eq!(state.mistakes.len(), 1);
    ///
    /// state.clear_mistakes();
    /// assert_eq!(state.mistakes.len(), 0);
    /// ```
    pub fn clear_mistakes(&mut self) {
        self.mistakes.clear();
    }

    /// Clears stale mistakes older than the specified reset interval.
    ///
    /// This method removes mistakes that are older than the specified duration.
    /// Useful for preventing unlimited growth of the mistakes array.
    ///
    /// # Arguments
    ///
    /// * `reset_interval` - Duration after which mistakes are considered stale
    ///
    /// # Returns
    ///
    /// The count of mistakes that were cleared
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::State;
    /// use std::time::Duration;
    ///
    /// let mut state = State::new();
    /// state.add_mistake("Old error".to_string(), None);
    ///
    /// // Clear mistakes older than 1 hour
    /// let cleared = state.clear_stale_mistakes(Duration::from_secs(3600));
    /// ```
    pub fn clear_stale_mistakes(&mut self, reset_interval: StdDuration) -> usize {
        let now = Utc::now();
        let reset_duration = chrono::TimeDelta::from_std(reset_interval)
            .expect("Duration conversion failed");

        let original_len = self.mistakes.len();
        self.mistakes.retain(|mistake| {
            now - mistake.timestamp < reset_duration
        });

        original_len - self.mistakes.len()
    }

    /// Updates the last run timestamp.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - The timestamp to set
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::State;
    /// use chrono::Utc;
    ///
    /// let mut state = State::new();
    /// let now = Utc::now();
    /// state.update_last_run(now);
    /// assert!(state.last_run.is_some());
    /// ```
    pub fn update_last_run(&mut self, timestamp: DateTime<Utc>) {
        self.last_run = Some(timestamp);
    }

    /// Updates the last success timestamp.
    ///
    /// Also resets the consecutive failures counter and updates status.
    pub fn update_last_success(&mut self, timestamp: DateTime<Utc>) {
        self.last_success = Some(timestamp);
        self.consecutive_failures = 0;
        self.status = "success".to_string();
    }

    /// Updates the last failure timestamp.
    ///
    /// Also increments the consecutive failures counter and updates status.
    pub fn update_last_failure(&mut self, timestamp: DateTime<Utc>) {
        self.last_failure = Some(timestamp);
        self.consecutive_failures += 1;
        self.status = "error".to_string();
    }

    /// Records an early termination event.
    ///
    /// Updates termination-related fields and optionally execution time.
    ///
    /// # Arguments
    ///
    /// * `reason` - The reason for early termination
    /// * `execution_time_ms` - Execution time before termination (optional)
    /// * `was_successful` - Whether the termination was considered successful
    pub fn record_early_termination(
        &mut self,
        reason: String,
        execution_time_ms: Option<u64>,
        was_successful: bool,
    ) {
        self.last_termination_reason = Some(reason);
        self.early_termination_count += 1;

        if was_successful {
            self.successful_terminations += 1;
        } else {
            self.failed_terminations += 1;
        }

        if let Some(time_ms) = execution_time_ms {
            self.total_execution_time_ms += time_ms;
            self.recalculate_average_execution_time();
        }
    }

    /// Records a successful completion with execution time.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - Timestamp of successful completion
    /// * `execution_time_ms` - Time taken for completion
    pub fn record_success(&mut self, timestamp: DateTime<Utc>, execution_time_ms: u64) {
        self.last_success = Some(timestamp);
        self.status = "success".to_string();
        self.consecutive_failures = 0;
        self.successful_terminations += 1;
        self.total_execution_time_ms += execution_time_ms;
        self.recalculate_average_execution_time();
    }

    /// Increments the error count.
    pub fn increment_error_count(&mut self) {
        self.error_count += 1;
    }

    /// Sets the lock information.
    pub fn set_lock(&mut self, lock: LockInfo) {
        self.lock = Some(lock);
    }

    /// Clears the lock information.
    pub fn clear_lock(&mut self) {
        self.lock = None;
    }

    /// Returns true if the state has any mistakes.
    pub fn has_mistakes(&self) -> bool {
        !self.mistakes.is_empty()
    }

    /// Returns true if the state is locked.
    pub fn is_locked(&self) -> bool {
        self.lock.is_some()
    }

    /// Resets the state to initial values, optionally keeping mistakes.
    ///
    /// # Arguments
    ///
    /// * `keep_mistakes` - If true, preserves the mistakes vector
    pub fn reset(&mut self, keep_mistakes: bool) {
        let mistakes = if keep_mistakes {
            self.mistakes.clone()
        } else {
            Vec::new()
        };

        *self = State {
            mistakes,
            ..State::new()
        };
    }

    /// Recalculates the average execution time based on total time and execution count.
    fn recalculate_average_execution_time(&mut self) {
        let total_executions = self.early_termination_count + self.successful_terminations;
        if total_executions > 0 {
            self.average_execution_time_ms = self.total_execution_time_ms / total_executions;
        }
    }
}

impl Default for State {
    fn default() -> Self {
        State::new()
    }
}

/// Generates a unique mistake ID using UUID v4.
///
/// # Example
///
/// ```
/// use automation_state::generate_mistake_id;
///
/// let id = generate_mistake_id();
/// assert!(!id.is_empty());
/// ```
pub fn generate_mistake_id() -> String {
    Uuid::new_v4().to_string()
}

/// Creates lock information with the current timestamp.
///
/// # Arguments
///
/// * `process_id` - Optional process ID
/// * `hostname` - Optional hostname
///
/// # Example
///
/// ```
/// use automation_state::create_lock_info;
///
/// let lock = create_lock_info(Some(1234), Some("worker-1".to_string()));
/// ```
pub fn create_lock_info(process_id: Option<u32>, hostname: Option<String>) -> LockInfo {
    LockInfo::new(process_id, hostname)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use serde_json::json;

    #[test]
    fn test_state_creation() {
        let state = State::new();
        assert_eq!(state.error_count, 0);
        assert_eq!(state.consecutive_failures, 0);
        assert_eq!(state.status, "idle");
        assert_eq!(state.early_termination_count, 0);
        assert_eq!(state.total_execution_time_ms, 0);
        assert_eq!(state.average_execution_time_ms, 0);
        assert_eq!(state.successful_terminations, 0);
        assert_eq!(state.failed_terminations, 0);
        assert!(state.mistakes.is_empty());
        assert!(state.last_run.is_none());
        assert!(state.last_success.is_none());
        assert!(state.last_failure.is_none());
        assert!(state.last_termination_reason.is_none());
        assert!(state.lock.is_none());
    }

    #[test]
    fn test_state_default() {
        let state = State::default();
        assert_eq!(state.error_count, 0);
        assert_eq!(state.status, "idle");
    }

    #[test]
    fn test_add_mistake() {
        let mut state = State::new();
        let mistake_id = state.add_mistake(
            "Task failed with timeout".to_string(),
            Some("architect".to_string()),
        );

        assert_eq!(state.mistakes.len(), 1);
        assert_eq!(state.mistakes[0].error_message, "Task failed with timeout");
        assert_eq!(state.mistakes[0].task, Some("architect".to_string()));
        assert_eq!(state.mistakes[0].id, mistake_id);
        assert!(state.has_mistakes());
    }

    #[test]
    fn test_add_mistake_without_task() {
        let mut state = State::new();
        state.add_mistake("Generic error".to_string(), None);

        assert_eq!(state.mistakes.len(), 1);
        assert_eq!(state.mistakes[0].task, None);
        assert!(state.mistakes[0].timestamp <= Utc::now());
    }

    #[test]
    fn test_add_mistake_with_context() {
        let mut state = State::new();
        let mistake_id = state.add_mistake_with_context(
            "Task failed with timeout".to_string(),
            Some("architect".to_string()),
            "Stack trace here".to_string(),
        );

        assert_eq!(state.mistakes.len(), 1);
        assert_eq!(state.mistakes[0].context, Some("Stack trace here".to_string()));
        assert_eq!(state.mistakes[0].id, mistake_id);
    }

    #[test]
    fn test_clear_mistakes() {
        let mut state = State::new();
        state.add_mistake("Error 1".to_string(), None);
        state.add_mistake("Error 2".to_string(), None);
        assert_eq!(state.mistakes.len(), 2);

        state.clear_mistakes();
        assert_eq!(state.mistakes.len(), 0);
        assert!(!state.has_mistakes());
    }

    #[test]
    fn test_clear_stale_mistakes() {
        let mut state = State::new();

        // Add current mistake
        state.add_mistake("Recent error".to_string(), None);

        // Add old mistake by modifying timestamp directly
        let mut old_mistake = Mistake::new("Old error".to_string(), None);
        old_mistake.timestamp = Utc::now() - Duration::from_secs(7200); // 2 hours ago
        state.mistakes.push(old_mistake);

        assert_eq!(state.mistakes.len(), 2);

        // Clear mistakes older than 1 hour
        let cleared = state.clear_stale_mistakes(Duration::from_secs(3600));
        assert_eq!(cleared, 1);
        assert_eq!(state.mistakes.len(), 1);
        assert_eq!(state.mistakes[0].error_message, "Recent error");
    }

    #[test]
    fn test_clear_stale_mistakes_none() {
        let mut state = State::new();
        state.add_mistake("Recent error".to_string(), None);

        // Clear mistakes older than 1 hour (should not clear any)
        let cleared = state.clear_stale_mistakes(Duration::from_secs(3600));
        assert_eq!(cleared, 0);
        assert_eq!(state.mistakes.len(), 1);
    }

    #[test]
    fn test_clear_stale_mistakes_all() {
        let mut state = State::new();

        // Add old mistakes
        let mut old_mistake1 = Mistake::new("Old error 1".to_string(), None);
        old_mistake1.timestamp = Utc::now() - Duration::from_secs(7200);
        state.mistakes.push(old_mistake1);

        let mut old_mistake2 = Mistake::new("Old error 2".to_string(), None);
        old_mistake2.timestamp = Utc::now() - Duration::from_secs(10800);
        state.mistakes.push(old_mistake2);

        assert_eq!(state.mistakes.len(), 2);

        // Clear mistakes older than 1 hour
        let cleared = state.clear_stale_mistakes(Duration::from_secs(3600));
        assert_eq!(cleared, 2);
        assert_eq!(state.mistakes.len(), 0);
    }

    #[test]
    fn test_update_last_run() {
        let mut state = State::new();
        let now = Utc::now();

        state.update_last_run(now);
        assert!(state.last_run.is_some());
        assert_eq!(state.last_run.unwrap(), now);
    }

    #[test]
    fn test_update_last_success() {
        let mut state = State::new();
        let now = Utc::now();

        state.update_last_success(now);
        assert!(state.last_success.is_some());
        assert_eq!(state.last_success.unwrap(), now);
        assert_eq!(state.status, "success");
        assert_eq!(state.consecutive_failures, 0);
    }

    #[test]
    fn test_update_last_failure() {
        let mut state = State::new();
        let now = Utc::now();

        state.update_last_failure(now);
        assert!(state.last_failure.is_some());
        assert_eq!(state.last_failure.unwrap(), now);
        assert_eq!(state.status, "error");
        assert_eq!(state.consecutive_failures, 1);

        // Test consecutive failures
        state.update_last_failure(now + Duration::from_secs(10));
        assert_eq!(state.consecutive_failures, 2);
    }

    #[test]
    fn test_record_early_termination() {
        let mut state = State::new();

        state.record_early_termination(
            "Timeout".to_string(),
            Some(5000),
            false,
        );

        assert_eq!(state.last_termination_reason, Some("Timeout".to_string()));
        assert_eq!(state.early_termination_count, 1);
        assert_eq!(state.failed_terminations, 1);
        assert_eq!(state.total_execution_time_ms, 5000);
        assert_eq!(state.average_execution_time_ms, 5000);
    }

    #[test]
    fn test_record_early_termination_successful() {
        let mut state = State::new();

        state.record_early_termination(
            "Task completed early".to_string(),
            Some(3000),
            true,
        );

        assert_eq!(state.early_termination_count, 1);
        assert_eq!(state.successful_terminations, 1);
        assert_eq!(state.failed_terminations, 0);
    }

    #[test]
    fn test_record_early_termination_without_time() {
        let mut state = State::new();

        state.record_early_termination(
            "Timeout".to_string(),
            None,
            false,
        );

        assert_eq!(state.early_termination_count, 1);
        assert_eq!(state.total_execution_time_ms, 0);
    }

    #[test]
    fn test_record_success() {
        let mut state = State::new();
        let now = Utc::now();

        state.record_success(now, 4000);

        assert_eq!(state.last_success, Some(now));
        assert_eq!(state.status, "success");
        assert_eq!(state.consecutive_failures, 0);
        assert_eq!(state.successful_terminations, 1);
        assert_eq!(state.total_execution_time_ms, 4000);
        assert_eq!(state.average_execution_time_ms, 4000);
    }

    #[test]
    fn test_increment_error_count() {
        let mut state = State::new();
        assert_eq!(state.error_count, 0);

        state.increment_error_count();
        assert_eq!(state.error_count, 1);

        state.increment_error_count();
        assert_eq!(state.error_count, 2);
    }

    #[test]
    fn test_lock_management() {
        let mut state = State::new();

        assert!(!state.is_locked());

        let lock = LockInfo::new(Some(1234), Some("worker-1".to_string()));
        state.set_lock(lock.clone());
        assert!(state.is_locked());
        assert_eq!(state.lock.as_ref().unwrap().process_id, Some(1234));

        state.clear_lock();
        assert!(!state.is_locked());
    }

    #[test]
    fn test_reset_keep_mistakes() {
        let mut state = State::new();
        state.add_mistake("Error".to_string(), None);
        state.increment_error_count();
        state.update_last_failure(Utc::now());

        state.reset(true);

        assert_eq!(state.error_count, 0);
        assert_eq!(state.status, "idle");
        assert_eq!(state.mistakes.len(), 1); // mistakes kept
        assert_eq!(state.consecutive_failures, 0);
    }

    #[test]
    fn test_reset_clear_mistakes() {
        let mut state = State::new();
        state.add_mistake("Error".to_string(), None);
        state.increment_error_count();

        state.reset(false);

        assert_eq!(state.error_count, 0);
        assert_eq!(state.mistakes.len(), 0); // mistakes cleared
    }

    #[test]
    fn test_serialization_deserialization() {
        let mut state = State::new();
        state.add_mistake("Error 1".to_string(), Some("task1".to_string()));
        state.update_last_run(Utc::now());
        state.increment_error_count();

        // Serialize
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("mistakes"));
        assert!(json.contains("Error 1"));

        // Deserialize
        let deserialized: State = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.mistakes.len(), 1);
        assert_eq!(deserialized.mistakes[0].error_message, "Error 1");
        assert_eq!(deserialized.mistakes[0].task, Some("task1".to_string()));
        assert_eq!(deserialized.error_count, 1);
    }

    #[test]
    fn test_pretty_serialization() {
        let state = State::new();

        let json = serde_json::to_string_pretty(&state).unwrap();
        assert!(json.contains("error_count"));
        assert!(json.contains("status"));
    }

    #[test]
    fn test_mistake_new() {
        let mistake = Mistake::new(
            "Test error".to_string(),
            Some("test-task".to_string()),
        );

        assert_eq!(mistake.error_message, "Test error");
        assert_eq!(mistake.task, Some("test-task".to_string()));
        assert!(!mistake.id.is_empty());
        assert!(mistake.timestamp <= Utc::now());
        assert!(mistake.context.is_none());
    }

    #[test]
    fn test_mistake_new_with_context() {
        let mistake = Mistake::new_with_context(
            "Test error".to_string(),
            Some("test-task".to_string()),
            "Context here".to_string(),
        );

        assert_eq!(mistake.context, Some("Context here".to_string()));
    }

    #[test]
    fn test_mistake_age() {
        let before = Utc::now();

        thread::sleep(StdDuration::from_millis(100));

        let mistake = Mistake::new("Test".to_string(), None);
        let age = before - mistake.timestamp;

        // The mistake should have been created after the sleep, so its timestamp
        // should be after 'before', meaning 'before - timestamp' should be negative
        // or very small (close to 0)
        assert!(age.num_milliseconds() <= 0);
    }

    #[test]
    fn test_lock_info_new() {
        let lock = LockInfo::new(Some(1234), Some("hostname".to_string()));

        assert_eq!(lock.process_id, Some(1234));
        assert_eq!(lock.hostname, Some("hostname".to_string()));
        assert!(lock.acquired_at <= Utc::now());
    }

    #[test]
    fn test_lock_info_age() {
        let lock = LockInfo::new(None, None);

        thread::sleep(StdDuration::from_millis(100));

        let age = lock.age();
        // The lock should be at least some age (near 100ms)
        assert!(age.num_milliseconds() >= 80);
    }

    #[test]
    fn test_generate_mistake_id() {
        let id1 = generate_mistake_id();
        let id2 = generate_mistake_id();

        assert!(!id1.is_empty());
        assert!(!id2.is_empty());
        assert_ne!(id1, id2); // UUIDs should be unique
    }

    #[test]
    fn test_create_lock_info() {
        let lock = create_lock_info(Some(5678), Some("test-host".to_string()));

        assert_eq!(lock.process_id, Some(5678));
        assert_eq!(lock.hostname, Some("test-host".to_string()));
    }

    #[test]
    fn test_average_execution_time_calculation() {
        let mut state = State::new();
        let now = Utc::now();

        // First execution: 4000ms
        state.record_success(now, 4000);
        assert_eq!(state.average_execution_time_ms, 4000);

        // Second execution: 6000ms
        state.record_success(now + Duration::from_secs(10), 6000);
        assert_eq!(state.average_execution_time_ms, 5000); // (4000 + 6000) / 2

        // Third execution: 8000ms
        state.record_success(now + Duration::from_secs(20), 8000);
        assert_eq!(state.average_execution_time_ms, 6000); // (4000 + 6000 + 8000) / 3
    }

    #[test]
    fn test_mergeable_trait_taskstate_basic_merge() {
        let mut old_state = TaskState::new();
        let new_state = TaskState::new();

        // Set some fields in new_state that don't exist in old_state
        let mut state_with_values = new_state.clone();
        let now = Utc::now();
        state_with_values.last_run = Some(now);

        // Merge should add the missing last_run field
        old_state.merge_from(&state_with_values).unwrap();
        assert_eq!(old_state.last_run, Some(now));
    }

    #[test]
    fn test_mergeable_trait_preserves_existing_fields() {
        let now1 = Utc::now();
        let mut old_state = TaskState {
            last_run: Some(now1),
            last_success: Some(now1),
            error_count: 5,
            consecutive_failures: 2,
            status: TaskStatus::Failed,
            ..TaskState::default()
        };

        let now2 = now1 + chrono::Duration::hours(1);
        let new_state = TaskState {
            last_run: Some(now2), // Should NOT override existing value
            last_success: None,  // Should be ignored since old has value
            error_count: 0,      // Should NOT override existing value
            consecutive_failures: 0,
            status: TaskStatus::Idle,
            ..TaskState::default()
        };

        old_state.merge_from(&new_state).unwrap();

        // Existing fields should be preserved
        assert_eq!(old_state.last_run, Some(now1));
        assert_eq!(old_state.last_success, Some(now1));
        assert_eq!(old_state.error_count, 5);
        assert_eq!(old_state.consecutive_failures, 2);
        assert_eq!(old_state.status, TaskStatus::Failed);
    }

    #[test]
    fn test_mergeable_trait_empty_states() {
        let mut state1 = TaskState::new();
        let state2 = TaskState::new();

        // Merging two empty states should work and produce an empty state
        state1.merge_from(&state2).unwrap();
        assert_eq!(state1, TaskState::default());
    }

    #[test]
    fn test_mergeable_trait_state_with_mistakes_array() {
        use crate::Mergeable;

        let mut old_state = State::new();
        let mut new_state = State::new();

        // Add mistakes to both states
        old_state.add_mistake("Old error".to_string(), Some("task1".to_string()));
        new_state.add_mistake("New error".to_string(), Some("task2".to_string()));

        // Merge - old_state's mistakes should be preserved (arrays not empty)
        old_state.merge_from(&new_state).unwrap();

        // Mistakes array should be preserved (not replaced) since old array is not empty
        assert_eq!(old_state.mistakes.len(), 1);
        assert_eq!(old_state.mistakes[0].error_message, "Old error");
        assert_eq!(old_state.mistakes[0].task, Some("task1".to_string()));
    }

    #[test]
    fn test_mergeable_trait_with_all_taskstate_fields() {
        use crate::Mergeable;

        let mut old_state = State::new();
        let mut new_state = State::new();

        // Only add mistakes to new_state (old_state array is empty)
        new_state.add_mistake("New error".to_string(), Some("task2".to_string()));

        // Merge - empty mistakes array is filled (existing non-empty arrays preserved)
        old_state.merge_from(&new_state).unwrap();

        // Mistakes array is filled from new_state since old array is empty
        assert_eq!(old_state.mistakes.len(), 1);
        assert_eq!(old_state.mistakes[0].error_message, "New error");
        assert_eq!(old_state.mistakes[0].task, Some("task2".to_string()));
    }

    #[test]
    fn test_deep_merge_json_objects() {
        let base = json!({
            "a": 1,
            "b": {
                "x": 10,
                "y": 20
            }
        });
        let patch = json!({
            "b": {
                "y": 30,
                "z": 40
            },
            "c": 3
        });

        let merged = deep_merge_json(base, patch);

        assert_eq!(merged["a"], 1);
        assert_eq!(merged["b"]["x"], 10); // Preserved from base
        assert_eq!(merged["b"]["y"], 20); // Preserved from base (existing value)
        assert_eq!(merged["b"]["z"], 40); // Added from patch (new key)
        assert_eq!(merged["c"], 3); // Added from patch (new key)
    }

    #[test]
    fn test_deep_merge_json_null_handling() {
        let base = json!({
            "a": null,
            "b": {
                "x": 10
            }
        });
        let patch = json!({
            "a": 1,
            "b": null
        });

        let merged = deep_merge_json(base, patch);

        assert_eq!(merged["a"], 1); // Null replaced with value
        assert_eq!(merged["b"]["x"], 10); // Nested object's existing value preserved
    }

    #[test]
    fn test_deep_merge_json_arrays() {
        // Test 1: Non-empty array is preserved
        let base = json!({
            "items": [1, 2, 3]
        });
        let patch = json!({
            "items": [4, 5, 6]
        });

        let merged = deep_merge_json(base, patch);

        // Arrays are preserved (not replaced) if base is not empty
        assert_eq!(merged["items"], json!([1, 2, 3]));
    }

    #[test]
    fn test_deep_merge_json_deeply_nested() {
        let base = json!({
            "level1": {
                "level2": {
                    "level3": {
                        "value": "base"
                    }
                }
            }
        });
        let patch = json!({
            "level1": {
                "level2": {
                    "level3": {
                        "value": "patched"
                    },
                    "new_field": "added"
                }
            }
        });

        let merged = deep_merge_json(base, patch);

        assert_eq!(merged["level1"]["level2"]["level3"]["value"], "base");
        assert_eq!(merged["level1"]["level2"]["new_field"], "added");
    }

    #[test]
    fn test_mergeable_trait_with_taskstate_fields() {
        let mut old_state = TaskState::new();
        let now1 = Utc::now() - chrono::Duration::hours(24); // 1 day ago

        old_state.last_run = Some(now1);
        old_state.error_count = 3;
        old_state.consecutive_failures = 1;
        old_state.status = TaskStatus::Success;
        old_state.successful_terminations = 2;
        old_state.total_execution_time_ms = 10000;
        old_state.average_execution_time_ms = 5000;

        let now2 = Utc::now();
        let new_state = TaskState {
            last_run: Some(now2),
            last_success: Some(now2),
            last_failure: Some(now2),
            last_termination_reason: Some("Test".to_string()),
            early_termination_count: 5,
            failed_terminations: 1,
            ..TaskState::default()
        };

        old_state.merge_from(&new_state).unwrap();

        // Existing fields preserved
        assert_eq!(old_state.last_run, Some(now1));
        assert_eq!(old_state.error_count, 3);
        assert_eq!(old_state.consecutive_failures, 1);
        assert_eq!(old_state.status, TaskStatus::Success);
        assert_eq!(old_state.successful_terminations, 2);
        assert_eq!(old_state.total_execution_time_ms, 10000);
        assert_eq!(old_state.average_execution_time_ms, 5000);

        // Missing fields added from new_state
        assert_eq!(old_state.last_success, Some(now2));
        assert_eq!(old_state.last_failure, Some(now2));
        assert_eq!(old_state.last_termination_reason, Some("Test".to_string()));
        assert_eq!(old_state.early_termination_count, 0); // Existing value preserved
        assert_eq!(old_state.failed_terminations, 1);
    }

    #[test]
    fn test_merge_missing_fields_deprecated() {
        let mut old_state = TaskState::new();
        let now = Utc::now();
        let mut new_state = TaskState::new();
        new_state.last_run = Some(now);

        // The deprecated method should still work for backward compatibility
        #[allow(deprecated)]
        {
            old_state.merge_missing_fields(&new_state);
        }

        assert_eq!(old_state.last_run, Some(now));
    }

    #[test]
    fn test_average_execution_time_with_early_termination() {
        let mut state = State::new();

        // Early termination: 2000ms
        state.record_early_termination("Timeout".to_string(), Some(2000), false);
        assert_eq!(state.average_execution_time_ms, 2000);

        // Successful execution: 4000ms
        state.record_success(Utc::now(), 4000);
        assert_eq!(state.average_execution_time_ms, 3000); // (2000 + 4000) / 2
    }
}

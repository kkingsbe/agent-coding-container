//! File-based locking using fs2 for cross-platform file locking.
//!
//! This module provides file-based locking mechanisms that allow multiple processes
//! and agents to coordinate access to shared resources through lock files. It uses
//! the `fs2` crate for cross-platform file locking.
//!
//! # Locking Levels
//!
//! This module provides two distinct locking levels for different use cases:
//!
//! ## Agent-Level Locking (`LockManager`)
//!
//! Used for coordinating access to agent-level resources. Each agent has a lock file
//! with its process ID (PID). Locks are released when the process ends.
//!
//! - **Purpose**: Ensure only one instance of an agent runs at a time
//! - **Mechanism**: File-level locks via `fs2::FileExt` + PID tracking
//! - **Lifetime**: Process lifetime (released on process exit)
//! - **Use case**: Preventing duplicate agent instances
//!
//! ## Task-Level Locking (`TaskLockManager`)
//!
//! Used for coordinating access to individual tasks by parallel agents. Tasks have
//! lease files with expiration times. Locks expire after the lease duration.
//!
//! - **Purpose**: Enable parallel agents to coordinate task execution
//! - **Mechanism**: JSON lease files with time-based expiration
//! - **Lifetime**: Lease duration (renewable by holder)
//! - **Use case**: Parallel task execution across multiple agents
//!
//! # Locking Strategy
//!
//! The agent-level locking system uses a combination of:
//! - **File-level locks** via `fs2::FileExt` to prevent concurrent access
//! - **Lock file metadata** to track which process and host holds the lock
//! - **Stale lock detection** to clean up locks from dead processes
//!
//! The task-level locking system uses:
//! - **Lease files** stored as JSON with expiration timestamps
//! - **Agent ID tracking** to identify lease holders
//! - **Time-based expiration** for automatic lock release
//! - **Lease renewal** for long-running task execution
//!
//! # RAII Pattern
//!
//! Agent-level lock handles (`LockHandle`) implement the `Drop` trait, ensuring locks
//! are automatically released when the handle goes out of scope. This prevents resource
//! leaks and makes the API safe and ergonomic.
//!
//! Task-level locks (`TaskLock`) represent the state of a lease but do not use RAII
//! for release, as leases are explicitly managed through `TaskLockManager` methods.
//!
//! # Stale Lock Handling
//!
//! The agent-level system can detect and clean up stale locks in several scenarios:
//! - Process is dead (PID no longer exists)
//! - Lock file is older than a specified timeout
//! - Lock is from a different host and has expired
//! - Lock was created by the current process before a restart
//!
//! The task-level system handles expired leases through:
//! - Automatic lease expiration checking
//! - `cleanup_expired_leases()` for batch cleanup
//! - `force_release()` for recovery scenarios
//!
//! # Example: Agent-Level Locking
//!
//! ```no_run
//! use automation_state::lock::{LockManager, get_current_process_id, get_hostname};
//! use std::time::Duration;
//! use std::path::PathBuf;
//!
//! // Create a lock manager
//! let lock_manager = LockManager::new(
//!     PathBuf::from("/tmp/my-task.lock"),
//!     Duration::from_secs(30)
//! );
//!
//! // Acquire a lock (blocking with timeout)
//! let lock_handle = lock_manager.acquire_lock().unwrap();
//!
//! // Do work while holding the lock
//! println!("Lock acquired, doing work...");
//!
//! // Lock is automatically released when lock_handle goes out of scope
//! // Or you can explicitly release it:
//! drop(lock_handle);
//! ```
//!
//! # Example: Task-Level Locking
//!
//! ```no_run
//! use automation_state::lock::TaskLockManager;
//! use std::path::PathBuf;
//! use std::time::Duration;
//!
//! // Create a task lock manager
//! let task_manager = TaskLockManager::new(
//!     PathBuf::from(".state/tasks/locks")
//! ).unwrap();
//!
//! // Acquire a task lock with 60 second lease
//! let task_lock = task_manager.acquire_lock(
//!     "task-123",
//!     "agent-1",
//!     Duration::from_secs(60)
//! ).unwrap();
//!
//! // Do work on the task...
//!
//! // Renew lease if task takes longer than expected
//! task_manager.renew_lease(
//!     "task-123",
//!     "agent-1",
//!     Duration::from_secs(60)
//! ).unwrap();
//!
//! // Release lock when done
//! task_manager.release_lock("task-123", "agent-1").unwrap();
//! ```
//!
//! # Non-blocking Lock Acquisition (Agent-Level)
//!
//! ```no_run
//! use automation_state::lock::LockManager;
//! use std::path::PathBuf;
//!
//! let lock_manager = LockManager::new(
//!     PathBuf::from("/tmp/my-task.lock"),
//!     Default::default()
//! );
//!
//! // Try to acquire lock without blocking
//! match lock_manager.try_acquire_lock().unwrap() {
//!     Some(lock_handle) => {
//!         // Lock acquired, do work
//!     }
//!     None => {
//!         // Lock is held by another process
//!         println!("Resource is busy, try again later");
//!     }
//! }
//! ```

use automation_common::{Result, AutomationError};
use crate::state::LockInfo;
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Default timeout for lock acquisition (30 seconds)
const DEFAULT_LOCK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Default retry interval when waiting for a lock (100ms)
const LOCK_RETRY_INTERVAL: Duration = Duration::from_millis(100);

/// Default stale lock cleanup timeout (5 minutes)
#[allow(dead_code)]
const DEFAULT_STALE_TIMEOUT: Duration = Duration::from_secs(300);

/// Default cross-host lock cleanup threshold (1 minute)
#[deprecated(note = "Use LockCleanupConfig::default() instead")]
const CROSS_HOST_LOCK_THRESHOLD: Duration = Duration::from_secs(60);

/// Configuration for lock cleanup behavior.
///
/// This struct encapsulates all thresholds and flags that control
/// when locks are considered stale and should be cleaned up.
///
/// # Example
///
/// ```
/// use automation_state::lock::LockCleanupConfig;
/// use std::time::Duration;
///
/// let config = LockCleanupConfig {
///     stale_timeout: Duration::from_secs(300),
///     cross_host_timeout: Duration::from_secs(7200),
///     check_process_alive: true,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct LockCleanupConfig {
    /// Maximum age for a lock before it's considered stale (for current host).
    ///
    /// When a lock on the current host is older than this duration,
    /// it will be considered for cleanup even if the process is still running.
    pub stale_timeout: Duration,

    /// Maximum age for cross-host locks (different hostname).
    ///
    /// Locks from different hosts are cleaned up more aggressively to prevent
    /// deadlocks when multiple hosts are competing for the same resource.
    /// This should typically be shorter than `stale_timeout`.
    pub cross_host_timeout: Duration,

    /// Whether to check if the process is still alive.
    ///
    /// When true, locks from processes that are no longer running will be
    /// cleaned up regardless of their age. This may not work correctly in
    /// all environments (e.g., containers with PID namespaces).
    pub check_process_alive: bool,
}

impl Default for LockCleanupConfig {
    fn default() -> Self {
        Self {
            stale_timeout: Duration::from_secs(300),      // 5 minutes
            cross_host_timeout: Duration::from_secs(60),  // 1 minute
            check_process_alive: true,
        }
    }
}

/// Represents the evaluation result for a lock cleanup decision.
///
/// This enum makes the cleanup decision logic explicit and testable.
#[derive(Debug, PartialEq)]
enum LockCleanupDecision {
    /// Lock should be cleaned up
    Cleanup { reason: String },
    /// Lock should be kept
    Keep,
}

impl LockCleanupDecision {
    /// Creates a cleanup decision with the given reason.
    fn cleanup(reason: impl Into<String>) -> Self {
        Self::Cleanup { reason: reason.into() }
    }

    /// Returns true if the lock should be cleaned up.
    fn should_cleanup(&self) -> bool {
        matches!(self, LockCleanupDecision::Cleanup { .. })
    }
}

/// Context for evaluating whether a lock should be cleaned up.
///
/// This struct encapsulates all information needed to make a cleanup
/// decision, making the evaluation logic pure and easily testable.
struct LockEvaluationContext {
    lock_info: LockInfo,
    lock_age: Duration,
    current_pid: u32,
    current_hostname: String,
    config: LockCleanupConfig,
}

impl LockEvaluationContext {
    /// Creates a new lock evaluation context.
    fn new(
        lock_info: LockInfo,
        current_pid: u32,
        current_hostname: String,
        config: LockCleanupConfig,
    ) -> Self {
        let lock_age = lock_info.age();
        Self {
            lock_info,
            lock_age: lock_age.to_std()
                .unwrap_or_else(|_| Duration::from_secs(0)),
            current_pid,
            current_hostname,
            config,
        }
    }

    /// Evaluates whether the lock should be cleaned up.
    ///
    /// This method implements the lock cleanup decision rules in a clear,
    /// linear fashion. Each rule is explicitly named and documented.
    ///
    /// # Rules
    ///
    /// 1. **No PID**: Locks without a PID are invalid and should be cleaned up
    /// 2. **Process Dead**: Locks from non-running processes should be cleaned up
    /// 3. **Stale Timeout**: Locks older than the threshold are stale
    /// 4. **Cross-Host**: Locks from different hosts use a shorter timeout
    /// 5. **Current Process**: Locks from the current PID that passed all above checks
    ///    are stale (from previous runs)
    /// 6. **Keep**: Locks that pass all checks are still valid
    ///
    /// # Returns
    ///
    /// A `LockCleanupDecision` indicating whether to clean up or keep the lock.
    fn evaluate(&self) -> LockCleanupDecision {
        // Rule 1: No PID means lock is invalid
        let lock_pid = match self.lock_info.process_id {
            Some(pid) => pid,
            None => return LockCleanupDecision::cleanup("no PID in lock info"),
        };

        // Rule 2: Process not running
        if self.config.check_process_alive && !is_process_alive(lock_pid) {
            return LockCleanupDecision::cleanup("process is not running");
        }

        // Rule 3: Lock too old (stale)
        if self.lock_age > self.config.stale_timeout {
            return LockCleanupDecision::cleanup("lock age exceeds threshold");
        }

        // Rule 4: Cross-host lock
        if self.lock_info.hostname.as_deref() != Some(self.current_hostname.as_str()) {
            if self.lock_age > self.config.cross_host_timeout {
                return LockCleanupDecision::cleanup("cross-host lock expired");
            }
            return LockCleanupDecision::Keep;
        }

        // Rule 5: Lock from current process that passed all above checks is stale (from previous run)
        // This handles the case where a process restarts and gets the same PID
        if lock_pid == self.current_pid {
            return LockCleanupDecision::cleanup("stale lock from current process");
        }

        // Rule 6: Lock is still valid
        LockCleanupDecision::Keep
    }
}

/// Lock file content structure (for backward compatibility with Node.js)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyLockData {
    pid: Option<u32>,
    timestamp: Option<i64>,
    host: Option<String>,
}

/// A file-based lock manager that coordinates access to shared resources.
///
/// The `LockManager` provides methods to acquire, release, and manage file-based locks.
/// It uses the `fs2` crate for cross-platform file locking capabilities.
///
/// # Example
///
/// ```no_run
/// use automation_state::lock::LockManager;
/// use std::path::PathBuf;
/// use std::time::Duration;
///
/// let manager = LockManager::new(
///     PathBuf::from("/tmp/task.lock"),
///     Duration::from_secs(10)
/// );
///
/// // Acquire a lock
/// let lock = manager.acquire_lock().unwrap();
/// // ... work ...
/// drop(lock); // Auto-release
/// ```
#[derive(Debug, Clone)]
pub struct LockManager {
    /// Path to the lock file
    lock_file_path: PathBuf,
    /// Timeout for lock acquisition
    timeout: Duration,
}

impl LockManager {
    /// Creates a new lock manager with the specified lock file path and timeout.
    ///
    /// # Arguments
    ///
    /// * `lock_file_path` - Path to the lock file
    /// * `timeout` - Maximum time to wait for lock acquisition
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::lock::LockManager;
    /// use std::path::PathBuf;
    /// use std::time::Duration;
    ///
    /// let manager = LockManager::new(
    ///     PathBuf::from("/tmp/my-task.lock"),
    ///     Duration::from_secs(30)
    /// );
    /// ```
    pub fn new(lock_file_path: PathBuf, timeout: Duration) -> Self {
        LockManager {
            lock_file_path,
            timeout,
        }
    }

    /// Creates a new lock manager with default timeout (30 seconds).
    ///
    /// # Arguments
    ///
    /// * `lock_file_path` - Path to the lock file
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::lock::LockManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = LockManager::new_default(PathBuf::from("/tmp/my-task.lock"));
    /// ```
    pub fn new_default(lock_file_path: PathBuf) -> Self {
        LockManager::new(lock_file_path, DEFAULT_LOCK_TIMEOUT)
    }

    /// Acquires an exclusive lock on the lock file.
    ///
    /// This method will block until the lock is acquired or the timeout is reached.
    /// If the lock file doesn't exist, it will be created. The lock information
    /// (process ID, hostname, timestamp) is written to the lock file.
    ///
    /// # Arguments
    ///
    /// # Returns
    ///
    /// A `LockHandle` that will automatically release the lock when dropped.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The lock file cannot be created or opened
    /// - The lock cannot be acquired within the timeout period
    /// - Lock information cannot be written to the file
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::lock::LockManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = LockManager::new_default(PathBuf::from("/tmp/task.lock"));
    /// let lock = manager.acquire_lock().unwrap();
    /// // ... work while holding lock ...
    /// drop(lock); // Lock is automatically released
    /// ```
    pub fn acquire_lock(&self) -> Result<LockHandle> {
        let start_time = std::time::Instant::now();
        let pid = get_current_process_id();
        let hostname = get_hostname();

        // Ensure parent directory exists
        if let Some(parent) = self.lock_file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| AutomationError::LockError {
                task: self.lock_file_path.to_string_lossy().to_string(),
                message: format!("Failed to create lock directory: {}", e),
            })?;
        }

        loop {
            // Try to create and open the file
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&self.lock_file_path)
                .map_err(|e| AutomationError::LockError {
                    task: self.lock_file_path.to_string_lossy().to_string(),
                    message: format!("Failed to open lock file: {}", e),
                })?;

            // Try to acquire exclusive lock
            match file.try_lock_exclusive() {
                Ok(()) => {
                    // Lock acquired successfully, write lock info
                    let lock_info = LockInfo::new(Some(pid), Some(hostname));
                    let lock_info_json = serde_json::to_string_pretty(&lock_info)?;

                    // Write lock info to file
                    file.set_len(0)
                        .and_then(|_| file.write_all(lock_info_json.as_bytes()))
                        .and_then(|_| file.flush())
                        .map_err(|e| AutomationError::LockError {
                            task: self.lock_file_path.to_string_lossy().to_string(),
                            message: format!("Failed to write lock info: {}", e),
                        })?;

                    return Ok(LockHandle {
                        file,
                        path: self.lock_file_path.clone(),
                    });
                }
                Err(_) => {
                    // Lock is held by another process
                    if start_time.elapsed() >= self.timeout {
                        return Err(AutomationError::LockTimeout {
                            task: self.lock_file_path.to_string_lossy().to_string(),
                            timeout: self.timeout.as_millis() as u64,
                        });
                    }

                    // Wait before retrying
                    std::thread::sleep(LOCK_RETRY_INTERVAL);
                }
            }
        }
    }

    /// Attempts to acquire a lock without blocking.
    ///
    /// Returns `Some(LockHandle)` if the lock was acquired immediately,
    /// or `None` if the lock is held by another process.
    ///
    /// # Returns
    ///
    /// * `Some(LockHandle)` - Lock was acquired
    /// * `None` - Lock is held by another process
    ///
    /// # Errors
    ///
    /// Returns an error if the lock file cannot be created or opened.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::lock::LockManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = LockManager::new_default(PathBuf::from("/tmp/task.lock"));
    ///
    /// match manager.try_acquire_lock().unwrap() {
    ///     Some(lock) => {
    ///         // Lock acquired, do work
    ///         println!("Working with lock...");
    ///         drop(lock);
    ///     }
    ///     None => {
    ///         // Lock is busy
    ///         println!("Resource is busy");
    ///     }
    /// }
    /// ```
    pub fn try_acquire_lock(&self) -> Result<Option<LockHandle>> {
        let pid = get_current_process_id();
        let hostname = get_hostname();

        // Ensure parent directory exists
        if let Some(parent) = self.lock_file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| AutomationError::LockError {
                task: self.lock_file_path.to_string_lossy().to_string(),
                message: format!("Failed to create lock directory: {}", e),
            })?;
        }

        // Try to create and open the file
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.lock_file_path)
            .map_err(|e| AutomationError::LockError {
                task: self.lock_file_path.to_string_lossy().to_string(),
                message: format!("Failed to open lock file: {}", e),
            })?;

        // Try to acquire exclusive lock
        match file.try_lock_exclusive() {
            Ok(()) => {
                // Lock acquired successfully, write lock info
                let lock_info = LockInfo::new(Some(pid), Some(hostname));
                let lock_info_json = serde_json::to_string_pretty(&lock_info)?;

                // Write lock info to file
                file.set_len(0)
                    .and_then(|_| file.write_all(lock_info_json.as_bytes()))
                    .and_then(|_| file.flush())
                    .map_err(|e| AutomationError::LockError {
                        task: self.lock_file_path.to_string_lossy().to_string(),
                        message: format!("Failed to write lock info: {}", e),
                    })?;

                Ok(Some(LockHandle {
                    file,
                    path: self.lock_file_path.clone(),
                }))
            }
            Err(_) => {
                // Lock is held by another process
                Ok(None)
            }
        }
    }

    /// Checks if the lock is currently held by any process.
    ///
    /// This method attempts to acquire a non-blocking lock. If it succeeds,
    /// it immediately releases the lock and returns `false`. If it fails,
    /// the lock is held by another process and it returns `true`.
    ///
    /// # Returns
    ///
    /// * `true` - Lock is held by another process
    /// * `false` - Lock is not held
    ///
    /// # Errors
    ///
    /// Returns an error if the lock file cannot be opened.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::lock::LockManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = LockManager::new_default(PathBuf::from("/tmp/task.lock"));
    ///
    /// if manager.is_locked().unwrap() {
    ///     println!("Resource is locked by another process");
    /// } else {
    ///     println!("Resource is available");
    /// }
    /// ```
    pub fn is_locked(&self) -> Result<bool> {
        // Ensure parent directory exists
        if let Some(parent) = self.lock_file_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Try to open the file
        let file = match OpenOptions::new()
            .read(true)
            .write(true)
            .create(false)
            .open(&self.lock_file_path)
        {
            Ok(f) => f,
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
                // Lock file doesn't exist, so it's not locked
                return Ok(false);
            }
            Err(e) => {
                return Err(AutomationError::LockError {
                    task: self.lock_file_path.to_string_lossy().to_string(),
                    message: format!("Failed to open lock file: {}", e),
                });
            }
        };

        // Try to acquire exclusive lock
        match file.try_lock_exclusive() {
            Ok(()) => {
                // Lock was not held, immediately release and return false
                let _ = file.unlock();
                Ok(false)
            }
            Err(_) => {
                // Lock is held by another process
                Ok(true)
            }
        }
    }

    /// Cleans up stale locks.
    ///
    /// A lock is considered stale if:
    /// - The process that created it is no longer running
    /// - The lock file is older than `stale_timeout`
    /// - The lock is from a different host and has expired
    /// - The lock is from the same PID but the process was restarted
    ///
    /// This method uses a configurable `LockCleanupConfig` to control cleanup behavior.
    /// The cross-host timeout defaults to 1 minute for backward compatibility.
    ///
    /// # Arguments
    ///
    /// * `stale_timeout` - Maximum age of a lock before it's considered stale
    ///
    /// # Returns
    ///
    /// * `true` - A stale lock was cleaned up
    /// * `false` - No stale lock was found
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::lock::LockManager;
    /// use std::path::PathBuf;
    /// use std::time::Duration;
    ///
    /// let manager = LockManager::new_default(PathBuf::from("/tmp/task.lock"));
    ///
    /// // Clean up locks older than 5 minutes
    /// if manager.cleanup_stale_lock(Duration::from_secs(300)).unwrap() {
    ///     println!("Cleaned up stale lock");
    /// }
    /// ```
    pub fn cleanup_stale_lock(&self, stale_timeout: Duration) -> Result<bool> {
        self.cleanup_stale_lock_with_config(
            stale_timeout,
            LockCleanupConfig::default(),
        )
    }

    /// Cleans up stale locks with custom configuration.
    ///
    /// This method allows full control over lock cleanup behavior by accepting
    /// a `LockCleanupConfig` parameter. Use this when you need to customize
    /// the cross-host timeout or process alive checking behavior.
    ///
    /// # Arguments
    ///
    /// * `stale_timeout` - Maximum age of a lock before it's considered stale
    /// * `config` - Configuration for lock cleanup behavior
    ///
    /// # Returns
    ///
    /// * `true` - A stale lock was cleaned up
    /// * `false` - No stale lock was found
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::lock::{LockManager, LockCleanupConfig};
    /// use std::path::PathBuf;
    /// use std::time::Duration;
    ///
    /// let manager = LockManager::new_default(PathBuf::from("/tmp/task.lock"));
    ///
    /// let config = LockCleanupConfig {
    ///     stale_timeout: Duration::from_secs(300),
    ///     cross_host_timeout: Duration::from_secs(7200),
    ///     check_process_alive: true,
    /// };
    ///
    /// if manager.cleanup_stale_lock_with_config(Duration::from_secs(300), config).unwrap() {
    ///     println!("Cleaned up stale lock");
    /// }
    /// ```
    pub fn cleanup_stale_lock_with_config(
        &self,
        stale_timeout: Duration,
        config: LockCleanupConfig,
    ) -> Result<bool> {
        // Check if lock file exists
        if !self.lock_file_path.exists() {
            return Ok(false);
        }

        // Try to read lock info
        let lock_info = match self.read_lock_info_from_file() {
            Ok(Some(info)) => info,
            Ok(None) => {
                // No valid lock info, try to remove the file
                self.remove_lock_file()?;
                return Ok(true);
            }
            Err(_) => {
                // Error reading lock info, try to remove corrupted file
                self.remove_lock_file()?;
                return Ok(true);
            }
        };

        // Build evaluation context with custom stale_timeout
        let full_config = LockCleanupConfig {
            stale_timeout,
            ..config
        };

        let context = LockEvaluationContext::new(
            lock_info.clone(),
            get_current_process_id(),
            get_hostname(),
            full_config,
        );

        // Evaluate lock
        match context.evaluate() {
            LockCleanupDecision::Cleanup { reason } => {
                tracing::debug!(
                    "Cleaning lock {:?}: {}",
                    self.lock_file_path,
                    reason
                );
                self.remove_lock_file()?;
                Ok(true)
            }
            LockCleanupDecision::Keep => Ok(false),
        }
    }

    /// Reads lock information from the lock file.
    ///
    /// # Returns
    ///
    /// * `Some(LockInfo)` - Lock information was successfully read
    /// * `None` - Lock file doesn't exist or is empty
    ///
    /// # Errors
    ///
    /// Returns an error if the lock file cannot be read or parsed.
    fn read_lock_info_from_file(&self) -> Result<Option<LockInfo>> {
        if !self.lock_file_path.exists() {
            return Ok(None);
        }

        let file = OpenOptions::new()
            .read(true)
            .write(false)
            .create(false)
            .open(&self.lock_file_path)?;

        let mut content = String::new();
        (&file).read_to_string(&mut content)?;

        // Try to parse as LockInfo (new format)
        if let Ok(lock_info) = serde_json::from_str::<LockInfo>(&content) {
            return Ok(Some(lock_info));
        }

        // Try to parse as LegacyLockData (Node.js format)
        if let Ok(legacy_data) = serde_json::from_str::<LegacyLockData>(&content) {
            let lock_info = LockInfo {
                acquired_at: legacy_data
                    .timestamp
                    .map(|ts| DateTime::<Utc>::from_timestamp_millis(ts))
                    .flatten()
                    .unwrap_or_else(Utc::now),
                process_id: legacy_data.pid,
                hostname: legacy_data.host,
            };
            return Ok(Some(lock_info));
        }

        Err(AutomationError::LockError {
            task: self.lock_file_path.to_string_lossy().to_string(),
            message: "Invalid lock file format".to_string(),
        })
    }

    /// Removes the lock file.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock file cannot be removed.
    fn remove_lock_file(&self) -> Result<()> {
        fs::remove_file(&self.lock_file_path)?;
        Ok(())
    }
}

/// A handle to an acquired lock.
///
/// The `LockHandle` represents an acquired lock and implements the `Drop` trait
/// to automatically release the lock when it goes out of scope. This follows the
/// RAII (Resource Acquisition Is Initialization) pattern.
///
/// # Example
///
/// ```no_run
/// use automation_state::lock::LockManager;
/// use std::path::PathBuf;
///
/// let manager = LockManager::new_default(PathBuf::from("/tmp/task.lock"));
/// {
///     let lock = manager.acquire_lock().unwrap();
///     // ... work while holding lock ...
/// } // Lock is automatically released here
/// ```
#[derive(Debug)]
pub struct LockHandle {
    file: File,
    path: PathBuf,
}

impl LockHandle {
    /// Returns the path to the lock file.
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::lock::LockManager;
    /// use std::path::PathBuf;
    /// use tempfile::tempdir;
    ///
    /// let temp_dir = tempdir().unwrap();
    /// let lock_path = temp_dir.path().join("test.lock");
    /// let manager = LockManager::new_default(lock_path.clone());
    /// let lock = manager.acquire_lock().unwrap();
    ///
    /// assert_eq!(lock.path(), lock_path.as_path());
    /// ```
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads and returns the lock information from the lock file.
    ///
    /// # Returns
    ///
    /// The `LockInfo` stored in the lock file.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock file cannot be read or parsed.
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::lock::LockManager;
    /// use std::path::PathBuf;
    /// use tempfile::tempdir;
    ///
    /// let temp_dir = tempdir().unwrap();
    /// let lock_path = temp_dir.path().join("test.lock");
    /// let manager = LockManager::new_default(lock_path);
    /// let lock = manager.acquire_lock().unwrap();
    ///
    /// let lock_info = lock.lock_info().unwrap();
    /// assert!(lock_info.process_id.is_some());
    /// ```
    pub fn lock_info(&mut self) -> Result<LockInfo> {
        let mut content = String::new();
        self.file.read_to_string(&mut content)?;

        serde_json::from_str::<LockInfo>(&content).map_err(|e| AutomationError::LockError {
            task: self.path.to_string_lossy().to_string(),
            message: format!("Failed to parse lock info: {}", e),
        })
    }

    /// Explicitly releases the lock and removes the lock file.
    ///
    /// This method is called automatically when the `LockHandle` is dropped,
    /// but can be called explicitly if needed.
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::lock::LockManager;
    /// use std::path::PathBuf;
    /// use tempfile::tempdir;
    ///
    /// let temp_dir = tempdir().unwrap();
    /// let lock_path = temp_dir.path().join("test.lock");
    /// let manager = LockManager::new_default(lock_path);
    /// let lock = manager.acquire_lock().unwrap();
    ///
    /// lock.release();
    /// assert!(!lock_path.exists());
    /// ```
    pub fn release(self) {
        // The Drop implementation will handle release
        drop(self);
    }
}

impl Drop for LockHandle {
    fn drop(&mut self) {
        // Release the file lock
        if let Err(e) = self.file.unlock() {
            tracing::warn!(
                "Failed to unlock file {:?}: {}",
                self.path,
                e
            );
        }

        // Remove the lock file
        if let Err(e) = fs::remove_file(&self.path) {
            if e.kind() != io::ErrorKind::NotFound {
                tracing::warn!(
                    "Failed to remove lock file {:?}: {}",
                    self.path,
                    e
                );
            }
        }
    }
}

/// Returns the current process ID.
///
/// # Returns
///
/// The current process ID as a `u32`.
///
/// # Example
///
/// ```
/// use automation_state::lock::get_current_process_id;
///
/// let pid = get_current_process_id();
/// assert!(pid > 0);
/// ```
pub fn get_current_process_id() -> u32 {
    std::process::id()
}

/// Returns the system hostname.
///
/// # Returns
///
/// The hostname as a `String`, or "unknown" if it cannot be determined.
///
/// # Example
///
/// ```
/// use automation_state::lock::get_hostname;
///
/// let hostname = get_hostname();
/// assert!(!hostname.is_empty());
/// ```
pub fn get_hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| {
            // Try to get from system on Unix
            #[cfg(unix)]
            {
                use std::process::Command;
                Command::new("hostname")
                    .output()
                    .ok()
                    .and_then(|output| String::from_utf8(output.stdout).ok())
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            }

            #[cfg(not(unix))]
            {
                "unknown".to_string()
            }
        })
}

/// Checks if a process with the given PID is still running.
///
/// # Arguments
///
/// * `pid` - The process ID to check
///
/// # Returns
///
/// * `true` - The process is running
/// * `false` - The process is not running
///
/// # Example
///
/// ```
/// use automation_state::lock::{get_current_process_id, is_process_alive};
///
/// let current_pid = get_current_process_id();
/// assert!(is_process_alive(current_pid));
///
/// // Non-existent PID should return false
/// assert!(!is_process_alive(9999999));
/// ```
pub fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        use std::process::Command;
        
        // Try to check if process exists using kill -0
        let result = Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .output();
        
        match result {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    #[cfg(windows)]
    {
        use std::process::Command;
        
        // Try to check if process exists using tasklist
        let result = Command::new("tasklist")
            .arg("/FI")
            .arg(format!("PID eq {}", pid))
            .output();
        
        match result {
            Ok(output) => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                output_str.contains(&pid.to_string())
            }
            Err(_) => false,
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        // Default: assume process is alive if PID matches current
        pid == std::process::id()
    }
}

// ============================================================================
// TaskLockManager Module
// ============================================================================

/// Lease data structure for task locks.
///
/// Represents the lease information stored in JSON files for task-level locking.
/// This differs from LockInfo which is used for agent-level (PID-based) locking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct LeaseData {
    /// Agent ID that holds the lease
    holder: String,
    /// Timestamp when the lease was acquired
    acquired_at: DateTime<Utc>,
    /// Timestamp when the lease expires
    expires_at: DateTime<Utc>,
}

impl LeaseData {
    /// Creates a new lease data entry.
    ///
    /// # Arguments
    ///
    /// * `holder` - The agent ID holding the lease
    /// * `lease_duration` - Duration until the lease expires
    fn new(holder: String, lease_duration: Duration) -> Self {
        let now = Utc::now();
        LeaseData {
            holder,
            acquired_at: now,
            expires_at: now + ChronoDuration::from_std(lease_duration)
                .unwrap_or_else(|_| ChronoDuration::seconds(300)),
        }
    }

    /// Checks if the lease is currently expired.
    fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Renews the lease with a new duration.
    ///
    /// # Arguments
    ///
    /// * `lease_duration` - Duration until the new lease expires
    fn renew(&mut self, lease_duration: Duration) {
        let now = Utc::now();
        self.acquired_at = now;
        self.expires_at = now + ChronoDuration::from_std(lease_duration)
            .unwrap_or_else(|_| ChronoDuration::seconds(300));
    }
}

/// A task lock with lease support.
///
/// Represents a lock on a specific task that can be held by an agent
/// for a limited duration (lease). After the lease expires, the lock
/// can be acquired by another agent.
///
/// This differs from `LockHandle` which is for agent-level locking
/// based on process IDs and file locks.
///
/// # Example
///
/// ```
/// use automation_state::lock::TaskLock;
/// use chrono::Utc;
///
/// let lock = TaskLock::new(
///     "/tmp/task1.lease".into(),
///     "agent-1".to_string(),
///     Utc::now(),
///     Utc::now() + chrono::Duration::seconds(60)
/// );
///
/// assert!(lock.is_held());
/// assert!(!lock.is_expired());
/// assert!(lock.holder_matches("agent-1"));
/// ```
#[derive(Debug, Clone)]
pub struct TaskLock {
    /// Path to the lease file
    lock_path: PathBuf,
    /// Agent ID that holds the lock (None if not locked)
    lease_holder: Option<String>,
    /// Timestamp when the lease was acquired
    lease_acquired_at: Option<DateTime<Utc>>,
    /// Timestamp when the lease expires
    lease_expires_at: Option<DateTime<Utc>>,
}

impl TaskLock {
    /// Creates a new task lock (unlocked state).
    ///
    /// # Arguments
    ///
    /// * `lock_path` - Path to the lease file
    pub fn new(lock_path: PathBuf) -> Self {
        TaskLock {
            lock_path,
            lease_holder: None,
            lease_acquired_at: None,
            lease_expires_at: None,
        }
    }

    /// Creates a new task lock from lease data.
    ///
    /// # Arguments
    ///
    /// * `lock_path` - Path to the lease file
    /// * `lease_data` - The lease data from the file
    fn from_lease_data(lock_path: PathBuf, lease_data: LeaseData) -> Self {
        TaskLock {
            lock_path,
            lease_holder: Some(lease_data.holder),
            lease_acquired_at: Some(lease_data.acquired_at),
            lease_expires_at: Some(lease_data.expires_at),
        }
    }

    /// Creates a new task lock with explicit lease information.
    ///
    /// # Arguments
    ///
    /// * `lock_path` - Path to the lease file
    /// * `holder` - Agent ID holding the lock
    /// * `acquired_at` - When the lease was acquired
    /// * `expires_at` - When the lease expires
    pub fn with_lease(
        lock_path: PathBuf,
        holder: String,
        acquired_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Self {
        TaskLock {
            lock_path,
            lease_holder: Some(holder),
            lease_acquired_at: Some(acquired_at),
            lease_expires_at: Some(expires_at),
        }
    }

    /// Returns the path to the lease file.
    pub fn path(&self) -> &Path {
        &self.lock_path
    }

    /// Returns the lease holder agent ID.
    pub fn holder(&self) -> Option<&str> {
        self.lease_holder.as_deref()
    }

    /// Returns when the lease was acquired.
    pub fn acquired_at(&self) -> Option<DateTime<Utc>> {
        self.lease_acquired_at
    }

    /// Returns when the lease expires.
    pub fn expires_at(&self) -> Option<DateTime<Utc>> {
        self.lease_expires_at
    }

    /// Checks if the lock is currently held by any agent.
    ///
    /// # Returns
    ///
    /// * `true` - Lock is held
    /// * `false` - Lock is not held
    pub fn is_held(&self) -> bool {
        self.lease_holder.is_some()
    }

    /// Checks if the lease is expired.
    ///
    /// # Returns
    ///
    /// * `true` - Lease is expired or not held
    /// * `false` - Lease is still valid
    pub fn is_expired(&self) -> bool {
        match self.lease_expires_at {
            Some(expires_at) => Utc::now() > expires_at,
            None => true, // No expiry means not locked
        }
    }

    /// Checks if the lock is held by the specified agent.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - The agent ID to check
    ///
    /// # Returns
    ///
    /// * `true` - Lock is held by this agent
    /// * `false` - Lock is held by another agent or not held
    pub fn holder_matches(&self, agent_id: &str) -> bool {
        match &self.lease_holder {
            Some(holder) => holder == agent_id,
            None => false,
        }
    }

    /// Calculates the remaining lease duration.
    ///
    /// # Returns
    ///
    /// The remaining duration as a std::time::Duration, or None if not held.
    pub fn remaining_duration(&self) -> Option<Duration> {
        self.lease_expires_at.and_then(|expires_at| {
            let now = Utc::now();
            if expires_at > now {
                (expires_at - now).to_std().ok()
            } else {
                Some(Duration::ZERO)
            }
        })
    }
}

/// A manager for task-level locks with lease support.
///
/// `TaskLockManager` provides task-level locking with time-based leases,
/// allowing parallel agents to coordinate access to individual tasks.
/// This is distinct from `LockManager` which provides agent-level locking
/// based on process IDs.
///
/// # Task Locking vs Agent Locking
///
/// - **LockManager**: Used for agent-level coordination. Each agent has
///   a lock file with its PID. Locks are released when the process ends.
/// - **TaskLockManager**: Used for task-level coordination. Tasks have
///   lease files with expiration times. Locks expire after the lease duration.
///
/// # Example
///
/// ```no_run
/// use automation_state::lock::TaskLockManager;
/// use std::path::PathBuf;
/// use std::time::Duration;
///
/// let manager = TaskLockManager::new(
///     PathBuf::from(".state/tasks/locks")
/// ).unwrap();
///
/// // Acquire a task lock with 60 second lease
/// let lock = manager.acquire_lock(
///     "task-123",
///     "agent-1",
///     Duration::from_secs(60)
/// ).unwrap();
///
/// // Renew the lease before it expires
/// manager.renew_lease(
///     "task-123",
///     "agent-1",
///     Duration::from_secs(60)
/// ).unwrap();
///
/// // Release the lock when done
/// manager.release_lock("task-123", "agent-1").unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct TaskLockManager {
    /// Base directory for lease files
    base_dir: PathBuf,
}

impl TaskLockManager {
    /// Default base directory for task locks
    const DEFAULT_BASE_DIR: &'static str = ".state/tasks/locks";

    /// Default lease duration (5 minutes)
    const DEFAULT_LEASE_DURATION: Duration = Duration::from_secs(300);

    /// Creates a new task lock manager.
    ///
    /// Creates the lock directory if it doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `base_dir` - Base directory for lease files
    ///
    /// # Returns
    ///
    /// A new `TaskLockManager` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock directory cannot be created.
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::lock::TaskLockManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = TaskLockManager::new(
    ///     PathBuf::from(".state/tasks/locks")
    /// ).unwrap();
    /// ```
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        // Create the lock directory if it doesn't exist
        fs::create_dir_all(&base_dir).map_err(|e| AutomationError::LockError {
            task: base_dir.to_string_lossy().to_string(),
            message: format!("Failed to create lock directory: {}", e),
        })?;

        Ok(TaskLockManager { base_dir })
    }

    /// Creates a new task lock manager with the default base directory.
    ///
    /// # Returns
    ///
    /// A new `TaskLockManager` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock directory cannot be created.
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::lock::TaskLockManager;
    ///
    /// let manager = TaskLockManager::default().unwrap();
    /// ```
    pub fn new_default() -> Result<Self> {
        Self::new(PathBuf::from(Self::DEFAULT_BASE_DIR))
    }

    /// Gets the lease file path for a task.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID
    ///
    /// # Returns
    ///
    /// The path to the lease file.
    fn lease_path(&self, task_id: &str) -> PathBuf {
        self.base_dir.join(format!("{}.lease", task_id))
    }

    /// Acquires an exclusive task lock with a lease.
    ///
    /// Creates a lease file with the specified duration. If a valid lease
    /// already exists (not expired), the acquisition fails.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID to lock
    /// * `agent_id` - The agent ID acquiring the lock
    /// * `lease_duration` - Duration until the lease expires
    ///
    /// # Returns
    ///
    /// A `TaskLock` representing the acquired lock.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The lease file cannot be created or written
    /// - The task is already locked by another agent
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::lock::TaskLockManager;
    /// use std::time::Duration;
    ///
    /// let manager = TaskLockManager::default().unwrap();
    ///
    /// let lock = manager.acquire_lock(
    ///     "task-123",
    ///     "agent-1",
    ///     Duration::from_secs(60)
    /// ).unwrap();
    /// ```
    pub fn acquire_lock(
        &self,
        task_id: &str,
        agent_id: &str,
        lease_duration: Duration,
    ) -> Result<TaskLock> {
        let lease_path = self.lease_path(task_id);

        // Check if a valid lease already exists
        if let Ok(Some(existing_lock)) = self.get_lock(task_id) {
            if !existing_lock.is_expired() && !existing_lock.holder_matches(agent_id) {
                return Err(AutomationError::LockError {
                    task: task_id.to_string(),
                    message: format!(
                        "Task is already locked by agent {}",
                        existing_lock.holder().unwrap_or("unknown")
                    ),
                });
            }
        }

        // Create new lease data
        let lease_data = LeaseData::new(agent_id.to_string(), lease_duration);

        // Write lease file atomically
        self.write_lease_atomically(&lease_path, &lease_data)?;

        // Create and return TaskLock
        Ok(TaskLock::from_lease_data(lease_path, lease_data))
    }

    /// Renews the lease for a task.
    ///
    /// Extends the lease expiration time for a task currently held by
    /// the specified agent.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID
    /// * `agent_id` - The agent ID holding the lock
    /// * `lease_duration` - New lease duration
    ///
    /// # Returns
    ///
    /// * `true` - Lease was renewed
    /// * `false` - Lock not held by this agent
    ///
    /// # Errors
    ///
    /// Returns an error if the lease file cannot be read or written.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::lock::TaskLockManager;
    /// use std::time::Duration;
    ///
    /// let manager = TaskLockManager::default().unwrap();
    ///
    /// // Renew the lease
    /// let renewed = manager.renew_lease(
    ///     "task-123",
    ///     "agent-1",
    ///     Duration::from_secs(60)
    /// ).unwrap();
    /// ```
    pub fn renew_lease(
        &self,
        task_id: &str,
        agent_id: &str,
        lease_duration: Duration,
    ) -> Result<bool> {
        let lease_path = self.lease_path(task_id);

        // Read existing lease
        let mut lease_data = match self.read_lease(&lease_path)? {
            Some(data) => data,
            None => return Ok(false), // No lease exists
        };

        // Check if held by this agent
        if lease_data.holder != agent_id {
            return Ok(false);
        }

        // Renew the lease
        lease_data.renew(lease_duration);

        // Write back atomically
        self.write_lease_atomically(&lease_path, &lease_data)?;

        Ok(true)
    }

    /// Releases the task lock.
    ///
    /// Removes the lease file for a task, releasing the lock.
    /// Only the agent holding the lock can release it.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID
    /// * `agent_id` - The agent ID attempting to release
    ///
    /// # Returns
    ///
    /// * `true` - Lock was released
    /// * `false` - Lock was not held by this agent
    ///
    /// # Errors
    ///
    /// Returns an error if the lease file cannot be read or removed.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::lock::TaskLockManager;
    ///
    /// let manager = TaskLockManager::default().unwrap();
    ///
    /// let released = manager.release_lock("task-123", "agent-1").unwrap();
    /// ```
    pub fn release_lock(&self, task_id: &str, agent_id: &str) -> Result<bool> {
        let lease_path = self.lease_path(task_id);

        // Read existing lease to verify ownership
        if let Ok(Some(lease_data)) = self.read_lease(&lease_path) {
            if lease_data.holder != agent_id {
                return Ok(false);
            }
        } else {
            // No lease exists or read error
            return Ok(false);
        }

        // Remove the lease file
        fs::remove_file(&lease_path).map_err(|e| AutomationError::LockError {
            task: task_id.to_string(),
            message: format!("Failed to remove lease file: {}", e),
        })?;

        Ok(true)
    }

    /// Gets the current lock information for a task.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID
    ///
    /// # Returns
    ///
    /// * `Some(TaskLock)` - Lock information if task is locked
    /// * `None` - Task is not locked
    ///
    /// # Errors
    ///
    /// Returns an error if the lease file cannot be read.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::lock::TaskLockManager;
    ///
    /// let manager = TaskLockManager::default().unwrap();
    ///
    /// if let Some(lock) = manager.get_lock("task-123").unwrap() {
    ///     println!("Locked by agent: {:?}", lock.holder());
    /// }
    /// ```
    pub fn get_lock(&self, task_id: &str) -> Result<Option<TaskLock>> {
        let lease_path = self.lease_path(task_id);

        match self.read_lease(&lease_path)? {
            Some(lease_data) => {
                let lock = TaskLock::from_lease_data(lease_path, lease_data);
                Ok(Some(lock))
            }
            None => Ok(None),
        }
    }

    /// Checks if a task is currently locked.
    ///
    /// A task is considered locked if a valid lease file exists
    /// (i.e., not expired).
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID
    ///
    /// # Returns
    ///
    /// * `true` - Task is locked (with valid lease)
    /// * `false` - Task is not locked or lease is expired
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::lock::TaskLockManager;
    ///
    /// let manager = TaskLockManager::default().unwrap();
    ///
    /// if manager.is_locked("task-123").unwrap() {
    ///     println!("Task is locked");
    /// }
    /// ```
    pub fn is_locked(&self, task_id: &str) -> Result<bool> {
        match self.get_lock(task_id)? {
            Some(lock) => Ok(lock.is_held() && !lock.is_expired()),
            None => Ok(false),
        }
    }

    /// Cleans up expired lease files.
    ///
    /// Removes lease files that are older than the specified duration.
    ///
    /// # Arguments
    ///
    /// * `older_than` - Remove leases older than this duration
    ///
    /// # Returns
    ///
    /// A list of task IDs whose leases were cleaned up.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::lock::TaskLockManager;
    /// use std::time::Duration;
    ///
    /// let manager = TaskLockManager::default().unwrap();
    ///
    /// let cleaned = manager.cleanup_expired_leases(
    ///     Duration::from_secs(3600) // 1 hour
    /// ).unwrap();
    /// ```
    pub fn cleanup_expired_leases(&self, older_than: Duration) -> Result<Vec<String>> {
        let mut cleaned_tasks = Vec::new();

        // Read the lock directory
        let entries = match fs::read_dir(&self.base_dir) {
            Ok(entries) => entries,
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    return Ok(cleaned_tasks);
                }
                return Err(AutomationError::LockError {
                    task: self.base_dir.to_string_lossy().to_string(),
                    message: format!("Failed to read lock directory: {}", e),
                });
            }
        };

        // Process each lease file
        for entry in entries.flatten() {
            let path = entry.path();

            // Only process .lease files
            if path.extension().and_then(|s| s.to_str()) != Some("lease") {
                continue;
            }

            // Read lease data
            if let Ok(Some(lease_data)) = self.read_lease(&path) {
                // Check if expired
                if lease_data.is_expired() {
                    // Extract task ID from filename
                    if let Some(task_id) = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                    {
                        // Remove the expired lease
                        if fs::remove_file(&path).is_ok() {
                            cleaned_tasks.push(task_id);
                        }
                    }
                }
            } else {
                // Invalid lease file, remove it
                let _ = fs::remove_file(&path);
            }
        }

        Ok(cleaned_tasks)
    }

    /// Force releases a task lock without checking ownership.
    ///
    /// This is for recovery scenarios when an agent has crashed
    /// or the lease needs to be forcibly removed.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID
    ///
    /// # Returns
    ///
    /// * `true` - Lock was force released
    /// * `false` - No lock existed
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::lock::TaskLockManager;
    ///
    /// let manager = TaskLockManager::default().unwrap();
    ///
    /// // Force release a stuck lock
    /// manager.force_release("task-123").unwrap();
    /// ```
    pub fn force_release(&self, task_id: &str) -> Result<bool> {
        let lease_path = self.lease_path(task_id);

        if !lease_path.exists() {
            return Ok(false);
        }

        fs::remove_file(&lease_path).map_err(|e| AutomationError::LockError {
            task: task_id.to_string(),
            message: format!("Failed to force release lock: {}", e),
        })?;

        Ok(true)
    }

    /// Reads lease data from a file.
    ///
    /// # Arguments
    ///
    /// * `lease_path` - Path to the lease file
    ///
    /// # Returns
    ///
    /// * `Some(LeaseData)` - Lease data if file exists and is valid
    /// * `None` - File doesn't exist
    fn read_lease(&self, lease_path: &Path) -> Result<Option<LeaseData>> {
        if !lease_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(lease_path).map_err(|e| {
            AutomationError::LockError {
                task: lease_path.to_string_lossy().to_string(),
                message: format!("Failed to read lease file: {}", e),
            }
        })?;

        let lease_data: LeaseData = serde_json::from_str(&content).map_err(|e| {
            AutomationError::LockError {
                task: lease_path.to_string_lossy().to_string(),
                message: format!("Failed to parse lease file: {}", e),
            }
        })?;

        Ok(Some(lease_data))
    }

    /// Writes lease data to a file atomically.
    ///
    /// Uses a temporary file and rename to ensure atomic writes.
    ///
    /// # Arguments
    ///
    /// * `lease_path` - Path to the lease file
    /// * `lease_data` - Lease data to write
    fn write_lease_atomically(&self, lease_path: &Path, lease_data: &LeaseData) -> Result<()> {
        // Create parent directory if needed
        if let Some(parent) = lease_path.parent() {
            fs::create_dir_all(parent).map_err(|e| AutomationError::LockError {
                task: parent.to_string_lossy().to_string(),
                message: format!("Failed to create parent directory: {}", e),
            })?;
        }

        // Serialize lease data
        let content = serde_json::to_string_pretty(lease_data).map_err(|e| {
            AutomationError::LockError {
                task: lease_path.to_string_lossy().to_string(),
                message: format!("Failed to serialize lease data: {}", e),
            }
        })?;

        // Write to temporary file first
        let temp_path = PathBuf::from(format!("{}.tmp", lease_path.display()));
        fs::write(&temp_path, content).map_err(|e| AutomationError::LockError {
            task: temp_path.to_string_lossy().to_string(),
            message: format!("Failed to write temporary file: {}", e),
        })?;

        // Atomic rename
        fs::rename(&temp_path, lease_path).map_err(|e| AutomationError::LockError {
            task: lease_path.to_string_lossy().to_string(),
            message: format!("Failed to rename temporary file: {}", e),
        })?;

        Ok(())
    }
}

impl Default for TaskLockManager {
    fn default() -> Self {
        // Create with default directory, ignoring creation errors
        // for Default trait
        Self::new(PathBuf::from(TaskLockManager::DEFAULT_BASE_DIR))
            .unwrap_or_else(|_| TaskLockManager {
                base_dir: PathBuf::from(TaskLockManager::DEFAULT_BASE_DIR),
            })
    }
}

/// Default implementation for TaskLock (creates an unlocked lock).
impl Default for TaskLock {
    fn default() -> Self {
        TaskLock::new(PathBuf::from(".state/tasks/locks/default.lease"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::time::Duration as StdDuration;

    #[test]
    fn test_get_current_process_id() {
        let pid = get_current_process_id();
        assert!(pid > 0);
        assert_eq!(pid, std::process::id());
    }

    #[test]
    fn test_get_hostname() {
        let hostname = get_hostname();
        assert!(!hostname.is_empty());
        assert_ne!(hostname, "unknown");
    }

    #[test]
    fn test_is_process_alive_current() {
        let pid = get_current_process_id();
        assert!(is_process_alive(pid));
    }

    #[test]
    fn test_is_process_alive_nonexistent() {
        // Use a very high PID that likely doesn't exist
        assert!(!is_process_alive(9999999));
    }

    #[test]
    fn test_lock_manager_creation() {
        let lock_path = PathBuf::from("/tmp/test.lock");
        let manager = LockManager::new(lock_path.clone(), StdDuration::from_secs(10));
        assert_eq!(manager.lock_file_path, lock_path);
        assert_eq!(manager.timeout, StdDuration::from_secs(10));
    }

    #[test]
    fn test_lock_manager_new_default() {
        let lock_path = PathBuf::from("/tmp/test.lock");
        let manager = LockManager::new_default(lock_path.clone());
        assert_eq!(manager.lock_file_path, lock_path);
        assert_eq!(manager.timeout, DEFAULT_LOCK_TIMEOUT);
    }

    #[test]
    fn test_lock_acquire_and_release() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");
        let manager = LockManager::new_default(lock_path.clone());

        // Acquire lock
        let lock = manager.acquire_lock().unwrap();
        assert_eq!(lock.path(), lock_path.as_path());

        // Verify lock is held
        assert!(manager.is_locked().unwrap());

        // Release lock
        drop(lock);

        // Verify lock is released
        assert!(!manager.is_locked().unwrap());
    }

    #[test]
    fn test_try_acquire_lock_success() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");
        let manager = LockManager::new_default(lock_path);

        // Try to acquire lock
        let lock_handle = manager.try_acquire_lock().unwrap();
        assert!(lock_handle.is_some());

        drop(lock_handle);
    }

    #[test]
    fn test_try_acquire_lock_busy() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");
        let manager = LockManager::new_default(lock_path);

        // Acquire lock
        let _lock1 = manager.acquire_lock().unwrap();

        // Try to acquire again - should fail
        let lock_handle2 = manager.try_acquire_lock().unwrap();
        assert!(lock_handle2.is_none());
    }

    #[test]
    fn test_concurrent_lock_acquisition() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");
        let manager = LockManager::new(lock_path.clone(), Duration::from_millis(100));

        // Acquire first lock
        let _lock1 = manager.acquire_lock().unwrap();

        // Try to acquire second lock - should timeout
        let result = manager.acquire_lock();
        assert!(result.is_err());

        match result {
            Err(AutomationError::LockTimeout { task, timeout }) => {
                assert_eq!(task, lock_path.to_string_lossy().to_string());
                assert_eq!(timeout, 100);
            }
            _ => panic!("Expected LockTimeout error"),
        }
    }

    #[test]
    fn test_lock_info() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");
        let manager = LockManager::new_default(lock_path.clone());

        let mut lock = manager.acquire_lock().unwrap();
        let lock_info = lock.lock_info().unwrap();

        assert!(lock_info.process_id.is_some());
        assert!(lock_info.hostname.is_some());
        assert!(lock_info.acquired_at <= Utc::now());
    }

    #[test]
    fn test_cleanup_stale_lock_dead_process() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");
        let manager = LockManager::new_default(lock_path.clone());

        // Create a lock with a dead process
        {
            let _lock = manager.acquire_lock().unwrap();
            // Write lock info with dead PID
            let stale_lock_info = LockInfo {
                acquired_at: Utc::now(),
                process_id: Some(9999999), // Non-existent PID
                hostname: Some("test-host".to_string()),
            };
            let lock_info_json = serde_json::to_string(&stale_lock_info).unwrap();
            fs::write(&lock_path, lock_info_json).unwrap();
        }

        // Clean up stale lock
        let cleaned = manager.cleanup_stale_lock(Duration::from_secs(300)).unwrap();
        assert!(cleaned);
        assert!(!lock_path.exists());
    }

    #[test]
    fn test_cleanup_stale_lock_old() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");
        let manager = LockManager::new_default(lock_path.clone());

        // Create a lock that's too old
        {
            let _lock = manager.acquire_lock().unwrap();
            // Write lock info with old timestamp
            let old_time = Utc::now() - ChronoDuration::seconds(600); // 10 minutes ago
            let stale_lock_info = LockInfo {
                acquired_at: old_time,
                process_id: Some(get_current_process_id()),
                hostname: Some(get_hostname()),
            };
            let lock_info_json = serde_json::to_string(&stale_lock_info).unwrap();
            fs::write(&lock_path, lock_info_json).unwrap();
        }

        // Clean up stale lock
        let cleaned = manager.cleanup_stale_lock(Duration::from_secs(300)).unwrap();
        assert!(cleaned);
        assert!(!lock_path.exists());
    }

    #[test]
    fn test_cleanup_stale_lock_cross_host() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");
        let manager = LockManager::new_default(lock_path.clone());

        // Create a lock from a different host
        {
            let _lock = manager.acquire_lock().unwrap();
            // Write lock info with different hostname and old timestamp
            let old_time = Utc::now() - ChronoDuration::seconds(70); // Older than 1 min threshold
            let stale_lock_info = LockInfo {
                acquired_at: old_time,
                process_id: Some(12345),
                hostname: Some("different-host".to_string()),
            };
            let lock_info_json = serde_json::to_string(&stale_lock_info).unwrap();
            fs::write(&lock_path, lock_info_json).unwrap();
        }

        // Clean up stale lock
        let cleaned = manager.cleanup_stale_lock(Duration::from_secs(300)).unwrap();
        assert!(cleaned);
        assert!(!lock_path.exists());
    }

    #[test]
    fn test_cleanup_stale_lock_same_pid() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");
        let manager = LockManager::new_default(lock_path.clone());

        // Create a lock with current PID (simulating stale lock from restart)
        {
            let _lock = manager.acquire_lock().unwrap();
            // Write lock info with current PID but old timestamp
            let old_time = Utc::now() - ChronoDuration::seconds(10);
            let stale_lock_info = LockInfo {
                acquired_at: old_time,
                process_id: Some(get_current_process_id()),
                hostname: Some(get_hostname()),
            };
            let lock_info_json = serde_json::to_string(&stale_lock_info).unwrap();
            fs::write(&lock_path, lock_info_json).unwrap();
        }

        // Clean up stale lock
        let cleaned = manager.cleanup_stale_lock(Duration::from_secs(300)).unwrap();
        assert!(cleaned);
        assert!(!lock_path.exists());
    }

    #[test]
    fn test_cleanup_stale_lock_no_cleanup_needed() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");
        let manager = LockManager::new_default(lock_path.clone());

        // Create a valid lock with running process
        let _lock = manager.acquire_lock().unwrap();

        // Try to clean up - should not clean
        let cleaned = manager.cleanup_stale_lock(Duration::from_secs(300)).unwrap();
        assert!(!cleaned);
        assert!(lock_path.exists());
    }

    #[test]
    fn test_cleanup_stale_lock_no_file() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");
        let manager = LockManager::new_default(lock_path.clone());

        // No lock file exists
        let cleaned = manager.cleanup_stale_lock(Duration::from_secs(300)).unwrap();
        assert!(!cleaned);
    }

    #[test]
    fn test_cleanup_stale_lock_corrupted_file() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");
        let manager = LockManager::new_default(lock_path.clone());

        // Create corrupted lock file
        fs::write(&lock_path, "invalid json {{{").unwrap();

        // Clean up corrupted file
        let cleaned = manager.cleanup_stale_lock(Duration::from_secs(300)).unwrap();
        assert!(cleaned);
        assert!(!lock_path.exists());
    }

    #[test]
    fn test_raii_lock_release() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");
        let manager = LockManager::new_default(lock_path.clone());

        {
            let _lock = manager.acquire_lock().unwrap();
            assert!(lock_path.exists());
            assert!(manager.is_locked().unwrap());
        } // Lock should be automatically released here

        assert!(!lock_path.exists());
        assert!(!manager.is_locked().unwrap());
    }

    #[test]
    fn test_lock_handle_release_method() {
        let temp_dir = tempdir().unwrap();
        let lock_path = temp_dir.path().join("test.lock");
        let manager = LockManager::new_default(lock_path.clone());

        let lock = manager.acquire_lock().unwrap();
        assert!(lock_path.exists());

        lock.release();

        assert!(!lock_path.exists());
    }

    // ============================================================================
    // TaskLock and TaskLockManager Tests
    // ============================================================================

    #[test]
    fn test_task_lock_new() {
        let lock = TaskLock::new(PathBuf::from("/tmp/test.lease"));
        assert_eq!(
            lock.path(),
            PathBuf::from("/tmp/test.lease").as_path()
        );
        assert!(!lock.is_held());
        assert!(lock.is_expired());
    }

    #[test]
    fn test_task_lock_with_lease() {
        let now = Utc::now();
        let expires = now + ChronoDuration::seconds(60);

        let lock = TaskLock::with_lease(
            PathBuf::from("/tmp/test.lease"),
            "agent-1".to_string(),
            now,
            expires,
        );

        assert!(lock.is_held());
        assert!(!lock.is_expired());
        assert!(lock.holder_matches("agent-1"));
        assert!(!lock.holder_matches("agent-2"));
        assert_eq!(lock.holder(), Some("agent-1"));
        assert_eq!(lock.acquired_at(), Some(now));
        assert_eq!(lock.expires_at(), Some(expires));
    }

    #[test]
    fn test_task_lock_expired() {
        let now = Utc::now();
        let expires = now - ChronoDuration::seconds(60); // Expired

        let lock = TaskLock::with_lease(
            PathBuf::from("/tmp/test.lease"),
            "agent-1".to_string(),
            now - ChronoDuration::seconds(120),
            expires,
        );

        assert!(lock.is_held());
        assert!(lock.is_expired());
    }

    #[test]
    fn test_task_lock_remaining_duration() {
        let now = Utc::now();
        let expires = now + ChronoDuration::seconds(60);

        let lock = TaskLock::with_lease(
            PathBuf::from("/tmp/test.lease"),
            "agent-1".to_string(),
            now,
            expires,
        );

        let remaining = lock.remaining_duration();
        assert!(remaining.is_some());
        let duration = remaining.unwrap();
        assert!(duration.as_secs() >= 59);
        assert!(duration.as_secs() <= 61);
    }

    #[test]
    fn test_task_lock_remaining_duration_expired() {
        let now = Utc::now();
        let expires = now - ChronoDuration::seconds(60);

        let lock = TaskLock::with_lease(
            PathBuf::from("/tmp/test.lease"),
            "agent-1".to_string(),
            now - ChronoDuration::seconds(120),
            expires,
        );

        let remaining = lock.remaining_duration();
        assert_eq!(remaining, Some(Duration::ZERO));
    }

    #[test]
    fn test_lease_data_new() {
        let lease = LeaseData::new("agent-1".to_string(), Duration::from_secs(60));

        assert_eq!(lease.holder, "agent-1");
        assert!(!lease.is_expired());
        assert_eq!(lease.holder, "agent-1");
    }

    #[test]
    fn test_lease_data_renew() {
        let mut lease = LeaseData::new("agent-1".to_string(), Duration::from_secs(60));

        // Wait a moment and renew
        std::thread::sleep(Duration::from_millis(10));

        let old_expires = lease.expires_at;
        lease.renew(Duration::from_secs(120));

        assert!(lease.expires_at > old_expires);
    }

    #[test]
    fn test_lease_data_expired() {
        let mut lease = LeaseData::new("agent-1".to_string(), Duration::from_millis(10));

        // Wait for expiry
        std::thread::sleep(Duration::from_millis(20));

        assert!(lease.is_expired());
    }

    #[test]
    fn test_task_lock_manager_new() {
        let temp_dir = tempdir().unwrap();
        let base_dir = temp_dir.path().join("locks");

        let manager = TaskLockManager::new(base_dir.clone()).unwrap();

        assert_eq!(manager.base_dir, base_dir);
        assert!(base_dir.exists());
    }

    #[test]
    fn test_task_lock_manager_default() {
        let manager = TaskLockManager::new_default().unwrap();

        assert_eq!(
            manager.base_dir,
            PathBuf::from(TaskLockManager::DEFAULT_BASE_DIR)
        );
    }

    #[test]
    fn test_task_lock_acquire_release() {
        let temp_dir = tempdir().unwrap();
        let base_dir = temp_dir.path().join("locks");
        let manager = TaskLockManager::new(base_dir).unwrap();

        // Acquire lock
        let lock = manager
            .acquire_lock("task-1", "agent-1", Duration::from_secs(60))
            .unwrap();

        assert!(lock.is_held());
        assert!(lock.holder_matches("agent-1"));

        // Verify lock exists
        assert!(manager.is_locked("task-1").unwrap());

        // Release lock
        let released = manager.release_lock("task-1", "agent-1").unwrap();
        assert!(released);

        // Verify lock is released
        assert!(!manager.is_locked("task-1").unwrap());
    }

    #[test]
    fn test_task_lock_acquire_conflict() {
        let temp_dir = tempdir().unwrap();
        let base_dir = temp_dir.path().join("locks");
        let manager = TaskLockManager::new(base_dir).unwrap();

        // Acquire lock with agent-1
        manager
            .acquire_lock("task-1", "agent-1", Duration::from_secs(60))
            .unwrap();

        // Try to acquire with agent-2 - should fail
        let result = manager.acquire_lock("task-1", "agent-2", Duration::from_secs(60));

        assert!(result.is_err());
        match result {
            Err(AutomationError::LockError { task, message }) => {
                assert_eq!(task, "task-1");
                assert!(message.contains("already locked"));
                assert!(message.contains("agent-1"));
            }
            _ => panic!("Expected LockError"),
        }
    }

    #[test]
    fn test_task_lock_acquire_expired() {
        let temp_dir = tempdir().unwrap();
        let base_dir = temp_dir.path().join("locks");
        let manager = TaskLockManager::new(base_dir).unwrap();

        // Acquire lock with very short expiry
        manager
            .acquire_lock("task-1", "agent-1", Duration::from_millis(10))
            .unwrap();

        // Wait for expiry
        std::thread::sleep(Duration::from_millis(20));

        // Acquire lock with agent-2 - should succeed because first lease expired
        let lock = manager
            .acquire_lock("task-1", "agent-2", Duration::from_secs(60))
            .unwrap();

        assert!(lock.holder_matches("agent-2"));
    }

    #[test]
    fn test_task_lock_renew_lease() {
        let temp_dir = tempdir().unwrap();
        let base_dir = temp_dir.path().join("locks");
        let manager = TaskLockManager::new(base_dir).unwrap();

        // Acquire lock
        manager
            .acquire_lock("task-1", "agent-1", Duration::from_millis(100))
            .unwrap();

        // Wait a bit and renew
        std::thread::sleep(Duration::from_millis(20));

        let renewed = manager
            .renew_lease("task-1", "agent-1", Duration::from_secs(60))
            .unwrap();

        assert!(renewed);

        // Check that lock is still valid
        assert!(manager.is_locked("task-1").unwrap());
    }

    #[test]
    fn test_task_lock_renew_lease_wrong_agent() {
        let temp_dir = tempdir().unwrap();
        let base_dir = temp_dir.path().join("locks");
        let manager = TaskLockManager::new(base_dir).unwrap();

        // Acquire lock with agent-1
        manager
            .acquire_lock("task-1", "agent-1", Duration::from_secs(60))
            .unwrap();

        // Try to renew with agent-2 - should fail
        let renewed = manager
            .renew_lease("task-1", "agent-2", Duration::from_secs(60))
            .unwrap();

        assert!(!renewed);
    }

    #[test]
    fn test_task_lock_renew_lease_nonexistent() {
        let temp_dir = tempdir().unwrap();
        let base_dir = temp_dir.path().join("locks");
        let manager = TaskLockManager::new(base_dir).unwrap();

        // Try to renew non-existent lock
        let renewed = manager
            .renew_lease("task-1", "agent-1", Duration::from_secs(60))
            .unwrap();

        assert!(!renewed);
    }

    #[test]
    fn test_task_lock_get_lock() {
        let temp_dir = tempdir().unwrap();
        let base_dir = temp_dir.path().join("locks");
        let manager = TaskLockManager::new(base_dir).unwrap();

        // Get non-existent lock
        let lock = manager.get_lock("task-1").unwrap();
        assert!(lock.is_none());

        // Acquire lock
        manager
            .acquire_lock("task-1", "agent-1", Duration::from_secs(60))
            .unwrap();

        // Get existing lock
        let lock = manager.get_lock("task-1").unwrap();
        assert!(lock.is_some());
        let lock = lock.unwrap();
        assert!(lock.holder_matches("agent-1"));
    }

    #[test]
    fn test_task_lock_is_locked() {
        let temp_dir = tempdir().unwrap();
        let base_dir = temp_dir.path().join("locks");
        let manager = TaskLockManager::new(base_dir).unwrap();

        // Not locked initially
        assert!(!manager.is_locked("task-1").unwrap());

        // Acquire lock
        manager
            .acquire_lock("task-1", "agent-1", Duration::from_secs(60))
            .unwrap();

        // Now locked
        assert!(manager.is_locked("task-1").unwrap());

        // Release lock
        manager.release_lock("task-1", "agent-1").unwrap();

        // Not locked again
        assert!(!manager.is_locked("task-1").unwrap());
    }

    #[test]
    fn test_task_lock_is_locked_expired() {
        let temp_dir = tempdir().unwrap();
        let base_dir = temp_dir.path().join("locks");
        let manager = TaskLockManager::new(base_dir).unwrap();

        // Acquire lock with very short expiry
        manager
            .acquire_lock("task-1", "agent-1", Duration::from_millis(10))
            .unwrap();

        // Wait for expiry
        std::thread::sleep(Duration::from_millis(20));

        // Not considered locked because expired
        assert!(!manager.is_locked("task-1").unwrap());
    }

    #[test]
    fn test_task_lock_cleanup_expired_leases() {
        let temp_dir = tempdir().unwrap();
        let base_dir = temp_dir.path().join("locks");
        let manager = TaskLockManager::new(base_dir).unwrap();

        // Create multiple locks with different expiry times
        manager
            .acquire_lock("task-1", "agent-1", Duration::from_millis(10))
            .unwrap();

        manager
            .acquire_lock("task-2", "agent-2", Duration::from_secs(60))
            .unwrap();

        manager
            .acquire_lock("task-3", "agent-3", Duration::from_millis(10))
            .unwrap();

        // Wait for expiry of task-1 and task-3
        std::thread::sleep(Duration::from_millis(20));

        // Clean up expired leases
        let cleaned = manager
            .cleanup_expired_leases(Duration::from_secs(0))
            .unwrap();

        // Should have cleaned up task-1 and task-3
        assert_eq!(cleaned.len(), 2);
        assert!(cleaned.contains(&"task-1".to_string()) || cleaned.contains(&"task-3".to_string()));

        // task-2 should still be locked
        assert!(manager.is_locked("task-2").unwrap());
    }

    #[test]
    fn test_task_lock_force_release() {
        let temp_dir = tempdir().unwrap();
        let base_dir = temp_dir.path().join("locks");
        let manager = TaskLockManager::new(base_dir).unwrap();

        // Acquire lock
        manager
            .acquire_lock("task-1", "agent-1", Duration::from_secs(60))
            .unwrap();

        assert!(manager.is_locked("task-1").unwrap());

        // Force release
        let released = manager.force_release("task-1").unwrap();
        assert!(released);

        // Lock should be gone
        assert!(!manager.is_locked("task-1").unwrap());
    }

    #[test]
    fn test_task_lock_force_release_nonexistent() {
        let temp_dir = tempdir().unwrap();
        let base_dir = temp_dir.path().join("locks");
        let manager = TaskLockManager::new(base_dir).unwrap();

        // Force release non-existent lock
        let released = manager.force_release("task-1").unwrap();
        assert!(!released);
    }

    #[test]
    fn test_task_lock_release_wrong_agent() {
        let temp_dir = tempdir().unwrap();
        let base_dir = temp_dir.path().join("locks");
        let manager = TaskLockManager::new(base_dir).unwrap();

        // Acquire lock with agent-1
        manager
            .acquire_lock("task-1", "agent-1", Duration::from_secs(60))
            .unwrap();

        // Try to release with agent-2 - should fail
        let released = manager.release_lock("task-1", "agent-2").unwrap();
        assert!(!released);

        // Lock should still be held
        assert!(manager.is_locked("task-1").unwrap());
    }

    #[test]
    fn test_task_lock_lease_path() {
        let temp_dir = tempdir().unwrap();
        let base_dir = temp_dir.path().join("locks");
        let manager = TaskLockManager::new(base_dir.clone()).unwrap();

        let lease_path = manager.lease_path("task-1");
        assert_eq!(lease_path, base_dir.join("task-1.lease"));
    }

    #[test]
    fn test_task_lock_concurrent_access() {
        let temp_dir = tempdir().unwrap();
        let base_dir = temp_dir.path().join("locks");
        let manager = TaskLockManager::new(base_dir).unwrap();

        // Acquire multiple locks for different tasks
        let lock1 = manager
            .acquire_lock("task-1", "agent-1", Duration::from_secs(60))
            .unwrap();

        let lock2 = manager
            .acquire_lock("task-2", "agent-1", Duration::from_secs(60))
            .unwrap();

        let lock3 = manager
            .acquire_lock("task-3", "agent-1", Duration::from_secs(60))
            .unwrap();

        // All locks should be held
        assert!(manager.is_locked("task-1").unwrap());
        assert!(manager.is_locked("task-2").unwrap());
        assert!(manager.is_locked("task-3").unwrap());
    }
}

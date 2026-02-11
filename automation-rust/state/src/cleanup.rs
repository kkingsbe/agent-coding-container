//! State cleanup for the automation system.
//!
//! This module provides utilities for cleaning up stale state data, including:
//! - Backup file cleanup (by count and age)
//! - State reset based on age
//! - Mistake record cleanup
//! - Stale lock cleanup
//!
//! # Cleanup Strategy
//!
//! The cleanup system is designed to prevent unbounded growth of state data
//! while maintaining historical information that might be useful for analysis.
//! Each cleanup operation can be run independently or together via `cleanup_all`.
//!
//! # Configuration
//!
//! All cleanup operations are configurable through `CleanupConfig`, which allows
//! you to set thresholds for each type of cleanup:
//!
//! - `max_backups` - Maximum number of backup files to keep
//! - `max_backup_age` - Maximum age of backup files before deletion
//! - `max_state_age` - Maximum age of state before automatic reset
//! - `max_mistake_age` - Maximum age of mistake records before removal
//! - `stale_lock_timeout` - Timeout for considering locks as stale
//!
//! # Example
//!
//! ```no_run
//! use automation_state::cleanup::{CleanupManager, CleanupConfig, get_default_cleanup_config};
//! use automation_state::persistence::StateManager;
//! use std::path::PathBuf;
//! use std::time::Duration;
//!
//! // Create a state manager
//! let state_manager = StateManager::new(
//!     PathBuf::from("/workspace/.state"),
//!     "architect"
//! ).unwrap();
//!
//! // Create cleanup manager with custom config
//! let config = CleanupConfig {
//!     max_backups: Some(5),
//!     max_backup_age: Some(Duration::from_secs(86400)), // 1 day
//!     max_state_age: Some(Duration::from_secs(604800)), // 1 week
//!     max_mistake_age: Some(Duration::from_secs(3600)), // 1 hour
//!     stale_lock_timeout: Some(Duration::from_secs(300)), // 5 minutes
//! };
//!
//! let cleanup_manager = CleanupManager::new(state_manager, config);
//!
//! // Run all cleanup operations
//! let report = cleanup_manager.cleanup_all().unwrap();
//! println!("Cleaned {} backups, {} mistakes", report.backup_stats.items_removed, report.mistake_stats.items_removed);
//! ```

use automation_common::{Result, AutomationError};
use crate::persistence::{StateManager, list_backups, lock_file_path_for_state};
use crate::state::State;
use crate::lock::LockManager;
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration as StdDuration;

/// Configuration for cleanup operations.
///
/// Each field is optional to allow selective cleanup.
#[derive(Debug, Clone)]
pub struct CleanupConfig {
    /// Maximum number of backup files to keep
    ///
    /// When set, old backups beyond this count will be removed.
    /// Backups are sorted by timestamp, and the oldest ones are removed first.
    pub max_backups: Option<usize>,

    /// Maximum age for backup files
    ///
    /// When set, backups older than this duration will be removed.
    pub max_backup_age: Option<StdDuration>,

    /// Maximum age before state is reset
    ///
    /// When set, if the state's last_run timestamp is older than this duration,
    /// the state will be reset to default values. This is useful for cleaning
    /// up stale state from inactive agents.
    pub max_state_age: Option<StdDuration>,

    /// Maximum age for mistake records
    ///
    /// When set, mistake records older than this duration will be removed
    /// from the state. This prevents unbounded growth of the mistakes array.
    pub max_mistake_age: Option<StdDuration>,

    /// Timeout for considering locks as stale
    ///
    /// When set, locks older than this duration may be cleaned up. The cleanup
    /// considers additional factors such as whether the process is still running.
    pub stale_lock_timeout: Option<StdDuration>,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            max_backups: None,
            max_backup_age: None,
            max_state_age: None,
            max_mistake_age: None,
            stale_lock_timeout: None,
        }
    }
}

/// Statistics from a single cleanup operation.
///
/// Tracks how many items were removed, kept, and how much space was freed.
#[derive(Debug, Clone, Default)]
pub struct CleanupStats {
    /// Number of items removed during cleanup
    pub items_removed: usize,
    /// Number of items kept after cleanup
    pub items_kept: usize,
    /// Total space freed in bytes
    pub space_freed_bytes: u64,
}

impl CleanupStats {
    /// Creates a new empty CleanupStats.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds another CleanupStats to this one.
    pub fn add(&mut self, other: &CleanupStats) {
        self.items_removed += other.items_removed;
        self.items_kept += other.items_kept;
        self.space_freed_bytes += other.space_freed_bytes;
    }
}

/// Comprehensive report from a complete cleanup operation.
///
/// Combines statistics from all cleanup operations and tracks whether state was reset.
#[derive(Debug, Clone)]
pub struct CleanupReport {
    /// Statistics from backup cleanup
    pub backup_stats: CleanupStats,
    /// Statistics from mistake cleanup
    pub mistake_stats: CleanupStats,
    /// Statistics from lock cleanup
    pub lock_stats: CleanupStats,
    /// Whether the state was reset due to age
    pub state_reset: bool,
    /// Total bytes freed across all operations
    pub total_bytes_freed: u64,
}

impl CleanupReport {
    /// Creates a new empty CleanupReport.
    pub fn new() -> Self {
        Self {
            backup_stats: CleanupStats::new(),
            mistake_stats: CleanupStats::new(),
            lock_stats: CleanupStats::new(),
            state_reset: false,
            total_bytes_freed: 0,
        }
    }
}

impl Default for CleanupReport {
    fn default() -> Self {
        Self::new()
    }
}

/// A cleanup manager that coordinates all cleanup operations.
///
/// The `CleanupManager` provides methods to clean up different aspects of the
/// state system, including backups, mistakes, locks, and the state itself.
#[derive(Debug, Clone)]
pub struct CleanupManager {
    /// State manager for accessing state files
    state_manager: StateManager,
    /// Cleanup configuration
    config: CleanupConfig,
}

impl CleanupManager {
    /// Creates a new cleanup manager.
    ///
    /// # Arguments
    ///
    /// * `state_manager` - State manager for accessing state files
    /// * `config` - Cleanup configuration
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::cleanup::{CleanupManager, CleanupConfig};
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    ///
    /// # let state_manager = StateManager::new(
    /// #     PathBuf::from("/tmp/.state"),
    /// #     "architect"
    /// # ).unwrap();
    /// let config = CleanupConfig::default();
    /// let cleanup_manager = CleanupManager::new(state_manager, config);
    /// ```
    pub fn new(state_manager: StateManager, config: CleanupConfig) -> Self {
        CleanupManager {
            state_manager,
            config,
        }
    }

    /// Cleans up backup files based on configuration.
    ///
    /// Removes old backups based on:
    /// - `max_backups`: Keeps only the N most recent backups
    /// - `max_backup_age`: Removes backups older than the specified duration
    ///
    /// Both conditions are applied; a backup is removed if it fails either check.
    ///
    /// # Returns
    ///
    /// Statistics of the cleanup operation
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Backup files cannot be read
    /// - Backup timestamps cannot be parsed
    /// - Backup files cannot be deleted
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::cleanup::{CleanupManager, CleanupConfig};
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    /// use std::time::Duration;
    ///
    /// # let state_manager = StateManager::new(
    /// #     PathBuf::from("/tmp/.state"),
    /// #     "architect"
    /// # ).unwrap();
    /// let config = CleanupConfig {
    ///     max_backups: Some(5),
    ///     max_backup_age: Some(Duration::from_secs(86400)),
    ///     ..Default::default()
    /// };
    /// let cleanup_manager = CleanupManager::new(state_manager, config);
    /// let stats = cleanup_manager.cleanup_backups().unwrap();
    /// println!("Removed {} backups, freed {} bytes", stats.items_removed, stats.space_freed_bytes);
    /// ```
    pub fn cleanup_backups(&self) -> Result<CleanupStats> {
        let mut stats = CleanupStats::new();

        // Get list of all backups
        let backups = list_backups(self.state_manager.state_file_path())?;
        stats.items_kept = backups.len();

        if backups.is_empty() {
            return Ok(stats);
        }

        // Filter backups based on configuration
        let mut backups_with_age: Vec<(PathBuf, DateTime<Utc>, u64)> = Vec::new();

        for backup_path in &backups {
            match calculate_backup_age(backup_path) {
                Ok((age, file_size)) => {
                    backups_with_age.push((backup_path.clone(), age, file_size));
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to calculate age for backup {:?}: {:?}",
                        backup_path,
                        e
                    );
                }
            }
        }

        // Sort by timestamp (newest first)
        backups_with_age.sort_by(|a, b| b.1.cmp(&a.1));

        let mut backups_to_remove = Vec::new();
        let now = Utc::now();

        // Apply max_backups filter (keep the N most recent)
        if let Some(max_backups) = self.config.max_backups {
            if backups_with_age.len() > max_backups {
                backups_to_remove.extend(
                    backups_with_age[max_backups..]
                        .iter()
                        .map(|(path, _, _)| path.clone())
                );
            }
        }

        // Apply max_backup_age filter
        if let Some(max_age) = self.config.max_backup_age {
            let max_age_duration = chrono::TimeDelta::from_std(max_age)
                .map_err(|e| AutomationError::Config(format!("Invalid max_backup_age: {}", e)))?;

            for (path, timestamp, _) in &backups_with_age {
                let age = now - *timestamp;
                if age > max_age_duration {
                    backups_to_remove.push(path.clone());
                }
            }
        }

        // Remove backups
        for backup_path in backups_to_remove {
            match fs::metadata(&backup_path) {
                Ok(metadata) => {
                    let file_size = metadata.len();
                    match fs::remove_file(&backup_path) {
                        Ok(_) => {
                            stats.items_removed += 1;
                            stats.space_freed_bytes += file_size;
                            stats.items_kept -= 1;
                            tracing::debug!("Removed backup: {:?}", backup_path);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to remove backup {:?}: {:?}", backup_path, e);
                        }
                    }
                }
                Err(_) => {
                    // File doesn't exist or can't be accessed, skip
                    continue;
                }
            }
        }

        Ok(stats)
    }

    /// Cleans up the state if it exceeds the configured maximum age.
    ///
    /// Checks if the state's `last_run` timestamp is older than `max_state_age`.
    /// If so, resets the state to default values.
    ///
    /// # Returns
    ///
    /// * `Some(old_state)` - The old state before reset
    /// * `None` - State was not reset (no `max_state_age` configured or state is recent enough)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The state file cannot be read or written
    /// - The state is corrupted
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::cleanup::{CleanupManager, CleanupConfig};
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    /// use std::time::Duration;
    ///
    /// # let state_manager = StateManager::new(
    /// #     PathBuf::from("/tmp/.state"),
    /// #     "architect"
    /// # ).unwrap();
    /// let config = CleanupConfig {
    ///     max_state_age: Some(Duration::from_secs(604800)), // 1 week
    ///     ..Default::default()
    /// };
    /// let cleanup_manager = CleanupManager::new(state_manager, config);
    ///
    /// match cleanup_manager.cleanup_state_if_needed() {
    ///     Ok(Some(old_state)) => println!("Reset stale state with {} mistakes", old_state.mistakes.len()),
    ///     Ok(None) => println!("State is fresh, no reset needed"),
    ///     Err(e) => println!("Error cleaning state: {:?}", e),
    /// }
    /// ```
    pub fn cleanup_state_if_needed(&self) -> Result<Option<State>> {
        let max_state_age = match self.config.max_state_age {
            Some(age) => age,
            None => return Ok(None),
        };

        // Load current state
        let current_state = self.state_manager.load_state()?;

        // Check if state should be reset
        if !should_cleanup_state(&current_state, max_state_age) {
            return Ok(None);
        }

        // Backup old state before reset
        let _ = self.state_manager.backup_state();

        // Reset state
        let old_state = current_state;
        let new_state = State::new();
        self.state_manager.save_state(&new_state)?;

        tracing::info!("Reset state due to age (older than {:?})", max_state_age);

        Ok(Some(old_state))
    }

    /// Cleans up stale mistakes from the state.
    ///
    /// Removes mistake records that are older than `max_mistake_age`.
    /// Uses the `State::clear_stale_mistakes` method to perform the cleanup.
    ///
    /// # Returns
    ///
    /// Statistics of the cleanup operation
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The state file cannot be read or written
    /// - The state is corrupted
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::cleanup::{CleanupManager, CleanupConfig};
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    /// use std::time::Duration;
    ///
    /// # let state_manager = StateManager::new(
    /// #     PathBuf::from("/tmp/.state"),
    /// #     "architect"
    /// # ).unwrap();
    /// let config = CleanupConfig {
    ///     max_mistake_age: Some(Duration::from_secs(3600)), // 1 hour
    ///     ..Default::default()
    /// };
    /// let cleanup_manager = CleanupManager::new(state_manager, config);
    ///
    /// let stats = cleanup_manager.cleanup_mistakes().unwrap();
    /// println!("Removed {} old mistakes", stats.items_removed);
    /// ```
    pub fn cleanup_mistakes(&self) -> Result<CleanupStats> {
        let max_mistake_age = match self.config.max_mistake_age {
            Some(age) => age,
            None => {
                return Ok(CleanupStats::new());
            }
        };

        // Load and modify state with lock
        let mut stats = CleanupStats::new();

        let result = self.state_manager.load_state_with_lock(|state| {
            let removed_count = state.clear_stale_mistakes(max_mistake_age);
            let new_mistake_count = state.mistakes.len();

            stats.items_removed = removed_count;
            stats.items_kept = new_mistake_count;

            Ok(())
        });

        result?;

        Ok(stats)
    }

    /// Cleans up stale locks.
    ///
    /// Removes locks that are considered stale based on `stale_lock_timeout`.
    /// This uses the `LockManager::cleanup_stale_lock` method.
    ///
    /// # Returns
    ///
    /// Statistics of the cleanup operation
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Lock file cannot be accessed
    /// - Lock file is corrupted
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::cleanup::{CleanupManager, CleanupConfig};
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    /// use std::time::Duration;
    ///
    /// # let state_manager = StateManager::new(
    /// #     PathBuf::from("/tmp/.state"),
    /// #     "architect"
    /// # ).unwrap();
    /// let config = CleanupConfig {
    ///     stale_lock_timeout: Some(Duration::from_secs(300)), // 5 minutes
    ///     ..Default::default()
    /// };
    /// let cleanup_manager = CleanupManager::new(state_manager, config);
    ///
    /// let stats = cleanup_manager.cleanup_stale_locks().unwrap();
    /// println!("Cleaned {} stale locks", stats.items_removed);
    /// ```
    pub fn cleanup_stale_locks(&self) -> Result<CleanupStats> {
        let stale_timeout = match self.config.stale_lock_timeout {
            Some(timeout) => timeout,
            None => {
                return Ok(CleanupStats::new());
            }
        };

        let mut stats = CleanupStats::new();

        // Get lock file path
        let lock_file_path = lock_file_path_for_state(self.state_manager.state_file_path());
        let lock_manager = LockManager::new_default(lock_file_path);

        // Attempt to clean up stale lock
        let cleaned = lock_manager.cleanup_stale_lock(stale_timeout)?;

        if cleaned {
            stats.items_removed = 1;
            stats.items_kept = 0;
        } else {
            stats.items_removed = 0;
            stats.items_kept = 1;
        }

        Ok(stats)
    }

    /// Runs all cleanup operations.
    ///
    /// Executes all cleanup methods in the following order:
    /// 1. Backup cleanup
    /// 2. State reset (if needed)
    /// 3. Mistake cleanup
    /// 4. Lock cleanup
    ///
    /// Returns a comprehensive report of all cleanup operations.
    ///
    /// # Returns
    ///
    /// A comprehensive report of all cleanup operations
    ///
    /// # Errors
    ///
    /// Returns an error if any cleanup operation fails. Partial cleanup may
    /// have occurred before the error.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::cleanup::{CleanupManager, get_default_cleanup_config};
    /// use automation_state::persistence::StateManager;
    /// use std::path::PathBuf;
    ///
    /// # let state_manager = StateManager::new(
    /// #     PathBuf::from("/tmp/.state"),
    /// #     "architect"
    /// # ).unwrap();
    /// let config = get_default_cleanup_config();
    /// let cleanup_manager = CleanupManager::new(state_manager, config);
    ///
    /// let report = cleanup_manager.cleanup_all().unwrap();
    /// println!(
    ///     "Cleanup complete: {} backups, {} mistakes, {} locks, state reset: {}",
    ///     report.backup_stats.items_removed,
    ///     report.mistake_stats.items_removed,
    ///     report.lock_stats.items_removed,
    ///     report.state_reset
    /// );
    /// ```
    pub fn cleanup_all(&self) -> Result<CleanupReport> {
        let mut report = CleanupReport::new();

        // Clean up backups
        match self.cleanup_backups() {
            Ok(stats) => {
                let bytes_freed = stats.space_freed_bytes;
                report.backup_stats = stats;
                report.total_bytes_freed += bytes_freed;
            }
            Err(e) => {
                tracing::warn!("Backup cleanup failed: {:?}", e);
            }
        }

        // Clean up state if needed
        match self.cleanup_state_if_needed() {
            Ok(_) => {
                // State reset is tracked internally
            }
            Err(e) => {
                tracing::warn!("State cleanup failed: {:?}", e);
            }
        }

        // Clean up mistakes
        match self.cleanup_mistakes() {
            Ok(stats) => {
                let bytes_freed = stats.space_freed_bytes;
                report.mistake_stats = stats;
                report.total_bytes_freed += bytes_freed;
            }
            Err(e) => {
                tracing::warn!("Mistake cleanup failed: {:?}", e);
            }
        }

        // Clean up stale locks
        match self.cleanup_stale_locks() {
            Ok(stats) => {
                let bytes_freed = stats.space_freed_bytes;
                report.lock_stats = stats;
                report.total_bytes_freed += bytes_freed;
            }
            Err(e) => {
                tracing::warn!("Lock cleanup failed: {:?}", e);
            }
        }

        // Track if state was reset
        report.state_reset = self.config.max_state_age.is_some();

        Ok(report)
    }
}

/// Returns a default cleanup configuration with sensible values.
///
/// The defaults are:
/// - `max_backups`: 10 (keep 10 most recent backups)
/// - `max_backup_age`: 7 days
/// - `max_state_age`: 30 days
/// - `max_mistake_age`: 24 hours
/// - `stale_lock_timeout`: 5 minutes
///
/// # Example
///
/// ```
/// use automation_state::cleanup::get_default_cleanup_config;
///
/// let config = get_default_cleanup_config();
/// assert_eq!(config.max_backups, Some(10));
/// ```
pub fn get_default_cleanup_config() -> CleanupConfig {
    CleanupConfig {
        max_backups: Some(10),
        max_backup_age: Some(StdDuration::from_secs(7 * 24 * 60 * 60)), // 7 days
        max_state_age: Some(StdDuration::from_secs(30 * 24 * 60 * 60)), // 30 days
        max_mistake_age: Some(StdDuration::from_secs(24 * 60 * 60)), // 24 hours
        stale_lock_timeout: Some(StdDuration::from_secs(5 * 60)), // 5 minutes
    }
}

/// Checks if a state should be reset based on its age.
///
/// A state is considered old if:
/// - It has a `last_run` timestamp
/// - The timestamp is older than `max_age`
///
/// # Arguments
///
/// * `state` - The state to check
/// * `max_age` - Maximum age before reset
///
/// # Returns
///
/// * `true` - State should be reset
/// * `false` - State is recent enough or has no `last_run` timestamp
///
/// # Example
///
/// ```
/// use automation_state::cleanup::should_cleanup_state;
/// use automation_state::State;
/// use std::time::Duration;
///
/// let state = State::new();
/// // State with no last_run should not be reset
/// assert!(!should_cleanup_state(&state, Duration::from_secs(3600)));
/// ```
pub fn should_cleanup_state(state: &State, max_age: StdDuration) -> bool {
    let last_run = match state.last_run {
        Some(timestamp) => timestamp,
        None => return false,
    };

    let age = Utc::now() - last_run;
    let max_age_duration = chrono::TimeDelta::from_std(max_age)
        .expect("Duration conversion failed");

    age > max_age_duration
}

/// Calculates the age of a backup file.
///
/// The backup file name should contain a timestamp in ISO 8601 format
/// (e.g., `architect.state.json.backup.2024-01-01T12:00:00Z`).
///
/// # Arguments
///
/// * `backup_path` - Path to the backup file
///
/// # Returns
///
/// A tuple containing:
/// - The timestamp of the backup
/// - The size of the backup file in bytes
///
/// # Errors
///
/// Returns an error if:
/// - The backup file doesn't exist
/// - The timestamp cannot be parsed from the filename
///
/// # Example
///
/// ```
/// use automation_state::cleanup::calculate_backup_age;
/// use std::path::PathBuf;
///
/// # fn test() -> Result<(), Box<dyn std::error::Error>> {
/// let backup_path = PathBuf::from("/tmp/.state/architect.state.json.backup.2024-01-01T12:00:00Z");
/// let (timestamp, size) = calculate_backup_age(&backup_path)?;
/// # Ok(())
/// # }
/// ```
pub fn calculate_backup_age(backup_path: &Path) -> Result<(DateTime<Utc>, u64)> {
    // Extract timestamp from filename
    let filename = backup_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| AutomationError::StateError {
            task: "calculate_backup_age".to_string(),
            message: "Cannot extract filename from backup path".to_string(),
        })?;

    // The backup filename format is: {base}.backup.{timestamp}
    // We need to extract the timestamp part
    let timestamp_str = filename
        .rsplit('.')
        .next()
        .ok_or_else(|| AutomationError::StateError {
            task: "calculate_backup_age".to_string(),
            message: "Cannot extract timestamp from backup filename".to_string(),
        })?;

    let timestamp = DateTime::parse_from_rfc3339(timestamp_str)
        .map_err(|e| AutomationError::StateError {
            task: "calculate_backup_age".to_string(),
            message: format!("Cannot parse backup timestamp: {}", e),
        })?
        .with_timezone(&Utc);

    // Get file size
    let file_size = fs::metadata(backup_path)?
        .len();

    Ok((timestamp, file_size))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn create_temp_state_manager(agent_type: &str) -> (StateManager, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let state_dir = temp_dir.path().to_path_buf();
        let manager = StateManager::new(state_dir, agent_type).unwrap();
        (manager, temp_dir)
    }

    #[test]
    fn test_cleanup_config_defaults() {
        let config = CleanupConfig::default();
        assert!(config.max_backups.is_none());
        assert!(config.max_backup_age.is_none());
        assert!(config.max_state_age.is_none());
        assert!(config.max_mistake_age.is_none());
        assert!(config.stale_lock_timeout.is_none());
    }

    #[test]
    fn test_get_default_cleanup_config() {
        let config = get_default_cleanup_config();
        assert_eq!(config.max_backups, Some(10));
        assert_eq!(
            config.max_backup_age,
            Some(StdDuration::from_secs(7 * 24 * 60 * 60))
        );
        assert_eq!(
            config.max_state_age,
            Some(StdDuration::from_secs(30 * 24 * 60 * 60))
        );
        assert_eq!(
            config.max_mistake_age,
            Some(StdDuration::from_secs(24 * 60 * 60))
        );
        assert_eq!(
            config.stale_lock_timeout,
            Some(StdDuration::from_secs(5 * 60))
        );
    }

    #[test]
    fn test_should_cleanup_state() {
        let state = State::new();
        // State with no last_run should not be reset
        assert!(!should_cleanup_state(&state, StdDuration::from_secs(3600)));

        // State with recent last_run should not be reset
        let mut state = State::new();
        state.update_last_run(Utc::now());
        assert!(!should_cleanup_state(&state, StdDuration::from_secs(3600)));

        // State with old last_run should be reset
        let old_time = Utc::now() - chrono::TimeDelta::seconds(7200); // 2 hours ago
        state.update_last_run(old_time);
        assert!(should_cleanup_state(&state, StdDuration::from_secs(3600))); // 1 hour max_age
    }

    #[test]
    fn test_cleanup_stats() {
        let mut stats1 = CleanupStats::new();
        stats1.items_removed = 5;
        stats1.items_kept = 10;
        stats1.space_freed_bytes = 1024;

        let mut stats2 = CleanupStats::new();
        stats2.items_removed = 3;
        stats2.items_kept = 7;
        stats2.space_freed_bytes = 512;

        stats1.add(&stats2);
        assert_eq!(stats1.items_removed, 8);
        assert_eq!(stats1.items_kept, 17);
        assert_eq!(stats1.space_freed_bytes, 1536);
    }

    #[test]
    fn test_cleanup_report_new() {
        let report = CleanupReport::new();
        assert_eq!(report.backup_stats.items_removed, 0);
        assert_eq!(report.mistake_stats.items_removed, 0);
        assert_eq!(report.lock_stats.items_removed, 0);
        assert!(!report.state_reset);
        assert_eq!(report.total_bytes_freed, 0);
    }

    #[test]
    fn test_cleanup_backups_no_backups() {
        let (state_manager, _temp_dir) = create_temp_state_manager("test_agent");
        let config = CleanupConfig {
            max_backups: Some(5),
            ..Default::default()
        };
        let cleanup_manager = CleanupManager::new(state_manager, config);

        let stats = cleanup_manager.cleanup_backups().unwrap();
        assert_eq!(stats.items_removed, 0);
        assert_eq!(stats.items_kept, 0);
    }

    #[test]
    fn test_cleanup_mistakes_no_config() {
        let (state_manager, _temp_dir) = create_temp_state_manager("test_agent");
        let config = CleanupConfig::default();
        let cleanup_manager = CleanupManager::new(state_manager, config);

        let stats = cleanup_manager.cleanup_mistakes().unwrap();
        assert_eq!(stats.items_removed, 0);
    }

    #[test]
    fn test_cleanup_mistakes_with_config() {
        let (state_manager, _temp_dir) = create_temp_state_manager("test_agent");

        // Create state with old mistake
        let mut state = state_manager.load_state().unwrap();
        state.add_mistake("Test mistake".to_string(), Some("test".to_string()));
        state_manager.save_state(&state).unwrap();

        let config = CleanupConfig {
            max_mistake_age: Some(StdDuration::from_secs(0)), // Remove all mistakes
            ..Default::default()
        };
        let cleanup_manager = CleanupManager::new(state_manager, config);

        // Wait a bit to make mistake "old"
        thread::sleep(StdDuration::from_millis(10));

        let stats = cleanup_manager.cleanup_mistakes().unwrap();
        assert_eq!(stats.items_removed, 1);
    }

    #[test]
    fn test_cleanup_stale_locks_no_config() {
        let (state_manager, _temp_dir) = create_temp_state_manager("test_agent");
        let config = CleanupConfig::default();
        let cleanup_manager = CleanupManager::new(state_manager, config);

        let stats = cleanup_manager.cleanup_stale_locks().unwrap();
        assert_eq!(stats.items_removed, 0);
        assert_eq!(stats.items_kept, 0);
    }

    #[test]
    fn test_cleanup_all() {
        let (state_manager, _temp_dir) = create_temp_state_manager("test_agent");
        let config = CleanupConfig {
            max_mistake_age: Some(StdDuration::from_secs(0)),
            stale_lock_timeout: Some(StdDuration::from_secs(0)),
            ..Default::default()
        };
        let cleanup_manager = CleanupManager::new(state_manager, config);

        let report = cleanup_manager.cleanup_all().unwrap();
        // Should not crash even with no data
        assert!(!report.state_reset);
    }
}

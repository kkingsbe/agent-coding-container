//! Integration tests for the state module.

use automation_state::LockManager;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_lock_acquire_release() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let lock_path = temp_dir.path().join("test.lock");

    let lock_manager = LockManager::new(lock_path.clone(), Duration::from_secs(5));

    let lock = lock_manager.acquire_lock().expect("Should acquire lock");
    drop(lock);

    // Should be able to acquire again
    let _lock2 = lock_manager.acquire_lock().expect("Should acquire again");
}

#[test]
fn test_lock_timeout() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let lock_path = temp_dir.path().join("test.lock");

    let lock_manager1 = LockManager::new(lock_path.clone(), Duration::from_millis(100));
    let lock_manager2 = LockManager::new(lock_path.clone(), Duration::from_millis(100));

    let _lock1 = lock_manager1.acquire_lock().expect("Should acquire lock");

    let result = lock_manager2.acquire_lock();
    assert!(result.is_err(), "Should timeout waiting for lock");
}
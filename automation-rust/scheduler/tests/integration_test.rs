//! Integration tests for the scheduler module.
//!
//! Tests basic functionality: task creation, lifecycle, and stats tracking.

use automation_scheduler::ScheduledTask;
use std::time::Duration;

#[test]
fn test_task_creation() {
    let task = ScheduledTask::new(
        "test-task",
        Duration::from_millis(100),
        false,
        3,
        Duration::from_millis(10),
    );

    // Verify task was created
    assert!(!task.is_task_running(), "Task should not be running initially");
    assert_eq!(task.get_stats().success_count, 0);
}

#[test]
fn test_task_different_intervals() {
    let short_interval = ScheduledTask::new(
        "short-interval",
        Duration::from_millis(50),
        false,
        3,
        Duration::from_millis(10),
    );

    let long_interval = ScheduledTask::new(
        "long-interval",
        Duration::from_secs(60),
        false,
        3,
        Duration::from_millis(10),
    );

    // Tasks should be created successfully
    assert!(!short_interval.is_task_running());
    assert!(!long_interval.is_task_running());
}

#[test]
fn test_task_different_retry_configs() {
    let no_retries = ScheduledTask::new(
        "no-retries",
        Duration::from_secs(60),
        false,
        0,
        Duration::from_millis(10),
    );

    let many_retries = ScheduledTask::new(
        "many-retries",
        Duration::from_secs(60),
        false,
        10,
        Duration::from_millis(10),
    );

    let long_backoff = ScheduledTask::new(
        "long-backoff",
        Duration::from_secs(60),
        false,
        3,
        Duration::from_secs(1),
    );

    // All tasks should be created successfully
    assert!(!no_retries.is_task_running());
    assert!(!many_retries.is_task_running());
    assert!(!long_backoff.is_task_running());
}

#[test]
fn test_task_immediate_flag() {
    let immediate_task = ScheduledTask::new(
        "immediate",
        Duration::from_secs(60),
        true,
        3,
        Duration::from_millis(10),
    );

    let delayed_task = ScheduledTask::new(
        "delayed",
        Duration::from_secs(60),
        false,
        3,
        Duration::from_millis(10),
    );

    // Both should be created successfully
    assert!(!immediate_task.is_task_running());
    assert!(!delayed_task.is_task_running());
}

#[test]
fn test_task_stats_initial_state() {
    let task = ScheduledTask::new(
        "stats-test",
        Duration::from_secs(60),
        false,
        3,
        Duration::from_millis(10),
    );

    let stats = task.get_stats();
    assert_eq!(stats.success_count, 0);
    assert_eq!(stats.failure_count, 0);
    assert!(stats.last_run_timestamp.is_none());
}

#[test]
fn test_multiple_tasks_independent() {
    let task1 = ScheduledTask::new(
        "task1",
        Duration::from_millis(100),
        false,
        3,
        Duration::from_millis(10),
    );

    let task2 = ScheduledTask::new(
        "task2",
        Duration::from_millis(100),
        false,
        3,
        Duration::from_millis(10),
    );

    let task3 = ScheduledTask::new(
        "task3",
        Duration::from_millis(100),
        false,
        3,
        Duration::from_millis(10),
    );

    // All tasks should be created independently
    assert!(!task1.is_task_running());
    assert!(!task2.is_task_running());
    assert!(!task3.is_task_running());

    let stats1 = task1.get_stats();
    let stats2 = task2.get_stats();
    let stats3 = task3.get_stats();

    assert_eq!(stats1.success_count, 0);
    assert_eq!(stats2.success_count, 0);
    assert_eq!(stats3.success_count, 0);
}

#[test]
fn test_task_config_values() {
    let task = ScheduledTask::new(
        "config-test",
        Duration::from_secs(120),
        true,
        5,
        Duration::from_millis(50),
    );

    // We can't access config directly, but we can verify task state
    let stats = task.get_stats();
    assert_eq!(stats.success_count, 0);
    assert_eq!(stats.failure_count, 0);
}

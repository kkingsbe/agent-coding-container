# Parallel Agent Instances Design Plan

## Executive Summary

This document outlines a comprehensive design for allowing multiple instances of the same agent type (e.g., multiple "PROMPT.md" agents) to run in parallel while preventing:
1. Task assignment conflicts (two agents working on the same task simultaneously)
2. Task hanging when the assigned agent fails or restarts

## Table of Contents

1. [Current Architecture Analysis](#current-architecture-analysis)
2. [Key Design Decisions](#key-design-decisions)
3. [Parallel Agent Instance Design](#parallel-agent-instance-design)
4. [Task Assignment Conflict Resolution](#task-assignment-conflict-resolution)
5. [Agent Failure/Recovery Mechanism](#agent-failurerecovery-mechanism)
6. [State Management Modifications](#state-management-modifications)
7. [Scheduler Modifications](#scheduler-modifications)
8. [Executor Modifications](#executor-modifications)
9. [Agent Modifications](#agent-modifications)
10. [Configuration Changes](#configuration-changes)
11. [Migration Strategy](#migration-strategy)
12. [Testing Strategy](#testing-strategy)

---

## Current Architecture Analysis

### Current Agent Execution Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    Current Architecture                        │
├─────────────────────────────────────────────────────────────────┤
│                                                         │
│  ┌──────────────┐         ┌──────────────┐           │
│  │  prompt.lock  │  ← Only │  Prompt Agent │           │
│  │  (exclusive)  │  ← One  │   Instance   │           │
│  └──────────────┘         └──────────────┘           │
│                                                         │
│  ┌──────────────┐         ┌──────────────┐           │
│  │ janitor.lock  │  ← Only │  Janitor Agent│           │
│  │  (exclusive)  │  ← One  │   Instance   │           │
│  └──────────────┘         └──────────────┘           │
│                                                         │
│  Agent reads all tasks from TODO.md → processes all → writes │
│  back to TODO.md and .prompt-output.md                      │
└─────────────────────────────────────────────────────────────────┘
```

### Current Locking Mechanism

**File: [`state/src/lock.rs`](state/src/lock.rs:265-320)**

The current `LockManager` provides:
- Exclusive file-based locks using `fs2` crate
- Lock files: `{state_dir}/{agent_type}.lock`
- RAII pattern for automatic lock release
- Stale lock detection with configurable timeout

**Current Lock File Structure:**
```json
{
  "acquired_at": "2024-02-09T21:30:45.123Z",
  "process_id": 12345,
  "hostname": "worker-1"
}
```

**Issue**: Each agent type has exactly ONE lock file. Only one instance can hold the lock at a time.

### Current State Management

**File: [`state/src/state.rs`](state/src/state.rs:584-613)**

The current `State` structure tracks:
- Execution timestamps (last_run, last_success, last_failure)
- Error counts and consecutive failures
- Status string (e.g., "idle", "running", "success", "error")
- Termination reasons and counts
- Mistake/error records

**Issue**: State is per-agent-type, not per-task. No tracking of:
- Which specific task is being worked on
- Task assignments to specific agent instances
- Task lease expiration
- Agent instance identity

### Current Agent Execution Flow

**File: [`agents/src/agent.rs`](agents/src/agent.rs:310-586)**

```
1. Acquire agent-type lock (blocks if another instance running)
2. Load state → Set status to "running"
3. Read workspace files (TODO.md, BACKLOG.md, etc.)
4. Execute prompt template with context substitution
5. Execute Kilo Code CLI
6. Write output file (.prompt-output.md)
7. Update state (success/failure/termination)
8. Release lock (RAII)
```

**Issues**:
1. No task-level coordination - agent processes all tasks it finds
2. If agent crashes mid-task, no recovery mechanism
3. No way to detect which task is being processed
4. Tasks can "hang" if agent dies without releasing lock

---

## Key Design Decisions

### 1. Move from Agent-Level to Task-Level Locking

**Decision**: Instead of locking at the agent type level, lock at the individual task level.

**Rationale**:
- Enables multiple agent instances to run in parallel
- Each instance can work on different tasks independently
- Natural coordination mechanism for task assignment

### 2. Implement Task Lease System with Timeouts

**Decision**: Use task leases with expiration times to handle agent failures.

**Rationale**:
- If an agent crashes, the lease expires and task becomes available
- Prevents tasks from being "claimed" indefinitely
- Enables detection of stale/in-progress tasks

### 3. Use Agent Registration with Heartbeats

**Decision**: Each agent instance registers itself and updates heartbeats periodically.

**Rationale**:
- Enables detection of dead/unresponsive agents
- Allows cleanup of orphaned tasks from crashed instances
- Provides visibility into active agent instances

### 4. Maintain Backward Compatibility

**Decision**: Keep existing lock mechanism for agent coordination, add new mechanisms as layers.

**Rationale**:
- Minimizes risk of breaking changes
- Allows gradual migration
- Existing single-instance agents continue to work

---

## Parallel Agent Instance Design

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│              Proposed Parallel Architecture                    │
├─────────────────────────────────────────────────────────────────┤
│                                                         │
│  ┌──────────────────┐         ┌──────────────────┐       │
│  │  Agent Registry │         │  Task Registry  │       │
│  │  (new)          │         │  (new)          │       │
│  └────────┬─────────┘         └────────┬─────────┘       │
│           │                            │                   │
│           │ Tracks active              │ Tracks task       │
│           │ agent instances            │ state/leases     │
│           │ with heartbeats          │                   │
│           ↓                            ↓                   │
│  ┌───────────────────────────────────────────────────────┐   │
│  │               State Directory                      │   │
│  ├───────────────────────────────────────────────────────┤   │
│  │  .state/                                         │   │
│  │    ├── agent-registry.json           (new)          │   │
│  │    ├── task-registry.json            (new)          │   │
│  │    ├── prompt.lock                 (existing)       │   │
│  │    ├── prompt.state.json           (existing)       │   │
│  │    └── tasks/                                    │   │
│  │         ├── task-001.lock              (new)        │   │
│  │         ├── task-002.lock              (new)        │   │
│  │         └── task-001.state.json        (new)        │   │
│  └───────────────────────────────────────────────────────┘   │
│                                                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │
│  │ Prompt      │  │ Prompt      │  │ Prompt      │ │
│  │ Instance 1  │  │ Instance 2  │  │ Instance 3  │ │
│  │ (new)       │  │ (new)       │  │ (new)       │ │
│  └─────────────┘  └─────────────┘  └─────────────┘ │
│       ↓                ↓                ↓              │
│  Claim Task A     Claim Task B    Claim Task C       │
│                                                         │
│  Each instance:                                          │
│  1. Registers with heartbeat                             │
│  2. Claims available tasks with leases                   │
│  3. Updates heartbeat while working                       │
│  4. Releases task on completion/cleanup                  │
└─────────────────────────────────────────────────────────────────┘
```

### Agent Registry Design

**Purpose**: Track all active agent instances with heartbeat monitoring.

**File**: `.state/agent-registry.json`

**Schema**:
```rust
/// Registry entry for an active agent instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegistration {
    /// Unique agent instance ID (UUID v4)
    pub agent_id: String,

    /// Agent type (architect, janitor, prompt, code_review)
    pub agent_type: String,

    /// Hostname where agent is running
    pub hostname: String,

    /// Process ID of the agent
    pub pid: u32,

    /// Time when this agent registered
    pub registered_at: DateTime<Utc>,

    /// Last heartbeat timestamp
    pub last_heartbeat: DateTime<Utc>,

    /// Current status of this agent
    pub status: AgentStatus,
}

/// Agent instance status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    /// Agent is active and idle (looking for work)
    Active,
    /// Agent is currently processing a task
    Working,
    /// Agent is shutting down gracefully
    ShuttingDown,
    /// Agent terminated unexpectedly (detected by heartbeat timeout)
    Stale,
}

/// The agent registry containing all registered instances
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegistry {
    /// All registered agent instances
    pub agents: Vec<AgentRegistration>,

    /// Heartbeat timeout threshold (in seconds)
    pub heartbeat_timeout_seconds: u64,
}
```

**Operations**:
- `register_agent()`: Register a new agent instance
- `update_heartbeat()`: Update heartbeat for an agent instance
- `unregister_agent()`: Remove an agent instance on shutdown
- `get_active_agents()`: Get all non-stale active agents
- `cleanup_stale_agents()`: Remove agents with expired heartbeats

### Task Registry Design

**Purpose**: Track all tasks with their current state, assignments, and leases.

**File**: `.state/task-registry.json`

**Schema**:
```rust
/// Registry entry for a task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEntry {
    /// Unique task ID (generated from task content hash or UUID)
    pub task_id: String,

    /// Task description or content
    pub task_content: String,

    /// Task status
    pub status: TaskEntryStatus,

    /// ID of agent currently assigned to this task (if any)
    pub assigned_agent_id: Option<String>,

    /// Lease expiration time
    pub lease_expires_at: Option<DateTime<Utc>>,

    /// Time when task was last modified
    pub last_modified: DateTime<Utc>,

    /// Number of times this task has been attempted
    pub attempt_count: u64,

    /// Last error if task failed
    pub last_error: Option<String>,

    /// History of task assignments
    pub assignment_history: Vec<TaskAssignment>,
}

/// Task entry status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskEntryStatus {
    /// Task is available for assignment
    Available,
    /// Task is currently being worked on
    InProgress,
    /// Task completed successfully
    Completed,
    /// Task failed and should be retried
    Failed,
    /// Task permanently failed (max retries exceeded)
    PermanentlyFailed,
}

/// Record of a task assignment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignment {
    /// Agent ID that was assigned
    pub agent_id: String,

    /// Time when assignment started
    pub started_at: DateTime<Utc>,

    /// Time when assignment ended (if completed)
    pub ended_at: Option<DateTime<Utc>>,

    /// Outcome of the assignment
    pub outcome: AssignmentOutcome,
}

/// Outcome of a task assignment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AssignmentOutcome {
    /// Assignment in progress
    InProgress,
    /// Task completed successfully
    Success,
    /// Task failed
    Failed,
    /// Agent crashed/died during assignment
    Abandoned,
}

/// The task registry containing all tracked tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRegistry {
    /// All tracked tasks
    pub tasks: HashMap<String, TaskEntry>,

    /// Default lease duration for task assignments (in seconds)
    pub default_lease_duration_seconds: u64,

    /// Maximum retry attempts before marking as permanently failed
    pub max_retry_attempts: u64,
}
```

**Operations**:
- `register_tasks()`: Register tasks from workspace files
- `sync_with_workspace()`: Sync registry with current TODO.md/BACKLOG.md
- `claim_task()`: Atomically claim an available task with a lease
- `renew_lease()`: Extend lease for a task being worked on
- `release_task()`: Mark task as completed or failed
- `abandon_task()`: Mark task as abandoned (agent crashed)
- `get_claimable_tasks()`: Get tasks available for claiming
- `cleanup_expired_leases()`: Release tasks with expired leases

---

## Task Assignment Conflict Resolution

### Task Claiming Algorithm

```
┌─────────────────────────────────────────────────────────────────┐
│               Task Claiming Flow                             │
└─────────────────────────────────────────────────────────────────┘

   Agent Instance A          Agent Instance B          Agent Instance C
         │                         │                         │
         │  1. Get available tasks  │                         │
         ├────────────────────────→│                         │
         │  [Task A, Task B, Task C]                        │
         │←─────────────────────────┤                         │
         │                         │                         │
         │  2. Try claim Task A   │                         │
         ├────────────────────────→│                         │
         │  Attempt claim A                                    │
         │←─────────────────────────┤                         │
         │  [SUCCESS]               │                         │
         │                         │  3. Try claim Task A   │
         │                         ├────────────────────────→│
         │                         │  Attempt claim A        │
         │                         │←─────────────────────────┤
         │                         │  [FAILED - already claimed]
         │                         │                         │
         │                         │  4. Try claim Task B   │
         │                         ├────────────────────────→│
         │                         │  Attempt claim B        │
         │                         │←─────────────────────────┤
         │                         │  [SUCCESS]               │
         │                                                   │  5. Try claim Task C
         │                                                   ├────────────────────────→
         │                                                   │  Attempt claim C
         │                                                   │←─────────────────────────
         │                                                   │  [SUCCESS]
         │                                                   │
         ↓                                                   ↓
   Working on A                                         Working on C
                                                         Working on B
```

### Claim Task Implementation

```rust
/// Atomically claim a task with a lease
///
/// Returns the claimed task entry if successful, or None if no tasks available.
/// Uses atomic file locking to prevent race conditions.
pub fn claim_task(
    &mut self,
    agent_id: &str,
    lease_duration: Duration,
) -> Result<Option<TaskEntry>> {
    // Acquire lock on task registry file
    let _lock = self.lock_manager.acquire_lock()?;

    // Load current registry
    let mut registry = self.load_registry()?;

    // Find available task
    let claimable_task = registry.tasks.values()
        .find(|task| task.status == TaskEntryStatus::Available)
        .filter(|task| {
            // Check retry limits
            task.attempt_count < registry.max_retry_attempts
        });

    let task_id = match claimable_task {
        Some(task) => task.task_id.clone(),
        None => return Ok(None),
    };

    // Update task entry
    if let Some(task) = registry.tasks.get_mut(&task_id) {
        task.status = TaskEntryStatus::InProgress;
        task.assigned_agent_id = Some(agent_id.to_string());
        task.lease_expires_at = Some(Utc::now() + ChronoDuration::from_std(lease_duration)?);
        task.attempt_count += 1;
        task.last_modified = Utc::now();

        // Record assignment in history
        task.assignment_history.push(TaskAssignment {
            agent_id: agent_id.to_string(),
            started_at: Utc::now(),
            ended_at: None,
            outcome: AssignmentOutcome::InProgress,
        });
    }

    // Save registry (atomic write)
    self.save_registry(&registry)?;

    // Create task-specific lock file for additional safety
    self.create_task_lock(&task_id, agent_id, lease_duration)?;

    Ok(registry.tasks.get(&task_id).cloned())
}
```

### Lease Renewal Mechanism

Agents must periodically renew their task lease while working:

```rust
/// Renew lease for a task being worked on
pub fn renew_lease(
    &mut self,
    task_id: &str,
    agent_id: &str,
    lease_duration: Duration,
) -> Result<()> {
    // Acquire lock
    let _lock = self.lock_manager.acquire_lock()?;

    // Load registry
    let mut registry = self.load_registry()?;

    // Validate assignment
    let task = registry.tasks.get_mut(task_id)
        .ok_or_else(|| AutomationError::StateError {
            task: task_id.to_string(),
            message: "Task not found".to_string(),
        })?;

    // Verify this agent owns the task
    if task.assigned_agent_id.as_deref() != Some(agent_id) {
        return Err(AutomationError::StateError {
            task: task_id.to_string(),
            message: "Agent does not own this task".to_string(),
        });
    }

    // Update lease expiration
    task.lease_expires_at = Some(Utc::now() + ChronoDuration::from_std(lease_duration)?);
    task.last_modified = Utc::now();

    // Save registry
    self.save_registry(&registry)?;

    Ok(())
}
```

---

## Agent Failure/Recovery Mechanism

### Heartbeat System

**Purpose**: Detect dead or unresponsive agent instances.

**Configuration**:
- Heartbeat interval: Every 30 seconds
- Heartbeat timeout: 90 seconds (3 missed heartbeats)
- Lease duration: 10 minutes (longer than heartbeat timeout)

**Heartbeat Flow**:

```
┌─────────────────────────────────────────────────────────────────┐
│                 Heartbeat Flow                               │
├─────────────────────────────────────────────────────────────────┤
│                                                         │
│  Agent Instance                                           │
│         │                                                 │
│         │──[30s]──► Register/Update heartbeat              │
│         │                                                   │
│         │──[30s]──► Update heartbeat                      │
│         │                                                   │
│         │──[CRASH]──► Agent dies                              │
│         │                                                   │
│         │  No heartbeat after 90s                              │
│         │                                                   │
│         ↓                                                   │
│  Agent marked as "Stale"                                    │
│         │                                                   │
│         │                                                   │
│  Cleanup Task:                                              │
│  1. Mark agent as stale in registry                         │
│  2. Abandon all tasks assigned to stale agent               │
│  3. Tasks return to Available status                         │
│  4. Other agents can claim them                              │
└─────────────────────────────────────────────────────────────────┘
```

### Cleanup Process

```rust
/// Cleanup stale agents and abandoned tasks
pub fn cleanup_stale_resources(&mut self) -> Result<CleanupStats> {
    let now = Utc::now();
    let mut stats = CleanupStats::default();

    // Acquire lock on both registries
    let _lock = self.registry_lock.acquire_lock()?;

    // Load registries
    let mut agent_registry = self.load_agent_registry()?;
    let mut task_registry = self.load_task_registry()?;

    let heartbeat_timeout = ChronoDuration::seconds(
        agent_registry.heartbeat_timeout_seconds as i64
    );

    // Find stale agents
    let stale_agents: Vec<String> = agent_registry.agents
        .iter()
        .filter(|agent| {
            match agent.status {
                AgentStatus::Active | AgentStatus::Working => {
                    now - agent.last_heartbeat > heartbeat_timeout
                }
                AgentStatus::Stale => true,  // Already stale
                _ => false,  // Shutting down, leave alone
            }
        })
        .map(|a| a.agent_id.clone())
        .collect();

    // Mark stale agents
    for agent_id in &stale_agents {
        if let Some(agent) = agent_registry.agents.iter_mut()
            .find(|a| &a.agent_id == agent_id)
        {
            agent.status = AgentStatus::Stale;
            stats.stale_agents_cleaned += 1;

            // Abandon tasks assigned to this agent
            for task in task_registry.tasks.values_mut() {
                if task.assigned_agent_id.as_deref() == Some(agent_id) {
                    task.status = TaskEntryStatus::Available;
                    task.assigned_agent_id = None;
                    task.lease_expires_at = None;

                    // Update last assignment as abandoned
                    if let Some(assignment) = task.assignment_history.last_mut() {
                        assignment.ended_at = Some(now);
                        assignment.outcome = AssignmentOutcome::Abandoned;
                    }

                    stats.abandoned_tasks += 1;
                }
            }
        }
    }

    // Save both registries
    self.save_agent_registry(&agent_registry)?;
    self.save_task_registry(&task_registry)?;

    Ok(stats)
}
```

### Startup Recovery

When an agent starts, it should:

1. **Check for orphaned tasks** from a previous instance with same PID
2. **Recover or abandon** based on task state
3. **Clean up stale locks**

```rust
/// Recover orphaned tasks on agent startup
pub fn recover_orphaned_tasks(&self, agent_id: &str) -> Result<Vec<String>> {
    // Load task registry
    let mut registry = self.load_task_registry()?;

    let mut recovered = Vec::new();

    // Find tasks assigned to this agent but potentially orphaned
    for task in registry.tasks.values_mut() {
        if task.assigned_agent_id.as_deref() == Some(agent_id) {
            match task.status {
                TaskEntryStatus::InProgress => {
                    // Task was in progress, mark as available for retry
                    task.status = TaskEntryStatus::Available;
                    task.assigned_agent_id = None;
                    task.lease_expires_at = None;
                    recovered.push(task.task_id.clone());
                }
                _ => {
                    // Task completed or failed, leave it alone
                }
            }
        }
    }

    // Save registry
    if !recovered.is_empty() {
        self.save_task_registry(&registry)?;
    }

    Ok(recovered)
}
```

---

## State Management Modifications

### New Module: `task_registry`

**File**: `state/src/task_registry.rs`

**Purpose**: Provide task-level state tracking and coordination.

**Key Types**:
```rust
pub use crate::task_registry::{
    TaskEntry,
    TaskEntryStatus,
    TaskAssignment,
    AssignmentOutcome,
    TaskRegistry,
    AgentRegistration,
    AgentStatus,
};
```

**Key Functions**:
```rust
impl TaskRegistry {
    pub fn new(state_dir: PathBuf, config: TaskRegistryConfig) -> Result<Self>;
    pub fn register_agent(&mut self, agent: AgentRegistration) -> Result<()>;
    pub fn update_heartbeat(&mut self, agent_id: &str) -> Result<()>;
    pub fn unregister_agent(&mut self, agent_id: &str) -> Result<()>;
    pub fn register_tasks(&mut self, tasks: Vec<(&str, &str)>) -> Result<()>;
    pub fn claim_task(&mut self, agent_id: &str, lease_duration: Duration) -> Result<Option<TaskEntry>>;
    pub fn renew_lease(&mut self, task_id: &str, agent_id: &str, lease_duration: Duration) -> Result<()>;
    pub fn release_task(&mut self, task_id: &str, agent_id: &str, outcome: AssignmentOutcome) -> Result<()>;
    pub fn abandon_task(&mut self, task_id: &str, agent_id: &str) -> Result<()>;
    pub fn cleanup_expired_leases(&mut self) -> Result<CleanupStats>;
    pub fn cleanup_stale_agents(&mut self) -> Result<CleanupStats>;
    pub fn get_claimable_tasks(&self) -> Vec<TaskEntry>;
    pub fn sync_with_workspace(&mut self, workspace_content: &WorkspaceContent) -> Result<SyncStats>;
}
```

### Modifications to Existing State

**File**: `state/src/state.rs`

**Additions to [`State`](state/src/state.rs:584-613) structure**:
```rust
/// Agent instance ID for this state (if applicable)
pub agent_instance_id: Option<String>,

/// Current task ID being worked on (if any)
pub current_task_id: Option<String>,
```

### New Lock Types

**File**: `state/src/lock.rs`

**Add new lock manager for task-level locking**:
```rust
/// Task-specific lock manager with lease support
pub struct TaskLockManager {
    lock_manager: LockManager,
    state_dir: PathBuf,
}

impl TaskLockManager {
    /// Create a lock for a specific task
    pub fn new(state_dir: PathBuf, task_id: &str) -> Result<Self>;

    /// Acquire task lock with lease
    pub fn acquire_with_lease(&self, agent_id: &str, lease_duration: Duration) -> Result<TaskLockHandle>;

    /// Renew existing lease
    pub fn renew_lease(&self, lease_duration: Duration) -> Result<()>;
}

/// Handle for a task lock with automatic lease renewal
pub struct TaskLockHandle {
    lock_handle: LockHandle,
    task_id: String,
    agent_id: String,
    state_dir: PathBuf,
    lease_duration: Duration,
    renewal_handle: Option<JoinHandle<()>>,
}
```

---

## Scheduler Modifications

### Background Heartbeat Task

**File**: `scheduler/src/task.rs`

**Add heartbeat scheduling**:
```rust
impl ScheduledTask {
    /// Add a periodic heartbeat task to the scheduled task
    pub async fn start_with_heartbeat<H, HF>(
        &mut self,
        handler: H,
        heartbeat_fn: HF,
        heartbeat_interval: Duration,
    ) -> Result<(), AutomationError>
    where
        H: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<()>> + Send,
        HF: Fn() -> HeartbeatResult + Send + Sync + 'static,
    {
        // ... existing task setup ...

        // Spawn heartbeat task
        let heartbeat_handle = tokio::spawn(async move {
            let mut heartbeat_timer = tokio::time::interval(heartbeat_interval);
            loop {
                heartbeat_timer.tick().await;
                match heartbeat_fn() {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Heartbeat failed: {}", e);
                        // Could trigger graceful shutdown
                    }
                }
            }
        });

        // ... rest of task logic ...
    }
}
```

---

## Executor Modifications

### Task Context Tracking

**File**: `executor/src/cli.rs`

**Add task ID to executor config**:
```rust
pub struct ExecutorConfig {
    /// ... existing fields ...

    /// Task ID for tracking (optional)
    pub task_id: Option<String>,

    /// Agent instance ID for tracking (optional)
    pub agent_instance_id: Option<String>,
}
```

---

## Agent Modifications

### Agent Startup Flow

**File**: `agents/src/agent.rs`

**New flow for parallel agent instances**:

```rust
impl AgentRunner {
    pub async fn run(&self) -> Result<()> {
        // Generate unique agent instance ID
        let agent_id = Uuid::new_v4().to_string();

        // Initialize task registry
        let mut task_registry = TaskRegistry::new(
            PathBuf::from(&self.config.state_dir),
            TaskRegistryConfig::default(),
        )?;

        // Register this agent instance
        task_registry.register_agent(AgentRegistration {
            agent_id: agent_id.clone(),
            agent_type: self.config.agent_type.to_string(),
            hostname: get_hostname(),
            pid: get_current_process_id(),
            registered_at: Utc::now(),
            last_heartbeat: Utc::now(),
            status: AgentStatus::Active,
        })?;

        info!(
            agent_id = %agent_id,
            "Agent registered"
        );

        // Recover any orphaned tasks from previous instance
        let recovered = task_registry.recover_orphaned_tasks(&agent_id)?;
        if !recovered.is_empty() {
            info!(
                recovered_count = recovered.len(),
                "Recovered orphaned tasks"
            );
        }

        // Start heartbeat task
        let agent_id_clone = agent_id.clone();
        let heartbeat_handle = tokio::spawn(async move {
            let mut heartbeat_timer = tokio::time::interval(Duration::from_secs(30));
            loop {
                heartbeat_timer.tick().await;
                if let Err(e) = task_registry.update_heartbeat(&agent_id_clone) {
                    error!("Heartbeat update failed: {}", e);
                }
            }
        });

        // Main work loop
        let mut task = create_scheduled_task(&format!("{}-{}", self.config.agent_type, agent_id));

        let workspace = self.config.workspace.clone();
        let state_dir = self.config.state_dir.clone();
        let agent_type = self.config.agent_type;
        let timeout = Duration::from_secs(self.config.timeout_seconds);
        let agent_id_inner = agent_id.clone();

        let handler = move || {
            let workspace = workspace.clone();
            let state_dir = state_dir.clone();
            let agent_id = agent_id.clone();
            let agent_type = agent_type;
            let timeout = timeout;
            let agent_id_clone = agent_id_inner.clone();

            async move {
                Self::execute_with_task_registry(
                    &workspace,
                    &state_dir,
                    &agent_id,
                    agent_type,
                    timeout,
                ).await
                .map_err(|e| anyhow::anyhow!("Handler execution failed: {}", e))
            }
        };

        task.start(handler).await?;

        // Cleanup on shutdown
        task_registry.unregister_agent(&agent_id)?;
        heartbeat_handle.abort();

        Ok(())
    }

    /// Execute agent using task registry for parallel support
    async fn execute_with_task_registry(
        workspace: &str,
        state_dir: &str,
        agent_id: &str,
        agent_type: AgentType,
        timeout: Duration,
    ) -> Result<()> {
        let task_registry = TaskRegistry::new(
            PathBuf::from(state_dir),
            TaskRegistryConfig::default(),
        )?;

        // Step 1: Sync task registry with workspace
        let workspace_path = Path::new(workspace);
        let workspace_manager = WorkspaceManager::new(workspace_path);

        let (todo, backlog, completed, blockers, prd) = tokio::join!(
            tokio::task::spawn_blocking(move || workspace_manager.read_todo_file()),
            tokio::task::spawn_blocking(move || workspace_manager.read_backlog_file()),
            tokio::task::spawn_blocking(move || workspace_manager.read_completed_file()),
            tokio::task::spawn_blocking(move || workspace_manager.read_blockers_file()),
            tokio::task::spawn_blocking(move || workspace_manager.read_prd_file()),
        );

        let todo = todo??;
        let backlog = backlog??;

        // Sync with registry
        task_registry.sync_with_workspace(&WorkspaceContent {
            todo: &todo,
            backlog: &backlog,
        })?;

        // Step 2: Claim a task
        let claimed_task = task_registry.claim_task(agent_id, Duration::from_secs(600))?; // 10 min lease

        let task_entry = match claimed_task {
            Some(task) => task,
            None => {
                info!("No tasks available to claim");
                return Ok(());
            }
        };

        info!(
            task_id = %task_entry.task_id,
            "Claimed task"
        );

        // Step 3: Setup lease renewal
        let task_id = task_entry.task_id.clone();
        let agent_id_clone = agent_id.to_string();
        let state_dir_clone = state_dir.to_string();
        let renewal_handle = tokio::spawn(async move {
            let mut renewal_timer = tokio::time::interval(Duration::from_secs(300)); // Every 5 min
            loop {
                renewal_timer.tick().await;
                if let Err(e) = task_registry.renew_lease(&task_id, &agent_id_clone, Duration::from_secs(600)) {
                    error!("Failed to renew task lease: {}", e);
                }
            }
        });

        // Step 4: Execute the task
        let result = Self::execute_task_internal(
            workspace,
            &task_entry.task_content,
            agent_type,
            timeout,
        ).await;

        // Step 5: Stop lease renewal
        renewal_handle.abort();

        // Step 6: Release task
        let outcome = match result {
            Ok(_) => AssignmentOutcome::Success,
            Err(_) => AssignmentOutcome::Failed,
        };

        task_registry.release_task(&task_entry.task_id, agent_id, outcome)?;

        Ok(result?)
    }
}
```

---

## Configuration Changes

### New Configuration Options

**File**: `common/src/workspace_config.rs` (additions)

```toml
[workspace]
path = "./workspace"
state_path = ".state"

[agents.parallel]
# Enable parallel agent instances (default: false for backward compatibility)
enabled = false

# Heartbeat interval in seconds (default: 30)
heartbeat_interval_seconds = 30

# Heartbeat timeout in seconds (default: 90)
heartbeat_timeout_seconds = 90

# Default task lease duration in seconds (default: 600)
lease_duration_seconds = 600

# Maximum retry attempts before marking task as permanently failed (default: 3)
max_retry_attempts = 3

# Enable automatic cleanup of stale resources (default: true)
auto_cleanup_enabled = true
```

---

## Migration Strategy

### Phase 1: Infrastructure (No Breaking Changes)

1. **Add new modules** without modifying existing behavior:
   - `state/src/task_registry.rs`
   - `state/src/agent_registry.rs`
   - `state/src/lock.rs` (extend with task locks)

2. **Add configuration** options (default to disabled):
   - `agents.parallel.enabled = false`
   - All parallel-specific settings default to safe values

### Phase 2: Opt-In Parallel Mode

1. **Modify agents** to check for parallel mode flag:
   - If disabled: use existing single-instance flow
   - If enabled: use new task registry flow

2. **Update documentation** with migration guide

### Phase 3: Default Enablement (Future)

1. Once validated in production, consider enabling parallel mode by default
2. Keep single-instance mode as fallback for compatibility

---

## Testing Strategy

### Unit Tests

- **TaskRegistry**: Test claiming, releasing, lease expiration
- **AgentRegistry**: Test registration, heartbeat, stale detection
- **LockManager**: Test task-level locks with leases

### Integration Tests

- **Parallel agents**: Spawn multiple agent instances, verify tasks distributed
- **Failure scenarios**: Kill agent mid-task, verify recovery
- **Lease expiration**: Verify tasks reclaimed after timeout

### Load Tests

- **Multiple instances**: Test with 10+ parallel instances
- **Concurrent claiming**: Race condition testing
- **Recovery under load**: Multiple failures and restarts

---

## Summary

This design provides:

1. **Parallel Agent Instances**: Multiple instances of the same agent type can run concurrently
2. **Task Assignment Coordination**: Tasks are atomically claimed with leases to prevent conflicts
3. **Fault Tolerance**: Agent failures are detected via heartbeats, tasks are recovered
4. **Backward Compatibility**: Existing single-instance mode continues to work
5. **Scalability**: Design scales from 1 to N agent instances
6. **Observability**: Full tracking of agents, tasks, assignments, and history

### Key Components to Implement

1. `state/src/task_registry.rs` - New module for task tracking
2. `state/src/agent_registry.rs` - New module for agent tracking
3. Extensions to `state/src/lock.rs` - Task-level locks with leases
4. Modifications to `agents/src/agent.rs` - Parallel execution flow
5. Configuration additions in `common/src/workspace_config.rs`
6. Documentation updates
7. Comprehensive test coverage

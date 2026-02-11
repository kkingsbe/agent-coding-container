//! Agent registry for tracking agent instances with heartbeats and status.
//!
//! This module provides an agent registry that enables parallel agent execution by:
//! - Tracking agent instances with unique identifiers
//! - Monitoring agent heartbeats for health checks
//! - Managing agent status (Active, Stale, Shutdown)
//! - Tracking task assignments per agent
//! - Providing cleanup functionality for stale and shutdown agents
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
//! use automation_state::agent_registry::{AgentRegistry, AgentStatus};
//! use std::time::Duration;
//!
//! // Create or load an agent registry
//! let mut registry = AgentRegistry::new().unwrap();
//!
//! // Register a new agent instance
//! registry.register_agent("prompt-agent-123".to_string(), "prompt".to_string()).unwrap();
//!
//! // Update heartbeat
//! registry.update_heartbeat("prompt-agent-123").unwrap();
//!
//! // Assign a task to an agent
//! registry.assign_task("prompt-agent-123", "task-1").unwrap();
//!
//! // Get agent statistics
//! let stats = registry.get_agent_stats("prompt-agent-123").unwrap();
//! println!("Agent uptime: {:?}", stats.uptime);
//! ```

use automation_common::{AutomationError, Result};
use crate::lock::{LockManager, get_current_process_id, get_hostname};
use crate::persistence::atomic_write;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

/// Base path for agent registry files
const REGISTRY_BASE_PATH: &str = ".state/agents";
/// Name of the registry file
const REGISTRY_FILE_NAME: &str = "registry.json";
/// Default timeout for lock acquisition (30 seconds)
const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(30);

/// Agent status enumeration.
///
/// Represents the current status of an agent in the registry.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    /// Agent is active and processing
    Active,
    /// Agent has not sent a recent heartbeat (potentially dead)
    Stale,
    /// Agent has been shut down gracefully
    Shutdown,
}

impl AgentStatus {
    /// Returns true if the agent is active.
    pub fn is_active(&self) -> bool {
        matches!(self, AgentStatus::Active)
    }

    /// Returns true if the agent is stale.
    pub fn is_stale(&self) -> bool {
        matches!(self, AgentStatus::Stale)
    }

    /// Returns true if the agent is shut down.
    pub fn is_shutdown(&self) -> bool {
        matches!(self, AgentStatus::Shutdown)
    }
}

impl Default for AgentStatus {
    fn default() -> Self {
        AgentStatus::Active
    }
}

/// Agent instance information.
///
/// Contains all metadata about an agent including its status, timestamps,
/// process information, and task assignments.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentInstance {
    /// Unique identifier for this agent instance
    pub agent_id: String,
    /// Type of agent (e.g., "prompt", "architect", "code_review")
    pub agent_type: String,
    /// Current status of the agent
    pub status: AgentStatus,
    /// Timestamp when the agent was started
    pub started_at: DateTime<Utc>,
    /// Timestamp of the last heartbeat from this agent
    pub last_heartbeat_at: DateTime<Utc>,
    /// Hostname where the agent is running (if available)
    pub hostname: Option<String>,
    /// Process ID of the agent (if available)
    pub pid: Option<u32>,
    /// Set of task IDs assigned to this agent
    pub tasks_assigned: HashSet<String>,
}

impl AgentInstance {
    /// Creates a new agent instance with the given ID and type.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - Unique identifier for the agent instance
    /// * `agent_type` - Type of the agent (e.g., "prompt", "architect")
    ///
    /// # Example
    ///
    /// ```
    /// use automation_state::agent_registry::{AgentInstance, AgentStatus};
    /// use chrono::Utc;
    ///
    /// let agent = AgentInstance::new("prompt-agent-123".to_string(), "prompt".to_string());
    /// assert_eq!(agent.status, AgentStatus::Active);
    /// assert_eq!(agent.agent_type, "prompt");
    /// ```
    pub fn new(agent_id: String, agent_type: String) -> Self {
        let now = Utc::now();
        AgentInstance {
            agent_id,
            agent_type,
            status: AgentStatus::Active,
            started_at: now,
            last_heartbeat_at: now,
            hostname: Some(get_hostname()),
            pid: Some(get_current_process_id()),
            tasks_assigned: HashSet::new(),
        }
    }

    /// Updates the heartbeat timestamp to the current time.
    ///
    /// This is called when an agent sends a heartbeat to indicate it's still alive.
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat_at = Utc::now();
        // If agent was stale and sent a heartbeat, mark as active again
        if self.status == AgentStatus::Stale {
            self.status = AgentStatus::Active;
        }
    }

    /// Marks the agent as shut down.
    pub fn mark_shutdown(&mut self) {
        self.status = AgentStatus::Shutdown;
    }

    /// Marks the agent as stale.
    pub fn mark_stale(&mut self) {
        self.status = AgentStatus::Stale;
    }

    /// Assigns a task to this agent.
    ///
    /// # Arguments
    ///
    /// * `task_id` - ID of the task to assign
    ///
    /// # Returns
    ///
    /// `true` if the task was newly assigned, `false` if it was already assigned
    pub fn assign_task(&mut self, task_id: &str) -> bool {
        self.tasks_assigned.insert(task_id.to_string())
    }

    /// Unassigns a task from this agent.
    ///
    /// # Arguments
    ///
    /// * `task_id` - ID of the task to unassign
    ///
    /// # Returns
    ///
    /// `true` if the task was assigned and removed, `false` otherwise
    pub fn unassign_task(&mut self, task_id: &str) -> bool {
        self.tasks_assigned.remove(task_id)
    }

    /// Checks if the heartbeat is stale based on the threshold.
    ///
    /// Returns `true` if the heartbeat is older than the specified threshold.
    pub fn is_heartbeat_stale(&self, threshold: Duration) -> bool {
        let age = Utc::now().signed_duration_since(self.last_heartbeat_at);
        age > ChronoDuration::from_std(threshold).unwrap_or_else(|_| ChronoDuration::zero())
    }

    /// Calculates the uptime of the agent.
    ///
    /// Returns the duration since the agent was started.
    pub fn uptime(&self) -> Duration {
        Utc::now()
            .signed_duration_since(self.started_at)
            .to_std()
            .unwrap_or_else(|_| Duration::from_secs(0))
    }

    /// Calculates the age of the last heartbeat.
    ///
    /// Returns the duration since the last heartbeat.
    pub fn last_heartbeat_age(&self) -> Duration {
        Utc::now()
            .signed_duration_since(self.last_heartbeat_at)
            .to_std()
            .unwrap_or_else(|_| Duration::from_secs(0))
    }
}

/// Statistics for an agent instance.
///
/// Provides a summary view of an agent's current state and performance metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentStats {
    /// Unique identifier for the agent
    pub agent_id: String,
    /// Type of the agent
    pub agent_type: String,
    /// Time since the agent was started
    pub uptime: Duration,
    /// Number of tasks currently assigned to this agent
    pub tasks_assigned: usize,
    /// Time since the last heartbeat
    pub last_heartbeat_age: Duration,
    /// Current status of the agent
    pub status: AgentStatus,
}

/// Registry for tracking agent instances with heartbeats and status.
///
/// The `AgentRegistry` provides a thread-safe mechanism for managing agent instances
/// across multiple parallel agents. It uses file-based locking to coordinate access
/// and persists state to disk for recovery after failures.
///
/// # Fields
///
/// * `agents` - HashMap mapping agent IDs to agent instances
/// * `registry_path` - Path to the registry file
/// * `lock_manager` - Lock manager for coordinating access
///
/// # Thread Safety
///
/// All operations that modify the registry acquire a file lock before making
/// changes. The lock is automatically released when the operation completes.
#[derive(Debug)]
pub struct AgentRegistry {
    /// Internal agent storage
    agents: HashMap<String, AgentInstance>,
    /// Path to the registry file on disk
    registry_path: PathBuf,
    /// Lock manager for coordinating access
    lock_manager: LockManager,
}

impl AgentRegistry {
    /// Creates a new agent registry or loads it from disk.
    ///
    /// If the registry file exists, it will be loaded. If it doesn't exist,
    /// a new empty registry will be created. The `.state/agents/` directory
    /// will be created if it doesn't exist.
    ///
    /// # Returns
    ///
    /// A new `AgentRegistry` instance
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
    /// use automation_state::agent_registry::AgentRegistry;
    ///
    /// let registry = AgentRegistry::new().unwrap();
    /// ```
    pub fn new() -> Result<Self> {
        let registry_dir = PathBuf::from(REGISTRY_BASE_PATH);
        let registry_path = registry_dir.join(REGISTRY_FILE_NAME);

        // Create directory if it doesn't exist
        fs::create_dir_all(&registry_dir).map_err(|e| AutomationError::StateError {
            task: "AgentRegistry".to_string(),
            message: format!("Failed to create registry directory: {}", e),
        })?;

        // Create lock manager
        let lock_file_path = registry_dir.join("registry.lock");
        let lock_manager = LockManager::new(lock_file_path, DEFAULT_LOCK_TIMEOUT);

        // Try to load existing registry or create new one
        let agents = if registry_path.exists() {
            let content = fs::read_to_string(&registry_path).map_err(|e| {
                AutomationError::StateError {
                    task: "AgentRegistry".to_string(),
                    message: format!("Failed to read registry file: {}", e),
                }
            })?;
            serde_json::from_str(&content).map_err(|e| {
                AutomationError::StateError {
                    task: "AgentRegistry".to_string(),
                    message: format!("Failed to parse registry file: {}", e),
                }
            })?
        } else {
            HashMap::new()
        };

        Ok(AgentRegistry {
            agents,
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
    /// use automation_state::agent_registry::AgentRegistry;
    ///
    /// let mut registry = AgentRegistry::new().unwrap();
    /// registry.register_agent("prompt-agent-123".to_string(), "prompt".to_string()).unwrap();
    /// registry.save().unwrap();
    /// ```
    pub fn save(&self) -> Result<()> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        // Serialize to JSON
        let content = serde_json::to_string_pretty(&self.agents).map_err(|e| {
            AutomationError::StateError {
                task: "AgentRegistry".to_string(),
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
    /// The loaded `AgentRegistry` instance
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
    /// use automation_state::agent_registry::AgentRegistry;
    ///
    /// let mut registry = AgentRegistry::new().unwrap();
    /// // After some time, reload from disk
    /// registry = AgentRegistry::load().unwrap();
    /// ```
    pub fn load() -> Result<Self> {
        let registry_dir = PathBuf::from(REGISTRY_BASE_PATH);
        let registry_path = registry_dir.join(REGISTRY_FILE_NAME);
        let lock_file_path = registry_dir.join("registry.lock");
        let lock_manager = LockManager::new(lock_file_path, DEFAULT_LOCK_TIMEOUT);

        // Acquire lock
        let _lock_handle = lock_manager.acquire_lock()?;

        // Load agents
        let agents = if registry_path.exists() {
            let content = fs::read_to_string(&registry_path).map_err(|e| {
                AutomationError::StateError {
                    task: "AgentRegistry".to_string(),
                    message: format!("Failed to read registry file: {}", e),
                }
            })?;
            serde_json::from_str(&content).map_err(|e| {
                AutomationError::StateError {
                    task: "AgentRegistry".to_string(),
                    message: format!("Failed to parse registry file: {}", e),
                }
            })?
        } else {
            HashMap::new()
        };

        Ok(AgentRegistry {
            agents,
            registry_path,
            lock_manager,
        })
    }

    /// Registers a new agent instance in the registry.
    ///
    /// If an agent with the same ID already exists, an error will be returned.
    /// The agent will be added with `Active` status.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - Unique identifier for the agent instance
    /// * `agent_type` - Type of the agent (e.g., "prompt", "architect")
    ///
    /// # Returns
    ///
    /// `Ok(())` if the agent was registered successfully
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - An agent with the same ID already exists
    /// - The lock cannot be acquired
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::agent_registry::AgentRegistry;
    ///
    /// let mut registry = AgentRegistry::new().unwrap();
    /// registry.register_agent("prompt-agent-123".to_string(), "prompt".to_string()).unwrap();
    /// ```
    pub fn register_agent(&mut self, agent_id: String, agent_type: String) -> Result<()> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        // Check if agent already exists
        if self.agents.contains_key(&agent_id) {
            return Err(AutomationError::StateError {
                task: agent_id.clone(),
                message: "Agent already exists in registry".to_string(),
            });
        }

        // Create new agent instance
        let agent = AgentInstance::new(agent_id.clone(), agent_type);
        self.agents.insert(agent_id, agent);

        // Persist changes
        self.save_with_lock()?;

        Ok(())
    }

    /// Updates the heartbeat timestamp for an agent.
    ///
    /// Returns `true` if the heartbeat was updated successfully,
    /// `false` if the agent was not found.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - ID of the agent to update
    ///
    /// # Returns
    ///
    /// * `true` - Heartbeat was updated
    /// * `false` - Agent not found
    ///
    /// # Errors
    ///
    /// Returns an error if the lock cannot be acquired.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::agent_registry::AgentRegistry;
    ///
    /// let mut registry = AgentRegistry::new().unwrap();
    /// if registry.update_heartbeat("prompt-agent-123").unwrap() {
    ///     println!("Heartbeat updated");
    /// } else {
    ///     println!("Agent not found");
    /// }
    /// ```
    pub fn update_heartbeat(&mut self, agent_id: &str) -> Result<bool> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        // Get agent
        match self.agents.get_mut(agent_id) {
            Some(agent) => {
                agent.update_heartbeat();
                self.save_with_lock()?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Assigns a task to an agent.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - ID of the agent to assign the task to
    /// * `task_id` - ID of the task to assign
    ///
    /// # Returns
    ///
    /// `Ok(())` if the task was assigned successfully
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The agent does not exist
    /// - The lock cannot be acquired
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::agent_registry::AgentRegistry;
    ///
    /// let mut registry = AgentRegistry::new().unwrap();
    /// registry.register_agent("prompt-agent-123".to_string(), "prompt".to_string()).unwrap();
    /// registry.assign_task("prompt-agent-123", "task-1").unwrap();
    /// ```
    pub fn assign_task(&mut self, agent_id: &str, task_id: &str) -> Result<()> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        // Get agent
        let agent = match self.agents.get_mut(agent_id) {
            Some(a) => a,
            None => {
                return Err(AutomationError::StateError {
                    task: agent_id.to_string(),
                    message: "Agent not found".to_string(),
                })
            }
        };

        // Assign task
        agent.assign_task(task_id);
        self.save_with_lock()?;

        Ok(())
    }

    /// Unassigns a task from an agent.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - ID of the agent to unassign the task from
    /// * `task_id` - ID of the task to unassign
    ///
    /// # Returns
    ///
    /// `Ok(())` if the task was unassigned successfully
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The agent does not exist
    /// - The lock cannot be acquired
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::agent_registry::AgentRegistry;
    ///
    /// let mut registry = AgentRegistry::new().unwrap();
    /// registry.unassign_task("prompt-agent-123", "task-1").unwrap();
    /// ```
    pub fn unassign_task(&mut self, agent_id: &str, task_id: &str) -> Result<()> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        // Get agent
        let agent = match self.agents.get_mut(agent_id) {
            Some(a) => a,
            None => {
                return Err(AutomationError::StateError {
                    task: agent_id.to_string(),
                    message: "Agent not found".to_string(),
                })
            }
        };

        // Unassign task
        agent.unassign_task(task_id);
        self.save_with_lock()?;

        Ok(())
    }

    /// Marks an agent as shut down.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - ID of the agent to mark as shut down
    ///
    /// # Returns
    ///
    /// `Ok(())` if the agent was marked as shut down successfully
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The agent does not exist
    /// - The lock cannot be acquired
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::agent_registry::AgentRegistry;
    ///
    /// let mut registry = AgentRegistry::new().unwrap();
    /// registry.mark_shutdown("prompt-agent-123").unwrap();
    /// ```
    pub fn mark_shutdown(&mut self, agent_id: &str) -> Result<()> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        // Get agent
        let agent = match self.agents.get_mut(agent_id) {
            Some(a) => a,
            None => {
                return Err(AutomationError::StateError {
                    task: agent_id.to_string(),
                    message: "Agent not found".to_string(),
                })
            }
        };

        // Mark as shutdown
        agent.mark_shutdown();
        self.save_with_lock()?;

        Ok(())
    }

    /// Gets a specific agent by ID.
    ///
    /// Returns `None` if the agent does not exist.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - ID of the agent to get
    ///
    /// # Returns
    ///
    /// * `Some(&AgentInstance)` - Reference to the agent
    /// * `None` - Agent not found
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::agent_registry::AgentRegistry;
    ///
    /// let registry = AgentRegistry::new().unwrap();
    /// if let Some(agent) = registry.get_agent("prompt-agent-123") {
    ///     println!("Agent type: {}", agent.agent_type);
    /// }
    /// ```
    pub fn get_agent(&self, agent_id: &str) -> Option<&AgentInstance> {
        self.agents.get(agent_id)
    }

    /// Gets all agents of a specific type.
    ///
    /// # Arguments
    ///
    /// * `agent_type` - Type of agents to retrieve (e.g., "prompt", "architect")
    ///
    /// # Returns
    ///
    /// A vector of references to agents of the specified type
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::agent_registry::AgentRegistry;
    ///
    /// let registry = AgentRegistry::new().unwrap();
    /// let prompt_agents = registry.get_agents_by_type("prompt");
    /// println!("Found {} prompt agents", prompt_agents.len());
    /// ```
    pub fn get_agents_by_type(&self, agent_type: &str) -> Vec<&AgentInstance> {
        self.agents
            .values()
            .filter(|agent| agent.agent_type == agent_type)
            .collect()
    }

    /// Gets all active agents.
    ///
    /// Returns a vector of references to agents with `Active` status.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::agent_registry::AgentRegistry;
    ///
    /// let registry = AgentRegistry::new().unwrap();
    /// let active_agents = registry.get_active_agents();
    /// println!("Found {} active agents", active_agents.len());
    /// ```
    pub fn get_active_agents(&self) -> Vec<&AgentInstance> {
        self.agents
            .values()
            .filter(|agent| agent.status.is_active())
            .collect()
    }

    /// Marks agents as stale if no recent heartbeat.
    ///
    /// Agents that haven't sent a heartbeat within the specified threshold
    /// will be marked as `Stale`. This does not remove the agents from the registry.
    ///
    /// # Arguments
    ///
    /// * `stale_threshold` - Duration after which an agent is considered stale
    ///
    /// # Returns
    ///
    /// A vector of agent IDs that were marked as stale
    ///
    /// # Errors
    ///
    /// Returns an error if the lock cannot be acquired.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::agent_registry::AgentRegistry;
    /// use std::time::Duration;
    ///
    /// let mut registry = AgentRegistry::new().unwrap();
    /// let stale_agents = registry.mark_stale_agents(Duration::from_secs(60)).unwrap();
    /// println!("Marked {} agents as stale", stale_agents.len());
    /// ```
    pub fn mark_stale_agents(&mut self, stale_threshold: Duration) -> Result<Vec<String>> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        let mut marked_agents = Vec::new();

        // Mark agents with stale heartbeats
        for (agent_id, agent) in self.agents.iter_mut() {
            if agent.status.is_active() && agent.is_heartbeat_stale(stale_threshold) {
                agent.mark_stale();
                marked_agents.push(agent_id.clone());
            }
        }

        // Persist changes if any agents were marked
        if !marked_agents.is_empty() {
            self.save_with_lock()?;
        }

        Ok(marked_agents)
    }

    /// Removes stale agents that haven't been updated.
    ///
    /// Agents that have been in `Stale` status for longer than the specified
    /// duration will be removed from the registry.
    ///
    /// # Arguments
    ///
    /// * `older_than` - Duration after which stale agents should be removed
    ///
    /// # Returns
    ///
    /// A vector of agent IDs that were removed
    ///
    /// # Errors
    ///
    /// Returns an error if the lock cannot be acquired.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::agent_registry::AgentRegistry;
    /// use std::time::Duration;
    ///
    /// let mut registry = AgentRegistry::new().unwrap();
    /// let removed = registry.cleanup_stale_agents(Duration::from_secs(300)).unwrap();
    /// println!("Removed {} stale agents", removed.len());
    /// ```
    pub fn cleanup_stale_agents(&mut self, older_than: Duration) -> Result<Vec<String>> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        let threshold_time = Utc::now() - ChronoDuration::from_std(older_than).unwrap();
        let mut removed_agents = Vec::new();

        // Remove stale agents that haven't had a heartbeat recently
        self.agents.retain(|agent_id, agent| {
            let keep = if agent.status.is_stale() {
                // Check when it became stale (last heartbeat before threshold)
                agent.last_heartbeat_at > threshold_time
            } else {
                true
            };

            if !keep {
                removed_agents.push(agent_id.clone());
            }
            keep
        });

        // Persist changes if any agents were removed
        if !removed_agents.is_empty() {
            self.save_with_lock()?;
        }

        Ok(removed_agents)
    }

    /// Removes shutdown agents.
    ///
    /// Agents that have been in `Shutdown` status for longer than the specified
    /// duration will be removed from the registry.
    ///
    /// # Arguments
    ///
    /// * `older_than` - Duration after which shutdown agents should be removed
    ///
    /// # Returns
    ///
    /// A vector of agent IDs that were removed
    ///
    /// # Errors
    ///
    /// Returns an error if the lock cannot be acquired.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::agent_registry::AgentRegistry;
    /// use std::time::Duration;
    ///
    /// let mut registry = AgentRegistry::new().unwrap();
    /// let removed = registry.cleanup_shutdown_agents(Duration::from_secs(60)).unwrap();
    /// println!("Removed {} shutdown agents", removed.len());
    /// ```
    pub fn cleanup_shutdown_agents(&mut self, older_than: Duration) -> Result<Vec<String>> {
        // Acquire lock
        let _lock_handle = self.lock_manager.acquire_lock()?;

        let threshold_time = Utc::now() - ChronoDuration::from_std(older_than).unwrap();
        let mut removed_agents = Vec::new();

        // Remove shutdown agents that have been shut down for a while
        self.agents.retain(|agent_id, agent| {
            let keep = if agent.status.is_shutdown() {
                // Keep if it was recently shut down
                agent.last_heartbeat_at > threshold_time
            } else {
                true
            };

            if !keep {
                removed_agents.push(agent_id.clone());
            }
            keep
        });

        // Persist changes if any agents were removed
        if !removed_agents.is_empty() {
            self.save_with_lock()?;
        }

        Ok(removed_agents)
    }

    /// Gets statistics for an agent.
    ///
    /// Returns `None` if the agent does not exist.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - ID of the agent to get statistics for
    ///
    /// # Returns
    ///
    /// * `Some(AgentStats)` - Statistics for the agent
    /// * `None` - Agent not found
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_state::agent_registry::AgentRegistry;
    ///
    /// let registry = AgentRegistry::new().unwrap();
    /// if let Some(stats) = registry.get_agent_stats("prompt-agent-123") {
    ///     println!("Agent uptime: {:?}", stats.uptime);
    ///     println!("Tasks assigned: {}", stats.tasks_assigned);
    /// }
    /// ```
    pub fn get_agent_stats(&self, agent_id: &str) -> Option<AgentStats> {
        let agent = self.agents.get(agent_id)?;
        Some(AgentStats {
            agent_id: agent.agent_id.clone(),
            agent_type: agent.agent_type.clone(),
            uptime: agent.uptime(),
            tasks_assigned: agent.tasks_assigned.len(),
            last_heartbeat_age: agent.last_heartbeat_age(),
            status: agent.status,
        })
    }

    /// Saves the registry without acquiring a lock.
    ///
    /// This is an internal helper method used when a lock is already held.
    /// It should only be called after acquiring a lock via `lock_manager.acquire_lock()`.
    fn save_with_lock(&self) -> Result<()> {
        // Serialize to JSON
        let content = serde_json::to_string_pretty(&self.agents).map_err(|e| {
            AutomationError::StateError {
                task: "AgentRegistry".to_string(),
                message: format!("Failed to serialize registry: {}", e),
            }
        })?;

        // Write atomically
        atomic_write(&self.registry_path, content.as_bytes())?;

        Ok(())
    }

    /// Returns the number of agents in the registry.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Returns a list of all agent IDs in the registry.
    pub fn list_agent_ids(&self) -> Vec<String> {
        self.agents.keys().cloned().collect()
    }

    /// Checks if an agent exists in the registry.
    pub fn agent_exists(&self, agent_id: &str) -> bool {
        self.agents.contains_key(agent_id)
    }
}

/// Generates a unique agent ID for the given agent type.
///
/// The ID is formatted as `{agent_type}-{uuid}` where the UUID is v4 random.
///
/// # Arguments
///
/// * `agent_type` - Type of the agent (e.g., "prompt", "architect")
///
/// # Returns
///
/// A unique agent ID string
///
/// # Example
///
/// ```
/// use automation_state::agent_registry::generate_agent_id;
///
/// let agent_id = generate_agent_id("prompt");
/// assert!(agent_id.starts_with("prompt-"));
/// ```
pub fn generate_agent_id(agent_type: &str) -> String {
    let uuid = Uuid::new_v4();
    format!("{}-{}", agent_type, uuid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    /// Helper function to create a temporary test directory
    fn setup_temp_registry() -> (tempfile::TempDir, AgentRegistry) {
        // We'll create a test registry without modifying the base path
        // by using the test registry methods
        let temp_dir = tempfile::tempdir().unwrap();
        let registry_dir = temp_dir.path().join(".state").join("agents");
        let registry_path = registry_dir.join(REGISTRY_FILE_NAME);

        // Create directory
        fs::create_dir_all(&registry_dir).unwrap();

        // Create lock manager
        let lock_file_path = registry_dir.join("registry.lock");
        let lock_manager = LockManager::new(lock_file_path, DEFAULT_LOCK_TIMEOUT);

        let registry = AgentRegistry {
            agents: HashMap::new(),
            registry_path,
            lock_manager,
        };

        (temp_dir, registry)
    }

    #[test]
    fn test_agent_status_default() {
        let status = AgentStatus::default();
        assert_eq!(status, AgentStatus::Active);
    }

    #[test]
    fn test_agent_status_is_active() {
        assert!(AgentStatus::Active.is_active());
        assert!(!AgentStatus::Stale.is_active());
        assert!(!AgentStatus::Shutdown.is_active());
    }

    #[test]
    fn test_agent_status_is_stale() {
        assert!(!AgentStatus::Active.is_stale());
        assert!(AgentStatus::Stale.is_stale());
        assert!(!AgentStatus::Shutdown.is_stale());
    }

    #[test]
    fn test_agent_status_is_shutdown() {
        assert!(!AgentStatus::Active.is_shutdown());
        assert!(!AgentStatus::Stale.is_shutdown());
        assert!(AgentStatus::Shutdown.is_shutdown());
    }

    #[test]
    fn test_agent_instance_new() {
        let agent = AgentInstance::new("test-agent-1".to_string(), "test".to_string());

        assert_eq!(agent.agent_id, "test-agent-1");
        assert_eq!(agent.agent_type, "test");
        assert_eq!(agent.status, AgentStatus::Active);
        assert!(agent.tasks_assigned.is_empty());
        assert!(agent.hostname.is_some());
        assert!(agent.pid.is_some());
    }

    #[test]
    fn test_agent_instance_update_heartbeat() {
        let mut agent = AgentInstance::new("test-agent-1".to_string(), "test".to_string());
        let original_heartbeat = agent.last_heartbeat_at;

        thread::sleep(Duration::from_millis(10));
        agent.update_heartbeat();

        assert!(agent.last_heartbeat_at > original_heartbeat);
        assert_eq!(agent.status, AgentStatus::Active);
    }

    #[test]
    fn test_agent_instance_update_heartbeat_reactivates_stale() {
        let mut agent = AgentInstance::new("test-agent-1".to_string(), "test".to_string());
        agent.mark_stale();

        agent.update_heartbeat();
        assert_eq!(agent.status, AgentStatus::Active);
    }

    #[test]
    fn test_agent_instance_mark_shutdown() {
        let mut agent = AgentInstance::new("test-agent-1".to_string(), "test".to_string());
        agent.mark_shutdown();

        assert_eq!(agent.status, AgentStatus::Shutdown);
    }

    #[test]
    fn test_agent_instance_mark_stale() {
        let mut agent = AgentInstance::new("test-agent-1".to_string(), "test".to_string());
        agent.mark_stale();

        assert_eq!(agent.status, AgentStatus::Stale);
    }

    #[test]
    fn test_agent_instance_assign_task() {
        let mut agent = AgentInstance::new("test-agent-1".to_string(), "test".to_string());

        assert!(agent.assign_task("task-1"));
        assert!(agent.assign_task("task-2"));
        assert!(!agent.assign_task("task-1")); // Duplicate
        assert_eq!(agent.tasks_assigned.len(), 2);
    }

    #[test]
    fn test_agent_instance_unassign_task() {
        let mut agent = AgentInstance::new("test-agent-1".to_string(), "test".to_string());

        agent.assign_task("task-1");
        agent.assign_task("task-2");

        assert!(agent.unassign_task("task-1"));
        assert!(!agent.unassign_task("task-3")); // Not assigned
        assert_eq!(agent.tasks_assigned.len(), 1);
    }

    #[test]
    fn test_agent_instance_is_heartbeat_stale() {
        let agent = AgentInstance::new("test-agent-1".to_string(), "test".to_string());

        // Fresh heartbeat should not be stale
        assert!(!agent.is_heartbeat_stale(Duration::from_secs(60)));

        // 0 second threshold should always be stale
        assert!(agent.is_heartbeat_stale(Duration::from_secs(0)));
    }

    #[test]
    fn test_agent_instance_uptime() {
        let agent = AgentInstance::new("test-agent-1".to_string(), "test".to_string());

        thread::sleep(Duration::from_millis(10));
        let uptime = agent.uptime();

        assert!(uptime >= Duration::from_millis(10));
    }

    #[test]
    fn test_agent_instance_last_heartbeat_age() {
        let agent = AgentInstance::new("test-agent-1".to_string(), "test".to_string());

        thread::sleep(Duration::from_millis(10));
        let age = agent.last_heartbeat_age();

        assert!(age >= Duration::from_millis(10));
    }

    #[test]
    fn test_agent_registry_register_agent() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        registry
            .register_agent("agent-1".to_string(), "prompt".to_string())
            .unwrap();

        assert!(registry.agent_exists("agent-1"));
        assert_eq!(registry.agent_count(), 1);
    }

    #[test]
    fn test_agent_registry_register_duplicate_agent() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        registry
            .register_agent("agent-1".to_string(), "prompt".to_string())
            .unwrap();

        let result = registry.register_agent("agent-1".to_string(), "code".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_registry_update_heartbeat() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        registry
            .register_agent("agent-1".to_string(), "prompt".to_string())
            .unwrap();

        let agent = registry.get_agent("agent-1").unwrap();
        let original_heartbeat = agent.last_heartbeat_at;

        thread::sleep(Duration::from_millis(10));
        let updated = registry.update_heartbeat("agent-1").unwrap();

        assert!(updated);
        let agent = registry.get_agent("agent-1").unwrap();
        assert!(agent.last_heartbeat_at > original_heartbeat);
    }

    #[test]
    fn test_agent_registry_update_heartbeat_not_found() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        let updated = registry.update_heartbeat("nonexistent").unwrap();
        assert!(!updated);
    }

    #[test]
    fn test_agent_registry_assign_task() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        registry
            .register_agent("agent-1".to_string(), "prompt".to_string())
            .unwrap();

        registry.assign_task("agent-1", "task-1").unwrap();

        let agent = registry.get_agent("agent-1").unwrap();
        assert!(agent.tasks_assigned.contains("task-1"));
    }

    #[test]
    fn test_agent_registry_assign_task_not_found() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        let result = registry.assign_task("nonexistent", "task-1");
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_registry_unassign_task() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        registry
            .register_agent("agent-1".to_string(), "prompt".to_string())
            .unwrap();

        registry.assign_task("agent-1", "task-1").unwrap();
        registry.unassign_task("agent-1", "task-1").unwrap();

        let agent = registry.get_agent("agent-1").unwrap();
        assert!(!agent.tasks_assigned.contains("task-1"));
    }

    #[test]
    fn test_agent_registry_mark_shutdown() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        registry
            .register_agent("agent-1".to_string(), "prompt".to_string())
            .unwrap();

        registry.mark_shutdown("agent-1").unwrap();

        let agent = registry.get_agent("agent-1").unwrap();
        assert_eq!(agent.status, AgentStatus::Shutdown);
    }

    #[test]
    fn test_agent_registry_get_agent() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        registry
            .register_agent("agent-1".to_string(), "prompt".to_string())
            .unwrap();

        let agent = registry.get_agent("agent-1");
        assert!(agent.is_some());
        assert_eq!(agent.unwrap().agent_type, "prompt");
    }

    #[test]
    fn test_agent_registry_get_agent_not_found() {
        let (_temp_dir, registry) = setup_temp_registry();

        let agent = registry.get_agent("nonexistent");
        assert!(agent.is_none());
    }

    #[test]
    fn test_agent_registry_get_agents_by_type() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        registry
            .register_agent("prompt-1".to_string(), "prompt".to_string())
            .unwrap();
        registry
            .register_agent("prompt-2".to_string(), "prompt".to_string())
            .unwrap();
        registry
            .register_agent("code-1".to_string(), "code".to_string())
            .unwrap();

        let prompt_agents = registry.get_agents_by_type("prompt");
        assert_eq!(prompt_agents.len(), 2);

        let code_agents = registry.get_agents_by_type("code");
        assert_eq!(code_agents.len(), 1);
    }

    #[test]
    fn test_agent_registry_get_active_agents() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        registry
            .register_agent("active-1".to_string(), "prompt".to_string())
            .unwrap();
        registry
            .register_agent("active-2".to_string(), "prompt".to_string())
            .unwrap();

        registry.mark_shutdown("active-1").unwrap();

        let active_agents = registry.get_active_agents();
        assert_eq!(active_agents.len(), 1);
    }

    #[test]
    fn test_agent_registry_mark_stale_agents() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        registry
            .register_agent("fresh-1".to_string(), "prompt".to_string())
            .unwrap();

        // Create an agent with an old heartbeat
        let mut stale_agent = AgentInstance::new("stale-1".to_string(), "prompt".to_string());
        stale_agent.last_heartbeat_at = Utc::now() - ChronoDuration::seconds(120);
        registry.agents.insert("stale-1".to_string(), stale_agent);

        let stale_agents = registry
            .mark_stale_agents(Duration::from_secs(60))
            .unwrap();

        assert_eq!(stale_agents.len(), 1);
        assert!(stale_agents.contains(&"stale-1".to_string()));

        let agent = registry.get_agent("stale-1").unwrap();
        assert_eq!(agent.status, AgentStatus::Stale);
    }

    #[test]
    fn test_agent_registry_cleanup_stale_agents() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        // Create a stale agent that's very old
        let mut old_stale = AgentInstance::new("old-stale".to_string(), "prompt".to_string());
        old_stale.mark_stale();
        old_stale.last_heartbeat_at = Utc::now() - ChronoDuration::seconds(600);
        registry
            .agents
            .insert("old-stale".to_string(), old_stale);

        // Create a stale agent that's recent
        let mut recent_stale = AgentInstance::new("recent-stale".to_string(), "prompt".to_string());
        recent_stale.mark_stale();
        recent_stale.last_heartbeat_at = Utc::now() - ChronoDuration::seconds(10);
        registry
            .agents
            .insert("recent-stale".to_string(), recent_stale);

        let removed = registry
            .cleanup_stale_agents(Duration::from_secs(300))
            .unwrap();

        assert_eq!(removed.len(), 1);
        assert!(removed.contains(&"old-stale".to_string()));
        assert!(registry.agent_exists("recent-stale"));
        assert!(!registry.agent_exists("old-stale"));
    }

    #[test]
    fn test_agent_registry_cleanup_shutdown_agents() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        // Create a shutdown agent that's old
        let mut old_shutdown = AgentInstance::new("old-shutdown".to_string(), "prompt".to_string());
        old_shutdown.mark_shutdown();
        old_shutdown.last_heartbeat_at = Utc::now() - ChronoDuration::seconds(600);
        registry
            .agents
            .insert("old-shutdown".to_string(), old_shutdown);

        // Create a shutdown agent that's recent
        let mut recent_shutdown = AgentInstance::new("recent-shutdown".to_string(), "prompt".to_string());
        recent_shutdown.mark_shutdown();
        recent_shutdown.last_heartbeat_at = Utc::now() - ChronoDuration::seconds(10);
        registry
            .agents
            .insert("recent-shutdown".to_string(), recent_shutdown);

        let removed = registry
            .cleanup_shutdown_agents(Duration::from_secs(300))
            .unwrap();

        assert_eq!(removed.len(), 1);
        assert!(removed.contains(&"old-shutdown".to_string()));
        assert!(registry.agent_exists("recent-shutdown"));
        assert!(!registry.agent_exists("old-shutdown"));
    }

    #[test]
    fn test_agent_registry_get_agent_stats() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        registry
            .register_agent("agent-1".to_string(), "prompt".to_string())
            .unwrap();
        registry.assign_task("agent-1", "task-1").unwrap();
        registry.assign_task("agent-1", "task-2").unwrap();

        let stats = registry.get_agent_stats("agent-1").unwrap();

        assert_eq!(stats.agent_id, "agent-1");
        assert_eq!(stats.agent_type, "prompt");
        assert_eq!(stats.tasks_assigned, 2);
        assert_eq!(stats.status, AgentStatus::Active);
    }

    #[test]
    fn test_agent_registry_get_agent_stats_not_found() {
        let (_temp_dir, registry) = setup_temp_registry();

        let stats = registry.get_agent_stats("nonexistent");
        assert!(stats.is_none());
    }

    #[test]
    fn test_agent_registry_agent_count() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        assert_eq!(registry.agent_count(), 0);

        registry
            .register_agent("agent-1".to_string(), "prompt".to_string())
            .unwrap();
        registry
            .register_agent("agent-2".to_string(), "code".to_string())
            .unwrap();

        assert_eq!(registry.agent_count(), 2);
    }

    #[test]
    fn test_agent_registry_list_agent_ids() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        registry
            .register_agent("agent-1".to_string(), "prompt".to_string())
            .unwrap();
        registry
            .register_agent("agent-2".to_string(), "code".to_string())
            .unwrap();

        let ids = registry.list_agent_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"agent-1".to_string()));
        assert!(ids.contains(&"agent-2".to_string()));
    }

    #[test]
    fn test_agent_registry_save_and_load() {
        let (_temp_dir, mut registry) = setup_temp_registry();

        registry
            .register_agent("agent-1".to_string(), "prompt".to_string())
            .unwrap();
        registry.assign_task("agent-1", "task-1").unwrap();

        // Save the registry
        registry.save().unwrap();

        // Load into a new registry
        let mut loaded_registry = AgentRegistry {
            agents: HashMap::new(),
            registry_path: registry.registry_path.clone(),
            lock_manager: registry.lock_manager.clone(),
        };

        // Manually load from file (since we're using temp registry)
        let content = fs::read_to_string(&registry.registry_path).unwrap();
        loaded_registry.agents = serde_json::from_str(&content).unwrap();

        assert!(loaded_registry.agent_exists("agent-1"));
        let agent = loaded_registry.get_agent("agent-1").unwrap();
        assert!(agent.tasks_assigned.contains("task-1"));
    }

    #[test]
    fn test_generate_agent_id() {
        let agent_id = generate_agent_id("prompt");
        assert!(agent_id.starts_with("prompt-"));
        assert_ne!(agent_id, generate_agent_id("prompt")); // Should be unique

        let code_id = generate_agent_id("code");
        assert!(code_id.starts_with("code-"));
        assert_ne!(agent_id, code_id);
    }

    #[test]
    fn test_agent_stats_serialization() {
        let stats = AgentStats {
            agent_id: "agent-1".to_string(),
            agent_type: "prompt".to_string(),
            uptime: Duration::from_secs(100),
            tasks_assigned: 5,
            last_heartbeat_age: Duration::from_secs(10),
            status: AgentStatus::Active,
        };

        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: AgentStats = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.agent_id, stats.agent_id);
        assert_eq!(deserialized.agent_type, stats.agent_type);
        assert_eq!(deserialized.tasks_assigned, stats.tasks_assigned);
        assert_eq!(deserialized.status, stats.status);
    }

    #[test]
    fn test_agent_instance_serialization() {
        let mut agent = AgentInstance::new("agent-1".to_string(), "prompt".to_string());
        agent.assign_task("task-1");
        agent.assign_task("task-2");

        let json = serde_json::to_string(&agent).unwrap();
        let deserialized: AgentInstance = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.agent_id, agent.agent_id);
        assert_eq!(deserialized.agent_type, agent.agent_type);
        assert_eq!(deserialized.status, agent.status);
        assert_eq!(deserialized.tasks_assigned.len(), 2);
    }

    #[test]
    fn test_agent_status_serialization() {
        let status = AgentStatus::Active;

        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"active\"");

        let deserialized: AgentStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, AgentStatus::Active);
    }
}

//! # Shared Agent Runner
//!
//! This module provides a shared implementation for all agent types
//! (architect, janitor, prompt) to execute tasks using
//! automation-rust system components.
//!
//! # Agent Handler Pattern
//!
//! Each agent follows the same execution pattern:
//! 1. Acquire lock with retry
//! 2. Update state to running
//! 3. Read workspace files in parallel
//! 4. Execute prompt template with context substitution
//! 5. Execute Kilo Code CLI with prompt
//! 6. Handle early termination / success / failure
//! 7. Write output file
//! 8. Update state with results
//! 9. Release lock

use automation_common::{Result, AutomationError};
use automation_executor::cli::{CLIExecutor, ExecutorConfig};
use automation_scheduler::create_scheduled_task;
use automation_state::{
    agent_registry::{AgentRegistry, generate_agent_id},
    lock::LockManager,
    persistence::StateManager,
    task_registry::TaskRegistry,
};
use automation_workspace::{manager::WorkspaceManager, template::{TemplateEngine, TemplateContext}};
use automation_common::workspace_config::{Config as WorkspaceConfig, ParallelConfig};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tracing::{error, info, instrument, warn};

/// Type of agent.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentType {
    /// Architect agent - long-running planning
    Architect,
    /// Janitor agent - cleanup and maintenance
    Janitor,
    /// Prompt agent - quick task execution
    Prompt,
    /// CodeReview agent - codebase analysis and review
    CodeReview,
}

impl fmt::Display for AgentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentType::Architect => write!(f, "architect"),
            AgentType::Janitor => write!(f, "janitor"),
            AgentType::Prompt => write!(f, "prompt"),
            AgentType::CodeReview => write!(f, "code-review"),
        }
    }
}

/// Represents the outcome of a handler execution for output formatting.
///
/// This enum captures the different scenarios that can occur during
/// agent execution, allowing for consistent output formatting across
/// all cases.
#[derive(Debug)]
pub enum HandlerOutput {
    /// Successful completion with stdout and stderr output
    Success {
        /// Standard output from the process
        stdout: String,
        /// Standard error from the process
        stderr: String,
    },
    /// Execution error occurred
    Error {
        /// The error message that occurred
        error: String,
    },
    /// Early termination of the process
    EarlyTermination {
        /// Reason for early termination
        reason: String,
        /// Standard output before termination
        stdout: String,
        /// Standard error before termination
        stderr: String,
    },
}

/// Writes handler output to a file in the workspace directory.
///
/// This function handles all three output scenarios (success, error, early termination)
/// with consistent formatting and error handling.
///
/// # Arguments
///
/// * `workspace_path` - Path to the workspace directory
/// * `agent_type_str` - Name of the agent (e.g., "architect", "janitor")
/// * `output` - The handler output to write
///
/// # Returns
///
/// * `Ok(())` - If output was written successfully
/// * `Err(AutomationError)` - If file write failed
///
/// # Example
///
/// ```no_run
/// use automation_agents::agent::{HandlerOutput, write_handler_output};
/// use std::path::Path;
///
/// # async fn example() -> automation_common::Result<()> {
/// let output = HandlerOutput::Success {
///     stdout: "Task completed".to_string(),
///     stderr: String::new(),
/// };
/// write_handler_output(Path::new("/workspace"), "architect", &output).await?;
/// # Ok(())
/// # }
/// ```
pub async fn write_handler_output(
    workspace_path: &Path,
    agent_type_str: &str,
    output: &HandlerOutput,
) -> Result<()> {
    let output_file = workspace_path.join(format!(".{}-output.md", agent_type_str));
    let output_content = match output {
        HandlerOutput::Success { stdout, stderr } => {
            format!(
                "# Agent Output - {}\n\n## STDOUT\n\n{}\n\n## STDERR\n\n{}\n",
                agent_type_str, stdout, stderr
            )
        }
        HandlerOutput::Error { error } => {
            format!(
                "# Agent Output - {}\n\n**Error:** {}\n\n## STDERR\n\nNo output available\n",
                agent_type_str, error
            )
        }
        HandlerOutput::EarlyTermination { reason, stdout, stderr } => {
            format!(
                "# Agent Output - {}\n\n**Termination:** {}\n\n## STDOUT\n\n{}\n\n## STDERR\n\n{}\n",
                agent_type_str, reason, stdout, stderr
            )
        }
    };

    let output_file = output_file.clone();
    tokio::task::spawn_blocking(move || {
        std::fs::write(&output_file, output_content)
            .map_err(|e| AutomationError::FileSystem(e))
    })
    .await
    .map_err(|e| AutomationError::Process(format!("Output write task failed: {}", e)))??;

    Ok(())
}

/// Context for passing agent and task information to execution handlers.
///
/// This struct provides the necessary context for agents to execute tasks
/// in parallel mode, including the agent ID, task ID, and workspace directory.
#[derive(Debug, Clone)]
pub struct AgentContext {
    /// Unique identifier for this agent instance
    pub agent_id: String,
    /// Unique identifier for the task being executed
    pub task_id: String,
    /// Path to the workspace directory
    pub workspace_dir: PathBuf,
}

/// Configuration for agent execution.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Type of agent to run
    pub agent_type: AgentType,

    /// Workspace directory path
    pub workspace: String,

    /// State directory path
    pub state_dir: String,

    /// Prompt template file path (relative to workspace)
    pub prompt_template: String,

    /// Interval in seconds between executions
    pub interval_seconds: u64,

    /// Timeout in seconds for each execution
    pub timeout_seconds: u64,

    /// Execute immediately on startup
    pub immediate: bool,

    /// Maximum retry attempts for lock acquisition
    pub max_lock_retries: u32,

    /// Lock timeout in seconds
    pub lock_timeout_seconds: u64,
}

/// Agent runner that executes tasks using automation system.
///
/// Supports both single-instance mode (backward compatible) and parallel mode
/// where multiple agents of the same type can run concurrently.
#[derive(Debug)]
pub struct AgentRunner {
    /// Agent configuration
    config: AgentConfig,
    /// Shutdown notification signal
    shutdown_notify: Arc<Notify>,
    /// Workspace configuration for runtime settings
    workspace_config: Option<WorkspaceConfig>,
    
    // Parallel mode fields
    /// Agent instance ID for parallel mode (None in single-instance mode)
    agent_id: Option<String>,
    /// Task registry for tracking task assignments (None in single-instance mode)
    task_registry: Option<Arc<Mutex<TaskRegistry>>>,
    /// Agent registry for tracking agent instances (None in single-instance mode)
    agent_registry: Option<Arc<Mutex<AgentRegistry>>>,
    /// Configuration for parallel mode (None in single-instance mode)
    parallel_config: Option<ParallelConfig>,
    /// Heartbeat interval in seconds (None in single-instance mode)
    heartbeat_interval: Option<Duration>,
    /// Handle to the heartbeat task (None in single-instance mode)
    heartbeat_handle: Option<JoinHandle<()>>,
}

impl AgentRunner {
    /// Creates a new agent runner with the given configuration.
    ///
    /// This constructor maintains backward compatibility with single-instance mode.
    /// To enable parallel mode, use [`AgentRunner::new_with_workspace_config()`].
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config,
            shutdown_notify: Arc::new(Notify::new()),
            workspace_config: None,
            agent_id: None,
            task_registry: None,
            agent_registry: None,
            parallel_config: None,
            heartbeat_interval: None,
            heartbeat_handle: None,
        }
    }

    /// Creates a new agent runner with workspace configuration for parallel mode support.
    ///
    /// This constructor reads the workspace configuration and enables parallel mode
    /// if configured. Parallel mode allows multiple instances of the same agent
    /// type to run concurrently with task coordination via registries.
    ///
    /// # Arguments
    ///
    /// * `config` - Agent configuration
    /// * `workspace_config_path` - Path to the workspace configuration file (e.g., `.automation-rust.toml`)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_agents::agent::{AgentRunner, AgentConfig};
    ///
    /// let agent_config = AgentConfig { /* ... */ };
    /// let runner = AgentRunner::new_with_workspace_config(
    ///     agent_config,
    ///     ".automation-rust.toml"
    /// ).unwrap();
    /// ```
    pub fn new_with_workspace_config(
        config: AgentConfig,
        workspace_config_path: &str,
    ) -> Result<Self> {
        // Load workspace configuration
        let ws_config = automation_common::workspace_config::parse_config_file(workspace_config_path)
            .map_err(|e| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: format!("Failed to load workspace config: {}", e),
            })?;

        // Check if parallel mode is enabled
        if ws_config.is_parallel_mode_enabled() {
            info!(
                agent_type = %config.agent_type,
                "Parallel mode enabled from workspace configuration"
            );
            
            // Generate unique agent ID
            let agent_type_str = config.agent_type.to_string();
            let agent_id = generate_agent_id(&agent_type_str);
            
            info!(
                agent_type = %agent_type_str,
                agent_id = %agent_id,
                "Generated unique agent ID"
            );

            // Clone parallel config
            let parallel_config = ws_config.parallel.clone();
            let heartbeat_interval = Duration::from_secs(
                parallel_config.heartbeat_interval.as_secs()
            );

            // Initialize registries
            let task_registry = TaskRegistry::new().map_err(|e| AutomationError::StateError {
                task: "TaskRegistry".to_string(),
                message: format!("Failed to initialize task registry: {}", e),
            })?;

            let agent_registry = AgentRegistry::new().map_err(|e| AutomationError::StateError {
                task: "AgentRegistry".to_string(),
                message: format!("Failed to initialize agent registry: {}", e),
            })?;

            Ok(Self {
                config,
                shutdown_notify: Arc::new(Notify::new()),
                workspace_config: Some(ws_config),
                agent_id: Some(agent_id),
                task_registry: Some(Arc::new(Mutex::new(task_registry))),
                agent_registry: Some(Arc::new(Mutex::new(agent_registry))),
                parallel_config: Some(parallel_config),
                heartbeat_interval: Some(heartbeat_interval),
                heartbeat_handle: None,
            })
        } else {
            info!(
                agent_type = %config.agent_type,
                "Parallel mode not enabled, running in single-instance mode"
            );
            
            // Single-instance mode (backward compatible)
            Ok(Self {
                config,
                shutdown_notify: Arc::new(Notify::new()),
                workspace_config: Some(ws_config),
                agent_id: None,
                task_registry: None,
                agent_registry: None,
                parallel_config: None,
                heartbeat_interval: None,
                heartbeat_handle: None,
            })
        }
    }

    /// Checks if parallel mode is active for this agent runner.
    ///
    /// Returns `true` if parallel mode is enabled and infrastructure is initialized.
    pub fn is_parallel_mode(&self) -> bool {
        self.task_registry.is_some() && self.agent_registry.is_some()
    }

    /// Gets the agent instance ID (if parallel mode is enabled).
    ///
    /// Returns `None` in single-instance mode.
    pub fn get_agent_id(&self) -> Option<&String> {
        self.agent_id.as_ref()
    }

    /// Initializes parallel mode infrastructure.
    ///
    /// This method sets up the task registry, agent registry, and generates
    /// a unique agent ID. It's called automatically by `new_with_workspace_config`
    /// but can be used to reinitialize if needed.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Parallel mode is not enabled in workspace configuration
    /// - Task registry or agent registry cannot be initialized
    #[instrument(skip(self))]
    pub fn initialize_parallel_mode(&mut self, config: &WorkspaceConfig) -> Result<()> {
        if !config.is_parallel_mode_enabled() {
            info!("Parallel mode not enabled in configuration");
            return Ok(());
        }

        info!("Initializing parallel mode infrastructure");

        // Generate unique agent ID
        let agent_type_str = self.config.agent_type.to_string();
        let agent_id = generate_agent_id(&agent_type_str);
        
        info!(
            agent_type = %agent_type_str,
            agent_id = %agent_id,
            "Generated unique agent ID for parallel mode"
        );

        // Initialize registries
        let task_registry = TaskRegistry::new().map_err(|e| AutomationError::StateError {
            task: "TaskRegistry".to_string(),
            message: format!("Failed to initialize task registry: {}", e),
        })?;

        let agent_registry = AgentRegistry::new().map_err(|e| AutomationError::StateError {
            task: "AgentRegistry".to_string(),
            message: format!("Failed to initialize agent registry: {}", e),
        })?;

        // Clone parallel config
        let parallel_config = config.parallel.clone();
        let heartbeat_interval = Duration::from_secs(
            parallel_config.heartbeat_interval.as_secs()
        );

        // Update fields
        self.agent_id = Some(agent_id);
        self.task_registry = Some(Arc::new(Mutex::new(task_registry)));
        self.agent_registry = Some(Arc::new(Mutex::new(agent_registry)));
        self.parallel_config = Some(parallel_config);
        self.heartbeat_interval = Some(heartbeat_interval);

        info!("Parallel mode infrastructure initialized successfully");
        Ok(())
    }

    /// Registers this agent instance with the agent registry.
    ///
    /// This method should be called during agent startup in parallel mode.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Parallel mode is not enabled
    /// - An agent with the same ID already exists
    /// - Registry cannot be accessed or updated
    #[instrument(skip(self))]
    pub fn register_agent(&mut self) -> Result<()> {
        if !self.is_parallel_mode() {
            info!("Skipping agent registration (single-instance mode)");
            return Ok(());
        }

        let agent_id = self.agent_id.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Agent ID not set in parallel mode".to_string(),
            })?;

        let agent_type = self.config.agent_type.to_string();
        
        info!(
            agent_id = %agent_id,
            agent_type = %agent_type,
            "Registering agent instance"
        );

        let agent_registry = self.agent_registry.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Agent registry not initialized".to_string(),
            })?;

        let mut registry = agent_registry.lock()
            .map_err(|e| AutomationError::StateError {
                task: "AgentRegistry".to_string(),
                message: format!("Failed to acquire agent registry lock: {}", e),
            })?;

        registry.register_agent(agent_id.clone(), agent_type.clone())?;

        info!(
            agent_id = %agent_id,
            "Agent registered successfully"
        );

        Ok(())
    }

    /// Sends a heartbeat update for this agent.
    ///
    /// This method should be called periodically (e.g., every 30 seconds)
    /// to indicate the agent is still alive and processing tasks.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Parallel mode is not enabled
    /// - Agent ID is not set
    /// - Registry cannot be accessed or updated
    #[instrument(skip(self))]
    pub fn send_heartbeat(&mut self) -> Result<()> {
        if !self.is_parallel_mode() {
            return Ok(());
        }

        let agent_id = self.agent_id.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Agent ID not set in parallel mode".to_string(),
            })?;

        let agent_registry = self.agent_registry.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Agent registry not initialized".to_string(),
            })?;

        let mut registry = agent_registry.lock()
            .map_err(|e| AutomationError::StateError {
                task: "AgentRegistry".to_string(),
                message: format!("Failed to acquire agent registry lock: {}", e),
            })?;

        let updated = registry.update_heartbeat(agent_id)?;

        if updated {
            info!(agent_id = %agent_id, "Heartbeat sent successfully");
        } else {
            warn!(
                agent_id = %agent_id,
                "Agent not found in registry during heartbeat"
            );
        }

        Ok(())
    }

    /// Tries to claim an available task from the task registry.
    ///
    /// Returns the task ID if a task was successfully claimed, `None` if no
    /// tasks are available or all are already claimed.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Parallel mode is not enabled
    /// - Agent ID is not set
    /// - Registry cannot be accessed
    #[instrument(skip(self))]
    pub fn claim_next_task(&mut self) -> Result<Option<String>> {
        if !self.is_parallel_mode() {
            return Ok(None);
        }

        let agent_id = self.agent_id.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Agent ID not set in parallel mode".to_string(),
            })?;

        let parallel_config = self.parallel_config.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Parallel config not set".to_string(),
            })?;

        let task_registry = self.task_registry.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Task registry not initialized".to_string(),
            })?;

        let mut registry = task_registry.lock()
            .map_err(|e| AutomationError::StateError {
                task: "TaskRegistry".to_string(),
                message: format!("Failed to acquire task registry lock: {}", e),
            })?;

        // Get available tasks
        let available_tasks = registry.get_available_tasks(10);
        
        if available_tasks.is_empty() {
            info!("No available tasks to claim");
            return Ok(None);
        }

        // Try to claim the first available task
        let task_to_claim = &available_tasks[0];
        let task_id = &task_to_claim.task_id;

        info!(
            agent_id = %agent_id,
            task_id = %task_id,
            "Attempting to claim task"
        );

        let claimed = registry.claim_task(
            task_id,
            agent_id,
            parallel_config.task_lease_duration,
        )?;

        if claimed {
            info!(
                agent_id = %agent_id,
                task_id = %task_id,
                "Task claimed successfully"
            );
            Ok(Some(task_id.clone()))
        } else {
            info!(
                task_id = %task_id,
                "Task could not be claimed (already claimed or not available)"
            );
            Ok(None)
        }
    }

    /// Renews the lease for an in-progress task.
    ///
    /// This method should be called periodically during long-running task execution
    /// to prevent the task lease from expiring.
    ///
    /// # Arguments
    ///
    /// * `task_id` - ID of the task to renew
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Parallel mode is not enabled
    /// - Agent ID or parallel config is not set
    /// - Task is not assigned to this agent
    /// - Registry cannot be accessed
    #[instrument(skip(self))]
    pub fn renew_task_lease(&mut self, task_id: &str) -> Result<()> {
        if !self.is_parallel_mode() {
            return Ok(());
        }

        let agent_id = self.agent_id.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Agent ID not set in parallel mode".to_string(),
            })?;

        let parallel_config = self.parallel_config.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Parallel config not set".to_string(),
            })?;

        let task_registry = self.task_registry.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Task registry not initialized".to_string(),
            })?;

        let mut registry = task_registry.lock()
            .map_err(|e| AutomationError::StateError {
                task: "TaskRegistry".to_string(),
                message: format!("Failed to acquire task registry lock: {}", e),
            })?;

        let renewed = registry.renew_lease(
            task_id,
            agent_id,
            parallel_config.task_lease_duration,
        )?;

        if renewed {
            info!(
                agent_id = %agent_id,
                task_id = %task_id,
                "Task lease renewed successfully"
            );
        } else {
            warn!(
                agent_id = %agent_id,
                task_id = %task_id,
                "Task lease could not be renewed (not assigned to this agent)"
            );
        }

        Ok(())
    }

    /// Marks a task as completed.
    ///
    /// This should be called after a task has been successfully executed.
    ///
    /// # Arguments
    ///
    /// * `task_id` - ID of the task to complete
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Parallel mode is not enabled
    /// - Agent ID is not set
    /// - Task is not assigned to this agent
    /// - Registry cannot be accessed
    #[instrument(skip(self))]
    pub fn complete_task(&mut self, task_id: &str) -> Result<()> {
        if !self.is_parallel_mode() {
            return Ok(());
        }

        let agent_id = self.agent_id.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Agent ID not set in parallel mode".to_string(),
            })?;

        let task_registry = self.task_registry.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Task registry not initialized".to_string(),
            })?;

        let mut registry = task_registry.lock()
            .map_err(|e| AutomationError::StateError {
                task: "TaskRegistry".to_string(),
                message: format!("Failed to acquire task registry lock: {}", e),
            })?;

        registry.complete_task(task_id, agent_id)?;

        info!(
            agent_id = %agent_id,
            task_id = %task_id,
            "Task marked as completed"
        );

        Ok(())
    }

    /// Marks a task as failed with a reason.
    ///
    /// This should be called if task execution fails.
    ///
    /// # Arguments
    ///
    /// * `task_id` - ID of the task to fail
    /// * `reason` - Human-readable reason for the failure
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Parallel mode is not enabled
    /// - Agent ID is not set
    /// - Task is not assigned to this agent
    /// - Registry cannot be accessed
    #[instrument(skip(self))]
    pub fn fail_task(&mut self, task_id: &str, reason: String) -> Result<()> {
        if !self.is_parallel_mode() {
            return Ok(());
        }

        let agent_id = self.agent_id.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Agent ID not set in parallel mode".to_string(),
            })?;

        let task_registry = self.task_registry.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Task registry not initialized".to_string(),
            })?;

        let mut registry = task_registry.lock()
            .map_err(|e| AutomationError::StateError {
                task: "TaskRegistry".to_string(),
                message: format!("Failed to acquire task registry lock: {}", e),
            })?;

        registry.fail_task(task_id, agent_id, reason.clone())?;

        error!(
            agent_id = %agent_id,
            task_id = %task_id,
            reason = %reason,
            "Task marked as failed"
        );

        Ok(())
    }

    /// Recovers tasks orphaned by a previous agent instance.
    ///
    /// Tasks that were claimed by an agent that has since shut down or become stale
    /// will be returned to the `Available` state so they can be claimed again.
    ///
    /// # Returns
    ///
    /// A list of task IDs that were recovered
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Parallel mode is not enabled
    /// - Parallel config is not set
    /// - Registry cannot be accessed
    #[instrument(skip(self))]
    pub fn recover_orphaned_tasks(&mut self) -> Result<Vec<String>> {
        if !self.is_parallel_mode() {
            return Ok(Vec::new());
        }

        let parallel_config = self.parallel_config.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Parallel config not set".to_string(),
            })?;

        let task_registry = self.task_registry.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Task registry not initialized".to_string(),
            })?;

        let mut registry = task_registry.lock()
            .map_err(|e| AutomationError::StateError {
                task: "TaskRegistry".to_string(),
                message: format!("Failed to acquire task registry lock: {}", e),
            })?;

        // Abandon stale tasks (those with expired leases)
        let abandoned = registry.abandon_stale_tasks(parallel_config.task_lease_duration)?;

        if !abandoned.is_empty() {
            info!(
                count = abandoned.len(),
                tasks = ?abandoned,
                "Recovered orphaned tasks"
            );
        }

        Ok(abandoned)
    }

    /// Cleans up stale resources in both task and agent registries.
    ///
    /// This method:
    /// 1. Marks stale agents in the agent registry
    /// 2. Removes old shutdown agents
    /// 3. Removes old completed tasks
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Parallel mode is not enabled
    /// - Registry cannot be accessed
    #[instrument(skip(self))]
    pub fn cleanup_stale_resources(&mut self) -> Result<()> {
        if !self.is_parallel_mode() {
            return Ok(());
        }

        let parallel_config = self.parallel_config.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Parallel config not set".to_string(),
            })?;

        // Cleanup agent registry
        if let Some(agent_registry) = &self.agent_registry {
            let mut registry = agent_registry.lock()
                .map_err(|e| AutomationError::StateError {
                    task: "AgentRegistry".to_string(),
                    message: format!("Failed to acquire agent registry lock: {}", e),
                })?;

            // Mark stale agents
            let marked_stale = registry.mark_stale_agents(parallel_config.stale_agent_threshold)?;
            if !marked_stale.is_empty() {
                info!(
                    count = marked_stale.len(),
                    "Marked agents as stale"
                );
            }

            // Remove old shutdown agents
            let removed_shutdown = registry.cleanup_shutdown_agents(parallel_config.stale_agent_retention)?;
            if !removed_shutdown.is_empty() {
                info!(
                    count = removed_shutdown.len(),
                    "Removed old shutdown agents"
                );
            }
        }

        // Cleanup task registry
        if let Some(task_registry) = &self.task_registry {
            let mut registry = task_registry.lock()
                .map_err(|e| AutomationError::StateError {
                    task: "TaskRegistry".to_string(),
                    message: format!("Failed to acquire task registry lock: {}", e),
                })?;

            // Remove old completed tasks
            let removed_completed = registry.cleanup_completed_tasks(parallel_config.completed_task_retention)?;
            if !removed_completed.is_empty() {
                info!(
                    count = removed_completed.len(),
                    "Removed old completed tasks"
                );
            }
        }

        Ok(())
    }

    /// Performs graceful shutdown, marking the agent as shutdown in the registry.
    ///
    /// This method should be called when the agent is shutting down to properly
    /// clean up registry state and signal other agents that this instance is no longer active.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Parallel mode is not enabled
    /// - Agent ID is not set
    /// - Registry cannot be accessed or updated
    #[instrument(skip(self))]
    pub fn shutdown(&mut self) -> Result<()> {
        if !self.is_parallel_mode() {
            info!("Skipping shutdown (single-instance mode)");
            return Ok(());
        }

        let agent_id = self.agent_id.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Agent ID not set in parallel mode".to_string(),
            })?;

        info!(
            agent_id = %agent_id,
            "Marking agent as shutdown"
        );

        let agent_registry = self.agent_registry.as_ref()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Agent registry not initialized".to_string(),
            })?;

        let mut registry = agent_registry.lock()
            .map_err(|e| AutomationError::StateError {
                task: "AgentRegistry".to_string(),
                message: format!("Failed to acquire agent registry lock: {}", e),
            })?;

        registry.mark_shutdown(agent_id)?;

        info!(
            agent_id = %agent_id,
            "Agent marked as shutdown successfully"
        );

        Ok(())
    }

    /// Runs agent as a long-running scheduled task.
    ///
    /// This method:
    /// 1. Sets up scheduled task with the configured interval
    /// 2. Executes the handler pattern for each run
    /// 3. Handles graceful shutdown signals (SIGTERM, SIGINT)
    /// 4. If parallel mode is enabled: registers agent, starts heartbeat, and handles cleanup
    #[instrument(skip(self))]
    pub async fn run(&mut self) -> Result<()> {
        let agent_type = self.config.agent_type;
        let agent_type_str = agent_type.to_string();
        
        info!(
            agent_type = %agent_type,
            interval = self.config.interval_seconds,
            timeout = self.config.timeout_seconds,
            immediate = self.config.immediate,
            parallel_mode = self.is_parallel_mode(),
            "Agent starting"
        );

        // Parallel mode initialization
        if self.is_parallel_mode() {
            // Register this agent instance
            self.register_agent()?;
            
            // Recover orphaned tasks from previous instances
            let recovered = self.recover_orphaned_tasks()?;
            if !recovered.is_empty() {
                info!(
                    count = recovered.len(),
                    "Recovered orphaned tasks on startup"
                );
            }
            
            // Clean up stale resources
            self.cleanup_stale_resources()?;
            
            // Start heartbeat task
            self.start_heartbeat_task()?;
        }

        let _interval = Duration::from_secs(self.config.interval_seconds);
        let _timeout = Duration::from_secs(self.config.timeout_seconds);
        let _max_retries = 3;
        let _retry_delay_base = Duration::from_millis(100);

        // Create scheduled task
        let mut task = create_scheduled_task(&agent_type_str);

        // Configure task with custom settings
        // Note: We need to use the ScheduledTask::new with full params
        // since create_scheduled_task uses defaults

        let shutdown_notify = Arc::clone(&self.shutdown_notify);
        let workspace = self.config.workspace.clone();
        let state_dir = self.config.state_dir.clone();
        let prompt_template = self.config.prompt_template.clone();
        let timeout_seconds = self.config.timeout_seconds;
        
        // Clone parallel mode fields for the handler
        let is_parallel = self.is_parallel_mode();
        let task_registry = self.task_registry.clone();
        let agent_id = self.agent_id.clone();
        let parallel_config = self.parallel_config.clone();

        // Create handler closure that captures necessary values
        let handler = move || {
            let workspace = workspace.clone();
            let state_dir = state_dir.clone();
            let prompt_template = prompt_template.clone();
            let agent_type = agent_type;
            let timeout = Duration::from_secs(timeout_seconds);
            let is_parallel = is_parallel;
            let task_registry = task_registry.clone();
            let agent_id = agent_id.clone();
            let parallel_config = parallel_config.clone();

            async move {
                Self::execute_handler_internal(
                    &workspace,
                    &state_dir,
                    &prompt_template,
                    agent_type,
                    timeout,
                    is_parallel,
                    task_registry,
                    agent_id,
                    parallel_config,
                )
                .await
                .map_err(|e| anyhow::anyhow!("Handler execution failed: {}", e))
            }
        };

        // Start the scheduled task
        task.start(handler).await.map_err(|e| AutomationError::ScheduleConfigInvalid {
            reason: format!("Failed to start scheduled task: {}", e),
        })?;

        info!("Agent scheduled and running");

        // Wait for shutdown signal
        shutdown_notify.notified().await;
        info!("Shutdown signal received, stopping agent");

        // Stop the task
        task.stop().await.map_err(|e| AutomationError::ScheduleConfigInvalid {
            reason: format!("Failed to stop scheduled task: {}", e),
        })?;

        // Parallel mode shutdown
        if self.is_parallel_mode() {
            // Stop heartbeat task
            self.stop_heartbeat_task();
            
            // Mark agent as shutdown in registry
            if let Err(e) = self.shutdown() {
                error!("Failed to mark agent as shutdown: {}", e);
            }
        }

        info!("Agent stopped successfully");
        Ok(())
    }

    /// Starts the heartbeat task for parallel mode.
    ///
    /// Spawns a background task that periodically sends heartbeats to the agent registry
    /// to indicate this agent is still alive.
    #[instrument(skip(self))]
    fn start_heartbeat_task(&mut self) -> Result<()> {
        if !self.is_parallel_mode() {
            return Ok(());
        }

        let heartbeat_interval = self.heartbeat_interval
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Heartbeat interval not set in parallel mode".to_string(),
            })?;

        let agent_registry = self.agent_registry.clone()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Agent registry not initialized".to_string(),
            })?;

        let agent_id = self.agent_id.clone()
            .ok_or_else(|| AutomationError::StateError {
                task: "AgentRunner".to_string(),
                message: "Agent ID not set in parallel mode".to_string(),
            })?;

        let shutdown_notify = Arc::clone(&self.shutdown_notify);

        info!(
            agent_id = %agent_id,
            interval_secs = heartbeat_interval.as_secs(),
            "Starting heartbeat task"
        );

        let heartbeat_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(heartbeat_interval);
            
            loop {
                tokio::select! {
                    _ = shutdown_notify.notified() => {
                        info!("Heartbeat task stopping (shutdown signal received)");
                        break;
                    }
                    _ = interval.tick() => {
                        // Send heartbeat
                        if let Ok(mut registry) = agent_registry.lock() {
                            let updated = registry.update_heartbeat(&agent_id);
                            if updated.is_ok() && updated.unwrap() {
                                info!(agent_id = %agent_id, "Heartbeat sent");
                            }
                        } else {
                            warn!("Failed to acquire agent registry for heartbeat");
                        }
                    }
                }
            }
        });

        self.heartbeat_handle = Some(heartbeat_handle);
        
        info!("Heartbeat task started");
        Ok(())
    }

    /// Stops the heartbeat task.
    ///
    /// This is called during shutdown to stop sending heartbeats.
    fn stop_heartbeat_task(&mut self) {
        if let Some(handle) = self.heartbeat_handle.take() {
            info!("Stopping heartbeat task");
            handle.abort();
            info!("Heartbeat task stopped");
        }
    }

    /// Executes a single agent run using the handler pattern.
    ///
    /// # Handler Pattern
    ///
    /// 1. Acquire lock with retry
    /// 2. Update state to running
    /// 3. Read workspace files in parallel
    /// 4. Execute prompt template with context substitution
    /// 5. Execute Kilo Code CLI with prompt
    /// 6. Handle early termination / success / failure
    /// 7. Write output file
    /// 8. Update state with results
    /// 9. Release lock (RAII pattern handles this automatically)
    #[instrument(skip(self))]
    async fn execute_handler(&self) -> Result<()> {
        Self::execute_handler_internal(
            &self.config.workspace,
            &self.config.state_dir,
            &self.config.prompt_template,
            self.config.agent_type,
            Duration::from_secs(self.config.timeout_seconds),
            self.is_parallel_mode(),
            self.task_registry.clone(),
            self.agent_id.clone(),
            self.parallel_config.clone(),
        )
        .await
    }

    /// Internal handler execution implementation.
    ///
    /// This is a static method to allow it to be used in closures
    /// for the scheduler integration.
    async fn execute_handler_internal(
        workspace: &str,
        state_dir: &str,
        prompt_template: &str,
        agent_type: AgentType,
        timeout: Duration,
        is_parallel: bool,
        task_registry: Option<Arc<Mutex<TaskRegistry>>>,
        agent_id: Option<String>,
        parallel_config: Option<ParallelConfig>,
    ) -> Result<()> {
        let start_time = std::time::Instant::now();
        let agent_type_str = agent_type.to_string();

        info!(
            agent_type = %agent_type_str,
            workspace,
            state_dir,
            prompt_template,
            is_parallel,
            "Starting handler execution"
        );

        // Parallel mode: Try to claim a task before proceeding
        let claimed_task_id = if is_parallel {
            let agent_id_ref = agent_id.as_ref().ok_or_else(|| {
                AutomationError::StateError {
                    task: "AgentRunner".to_string(),
                    message: "Agent ID not set in parallel mode".to_string(),
                }
            })?;

            let task_registry_ref = task_registry.as_ref().ok_or_else(|| {
                AutomationError::StateError {
                    task: "AgentRunner".to_string(),
                    message: "Task registry not initialized".to_string(),
                }
            })?;

            let parallel_config_ref = parallel_config.as_ref().ok_or_else(|| {
                AutomationError::StateError {
                    task: "AgentRunner".to_string(),
                    message: "Parallel config not set".to_string(),
                }
            })?;

            let mut registry = task_registry_ref.lock()
                .map_err(|e| AutomationError::StateError {
                    task: "TaskRegistry".to_string(),
                    message: format!("Failed to acquire task registry lock: {}", e),
                })?;

            // Get available tasks
            let available_tasks = registry.get_available_tasks(10);
            
            if available_tasks.is_empty() {
                info!("No available tasks to claim in parallel mode");
                return Ok(()); // Exit gracefully - no work to do
            }

            // Try to claim the first available task
            let task_to_claim = &available_tasks[0];
            let task_id = &task_to_claim.task_id;

            info!(
                agent_id = %agent_id_ref,
                task_id = %task_id,
                "Attempting to claim task in parallel mode"
            );

            let claimed = registry.claim_task(
                task_id,
                agent_id_ref,
                parallel_config_ref.task_lease_duration,
            )?;

            if claimed {
                info!(
                    agent_id = %agent_id_ref,
                    task_id = %task_id,
                    "Task claimed successfully in parallel mode"
                );
                Some(task_id.clone())
            } else {
                info!(
                    task_id = %task_id,
                    "Task could not be claimed in parallel mode (already claimed or not available)"
                );
                return Ok(()); // Exit gracefully - task was claimed by another agent
            }
        } else {
            None // Single-instance mode - no task ID to track
        };

        // Step 1: Acquire lock with retry
        // Lock file path: <state_dir>/<agent_type>.lock
        let lock_file_path = PathBuf::from(state_dir).join(format!("{}.lock", agent_type_str));
        let lock_manager = LockManager::new_default(lock_file_path);
        let _lock_handle = lock_manager.acquire_lock().map_err(|e| {
            error!(
                agent_type = %agent_type_str,
                error = %e,
                "Failed to acquire lock"
            );
            e
        })?;

        info!(
            agent_type = %agent_type_str,
            "Lock acquired successfully"
        );

        // Create state manager for state operations
        let state_manager = StateManager::new(PathBuf::from(state_dir), &agent_type_str)?;

        // Step 2: Update state to running
        state_manager.load_state_with_lock(|state| {
            state.last_run = Some(Utc::now());
            state.status = "running".to_string();
            Ok(())
        })?;

        info!(
            agent_type = %agent_type_str,
            "State updated to running"
        );

        // Step 3: Read workspace files in parallel
        let workspace_path = Path::new(workspace);
        let workspace_manager = WorkspaceManager::new(workspace_path);

        let wm_todo = workspace_manager.clone();
        let wm_backlog = workspace_manager.clone();
        let wm_completed = workspace_manager.clone();
        let wm_blockers = workspace_manager.clone();
        let wm_prd = workspace_manager.clone();

        let (todo_result, backlog_result, completed_result, blockers_result, prd_result) = tokio::join!(
            tokio::task::spawn_blocking(move || {
                wm_todo.read_todo_file()
            }),
            tokio::task::spawn_blocking(move || {
                wm_backlog.read_backlog_file()
            }),
            tokio::task::spawn_blocking(move || {
                wm_completed.read_completed_file()
            }),
            tokio::task::spawn_blocking(move || {
                wm_blockers.read_blockers_file()
            }),
            tokio::task::spawn_blocking(move || {
                wm_prd.read_prd_file()
            }),
        );

        let todo = todo_result.map_err(|e| AutomationError::Process(format!("TODO read join error: {}", e)))??;
        let backlog = backlog_result.map_err(|e| AutomationError::Process(format!("BACKLOG read join error: {}", e)))??;
        let completed = completed_result.map_err(|e| AutomationError::Process(format!("COMPLETED read join error: {}", e)))??;
        let blockers = blockers_result.map_err(|e| AutomationError::Process(format!("BLOCKERS read join error: {}", e)))??;
        let prd = prd_result.map_err(|e| AutomationError::Process(format!("PRD read join error: {}", e)))??;

        info!(
            agent_type = %agent_type_str,
            todo_exists = !todo.is_empty(),
            backlog_exists = !backlog.is_empty(),
            completed_exists = !completed.is_empty(),
            "Workspace files read successfully"
        );

        // Step 4: Execute prompt template with context substitution
        let template_path = workspace_path.join(prompt_template);
        let template_content = tokio::task::spawn_blocking(move || {
            std::fs::read_to_string(&template_path)
                .map_err(|e| AutomationError::FileSystem(e))
        })
        .await
        .map_err(|e| AutomationError::Process(format!("Template read task failed: {}", e)))??;

        let context = TemplateContext {
            todo: if todo.is_empty() { None } else { Some(todo) },
            backlog: if backlog.is_empty() { None } else { Some(backlog) },
            completed: if completed.is_empty() { None } else { Some(completed) },
            blockers: if blockers.is_empty() { None } else { Some(blockers) },
            prd: if prd.is_empty() { None } else { Some(prd) },
            workspace: workspace.to_string(),
            timestamp: Some(Utc::now().to_rfc3339()),
            agent_type: Some(agent_type_str.clone()),
        };

        let prompt = TemplateEngine::render(&template_content, &context);

        info!(
            agent_type = %agent_type_str,
            template = %prompt_template,
            prompt_length = prompt.len(),
            "Template rendered successfully"
        );

        // Step 5: Execute Kilo Code CLI with prompt
        let executor_config = ExecutorConfig {
            command: "kilocode".to_string(),
            args: vec![
                "--mode".to_string(),
                "orchestrator".to_string(),
                "--auto".to_string(),
                "--timeout".to_string(),
                timeout.as_secs().to_string(),
                "--workspace".to_string(),
                workspace.to_string(),
            ],
            input: Some(prompt),
            workspace: workspace.to_string(),
            timeout,
            output_buffer_limit: 10 * 1024 * 1024, // 10MB
            graceful_timeout: Duration::from_secs(5),
            force_timeout: Duration::from_secs(2),
        };

        let executor = CLIExecutor::new(executor_config);

        info!(
            agent_type = %agent_type_str,
            "Executing Kilo Code CLI"
        );

        // Step 6: Handle early termination / success / failure
        let result = executor.execute().await;

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(process_result) => {
                if process_result.terminated_early {
                    let reason = process_result.termination_reason.unwrap_or_else(|| "Unknown".to_string());
                    let reason_clone = reason.clone();
                    let reason_for_registry = reason.clone();
                    let early_termination_error = reason.clone();
                    
                    error!(
                        agent_type = %agent_type_str,
                        reason = %reason,
                        execution_time_ms,
                        "Process terminated early"
                    );

                    // Step 7: Write output file (even on early termination)
                    write_handler_output(
                        workspace_path,
                        &agent_type_str,
                        &HandlerOutput::EarlyTermination {
                            reason,
                            stdout: process_result.stdout,
                            stderr: process_result.stderr,
                        }
                    ).await?;

                    // Step 8: Update state with early termination
                    state_manager.load_state_with_lock(|state| {
                        state.last_failure = Some(Utc::now());
                        state.error_count += 1;
                        state.consecutive_failures += 1;
                        state.status = "failed".to_string();
                        state.last_termination_reason = Some(reason_for_registry.clone());
                        state.early_termination_count += 1;
                        state.total_execution_time_ms += execution_time_ms;
                        // Recalculate average
                        let total_executions = state.early_termination_count + state.successful_terminations;
                        if total_executions > 0 {
                            state.average_execution_time_ms = state.total_execution_time_ms / total_executions;
                        }
                        state.failed_terminations += 1;
                        Ok(())
                    })?;

                    // Parallel mode: Fail task in registry on early termination
                    if is_parallel {
                        if let (Some(task_id), Some(task_registry)) = (&claimed_task_id, task_registry) {
                            if let Some(agent_id_ref) = &agent_id {
                                let mut registry = task_registry.lock()
                                    .map_err(|log_err| AutomationError::StateError {
                                        task: "TaskRegistry".to_string(),
                                        message: format!("Failed to acquire task registry lock: {}", log_err),
                                    })?;

                                let error_msg = format!("Early termination: {}", early_termination_error);
                                if let Err(log_err) = registry.fail_task(task_id, agent_id_ref, error_msg.clone()) {
                                    error!(
                                        task_id = %task_id,
                                        error = %log_err,
                                        "Failed to mark task as failed in registry"
                                    );
                                } else {
                                    warn!(
                                        task_id = %task_id,
                                        reason = %error_msg,
                                        "Task marked as failed in registry (early termination)"
                                    );
                                }
                            }
                        }
                    }

                    return Err(AutomationError::Process(format!("Early termination: {}", reason_clone)));
                }

                // Success case
                info!(
                    agent_type = %agent_type_str,
                    execution_time_ms,
                    stdout_length = process_result.stdout.len(),
                    stderr_length = process_result.stderr.len(),
                    "Process completed successfully"
                );

                // Step 7: Write output file
                write_handler_output(
                    workspace_path,
                    &agent_type_str,
                    &HandlerOutput::Success {
                        stdout: process_result.stdout,
                        stderr: process_result.stderr,
                    }
                ).await?;

                // Step 8: Update state with success
                state_manager.load_state_with_lock(|state| {
                    state.last_success = Some(Utc::now());
                    state.status = "success".to_string();
                    state.consecutive_failures = 0;
                    state.successful_terminations += 1;
                    state.total_execution_time_ms += execution_time_ms;
                    // Recalculate average
                    let total_executions = state.early_termination_count + state.successful_terminations;
                    if total_executions > 0 {
                        state.average_execution_time_ms = state.total_execution_time_ms / total_executions;
                    }
                    Ok(())
                })?;

                // Parallel mode: Complete task in registry
                if is_parallel {
                    if let (Some(task_id), Some(task_registry)) = (&claimed_task_id, task_registry) {
                        if let Some(agent_id_ref) = &agent_id {
                            let mut registry = task_registry.lock()
                                .map_err(|e| AutomationError::StateError {
                                    task: "TaskRegistry".to_string(),
                                    message: format!("Failed to acquire task registry lock: {}", e),
                                })?;

                            if let Err(e) = registry.complete_task(task_id, agent_id_ref) {
                                error!(
                                    task_id = %task_id,
                                    error = %e,
                                    "Failed to complete task in registry"
                                );
                            } else {
                                info!(
                                    task_id = %task_id,
                                    "Task marked as completed in registry"
                                );
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!(
                    agent_type = %agent_type_str,
                    error = %e,
                    execution_time_ms,
                    "Process execution failed"
                );

                // Step 7: Write output file (on error)
                write_handler_output(
                    workspace_path,
                    &agent_type_str,
                    &HandlerOutput::Error { error: e.to_string() }
                ).await?;

                // Step 8: Update state with error
                state_manager.load_state_with_lock(|state| {
                    state.last_failure = Some(Utc::now());
                    state.error_count += 1;
                    state.consecutive_failures += 1;
                    state.status = "failed".to_string();
                    state.last_termination_reason = Some(e.to_string());
                    state.failed_terminations += 1;
                    state.total_execution_time_ms += execution_time_ms;
                    // Recalculate average
                    let total_executions = state.early_termination_count + state.successful_terminations + state.failed_terminations;
                    if total_executions > 0 {
                        state.average_execution_time_ms = state.total_execution_time_ms / total_executions;
                    }
                    Ok(())
                })?;

                // Parallel mode: Fail task in registry
                if is_parallel {
                    if let (Some(task_id), Some(task_registry)) = (&claimed_task_id, task_registry) {
                        if let Some(agent_id_ref) = &agent_id {
                            let mut registry = task_registry.lock()
                                .map_err(|log_err| AutomationError::StateError {
                                    task: "TaskRegistry".to_string(),
                                    message: format!("Failed to acquire task registry lock: {}", log_err),
                                })?;

                            let error_msg = e.to_string();
                            if let Err(log_err) = registry.fail_task(task_id, agent_id_ref, error_msg.clone()) {
                                error!(
                                    task_id = %task_id,
                                    error = %log_err,
                                    "Failed to mark task as failed in registry"
                                );
                            } else {
                                warn!(
                                    task_id = %task_id,
                                    reason = %error_msg,
                                    "Task marked as failed in registry"
                                );
                            }
                        }
                    }
                }

                return Err(e);
            }
        }

        // Step 9: Release lock (RAII pattern handles this automatically when _lock_handle goes out of scope)
        info!(
            agent_type = %agent_type_str,
            execution_time_ms,
            "Handler execution completed successfully"
        );

        Ok(())
    }

    /// Stops the agent gracefully.
    pub async fn stop(&self) {
        info!(
            agent_type = %self.config.agent_type,
            "Stopping agent"
        );
        self.shutdown_notify.notify_one();
    }
}



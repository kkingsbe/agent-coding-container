//! # Workspace Configuration
//!
//! This module provides configuration structs for parsing TOML configuration files
//! used by the automation-rust project. The configuration is defined in files named
//! `.automation-rust.toml` in workspace directories.
//!
//! ## Configuration Structure
//!
//! The TOML configuration file has the following structure:
//!
//! ```toml
//! [workspace]
//! path = "."
//! state_path = ".state"
//!
//! [logging]
//! level = "info"
//! format = "pretty"
//! colors = true
//!
//! [agents.architect]
//! interval_minutes = 40
//! timeout_minutes = 30
//! immediate = false
//! prompt_template = "prompts-architect/ARCHITECT.md"
//!
//! [agents.architect.lock]
//! max_retries = 3
//! timeout_seconds = 30
//!
//! [agents.janitor]
//! interval_minutes = 20
//! timeout_minutes = 15
//! immediate = false
//! prompt_template = "prompts-janitor/JANITOR.md"
//!
//! [agents.janitor.lock]
//! max_retries = 3
//! timeout_seconds = 30
//!
//! [agents.prompt]
//! interval_minutes = 5
//! timeout_minutes = 3
//! immediate = false
//! prompt_template = "prompts-prompt/PROMPT.md"
//!
//! [agents.prompt.lock]
//! max_retries = 3
//! timeout_seconds = 30
//! ```
//!
//! ## Parallel Mode Configuration
//!
//! Parallel mode enables multiple instances of the same agent type to run concurrently,
//! allowing for better resource utilization and task distribution. When parallel mode is
//! enabled, agents use heartbeats to signal their presence, and stale agents are
//! automatically removed from the registry.
//!
//! ### Enabling Parallel Mode
//!
//! Add the `[parallel]` section to your `.automation-rust.toml`:
//!
//! ```toml
//! [parallel]
//! enabled = true
//! heartbeat_interval = "30s"
//! stale_agent_threshold = "90s"
//! task_lease_duration = "10m"
//! max_instances_per_type = 5
//! completed_task_retention = "7d"
//! stale_agent_retention = "1h"
//! ```
//!
//! ### Parallel Mode Settings
//!
//! | Setting | Type | Default | Description |
//! |---------|------|---------|-------------|
//! | `enabled` | bool | `false` | Enable parallel mode for agents |
//! | `heartbeat_interval` | duration | `"30s"` | Duration between agent heartbeats |
//! | `stale_agent_threshold` | duration | `"90s"` | Threshold for marking an agent as stale |
//! | `task_lease_duration` | duration | `"10m"` | Default lease duration for task assignments |
//! | `max_instances_per_type` | number? | `None` | Maximum parallel instances per agent type |
//! | `completed_task_retention` | duration | `"7d"` | Duration to keep completed tasks |
//! | `stale_agent_retention` | duration | `"1h"` | Duration to keep stale agents before removal |
//!
//! ### Duration Format
//!
//! Duration fields support human-readable formats:
//! - Seconds: `30s`, `90s`, `120s`
//! - Minutes: `5m`, `10m`, `30m`
//! - Hours: `1h`, `2h`, `24h`
//! - Days: `1d`, `7d`, `30d`
//! - Weeks: `1w`, `2w`
//!
//! Example durations:
//! - `"30s"` - 30 seconds
//! - `"5m"` - 5 minutes
//! - `"2h"` - 2 hours
//! - `"7d"` - 7 days
//!
//! ### Backward Compatibility
//!
//! By default, parallel mode is **disabled** (`enabled = false`), maintaining backward
//! compatibility with existing single-instance deployments. To enable parallel mode,
//! explicitly set `enabled = true` in the `[parallel]` section.
//!
//! ### Checking Parallel Mode in Code
//!
//! ```no_run
//! use automation_common::workspace_config::{Config, parse_config_file};
//!
//! let config = parse_config_file(".automation-rust.toml").unwrap();
//! if config.is_parallel_mode_enabled() {
//!     println!("Running in parallel mode");
//! } else {
//!     println!("Running in single-instance mode");
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Duration;

/// Error type for configuration parsing
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// IO error when reading config file
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML parsing error
    #[error("TOML parsing error: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// Missing required field
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// Invalid value for field
    #[error("Invalid value for field '{field}': {message}")]
    InvalidValue { field: String, message: String },
}

/// Result type alias for config operations
pub type ConfigResult<T> = std::result::Result<T, ConfigError>;

/// Root configuration struct
///
/// This is the top-level configuration that contains all workspace settings,
/// logging configuration, agent configurations, and parallel mode settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Workspace configuration
    #[serde(default)]
    pub workspace: WorkspaceConfig,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,

    /// Agent configurations
    #[serde(default)]
    pub agents: AgentsConfig,

    /// Parallel agent mode configuration
    #[serde(default)]
    pub parallel: ParallelConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            workspace: WorkspaceConfig::default(),
            logging: LoggingConfig::default(),
            agents: AgentsConfig::default(),
            parallel: ParallelConfig::default(),
        }
    }
}

impl Config {
    /// Check if parallel mode is enabled for agents.
    ///
    /// Returns `true` if parallel mode is enabled in the configuration,
    /// allowing multiple instances of the same agent type to run concurrently.
    ///
    /// # Returns
    ///
    /// `true` if parallel mode is enabled, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use automation_common::workspace_config::parse_config_file;
    ///
    /// let config = parse_config_file(".automation-rust.toml").unwrap();
    /// if config.is_parallel_mode_enabled() {
    ///     println!("Parallel mode is enabled");
    /// } else {
    ///     println!("Running in single-instance mode");
    /// }
    /// ```
    pub fn is_parallel_mode_enabled(&self) -> bool {
        self.parallel.enabled
    }
}

/// Workspace configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// Path to the workspace directory
    #[serde(default = "default_workspace_path")]
    pub path: String,

    /// Path to the state directory
    #[serde(default = "default_state_path")]
    pub state_path: String,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            path: default_workspace_path(),
            state_path: default_state_path(),
        }
    }
}

fn default_workspace_path() -> String {
    ".".to_string()
}

fn default_state_path() -> String {
    ".state".to_string()
}

/// Logging configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (e.g., "debug", "info", "warn", "error")
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log format (e.g., "pretty", "json")
    #[serde(default = "default_log_format")]
    pub format: String,

    /// Whether to use colored output
    #[serde(default = "default_log_colors")]
    pub colors: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            colors: default_log_colors(),
        }
    }
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "pretty".to_string()
}

fn default_log_colors() -> bool {
    true
}

/// Container for all agent configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsConfig {
    /// Architect agent configuration
    #[serde(default)]
    pub architect: AgentConfig,

    /// Janitor agent configuration
    #[serde(default)]
    pub janitor: AgentConfig,

    /// Prompt agent configuration
    #[serde(default)]
    pub prompt: AgentConfig,

    /// CodeReview agent configuration
    #[serde(default)]
    pub code_review: AgentConfig,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            architect: AgentConfig::default_architect(),
            janitor: AgentConfig::default_janitor(),
            prompt: AgentConfig::default_prompt(),
            code_review: AgentConfig::default_code_review(),
        }
    }
}

/// Individual agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Interval between agent runs in minutes
    #[serde(default = "default_agent_interval_minutes")]
    pub interval_minutes: u64,

    /// Timeout for agent execution in minutes
    #[serde(default = "default_agent_timeout_minutes")]
    pub timeout_minutes: u64,

    /// Whether to run the agent immediately on startup
    #[serde(default = "default_agent_immediate")]
    pub immediate: bool,

    /// Path to the prompt template file
    #[serde(default = "default_agent_prompt_template")]
    pub prompt_template: String,

    /// Lock configuration for this agent
    #[serde(default)]
    pub lock: LockConfig,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            interval_minutes: 5,
            timeout_minutes: 3,
            immediate: false,
            prompt_template: String::new(),
            lock: LockConfig::default(),
        }
    }
}

fn default_agent_interval_minutes() -> u64 {
    5
}

fn default_agent_timeout_minutes() -> u64 {
    3
}

fn default_agent_immediate() -> bool {
    false
}

fn default_agent_prompt_template() -> String {
    String::new()
}

impl AgentConfig {
    /// Create default architect agent configuration
    pub fn default_architect() -> Self {
        Self {
            interval_minutes: 40,
            timeout_minutes: 30,
            immediate: false,
            prompt_template: "prompts-architect/ARCHITECT.md".to_string(),
            lock: LockConfig::default(),
        }
    }

    /// Create default janitor agent configuration
    pub fn default_janitor() -> Self {
        Self {
            interval_minutes: 20,
            timeout_minutes: 15,
            immediate: false,
            prompt_template: "prompts-janitor/JANITOR.md".to_string(),
            lock: LockConfig::default(),
        }
    }

    /// Create default prompt agent configuration
    pub fn default_prompt() -> Self {
        Self {
            interval_minutes: 5,
            timeout_minutes: 3,
            immediate: false,
            prompt_template: "prompts-prompt/PROMPT.md".to_string(),
            lock: LockConfig::default(),
        }
    }

    /// Create default code review agent configuration
    pub fn default_code_review() -> Self {
        Self {
            interval_minutes: 30,
            timeout_minutes: 25,
            immediate: false,
            prompt_template: String::new(), // Code review doesn't use prompt templates
            lock: LockConfig::default(),
        }
    }
}

/// Lock configuration for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockConfig {
    /// Maximum number of retry attempts for lock acquisition
    #[serde(default = "default_lock_max_retries")]
    pub max_retries: u32,

    /// Timeout for lock acquisition in seconds
    #[serde(default = "default_lock_timeout_seconds")]
    pub timeout_seconds: u64,
}

impl Default for LockConfig {
    fn default() -> Self {
        Self {
            max_retries: default_lock_max_retries(),
            timeout_seconds: default_lock_timeout_seconds(),
        }
    }
}

fn default_lock_max_retries() -> u32 {
    3
}

fn default_lock_timeout_seconds() -> u64 {
    30
}

/// Parallel mode configuration for agents.
///
/// This configuration enables multiple parallel instances of the same agent type,
/// allowing for better resource utilization and task distribution. When parallel mode
/// is enabled, agents use heartbeats to signal their presence, and stale agents are
/// automatically removed from the registry.
///
/// ## Example TOML Configuration
///
/// ```toml
/// [parallel]
/// enabled = true
/// heartbeat_interval = "30s"
/// stale_agent_threshold = "90s"
/// task_lease_duration = "10m"
/// max_instances_per_type = 5
/// completed_task_retention = "7d"
/// stale_agent_retention = "1h"
/// ```
///
/// ## Configuration Fields
///
/// - **enabled**: Enable parallel mode for agents (allows multiple instances)
/// - **heartbeat_interval**: Duration between agent heartbeats (default: 30 seconds)
/// - **stale_agent_threshold**: Threshold for marking an agent as stale (default: 90 seconds)
/// - **task_lease_duration**: Default lease duration for task assignments (default: 10 minutes)
/// - **max_instances_per_type**: Maximum number of parallel instances per agent type
/// - **completed_task_retention**: Duration after which completed tasks are cleaned up (default: 7 days)
/// - **stale_agent_retention**: Duration after which stale agents are removed (default: 1 hour)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelConfig {
    /// Enable parallel mode for agents (allows multiple instances)
    #[serde(default = "default_parallel_enabled")]
    pub enabled: bool,

    /// Duration between agent heartbeats (default: 30 seconds)
    #[serde(with = "duration_serde", default = "default_heartbeat_interval")]
    pub heartbeat_interval: Duration,

    /// Threshold for marking an agent as stale (default: 90 seconds)
    #[serde(with = "duration_serde", default = "default_stale_agent_threshold")]
    pub stale_agent_threshold: Duration,

    /// Default lease duration for task assignments (default: 10 minutes)
    #[serde(with = "duration_serde", default = "default_task_lease_duration")]
    pub task_lease_duration: Duration,

    /// Maximum number of parallel instances per agent type
    #[serde(default)]
    pub max_instances_per_type: Option<usize>,

    /// Duration after which completed tasks are cleaned up (default: 7 days)
    #[serde(with = "duration_serde", default = "default_completed_task_retention")]
    pub completed_task_retention: Duration,

    /// Duration after which stale agents are removed (default: 1 hour)
    #[serde(with = "duration_serde", default = "default_stale_agent_retention")]
    pub stale_agent_retention: Duration,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            enabled: default_parallel_enabled(),
            heartbeat_interval: default_heartbeat_interval(),
            stale_agent_threshold: default_stale_agent_threshold(),
            task_lease_duration: default_task_lease_duration(),
            max_instances_per_type: None,
            completed_task_retention: default_completed_task_retention(),
            stale_agent_retention: default_stale_agent_retention(),
        }
    }
}

// Default value functions for ParallelConfig fields
fn default_parallel_enabled() -> bool {
    false // Backward compatibility - single instance mode by default
}

fn default_heartbeat_interval() -> Duration {
    Duration::from_secs(30)
}

fn default_stale_agent_threshold() -> Duration {
    Duration::from_secs(90)
}

fn default_task_lease_duration() -> Duration {
    Duration::from_secs(10 * 60) // 10 minutes
}

fn default_completed_task_retention() -> Duration {
    Duration::from_secs(7 * 24 * 60 * 60) // 7 days
}

fn default_stale_agent_retention() -> Duration {
    Duration::from_secs(60 * 60) // 1 hour
}

/// Serde serialization/deserialization module for Duration types.
///
/// This module provides custom serialization and deserialization for `std::time::Duration`
/// to human-readable string formats like "30s", "10m", "1h", "7d".
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let seconds = duration.as_secs();
        let nanos = duration.subsec_nanos();

        if nanos == 0 {
            // Whole seconds - use largest sensible unit
            if seconds % (24 * 3600) == 0 && seconds >= 24 * 3600 {
                // Days
                serializer.serialize_str(&format!("{}d", seconds / (24 * 3600)))
            } else if seconds % 3600 == 0 && seconds >= 3600 {
                // Hours
                serializer.serialize_str(&format!("{}h", seconds / 3600))
            } else if seconds % 60 == 0 && seconds >= 60 {
                // Minutes
                serializer.serialize_str(&format!("{}m", seconds / 60))
            } else {
                // Seconds
                serializer.serialize_str(&format!("{}s", seconds))
            }
        } else {
            // Has nanoseconds, serialize as seconds with decimal
            let total_secs = seconds as f64 + nanos as f64 / 1_000_000_000.0;
            serializer.serialize_str(&format!("{:.3}s", total_secs))
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_duration(&s).map_err(serde::de::Error::custom)
    }

    fn parse_duration(s: &str) -> Result<Duration, String> {
        let s = s.trim();
        if s.is_empty() {
            return Err("Empty duration string".to_string());
        }

        let (num_str, unit) = if let Some(pos) = s.find(|c: char| !c.is_numeric() && c != '.') {
            (&s[..pos], &s[pos..])
        } else {
            (s, "s") // Default to seconds
        };

        let value: f64 = num_str
            .parse()
            .map_err(|e| format!("Invalid number: {}", e))?;

        if value < 0.0 {
            return Err("Duration cannot be negative".to_string());
        }

        let seconds = match unit {
            "ns" | "nano" | "nanos" | "nanoseconds" => value / 1_000_000_000.0,
            "us" | "micro" | "micros" | "microseconds" => value / 1_000_000.0,
            "ms" | "milli" | "millis" | "milliseconds" => value / 1_000.0,
            "s" | "sec" | "secs" | "second" | "seconds" => value,
            "m" | "min" | "mins" | "minute" | "minutes" => value * 60.0,
            "h" | "hour" | "hours" => value * 3600.0,
            "d" | "day" | "days" => value * 24.0 * 3600.0,
            "w" | "week" | "weeks" => value * 7.0 * 24.0 * 3600.0,
            _ => return Err(format!("Unknown duration unit: {}", unit)),
        };

        let whole_secs = seconds.floor() as u64;
        let nanos = ((seconds - seconds.floor()) * 1_000_000_000.0) as u32;

        Ok(Duration::new(whole_secs, nanos))
    }
}

/// Parse a TOML configuration file
///
/// # Arguments
///
/// * `path` - Path to the TOML configuration file
///
/// # Returns
///
/// Returns a `Result<Config, ConfigError>` containing the parsed configuration
/// or an error if the file cannot be read or parsed.
///
/// # Example
///
/// ```no_run
/// use automation_common::workspace_config::parse_config_file;
///
/// let config = parse_config_file(".automation-rust.toml").unwrap();
/// println!("Workspace path: {}", config.workspace.path);
/// ```
pub fn parse_config_file<P: AsRef<Path>>(path: P) -> ConfigResult<Config> {
    let content = fs::read_to_string(path.as_ref())?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.workspace.path, ".");
        assert_eq!(config.workspace.state_path, ".state");
        assert_eq!(config.logging.level, "info");
        assert_eq!(config.logging.format, "pretty");
        assert_eq!(config.logging.colors, true);
    }

    #[test]
    fn test_default_workspace_config() {
        let config = WorkspaceConfig::default();
        assert_eq!(config.path, ".");
        assert_eq!(config.state_path, ".state");
    }

    #[test]
    fn test_default_logging_config() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, "info");
        assert_eq!(config.format, "pretty");
        assert_eq!(config.colors, true);
    }

    #[test]
    fn test_default_lock_config() {
        let config = LockConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.timeout_seconds, 30);
    }

    #[test]
    fn test_default_agent_config() {
        let config = AgentConfig::default();
        assert_eq!(config.interval_minutes, 5);
        assert_eq!(config.timeout_minutes, 3);
        assert_eq!(config.immediate, false);
        assert!(config.prompt_template.is_empty());
    }

    #[test]
    fn test_default_architect_config() {
        let config = AgentConfig::default_architect();
        assert_eq!(config.interval_minutes, 40);
        assert_eq!(config.timeout_minutes, 30);
        assert_eq!(config.immediate, false);
        assert_eq!(
            config.prompt_template,
            "prompts-architect/ARCHITECT.md"
        );
    }

    #[test]
    fn test_default_janitor_config() {
        let config = AgentConfig::default_janitor();
        assert_eq!(config.interval_minutes, 20);
        assert_eq!(config.timeout_minutes, 15);
        assert_eq!(config.immediate, false);
        assert_eq!(config.prompt_template, "prompts-janitor/JANITOR.md");
    }

    #[test]
    fn test_default_prompt_config() {
        let config = AgentConfig::default_prompt();
        assert_eq!(config.interval_minutes, 5);
        assert_eq!(config.timeout_minutes, 3);
        assert_eq!(config.immediate, false);
        assert_eq!(config.prompt_template, "prompts-prompt/PROMPT.md");
    }

    #[test]
    fn test_default_agents_config() {
        let config = AgentsConfig::default();
        assert_eq!(config.architect.interval_minutes, 40);
        assert_eq!(config.janitor.interval_minutes, 20);
        assert_eq!(config.prompt.interval_minutes, 5);
    }

    #[test]
    fn test_parse_complete_config() {
        let toml_content = r#"
[workspace]
path = "/workspace"
state_path = "/workspace/.state"

[logging]
level = "debug"
format = "json"
colors = false

[agents.architect]
interval_minutes = 50
timeout_minutes = 40
immediate = true
prompt_template = "custom/architect.md"

[agents.architect.lock]
max_retries = 5
timeout_seconds = 60

[agents.janitor]
interval_minutes = 25
timeout_minutes = 20
immediate = false
prompt_template = "custom/janitor.md"

[agents.janitor.lock]
max_retries = 4
timeout_seconds = 45

[agents.prompt]
interval_minutes = 10
timeout_minutes = 5
immediate = true
prompt_template = "custom/prompt.md"

[agents.prompt.lock]
max_retries = 6
timeout_seconds = 90
"#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.workspace.path, "/workspace");
        assert_eq!(config.workspace.state_path, "/workspace/.state");
        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.logging.format, "json");
        assert_eq!(config.logging.colors, false);
        assert_eq!(config.agents.architect.interval_minutes, 50);
        assert_eq!(config.agents.architect.timeout_minutes, 40);
        assert_eq!(config.agents.architect.immediate, true);
        assert_eq!(config.agents.architect.prompt_template, "custom/architect.md");
        assert_eq!(config.agents.architect.lock.max_retries, 5);
        assert_eq!(config.agents.architect.lock.timeout_seconds, 60);
    }

    #[test]
    fn test_parse_minimal_config() {
        let toml_content = r#"
[workspace]

[logging]

[agents.architect]

[agents.janitor]

[agents.prompt]
"#;
        let config: Config = toml::from_str(toml_content).unwrap();
        // All fields should use defaults
        assert_eq!(config.workspace.path, ".");
        assert_eq!(config.workspace.state_path, ".state");
        assert_eq!(config.logging.level, "info");
        assert_eq!(config.logging.colors, true);
        // When an empty section is present, AgentConfig::default() is used
        assert_eq!(config.agents.architect.interval_minutes, 5);
        assert_eq!(config.agents.janitor.interval_minutes, 5);
        assert_eq!(config.agents.prompt.interval_minutes, 5);
    }

    #[test]
    fn test_parse_config_file() {
        let toml_content = r#"
[workspace]
path = "/test"
state_path = "/test/state"

[logging]
level = "warn"
format = "pretty"

[agents.architect]
interval_minutes = 60

[agents.janitor]
interval_minutes = 30

[agents.prompt]
interval_minutes = 10
"#;
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{}", toml_content).unwrap();
        let config = parse_config_file(temp_file.path()).unwrap();
        assert_eq!(config.workspace.path, "/test");
        assert_eq!(config.logging.level, "warn");
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(config.workspace.path, parsed.workspace.path);
        assert_eq!(config.logging.level, parsed.logging.level);
    }

    #[test]
    fn test_default_parallel_config() {
        let config = ParallelConfig::default();
        assert_eq!(config.enabled, false);
        assert_eq!(config.heartbeat_interval, Duration::from_secs(30));
        assert_eq!(config.stale_agent_threshold, Duration::from_secs(90));
        assert_eq!(config.task_lease_duration, Duration::from_secs(600));
        assert_eq!(config.max_instances_per_type, None);
        assert_eq!(config.completed_task_retention, Duration::from_secs(604800)); // 7 days
        assert_eq!(config.stale_agent_retention, Duration::from_secs(3600)); // 1 hour
    }

    #[test]
    fn test_config_with_parallel_disabled() {
        let config = Config::default();
        assert!(!config.is_parallel_mode_enabled());
        assert_eq!(config.parallel.enabled, false);
    }

    #[test]
    fn test_parse_parallel_config_enabled() {
        let toml_content = r#"
[workspace]
path = "."
state_path = ".state"

[logging]
level = "info"

[agents.architect]

[parallel]
enabled = true
heartbeat_interval = "30s"
stale_agent_threshold = "90s"
task_lease_duration = "10m"
max_instances_per_type = 5
completed_task_retention = "7d"
stale_agent_retention = "1h"
"#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.parallel.enabled, true);
        assert_eq!(config.parallel.heartbeat_interval, Duration::from_secs(30));
        assert_eq!(config.parallel.stale_agent_threshold, Duration::from_secs(90));
        assert_eq!(config.parallel.task_lease_duration, Duration::from_secs(600));
        assert_eq!(config.parallel.max_instances_per_type, Some(5));
        assert_eq!(config.parallel.completed_task_retention, Duration::from_secs(604800));
        assert_eq!(config.parallel.stale_agent_retention, Duration::from_secs(3600));
        assert!(config.is_parallel_mode_enabled());
    }

    #[test]
    fn test_parse_config_without_parallel_section() {
        let toml_content = r#"
[workspace]
path = "."
state_path = ".state"

[logging]
level = "info"

[agents.architect]
"#;
        let config: Config = toml::from_str(toml_content).unwrap();
        // Should use default values
        assert_eq!(config.parallel.enabled, false);
        assert_eq!(config.parallel.heartbeat_interval, Duration::from_secs(30));
        assert_eq!(config.parallel.stale_agent_threshold, Duration::from_secs(90));
        assert!(!config.is_parallel_mode_enabled());
    }

    #[test]
    fn test_parallel_duration_formats() {
        // Test various duration formats
        let test_cases = vec![
            ("30s", 30),
            ("60s", 60),
            ("5m", 5 * 60),
            ("10m", 10 * 60),
            ("1h", 60 * 60),
            ("2h", 2 * 60 * 60),
            ("1d", 24 * 60 * 60),
            ("7d", 7 * 24 * 60 * 60),
        ];

        for (duration_str, expected_secs) in test_cases {
            let toml_content = format!(
                r#"
[workspace]
path = "."
state_path = ".state"

[logging]
level = "info"

[agents.architect]

[parallel]
enabled = true
heartbeat_interval = "{}"
"#,
                duration_str
            );
            let config: Config = toml::from_str(&toml_content).unwrap();
            assert_eq!(config.parallel.heartbeat_interval, Duration::from_secs(expected_secs));
        }
    }

    #[test]
    fn test_parallel_config_serialization() {
        let parallel = ParallelConfig {
            enabled: true,
            heartbeat_interval: Duration::from_secs(30),
            stale_agent_threshold: Duration::from_secs(90),
            task_lease_duration: Duration::from_secs(600),
            max_instances_per_type: Some(5),
            completed_task_retention: Duration::from_secs(604800),
            stale_agent_retention: Duration::from_secs(3600),
        };
        let toml_str = toml::to_string(&parallel).unwrap();
        let parsed: ParallelConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parallel.enabled, parsed.enabled);
        assert_eq!(parallel.heartbeat_interval, parsed.heartbeat_interval);
        assert_eq!(parallel.max_instances_per_type, parsed.max_instances_per_type);
    }

    #[test]
    fn test_parallel_max_instances_none() {
        let toml_content = r#"
[workspace]
path = "."
state_path = ".state"

[logging]
level = "info"

[agents.architect]

[parallel]
enabled = true
"#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.parallel.max_instances_per_type, None);
    }

    #[test]
    fn test_parallel_partial_config() {
        let toml_content = r#"
[workspace]
path = "."
state_path = ".state"

[logging]
level = "info"

[agents.architect]

[parallel]
enabled = true
heartbeat_interval = "60s"
"#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.parallel.enabled, true);
        assert_eq!(config.parallel.heartbeat_interval, Duration::from_secs(60));
        // Other fields should use defaults
        assert_eq!(config.parallel.stale_agent_threshold, Duration::from_secs(90));
        assert_eq!(config.parallel.task_lease_duration, Duration::from_secs(600));
        assert_eq!(config.parallel.completed_task_retention, Duration::from_secs(604800));
        assert_eq!(config.parallel.stale_agent_retention, Duration::from_secs(3600));
    }

    #[test]
    fn test_config_with_parallel_includes_workspace() {
        let toml_content = r#"
[workspace]
path = "/workspace"
state_path = "/workspace/.state"

[logging]
level = "debug"

[agents.architect]

[parallel]
enabled = true
"#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.workspace.path, "/workspace");
        assert_eq!(config.workspace.state_path, "/workspace/.state");
        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.parallel.enabled, true);
    }
}

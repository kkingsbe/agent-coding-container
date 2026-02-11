//! # Agent Bootstrap Module
//!
//! This module provides common initialization logic for all agent binaries.
//! It eliminates code duplication across architect.rs, janitor.rs, and prompt.rs
//! by providing a shared bootstrap function that handles:
//!
//! - Config file path determination and validation
//! - TOML config loading
//! - Workspace and state path extraction
//! - Logging initialization
//! - Workspace path resolution
//! - Agent configuration creation
//!
//! ## Usage
//!
//! Each agent binary can be reduced to ~20 lines:
//!
//! ```no_run
//! use automation_agents::{bootstrap, AgentRunner, AgentType};
//! use clap::Parser;
//!
//! #[derive(Parser, Debug)]
//! struct Args {
//!     #[arg(long)]
//!     config: Option<String>,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let args = Args::parse();
//!
//!     let config = bootstrap::bootstrap_agent(
//!         AgentType::Architect,
//!         args.config,
//!         |cfg| &cfg.agents.architect,
//!     )?;
//!
//!     let runner = AgentRunner::new(config);
//!     runner.run().await?;
//!
//!     Ok(())
//! }
//! ```

use crate::agent::{AgentConfig, AgentType};
use automation_common::workspace_config;
use std::path::{Path, PathBuf};

/// Bootstraps an agent by loading configuration and creating an AgentConfig.
///
/// This function handles all common initialization logic shared across
/// all agent binaries, including:
///
/// 1. Config file path determination (uses ".automation-rust.toml" if not provided)
/// 2. Config file existence validation
/// 3. TOML configuration loading
/// 4. Workspace and state path extraction with relative/absolute handling
/// 5. Logging initialization
/// 6. Workspace path resolution with canonicalization
/// 7. Agent-specific settings extraction via closure
/// 8. AgentConfig creation
/// 9. Configuration logging
///
/// # Arguments
///
/// * `agent_type` - The type of agent being bootstrapped (Architect, Janitor, or Prompt)
/// * `config_path` - Optional path to the TOML config file (defaults to ".automation-rust.toml")
/// * `extract_agent_settings` - A closure that extracts agent-specific settings from the TOML config
///
/// # Type Parameters
///
/// * `F` - A closure type that takes a reference to [`Config`] and returns a reference
///         to an [`AgentConfig`](workspace_config::AgentConfig)
///
/// # Returns
///
/// * `Ok(AgentConfig)` - The fully configured agent configuration ready for use
/// * `Err(Box<dyn std::error::Error>)` - Any error that occurred during bootstrapping
///
/// # Errors
///
/// This function returns an error if:
/// - The config file is not found
/// - The TOML config cannot be parsed
/// - The workspace path cannot be resolved
///
/// # Example
///
/// ```no_run
/// use automation_agents::{bootstrap, AgentType};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = bootstrap::bootstrap_agent(
///     AgentType::Architect,
///     Some("/path/to/config.toml".to_string()),
///     |cfg| &cfg.agents.architect,
/// )?;
/// # Ok(())
/// # }
/// ```
pub fn bootstrap_agent<F>(
    agent_type: AgentType,
    config_path: Option<String>,
    extract_agent_settings: F,
) -> Result<AgentConfig, Box<dyn std::error::Error>>
where
    F: FnOnce(&workspace_config::Config) -> &workspace_config::AgentConfig,
{
    // Determine config file path
    let config_path = config_path.unwrap_or_else(|| ".automation-rust.toml".to_string());
    let config_path_buf = PathBuf::from(&config_path);

    // Check if config file exists
    if !config_path_buf.exists() {
        return Err(format!(
            "Config file not found: {}. Please create a .automation-rust.toml file or specify a custom path with --config.",
            config_path
        ).into());
    }

    // Load the TOML configuration
    let toml_config = workspace_config::parse_config_file(&config_path_buf)
        .map_err(|e| format!("Failed to load config from {}: {}", config_path, e))?;

    // Extract workspace and state paths
    let workspace_path = PathBuf::from(&toml_config.workspace.path);
    let state_dir = if toml_config.workspace.state_path == ".state" {
        workspace_path.join(".state")
    } else {
        // Resolve relative to config file directory
        let base_dir = config_path_buf.parent().unwrap_or_else(|| Path::new("."));
        if PathBuf::from(&toml_config.workspace.state_path).is_absolute() {
            PathBuf::from(&toml_config.workspace.state_path)
        } else {
            base_dir.join(&toml_config.workspace.state_path)
        }
    };

    // Extract agent-specific settings using the provided closure
    let agent_settings = extract_agent_settings(&toml_config);

    // Initialize logging (use try_init to avoid panicking if already set up in tests)
    let log_level = match toml_config.logging.level.to_lowercase().as_str() {
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };

    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(log_level.into()),
        )
        .try_init();

    // Log agent-specific startup info
    tracing::info!(
        "{} agent starting with interval: {} minutes, timeout: {} minutes",
        agent_type,
        agent_settings.interval_minutes,
        agent_settings.timeout_minutes
    );

    // Resolve workspace path if relative (relative to config file directory)
    let workspace = if workspace_path.is_absolute() {
        workspace_path.to_string_lossy().to_string()
    } else {
        let base_dir = config_path_buf.parent().unwrap_or_else(|| Path::new("."));
        base_dir.join(&workspace_path)
            .canonicalize()
            .unwrap_or_else(|_| base_dir.join(&workspace_path))
            .to_string_lossy()
            .to_string()
    };

    // Create agent configuration from TOML settings
    let config = AgentConfig {
        agent_type,
        workspace: workspace.clone(),
        state_dir: state_dir.to_string_lossy().to_string(),
        prompt_template: agent_settings.prompt_template.clone(),
        interval_seconds: agent_settings.interval_minutes * 60, // Convert minutes to seconds
        timeout_seconds: agent_settings.timeout_minutes * 60, // Convert minutes to seconds
        immediate: agent_settings.immediate,
        max_lock_retries: agent_settings.lock.max_retries,
        lock_timeout_seconds: agent_settings.lock.timeout_seconds,
    };

    // Log the loaded configuration
    tracing::info!(
        workspace = %workspace,
        state_dir = %config.state_dir,
        interval_seconds = config.interval_seconds,
        timeout_seconds = config.timeout_seconds,
        immediate = config.immediate,
        "Agent configuration loaded"
    );

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Creates a temporary config file with default agent configurations
    fn create_test_config() -> NamedTempFile {
        let mut temp_file = NamedTempFile::new().unwrap();
        let config_content = r#"
[workspace]
path = "."
state_path = ".state"

[logging]
level = "info"
format = "pretty"
colors = true

[agents.architect]
interval_minutes = 40
timeout_minutes = 30
immediate = false
prompt_template = "prompts-architect/ARCHITECT.md"

[agents.architect.lock]
max_retries = 3
timeout_seconds = 30

[agents.janitor]
interval_minutes = 20
timeout_minutes = 15
immediate = false
prompt_template = "prompts-janitor/JANITOR.md"

[agents.janitor.lock]
max_retries = 3
timeout_seconds = 30

[agents.prompt]
interval_minutes = 5
timeout_minutes = 3
immediate = false
prompt_template = "prompts-prompt/PROMPT.md"

[agents.prompt.lock]
max_retries = 3
timeout_seconds = 30
"#;
        temp_file.write_all(config_content.as_bytes()).unwrap();
        temp_file
    }

    #[test]
    fn test_bootstrap_agent_architect() {
        let temp_config = create_test_config();
        let config_path = temp_config.path().to_string_lossy().to_string();

        // Create a minimal workspace directory for path resolution
        let workspace_dir = temp_config.path().parent().unwrap().join("workspace");
        fs::create_dir_all(&workspace_dir).unwrap();

        // Change directory to parent of config for relative path resolution
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_config.path().parent().unwrap()).unwrap();

        let result = bootstrap_agent(
            AgentType::Architect,
            Some(config_path),
            |cfg| &cfg.agents.architect,
        );

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.agent_type, AgentType::Architect);
        assert_eq!(config.interval_seconds, 2400); // 40 * 60
        assert_eq!(config.timeout_seconds, 1800); // 30 * 60
        assert!(!config.immediate);
        assert_eq!(config.max_lock_retries, 3);
        assert_eq!(config.lock_timeout_seconds, 30);
    }

    #[test]
    fn test_bootstrap_agent_janitor() {
        let temp_config = create_test_config();
        let config_path = temp_config.path().to_string_lossy().to_string();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_config.path().parent().unwrap()).unwrap();

        let result = bootstrap_agent(
            AgentType::Janitor,
            Some(config_path),
            |cfg| &cfg.agents.janitor,
        );

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.agent_type, AgentType::Janitor);
        assert_eq!(config.interval_seconds, 1200); // 20 * 60
        assert_eq!(config.timeout_seconds, 900); // 15 * 60
    }

    #[test]
    fn test_bootstrap_agent_prompt() {
        let temp_config = create_test_config();
        let config_path = temp_config.path().to_string_lossy().to_string();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_config.path().parent().unwrap()).unwrap();

        let result = bootstrap_agent(
            AgentType::Prompt,
            Some(config_path),
            |cfg| &cfg.agents.prompt,
        );

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.agent_type, AgentType::Prompt);
        assert_eq!(config.interval_seconds, 300); // 5 * 60
        assert_eq!(config.timeout_seconds, 180); // 3 * 60
    }

    #[test]
    fn test_bootstrap_agent_config_not_found() {
        let result = bootstrap_agent(
            AgentType::Architect,
            Some("/nonexistent/path/to/config.toml".to_string()),
            |cfg| &cfg.agents.architect,
        );

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Config file not found"));
    }

    #[test]
    fn test_bootstrap_agent_invalid_toml() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"invalid toml content [[[").unwrap();
        let config_path = temp_file.path().to_string_lossy().to_string();

        let result = bootstrap_agent(
            AgentType::Architect,
            Some(config_path),
            |cfg| &cfg.agents.architect,
        );

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Failed to load config"));
    }
}

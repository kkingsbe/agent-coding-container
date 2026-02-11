//! # automation-agents
//!
//! Agent entry points for the automation-rust project.
//!
//! This crate provides the main entry points for the three agent types:
//! - `architect` - Long-running planning agent (40 min interval)
//! - `janitor` - Cleanup and maintenance agent (20 min interval)
//! - `prompt` - Quick task execution agent (5 min interval)
//!
//! Each agent coordinates through shared workspace and state directories,
//! running as an independent scheduled process.

pub mod agent;
pub mod bootstrap;

// Re-export shared agent functionality
pub use agent::{AgentRunner, AgentConfig, AgentType, HandlerOutput, write_handler_output};
// Re-export bootstrap function for agent binaries
pub use bootstrap::bootstrap_agent;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}

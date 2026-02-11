//! # Architect Agent
//!
//! The architect agent is a long-running planning agent that:
//! - Runs on a 40-minute interval by default
//! - Reads workspace files for context
//! - Executes planning and architecture tasks
//! - Writes output to prompts-architect directory

use automation_agents::{AgentRunner, AgentType, bootstrap_agent};
use clap::Parser;

/// Architect Agent - Long-running planning and architecture agent
#[derive(Parser, Debug)]
#[command(name = "architect")]
#[command(about = "Architect agent for automation-rust", long_about = None)]
struct Args {
    /// Path to the TOML configuration file
    /// Defaults to .automation-rust.toml in the current directory if not provided
    #[arg(long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Bootstrap the agent using the shared initialization logic
    let config = bootstrap_agent(
        AgentType::Architect,
        args.config,
        |cfg| &cfg.agents.architect,
    )?;

    // Create and run agent
    let runner = AgentRunner::new(config);
    runner.run().await?;

    Ok(())
}

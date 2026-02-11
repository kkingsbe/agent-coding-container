//! # Janitor Agent
//!
//! The janitor agent is a cleanup and maintenance agent that:
//! - Runs on a 20-minute interval by default
//! - Performs cleanup tasks
//! - Maintains workspace health
//! - Writes output to prompts-janitor directory

use automation_agents::{AgentRunner, AgentType, bootstrap_agent};
use clap::Parser;

/// Janitor Agent - Cleanup and maintenance agent
#[derive(Parser, Debug)]
#[command(name = "janitor")]
#[command(about = "Janitor agent for automation-rust", long_about = None)]
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
        AgentType::Janitor,
        args.config,
        |cfg| &cfg.agents.janitor,
    )?;

    // Create and run agent
    let runner = AgentRunner::new(config);
    runner.run().await?;

    Ok(())
}

//! # Prompt Agent
//!
//! The prompt agent is a quick task execution agent that:
//! - Runs on a 5-minute interval by default
//! - Executes quick tasks and operations
//! - Writes output to prompts-prompt directory

use automation_agents::{AgentRunner, AgentType, bootstrap_agent};
use clap::Parser;

/// Prompt Agent - Quick task execution agent
#[derive(Parser, Debug)]
#[command(name = "prompt")]
#[command(about = "Prompt agent for automation-rust", long_about = None)]
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
        AgentType::Prompt,
        args.config,
        |cfg| &cfg.agents.prompt,
    )?;

    // Create and run agent
    let runner = AgentRunner::new(config);
    runner.run().await?;

    Ok(())
}

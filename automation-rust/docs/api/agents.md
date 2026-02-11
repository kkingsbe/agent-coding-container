# automation-agents API Reference

The `automation-agents` crate provides agent implementations and common agent behavior for the gastown system.

## Table of Contents

- [Overview](#overview)
- [Agent Trait](#agent-trait)
- [Agent Implementations](#agent-implementations)
- [CLI Arguments](#cli-arguments)
- [Usage Examples](#usage-examples)

## Overview

`automation-agents` provides:

- **Agent trait** - Common interface for all agent types
- **Three agent implementations** - Architect, Janitor, and Prompt agents
- **CLI argument parsing** - Consistent command-line interface
- **Agent-specific logic** - Each agent has its own execution behavior
- **Interval-based scheduling** - Agents run at different intervals

## Agent Trait

### `Agent`

The common trait that all agents implement.

```rust
#[async_trait]
pub trait Agent: Send + Sync {
    /// Get the agent type name
    fn agent_type(&self) -> &str;
    
    /// Get the default execution interval
    fn default_interval(&self) -> Duration;
    
    /// Execute the agent once
    async fn execute(&self) -> Result<TaskResult, AutomationError>;
    
    /// Run the agent with scheduling
    async fn run(&self) -> Result<(), AutomationError>;
    
    /// Run the agent once without scheduling
    async fn run_once(&self) -> Result<TaskResult, AutomationError>;
}
```

### Trait Methods

#### `agent_type()`

Get the agent type name.

```rust
fn agent_type(&self) -> &str
```

**Returns:**

- `&str` - Agent type ("architect", "janitor", or "prompt")

#### `default_interval()`

Get the default execution interval.

```rust
fn default_interval(&self) -> Duration
```

**Returns:**

- `Duration` - Default interval between executions

**Default Intervals:**

| Agent | Interval |
|-------|----------|
| Architect | 40 minutes |
| Janitor | 20 minutes |
| Prompt | 5 minutes |

#### `execute()`

Execute the agent once.

```rust
async fn execute(&self) -> Result<TaskResult, AutomationError>
```

**Returns:**

- `Result<TaskResult, AutomationError>` - Execution result or error

#### `run()`

Run the agent with scheduling (runs periodically).

```rust
async fn run(&self) -> Result<(), AutomationError>
```

**Returns:**

- `Result<(), AutomationError>` - Success or error

**Behavior:**

1. Acquire lock for this agent type
2. Load state
3. Execute agent-specific logic
4. Update workspace
5. Save state
6. Release lock
7. Wait for interval
8. Repeat until shutdown

#### `run_once()`

Run the agent once without scheduling.

```rust
async fn run_once(&self) -> Result<TaskResult, AutomationError>
```

**Returns:**

- `Result<TaskResult, AutomationError>` - Execution result or error

## Agent Implementations

### `ArchitectAgent`

The architect agent handles long-running planning and architecture tasks.

```rust
pub struct ArchitectAgent {
    config: Config,
}
```

**Purpose:** Generate plans and architectural decisions for the project.

**Default Interval:** 40 minutes

**Agent Type:** "architect"

#### `ArchitectAgent::new()`

Create a new architect agent.

```rust
impl ArchitectAgent {
    pub fn new(config: Config) -> Self
}
```

**Parameters:**

- `config` - Configuration for the agent

**Returns:**

- `ArchitectAgent` - New agent instance

**Example:**

```rust
use automation_agents::ArchitectAgent;
use automation_common::Config;

let config = Config::from_env()?;
let agent = ArchitectAgent::new(config);
agent.run().await?;
```

### `JanitorAgent`

The janitor agent handles cleanup and maintenance tasks.

```rust
pub struct JanitorAgent {
    config: Config,
}
```

**Purpose:** Clean up completed tasks, archive old files, and maintain workspace health.

**Default Interval:** 20 minutes

**Agent Type:** "janitor"

#### `JanitorAgent::new()`

Create a new janitor agent.

```rust
impl JanitorAgent {
    pub fn new(config: Config) -> Self
}
```

**Parameters:**

- `config` - Configuration for the agent

**Returns:**

- `JanitorAgent` - New agent instance

**Example:**

```rust
use automation_agents::JanitorAgent;
use automation_common::Config;

let config = Config::from_env()?;
let agent = JanitorAgent::new(config);
agent.run().await?;
```

### `PromptAgent`

The prompt agent handles quick task execution.

```rust
pub struct PromptAgent {
    config: Config,
}
```

**Purpose:** Execute quick tasks from the TODO list and update workspace.

**Default Interval:** 5 minutes

**Agent Type:** "prompt"

#### `PromptAgent::new()`

Create a new prompt agent.

```rust
impl PromptAgent {
    pub fn new(config: Config) -> Self
}
```

**Parameters:**

- `config` - Configuration for the agent

**Returns:**

- `PromptAgent` - New agent instance

**Example:**

```rust
use automation_agents::PromptAgent;
use automation_common::Config;

let config = Config::from_env()?;
let agent = PromptAgent::new(config);
agent.run().await?;
```

## CLI Arguments

### Common CLI Arguments

All agent binaries accept the following command-line arguments:

| Argument | Short | Required | Default | Description |
|----------|-------|----------|---------|-------------|
| `--workspace` | `-w` | Yes | - | Path to workspace directory |
| `--state` | `-s` | No | `<workspace>/.state` | Path to state directory |
| `--interval` | `-i` | No | Agent-specific | Execution interval in seconds |
| `--timeout` | `-t` | No | 1800 | Execution timeout in seconds |
| `--immediate` | `-I` | No | false | Run immediately on start |

### Using CLI Arguments

```bash
# Basic usage
./architect --workspace /path/to/workspace

# With all options
./architect \
    --workspace /workspace \
    --state /workspace/.state \
    --interval 2400 \
    --timeout 1800 \
    --immediate

# Short options
./prompt -w /workspace -s /workspace/.state -i 300 -t 900 -I
```

### Environment Variables

CLI arguments can also be set via environment variables:

| Environment Variable | Corresponds To |
|----------------------|----------------|
| `AUTOMATION_WORKSPACE` | `--workspace` |
| `AUTOMATION_STATE` | `--state` |
| `AUTOMATION_INTERVAL` | `--interval` |
| `AUTOMATION_TIMEOUT` | `--timeout` |
| `AUTOMATION_IMMEDIATE` | `--immediate` |

**Note:** CLI arguments take precedence over environment variables.

## Usage Examples

### Running Agents Directly

```bash
# Run architect agent
cd automation-rust
cargo run --bin architect -- --workspace /path/to/workspace

# Run janitor agent
cargo run --bin janitor -- --workspace /path/to/workspace --state /path/to/state

# Run prompt agent
cargo run --bin prompt -- -w /workspace -i 300 -I
```

### Running Release Binaries

```bash
# Build release binaries
cargo build --release

# Run architect
./target/release/architect --workspace /workspace

# Run janitor
./target/release/janitor --workspace /workspace

# Run prompt
./target/release/prompt --workspace /workspace
```

### Using in Code

```rust
use automation_agents::{ArchitectAgent, JanitorAgent, PromptAgent};
use automation_common::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    
    // Run prompt agent once
    let prompt_agent = PromptAgent::new(config.clone());
    let result = prompt_agent.run_once().await?;
    println!("Prompt agent result: {:?}", result);
    
    // Run janitor agent once
    let janitor_agent = JanitorAgent::new(config.clone());
    let result = janitor_agent.run_once().await?;
    println!("Janitor agent result: {:?}", result);
    
    // Run architect agent with scheduling
    let architect_agent = ArchitectAgent::new(config);
    architect_agent.run().await?;
    
    Ok(())
}
```

### Running Multiple Agents Concurrently

```rust
use automation_agents::{PromptAgent, JanitorAgent};
use automation_common::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    
    // Run prompt and janitor concurrently
    tokio::try_join!(
        PromptAgent::new(config.clone()).run(),
        JanitorAgent::new(config.clone()).run(),
    )?;
    
    Ok(())
}
```

### Custom Agent Configuration

```rust
use automation_agents::PromptAgent;
use automation_common::Config;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::new("/workspace", "/workspace/.state")
        .with_interval(Duration::from_secs(300))
        .with_timeout(Duration::from_secs(900))
        .with_immediate(true);
    
    let agent = PromptAgent::new(config);
    agent.run().await?;
    
    Ok(())
}
```

### Running with Shutdown Signal

```rust
use automation_agents::PromptAgent;
use automation_common::Config;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let agent = PromptAgent::new(config);
    
    // Run with Ctrl+C handling
    tokio::select! {
        result = agent.run() => result?,
        _ = signal::ctrl_c() => {
            println!("Received shutdown signal");
        }
    }
    
    Ok(())
}
```

### Agent State Management

```rust
use automation_agents::PromptAgent;
use automation_state::State;
use automation_common::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let agent = PromptAgent::new(config);
    
    // Run agent
    let result = agent.run_once().await?;
    
    // Load and inspect state
    let state = State::load("/workspace/.state/prompt.json").await?;
    println!("Last run: {}", state.last_run);
    println!("Status: {}", state.status);
    println!("Tasks processed: {}", state.tasks_processed);
    
    Ok(())
}
```

## Agent Execution Flow

All agents follow a similar execution flow:

```
┌─────────────────────────────────────┐
│        Agent Started                │
└──────────────┬──────────────────────┘
               │
               │ 1. Parse CLI args / env
               ▼
┌─────────────────────────────────────┐
│      Load Configuration             │
└──────────────┬──────────────────────┘
               │
               │ 2. Initialize logging
               ▼
┌─────────────────────────────────────┐
│       Initialize Logging            │
└──────────────┬──────────────────────┘
               │
               │ 3. Acquire lock
               ▼
┌─────────────────────────────────────┐
│      Acquire Exclusive Lock        │
│    (agent_type.lock)               │
└──────────────┬──────────────────────┘
               │
               │ 4. Load state
               ▼
┌─────────────────────────────────────┐
│        Load Agent State            │
│    (agent_type.json)               │
└──────────────┬──────────────────────┘
               │
               │ 5. Read workspace
               ▼
┌─────────────────────────────────────┐
│       Read Workspace Files         │
│   (TODO.md, BACKLOG.md, etc.)     │
└──────────────┬──────────────────────┘
               │
               │ 6. Execute agent logic
               ▼
┌─────────────────────────────────────┐
│     Agent-Specific Execution        │
│  (varies by agent type)            │
└──────────────┬──────────────────────┘
               │
               │ 7. Update workspace
               ▼
┌─────────────────────────────────────┐
│       Write Updated Files          │
└──────────────┬──────────────────────┘
               │
               │ 8. Save state
               ▼
┌─────────────────────────────────────┐
│       Save Agent State             │
└──────────────┬──────────────────────┘
               │
               │ 9. Release lock
               ▼
┌─────────────────────────────────────┐
│       Release Lock                 │
└──────────────┬──────────────────────┘
               │
               │ 10. Wait for interval
               ▼
┌─────────────────────────────────────┐
│         Wait for Interval           │
│    (unless --immediate or run_once) │
└─────────────────────────────────────┘
```

## Agent-Specific Behavior

### Architect Agent

**Purpose:** Generate plans and architectural decisions

**Typical Tasks:**
- Read PRD.md and project requirements
- Generate architecture plans
- Create task breakdowns
- Update PLANS.md or similar

**Execution:**
```bash
./architect --workspace /workspace
```

### Janitor Agent

**Purpose:** Clean up and maintain workspace

**Typical Tasks:**
- Archive completed tasks from COMPLETED.md
- Clean up old state files
- Remove duplicate entries
- Maintain workspace health

**Execution:**
```bash
./janitor --workspace /workspace
```

### Prompt Agent

**Purpose:** Execute quick tasks

**Typical Tasks:**
- Read tasks from TODO.md
- Execute tasks using prompt templates
- Update task status
- Move completed tasks to COMPLETED.md

**Execution:**
```bash
./prompt --workspace /workspace
```

## Error Handling

### Common Agent Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `AutomationError::Lock("Timeout")` | Lock already held | Check for other instances |
| `AutomationError::Workspace("Not found")` | File doesn't exist | Initialize workspace |
| `AutomationError::Execution("Timeout")` | Task took too long | Increase timeout |
| `AutomationError::Io` | Permission issues | Check file permissions |

### Error Handling Example

```rust
use automation_agents::PromptAgent;
use automation_common::{AutomationError, Config};

#[tokio::main]
async fn main() {
    let config = Config::from_env().expect("Failed to load config");
    let agent = PromptAgent::new(config);
    
    match agent.run().await {
        Ok(_) => println!("Agent completed successfully"),
        Err(AutomationError::Lock(msg)) => {
            eprintln!("Lock error: {}", msg);
            eprintln!("Check if another instance is running");
        }
        Err(AutomationError::Workspace(msg)) => {
            eprintln!("Workspace error: {}", msg);
            eprintln!("Ensure workspace is properly initialized");
        }
        Err(e) => {
            eprintln!("Error: {:?}", e);
        }
    }
}
```

## See Also

- [Architecture Documentation](../architecture.md) - System architecture overview
- [Common API](common.md) - Common types and utilities
- [State API](state.md) - State management module
- [Workspace API](workspace.md) - Workspace management module
- [Executor API](executor.md) - CLI executor module
- [Scheduler API](scheduler.md) - Task scheduler module
- [Deployment Guide](../deployment.md) - Deployment instructions

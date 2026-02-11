# gastown

![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)
![License](https://img.shields.io/badge/license-ISC-blue.svg)
![Build](https://img.shields.io/badge/build-passing-brightgreen.svg)

A Rust rewrite of the automation-parallel system, providing a high-performance parallel execution framework for AI agents with a simplified TOML-based configuration system.

## Quick Start (Get Running in 5 Minutes)

**Multi-Repo Design:** This project is designed to be installed **once globally** and then used across **multiple different repositories**. Each repo gets its own `.automation-rust.toml` configuration file, while the installed agents are available system-wide.

**The Workflow:**
1. Install the agents once globally (`cargo install --path agents`)
2. Create a `.automation-rust.toml` config file in each repo/workspace you want to use
3. Run the agents from any directory with a config file

---

### Option 1: Docker (Easiest)

**Configuration needed:** `.env` (for Docker) + `.automation-rust.toml` (agent behavior)

```bash
# 1. Clone or navigate to the project
cd gastown

# 2. Copy the example environment file
cp .env.example .env

# 3. (Optional) Edit .env to customize Docker-specific settings
vim .env

# 4. Start all agents using Docker Compose
docker-compose up -d
```

**That's it! The agents are now running.**

To verify everything is working:

```bash
# View logs to see the agents in action
docker-compose logs -f
```

To stop the agents:

```bash
docker-compose down
```

**What Docker does:** When you run `docker-compose up -d`, Docker will:
- Build the entire Rust project from source inside a container
- Compile all three agent binaries: `architect`, `janitor`, and `prompt`
- Start three long-running containers, one for each agent
- Set up persistent Docker volumes for workspace and state data
- Install the Kilo Code CLI (npm package) for compatibility with automation-parallel

### Option 2: Local Development (Recommended for Multi-Repo Use)

**Configuration needed:** `.automation-rust.toml` only (`.env` is not needed)

```bash
# 1. Clone or navigate to the project (ONE TIME only)
cd gastown

# 2. Install the agents ONCE globally to your system
#    After this, agents are available from any directory!
cargo install --path agents

# 3. Verify installation
architect --help
janitor --help
prompt --help
```

Now create a config file and start an agent:

```bash
# 4. Create a config file in your workspace (copy from example and customize)
# Note: The .automation-rust.toml file already exists in the gastown project root.
# Copy it to your workspace directory to use it.
cp ~/gastown/.automation-rust.toml .
# Edit .automation-rust.toml to customize settings for your workspace

# 5. Start an agent
architect
```

**That's it! The agent is now running.**

You can also run the other agents:

```bash
janitor
prompt
```

**How it works:**
- The agent automatically finds `.automation-rust.toml` in the current directory
- Each agent can be configured independently in the config file
- Use the agents in any directory with a config file - no need to reinstall!

For more detailed setup instructions, see the [Configuration](#configuration) and [Running the Project](#running-the-project) sections below.

---

## Features

- **Simplified Configuration**: Use TOML configuration files (`.automation-rust.toml`) in workspace directories
- **Enhanced Performance**: Leveraging Rust's zero-cost abstractions and async runtime (Tokio)
- **Reliability**: Strong type system and ownership model eliminate common runtime errors
- **Maintainability**: Modular, well-tested codebase with clear separation of concerns
- **Feature Parity**: 100% functional compatibility with the existing Node.js implementation

---

## Prerequisites

### Required for Local Development

- **Rust** toolchain (version 1.75 or later)
  ```bash
  # Install Rust using rustup (recommended)
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source $HOME/.cargo/env
  
  # Verify installation
  rustc --version
  cargo --version
  ```

- **Git** (for cloning the repository)
  ```bash
  # Ubuntu/Debian
  sudo apt-get install git
  
  # macOS
  brew install git
  
  # Windows
  # Download from https://git-scm.com/downloads
  ```

### Required for Docker Deployment

- **Docker** (version 20.10 or later)
  ```bash
  # Verify installation
  docker --version
  ```

- **Docker Compose** (version 2.0 or later)
  ```bash
  # Verify installation
  docker-compose --version
  ```

---

## Installation

The recommended way to install gastown is using `cargo install`, which installs all agent binaries to your system.

### Primary Method: Install Agents Globally

Install all three agents (`architect`, `janitor`, `prompt`) globally on your system for easy access from any directory:

```bash
# Navigate to the gastown directory
cd gastown

# Install all three agents globally
cargo install --path agents
```

This command installs the agent binaries to your cargo bin directory:
- **Linux/macOS**: `~/.cargo/bin/`
- **Windows**: `%USERPROFILE%\.cargo\bin\`

**Important:** Ensure your cargo bin directory is in your PATH. Add to `~/.bashrc` or `~/.zshrc` (Linux/macOS):
```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

**Verify installation:**
```bash
architect --help
janitor --help
prompt --help
```

If these commands work, you're ready to run the agents from anywhere!

### Using in Multiple Repositories

Once installed globally, the agents can be used across **any number of repositories** with zero additional installation:

```bash
# Install ONCE (as shown above)
cargo install --path agents

# Use in Repo A
cd ~/projects/repo-a
cp ~/gastown/.automation-rust.toml .
vim .automation-rust.toml  # Customize for repo-a's needs
architect  # Works automatically with local config

# Use in Repo B
cd ~/projects/repo-b
cp ~/gastown/.automation-rust.toml .
vim .automation-rust.toml  # Customize for repo-b's needs
janitor  # Works automatically with local config

# Use in Repo C with different config
cd ~/projects/repo-c
vim .automation-rust.toml  # Configure directly here
prompt  # Works automatically with local config
```

**Key Benefits:**
- **One-time installation**: Install once, use everywhere
- **Per-repo configuration**: Each repo has its own `.automation-rust.toml`
- **System-wide availability**: Agents are in your PATH, available from any directory
- **Isolated execution**: Each repo's configuration affects only that repo

### Alternative Method: Build Release Binaries

Build optimized binaries for manual deployment without installing to cargo:

```bash
# Navigate to the gastown directory
cd gastown

# Build optimized release binaries
cargo build --release

# The binaries will be available at:
#   target/release/architect
#   target/release/janitor
#   target/release/prompt

# Run the binaries directly
./target/release/architect
./target/release/janitor
./target/release/prompt

# Or copy binaries to your preferred location
cp target/release/architect /usr/local/bin/
cp target/release/janitor /usr/local/bin/
cp target/release/prompt /usr/local/bin/
```

### Docker Deployment (No Installation Required)

**Note:** Docker deployment is typically designed for single-workspace use. For multi-repo usage across multiple repositories, use the global installation method ([Option 2](#option-2-local-development-recommended-for-multi-repo-use)) which provides the full "install once, use everywhere" capability.

If you need to use Docker with multiple repos, you would need to:
1. Run separate docker-compose instances with different volumes mounted for each repo
2. Use different `CONTAINER_PREFIX` values in `.env` files to avoid conflicts
3. Manage multiple docker-compose.yml files or environment configurations

For most users, the global installation method provides a simpler and more efficient multi-repo workflow.

No installation needed - run directly from Docker. Docker will build the Rust project from source using your local code.

```bash
# Build the Docker image (this compiles your Rust code from source)
docker build -t automation-rust .

# Or use docker-compose to build and run all three agents
docker-compose build
docker-compose up -d
```

**What happens during the Docker build:**
- The Dockerfile copies your entire Rust workspace (all crates) into a container
- Runs `cargo build --release --bins` to compile the three agent binaries
- Creates an Alpine-based runtime image with the compiled binaries
- Installs the Kilo Code CLI for compatibility

**What happens when you run the containers:**
- Three containers start: `architect`, `janitor`, and `prompt`
- Each container runs its respective agent with configured intervals
- Docker volumes persist workspace and state data
- All containers communicate via the `automation-network` bridge network

---

## Configuration

The gastown system uses two separate configuration files for different purposes:

### Understanding Configuration Files

This project uses two distinct configuration files because they serve different purposes:

| File | Required? | Purpose | When Used |
|------|-----------|---------|----------|
| **`.automation-rust.toml`** | **Yes** | Main configuration file controlling all agent behavior | Always (local and Docker) |
| **`.env`** | **No** (Docker only) | Environment variables for Docker deployment | Only with Docker Compose |

---

#### `.automation-rust.toml` (Required - Always Used)

**What it configures:**
- **Workspace paths** - Where agents operate and store state
- **Logging behavior** - Log levels, formats, and colors
- **Agent settings** - All interval, timeout, and prompt template configurations
- **Lock configuration** - Retry attempts and timeout values for preventing concurrent execution

**Why it's required:**
- This is the **primary configuration source** for all agents
- It contains structured, validated settings using TOML format
- All agents read this file to determine how to behave
- Works for both local execution and Docker deployments

**Scope:**
- One file **per repository/workspace**
- Each repo can have different settings
- Allows per-repo customization while using the same installed agents

**Example:**
```toml
[workspace]
path = "."
state_path = ".state"

[logging]
level = "info"
format = "pretty"

[agents.architect]
interval_minutes = 40
timeout_minutes = 30
```

---

#### `.env` (Optional - Docker Only)

**What it configures:**
- **Docker-specific settings** - Container name prefixes, workspace paths inside containers
- **Environment variables** - Passed to Rust binaries as environment variables
- **Docker deployment parameters** - Log rotation, backtrace settings

**Why it's optional:**
- Only needed when using **Docker Compose**
- For local development with `cargo install`, agents read directly from `.automation-rust.toml`
- Environment variables can override or supplement TOML settings in Docker environments

**Scope:**
- Used **per Docker deployment** (can be one per repo for multi-repo Docker setups)
- Designed for containerized deployments where environment variables are the standard configuration pattern

**Example:**
```bash
# Container name prefix (auto-set by run scripts)
CONTAINER_PREFIX=

# Workspace path inside the container
AUTOMATION_WORKSPACE=/workspace

# Agent intervals (can also be set in .automation-rust.toml)
PROMPT_INTERVAL=5
JANITOR_INTERVAL=20
ARCHITECT_INTERVAL=40

# Log level (can also be set in .automation-rust.toml)
RUST_LOG=info
```

---

#### Why Both Files Exist

The two files serve different, complementary purposes:

**`.automation-rust.toml`** provides:
- ✅ Structured, validated configuration with clear syntax
- ✅ Type-safe settings (TOML schema is validated)
- ✅ Comments and documentation within the config file itself
- ✅ Easy to read and edit by hand
- ✅ Works the same way in both local and Docker environments

**`.env`** provides:
- ✅ Docker-native configuration pattern (industry standard)
- ✅ Easy integration with container orchestration tools
- ✅ Secure handling of sensitive data (secrets) that shouldn't be in Git
- ✅ Override capability for deployment-specific settings

**The key insight:** When using Docker Compose, `.env` is primarily for **Docker-level configuration** (container names, paths) and environment variable overrides. The actual agent behavior is still primarily controlled by `.automation-rust.toml`, but `.env` values can supplement or override those settings when passed as environment variables to the containers.

---

### Configuring `.automation-rust.toml` (Primary Configuration)

The `.automation-rust.toml` file is the main configuration file that controls all agent behavior. **Each repository/workspace needs its own `.automation-rust.toml` file** - this is how you customize behavior on a per-repo basis.

#### Per-Repo Configuration Model

When using gastown across multiple repos:
- **Install once globally** (see [Installation](#installation))
- **Create a separate `.automation-rust.toml` in each repo** you want to use
- **Customize settings per-repo** - each repo can have different intervals, timeouts, paths, etc.
- **Run agents from any repo** - they automatically use that repo's config file

#### Configuration File Location

- **Default**: Agents automatically look for `.automation-rust.toml` in the current working directory
- **Custom**: Use the `--config` flag to specify a different location:
  ```bash
  architect --config /path/to/custom-config.toml
  ```

#### Creating Configuration Files for Multiple Repositories

To use gastown across multiple repositories:

```bash
# Install agents globally (one time)
cargo install --path ~/gastown/agents

# Set up Repo A
cd ~/projects/repo-a
cp ~/gastown/.automation-rust.toml .
vim .automation-rust.toml  # Customize for repo-a's needs

# Set up Repo B
cd ~/projects/repo-b
cp ~/gastown/.automation-rust.toml .
vim .automation-rust.toml  # Customize for repo-b's needs

# Set up Repo C
cd ~/projects/repo-c
# Either copy from template or create from scratch
vim .automation-rust.toml
```

#### Configuration File Sections

##### 1. Workspace Configuration

```toml
[workspace]
# Path to your project directory (default: ".")
# Use "." for current directory, or specify absolute/relative paths
path = "/home/user/my-project"

# State directory for storing agent locks and state files (default: ".state")
# Relative to the workspace path
state_path = ".state"
```

##### 2. Logging Configuration

```toml
[logging]
# Log level: trace, debug, info, warn, error (default: "info")
# Use "debug" or "trace" for troubleshooting
level = "info"

# Log format: pretty (human-readable) or json (structured) (default: "pretty")
# Use "json" for production log aggregation systems
format = "pretty"

# Enable colored output (default: true)
# Automatically disabled when not writing to a terminal
colors = true
```

##### 3. Agent Configuration

Each agent (architect, janitor, prompt) can be configured independently:

```toml
[agents.architect]
# How often to run automatically, in minutes (default: 40)
# Set to 0 to disable automatic scheduling
interval_minutes = 40

# Maximum execution time, in minutes (default: 30)
# Should be less than interval_minutes to avoid overlap
timeout_minutes = 30

# Run immediately on startup? (default: false)
# Set to true to execute once before first scheduled run
immediate = false

# Path to prompt template file (default: "prompts-architect/ARCHITECT.md")
# Relative to workspace path
prompt_template = "prompts-architect/ARCHITECT.md"

# Lock configuration (optional - has good defaults)
[agents.architect.lock]
# Max retry attempts when acquiring lock (default: 3)
max_retries = 3

# Lock timeout in seconds (default: 30)
timeout_seconds = 30
```

Similar configuration sections exist for `[agents.janitor]` and `[agents.prompt]` with their own defaults:

| Agent  | Default `interval_minutes` | Default `timeout_minutes` |
|--------|----------------------------|---------------------------|
| architect | 40 | 30 |
| janitor | 20 | 15 |
| prompt | 5 | 3 |

#### Minimal Configuration Example

The smallest valid `.automation-rust.toml` file:

```toml
[workspace]
path = "."

[logging]
level = "info"

[agents.architect]

[agents.janitor]

[agents.prompt]
```

All fields have sensible defaults, so you only need to specify what you want to customize.

### Configuring `.env` (Optional - Docker Only)

> **Note:** This file is **only needed for Docker Compose deployments**. For local development with `cargo install`, you only need `.automation-rust.toml`.

When using Docker Compose, copy the example environment file and customize it:

```bash
# Copy the example file
cp .env.example .env

# Edit the file
vim .env
```

#### Key Environment Variables

```bash
# Container name prefix (to avoid conflicts with multiple workspaces)
CONTAINER_PREFIX=

# Workspace path inside the container (typically /workspace)
AUTOMATION_WORKSPACE=/workspace

# State path for storing locks and state files
AUTOMATION_STATE=/workspace/.state

# Agent intervals (in minutes)
PROMPT_INTERVAL=5
JANITOR_INTERVAL=20
ARCHITECT_INTERVAL=40

# Agent timeouts (in minutes)
PROMPT_TIMEOUT=3
JANITOR_TIMEOUT=15
ARCHITECT_TIMEOUT=30

# Run immediately on startup (true/false)
PROMPT_IMMEDIATE=true
JANITOR_IMMEDIATE=true
ARCHITECT_IMMEDIATE=true

# Lock timeout in seconds
LOCK_TIMEOUT=30

# Log level: error, warn, info, debug, trace
RUST_LOG=info

# Enable Rust backtraces for debugging
RUST_BACKTRACE=1

# Log rotation settings
LOG_MAX_SIZE_MB=10
LOG_MAX_FILES=3
```

**Note**: When using the simplified TOML configuration (`.automation-rust.toml`), many of these environment variables can be set in the TOML file instead. The `.env` file is primarily used for Docker Compose configuration.

---

## Running the Project

### Recommended: Run Installed Agents Globally

After installing with `cargo install --path agents`, you can run agents directly from any directory that has a `.automation-rust.toml` configuration file.

**The Multi-Repo Workflow:**

```bash
# Install agents ONCE globally (if not already done)
cargo install --path ~/gastown/agents

# Use in Repo A
cd ~/projects/repo-a
# Run any agent - automatically uses repo-a's .automation-rust.toml
architect
# Or run janitor or prompt

# Use in Repo B
cd ~/projects/repo-b
# Run any agent - automatically uses repo-b's .automation-rust.toml
janitor

# Use in Repo C
cd ~/projects/repo-c
# Run any agent - automatically uses repo-c's .automation-rust.toml
prompt
```

**Additional Options:**

```bash
# Run with custom config file
architect --config /path/to/config.toml
janitor --config /path/to/config.toml
prompt --config /path/to/config.toml

# Run with debug logging
RUST_LOG=debug architect

# Run in background (for long-running tasks)
cd ~/projects/my-repo
architect &
janitor &
prompt &
```

**Key Points:**
- Agents automatically look for `.automation-rust.toml` in the current working directory
- Each repo's configuration affects only that repo when running from that directory
- The same installed agents work with any number of repos
- No re-installation needed when adding new repos

### Alternative: Running with Cargo (Local Development)

Run agents using cargo without installing globally:

```bash
# Navigate to the gastown directory
cd gastown

# Build the project (first time)
cargo build

# Run agents using cargo (will auto-discover .automation-rust.toml)
cargo run --bin architect
cargo run --bin janitor
cargo run --bin prompt

# Run with custom config file
cargo run --bin architect -- --config /path/to/config.toml

# Run with debug logging
RUST_LOG=debug cargo run --bin architect
```

### Alternative: Running Release Binaries

Run directly from the built release binaries:

```bash
# Build release binaries (faster, optimized)
cargo build --release

# Run the release binaries directly
./target/release/architect
./target/release/janitor
./target/release/prompt

# Run with custom config
./target/release/architect --config /path/to/config.toml
```

### Alternative: Running with Docker Compose

**Note on Multi-Repo Usage:** Docker Compose is primarily designed for single-workpace use. Each docker-compose instance runs agents for one workspace. For using agents across multiple repos, see the [Multi-Repo Workflow](#multi-repo-workflow) section.

**How Docker Compose works with this project:**

Docker Compose builds and runs the entire Rust automation system in containers. Here's what happens:

1. **Build Process**: The `Dockerfile` is a multi-stage build that:
   - Stage 1: Uses Rust 1.82 Alpine to compile your local Rust source code from all workspace crates (common, state, workspace, executor, scheduler, agents)
   - Runs `cargo build --release --bins` to build optimized binaries for `architect`, `janitor`, and `prompt`
   - Stage 2: Uses Alpine Linux for runtime, copying only the compiled binaries

2. **Three Services Started**:
   - **architect**: Long-running planning and architecture agent (runs every 40 minutes, 30-minute timeout)
   - **janitor**: Cleanup and maintenance agent (runs every 20 minutes, 15-minute timeout)
   - **prompt**: Quick task execution agent (runs every 5 minutes, 3-minute timeout)

3. **Persistent Data**: Docker volumes store:
   - `automation-workspace`: Shared workspace directory for all agents
   - `automation-state`: Lock files and agent state data
   - Agent-specific prompt directories for each agent's output

4. **Configuration**: Uses environment variables from `.env` instead of `.automation-rust.toml` (the config file is not mounted into containers)

```bash
# Copy and configure the environment file
cp .env.example .env
vim .env

# Build the Docker image and start all agents in the background
# This compiles your Rust code from source and runs the three agents
docker-compose up -d

# View logs for all services
docker-compose logs -f

# View logs for a specific agent
docker-compose logs -f architect
docker-compose logs -f janitor
docker-compose logs -f prompt

# Stop all agents
docker-compose down

# Stop and remove volumes (WARNING: deletes all state data)
docker-compose down -v

# Restart a specific agent
docker-compose restart architect

# Check status of all services
docker-compose ps

# Rebuild the image (use after making Rust code changes)
docker-compose build --no-cache
docker-compose up -d
```

**Important notes about Docker and the Rust project:**
- Docker DOES use your local Rust project code - it copies and compiles it from source
- Changes to the Rust code require rebuilding the Docker image with `docker-compose build`
- The containers run as a non-root user (UID 1000) named "automation"
- Each agent has resource limits defined in docker-compose.yml (CPU and memory constraints)
- The Kilo Code CLI (`@kilocode/cli@0.26.0`) is installed in the containers for compatibility with automation-parallel

### Running Multiple Agents in Parallel

#### Using Background Jobs (Shell)

```bash
# Navigate to your workspace with .automation-rust.toml
cd /path/to/workspace

# Run all agents in the background
architect &
janitor &
prompt &

# Or use a shell loop
for agent in architect janitor prompt; do
    $agent &
done
```

#### Using Process Managers (Production)

For production deployments, consider using a process manager:

**Systemd** (Linux):
```ini
# /etc/systemd/system/automation-architect.service
[Unit]
Description=Automation Architect Agent
After=network.target

[Service]
Type=simple
WorkingDirectory=/path/to/workspace
ExecStart=/usr/local/bin/architect
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

```bash
# Enable and start the service
sudo systemctl enable automation-architect
sudo systemctl start automation-architect
```

**Supervisord** (Cross-platform):
```ini
# /etc/supervisor/conf.d/automation.conf
[program:automation-architect]
command=/usr/local/bin/architect
directory=/path/to/workspace
autostart=true
autorestart=true
user=automation
```

---

## Multi-Repo Workflow

The gastown project is designed to be used across multiple different repositories with a single global installation.

### Conceptual Model: Install Once, Use Everywhere

```
┌─────────────────────────────────────────────────────────────┐
│                    Global Installation                        │
│  (One-time: cargo install --path agents)                     │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  architect, janitor, prompt → Installed to ~/.cargo/bin  ││
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
        ▼                   ▼                   ▼
┌───────────────┐   ┌───────────────┐   ┌───────────────┐
│   Repo A      │   │   Repo B      │   │   Repo C      │
│ .automation-  │   │ .automation-  │   │ .automation-  │
│  rust.toml    │   │  rust.toml    │   │  rust.toml    │
└───────────────┘   └───────────────┘   └───────────────┘
```

**Key Design Principles:**
1. **Single Installation**: Install agents once globally, available system-wide
2. **Per-Repo Configuration**: Each repo has its own `.automation-rust.toml`
3. **Auto-Discovery**: Agents find the config file in the current directory
4. **Isolated Execution**: Each repo's config affects only that repo

### Complete Multi-Repo Example

```bash
# Step 1: Install ONCE (run this one time)
cd ~/gastown
cargo install --path agents

# Step 2: Set up Repo A
cd ~/projects/repo-a
# Create config file
cat > .automation-rust.toml << 'EOF'
[workspace]
path = "."

[logging]
level = "info"

[agents.architect]
interval_minutes = 40

[agents.janitor]
interval_minutes = 20

[agents.prompt]
interval_minutes = 5
EOF

# Run agents in repo-a
architect
janitor

# Step 3: Set up Repo B (different configuration!)
cd ~/projects/repo-b
# Create config file with different settings
cat > .automation-rust.toml << 'EOF'
[workspace]
path = "."

[logging]
level = "debug"  # More verbose for this repo

[agents.architect]
interval_minutes = 10  # More frequent for active development
immediate = true

[agents.janitor]
interval_minutes = 5

[agents.prompt]
interval_minutes = 2
EOF

# Run agents in repo-b
architect  # Will run every 10 minutes, not 40!
janitor   # Will run every 5 minutes, not 20!

# Step 4: Set up Repo C
cd ~/projects/repo-c
# Copy template and customize
cp ~/gastown/.automation-rust.toml .
vim .automation-rust.toml

# Run agents in repo-c
prompt
```

### Managing Configs Across Repos

#### Strategy 1: Template-Based

Keep a template config in your gastown directory and copy it to new repos:

```bash
# Create a template file
cat > ~/gastown/config-template.toml << 'EOF'
[workspace]
path = "."

[logging]
level = "info"
format = "pretty"
colors = true

[agents.architect]
interval_minutes = 40
timeout_minutes = 30

[agents.janitor]
interval_minutes = 20
timeout_minutes = 15

[agents.prompt]
interval_minutes = 5
timeout_minutes = 3
EOF

# Use template for new repos
cd ~/projects/new-repo
cp ~/gastown/config-template.toml .automation-rust.toml
# Customize as needed
vim .automation-rust.toml
```

#### Strategy 2: Version-Controlled Defaults

Check in a default `.automation-rust.toml` in your repo's template or monorepo root:

```bash
# For monorepo with multiple projects
~/monorepo/
├── .automation-rust.toml      # Default config
├── project-a/
│   └── .automation-rust.toml  # Override for project-a
├── project-b/
│   └── .automation-rust.toml  # Override for project-b
└── project-c/
    └── .automation-rust.toml  # Override for project-c
```

#### Strategy 3: Symbolic Links (Advanced)

For repos with identical configs, use symbolic links:

```bash
# Create a shared config directory
mkdir -p ~/configs/automation

# Create a shared config file
cat > ~/configs/automation/dev.toml << 'EOF'
[workspace]
path = "."
[logging]
level = "debug"
[agents.architect]
interval_minutes = 10
EOF

# Link repos to shared config
cd ~/projects/repo-a
ln -s ~/configs/automation/dev.toml .automation-rust.toml

cd ~/projects/repo-b
ln -s ~/configs/automation/dev.toml .automation-rust.toml
```

### Tips for Multi-Repo Usage

1. **Document Your Configs**: Add comments in `.automation-rust.toml` explaining why certain settings were chosen for that repo
2. **Version Control Configs**: Commit `.automation-rust.toml` to each repo so team members have the same settings
3. **Use Different Intervals**: Adjust agent intervals based on each repo's activity level
4. **Isolate State**: Keep each repo's `.state` directory separate (default behavior)
5. **Log Levels**: Use debug logging for active development repos, info/warn for production repos

### Verification

Verify that multi-repo setup is working correctly:

```bash
# Check that agents are installed
which architect  # Should show ~/.cargo/bin/architect
which janitor    # Should show ~/.cargo/bin/janitor
which prompt     # Should show ~/.cargo/bin/prompt

# Test in multiple repos
cd ~/projects/repo-a && architect -h  # Works!
cd ~/projects/repo-b && architect -h  # Works!
cd ~/projects/repo-c && architect -h  # Works!

# Check that each repo has its own config
cd ~/projects/repo-a && cat .automation-rust.toml
cd ~/projects/repo-b && cat .automation-rust.toml
# Configs should be different if you customized them
```

---

## Project Structure

### Workspace Crates

```
automation-rust/
├── Cargo.toml              # Workspace configuration
├── README.md               # This file
├── .automation-rust.toml   # Main configuration file
├── .env.example            # Environment variables template
├── common/                 # Shared types and utilities
│   ├── src/
│   │   ├── config.rs       # Configuration parsing
│   │   ├── error.rs        # Error types
│   │   ├── logging.rs      # Logging setup
│   │   └── workspace_config.rs  # Workspace config structs
├── state/                  # State management module
│   └── src/
│       ├── cleanup.rs      # Stale state cleanup
│       ├── lock.rs         # File-based exclusive locks
│       ├── persistence.rs  # JSON state persistence
│       └── state.rs        # State management
├── workspace/              # Workspace management module
│   └── src/
│       ├── files.rs        # File operations
│       ├── manager.rs      # Workspace manager
│       ├── markdown.rs     # Markdown file handling
│       └── template.rs     # Template processing
├── executor/               # CLI executor module
│   └── src/
│       ├── cli.rs          # Command-line interface
│       └── lib.rs          # Executor library
├── scheduler/              # Scheduler module
│   └── src/
│       ├── task.rs         # Task scheduling
│       └── lib.rs          # Scheduler library
├── agents/                 # Agent entry points
│   └── src/
│       ├── agent.rs        # Agent implementation
│       └── bin/
│           ├── architect.rs  # Architect agent binary
│           ├── janitor.rs    # Janitor agent binary
│           └── prompt.rs     # Prompt agent binary
├── docs/                   # Documentation
│   ├── architecture.md     # System architecture
│   ├── deployment.md      # Deployment guide
│   ├── troubleshooting.md # Troubleshooting guide
│   └── api/                # API documentation
└── examples/               # Example configurations
```

### Crate Descriptions

| Crate | Purpose |
|-------|---------|
| `automation-common` | Shared types, error handling, configuration, and logging utilities |
| `automation-state` | State management with JSON persistence and file-based locking |
| `automation-workspace` | Workspace management with Markdown file operations |
| `automation-executor` | CLI executor for spawning and monitoring external processes |
| `automation-scheduler` | Task scheduling with intervals and retry logic |
| `automation-agents` | Agent entry points (architect, janitor, prompt) |

---

## Development

### Building the Project

```bash
# Build all workspace members (debug mode)
cargo build

# Build optimized release binaries
cargo build --release

# Check code without building (faster)
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy --all-targets --all-features
```

### Running Tests

```bash
# Run all tests across all workspace members
cargo test

# Run tests for a specific crate
cargo test -p automation-state
cargo test -p automation-workspace
cargo test -p automation-executor
cargo test -p automation-scheduler

# Run tests with output displayed
cargo test -- --nocapture

# Run tests in release mode (faster)
cargo test --release

# Run specific test
cargo test test_lock_acquisition

# Run tests with debug logging
RUST_LOG=debug cargo test
```

### Adding Dependencies

To add a dependency to the workspace:

```bash
# Add to workspace (all crates)
cargo add serde --workspace

# Add to specific crate
cargo add regex -p automation-common
```

### Documentation

```bash
# Generate and open documentation
cargo doc --open

# Build documentation for all workspace members
cargo doc --document-private-items
```

---

## Troubleshooting

### Common Issues and Quick Fixes

#### Issue 1: Config File Not Found

**Error:**
```
Config file not found: .automation-rust.toml
```

**Solutions:**
```bash
# Ensure .automation-rust.toml exists in the current directory
ls -la .automation-rust.toml

# If using Docker, ensure volumes are mounted correctly
docker-compose config

# Use --config flag to specify the correct path
architect --config /absolute/path/to/.automation-rust.toml
```

#### Issue 2: Lock Acquisition Failed

**Error:**
```
Failed to acquire lock after N retries
```

**Solutions:**
```bash
# Check if another instance is running
ps aux | grep architect

# Check lock files
ls -la .state/*.lock

# Remove stale lock files (only if process is not running)
rm -f .state/*.lock

# Increase lock timeout in .automation-rust.toml
[agents.architect.lock]
timeout_seconds = 60
```

#### Issue 3: Permission Denied

**Error:**
```
Permission denied: .state/...
```

**Solutions:**
```bash
# Fix state directory permissions
chmod -R 755 .state
chown -R $USER:$USER .state

# For Docker, ensure proper user mapping
# The Docker container runs as user ID 1000
chown -R 1000:1000 /path/to/workspace
```

#### Issue 4: Docker Containers Not Starting

**Error:**
```
docker-compose up fails
```

**Solutions:**
```bash
# Check for port conflicts
netstat -tuln | grep -E ':(3000|3001|3002)'

# Verify environment variables
cat .env

# Check container logs
docker-compose logs architect

# Recreate containers
docker-compose down
docker-compose up -d --force-recreate
```

#### Issue 5: Agents Not Executing Tasks

**Symptoms:** Containers running, but no tasks being processed.

**Solutions:**
```bash
# Check agent intervals
cat .automation-rust.toml | grep interval_minutes

# Verify TODO.md exists and has tasks
cat TODO.md

# Force immediate execution by setting immediate=true
# or restart the agent

# Enable debug logging to see what's happening
RUST_LOG=debug architect
```

### Debug Mode

Enable detailed logging for troubleshooting:

```toml
# In .automation-rust.toml
[logging]
level = "debug"
```

Or use the `RUST_LOG` environment variable:

```bash
# Enable debug logging
RUST_LOG=debug architect

# Enable trace logging (most verbose)
RUST_LOG=trace architect

# Set for specific modules only
RUST_LOG=automation_state=debug,automation_workspace=info architect
```

### Getting More Help

For detailed troubleshooting, see:
- [Troubleshooting Guide](docs/troubleshooting.md) - Comprehensive issue diagnosis
- [Architecture Documentation](docs/architecture.md) - System design and internals
- [Deployment Guide](docs/deployment.md) - Production deployment instructions

---

## Documentation

Comprehensive documentation is available in the [`docs/`](docs/) directory:

- **[Deployment Guide](docs/deployment.md)** - Building, Docker deployment, running agents, and monitoring
- **[Migration Guide](docs/migration.md)** - Migrating from Node.js to Rust implementation
- **[Troubleshooting Guide](docs/troubleshooting.md)** - Common issues and solutions
- **[Architecture Documentation](docs/architecture.md)** - System architecture, design decisions, and technology choices
- **[API Documentation](docs/api/README.md)** - Module-by-module API reference

---

## Examples

### Example 1: Development Workspace

For active development with frequent agent runs and verbose logging:

```toml
# .automation-rust.toml - Development configuration
[workspace]
path = "."

[logging]
level = "debug"
colors = true

[agents.architect]
interval_minutes = 10
immediate = true

[agents.janitor]
interval_minutes = 5
immediate = true

[agents.prompt]
interval_minutes = 1
immediate = true
```

### Example 2: Production Workspace

For production with minimal logging and longer intervals:

```toml
# .automation-rust.toml - Production configuration
[workspace]
path = "/var/app"
state_path = "/var/app/.automation-state"

[logging]
level = "warn"
format = "json"
colors = false

[agents.architect]
interval_minutes = 60
timeout_minutes = 45
immediate = false

[agents.janitor]
interval_minutes = 30
timeout_minutes = 20
immediate = false

[agents.prompt]
interval_minutes = 10
timeout_minutes = 5
immediate = false
```

### Example 3: Multiple Workspaces

Manage multiple workspaces with separate configuration files. This is perfect for using the system across multiple repositories - see the [Multi-Repo Workflow](#multi-repo-workflow) section for a comprehensive guide.

```bash
# Workspace 1
cd /workspace1
architect  # Uses /workspace1/.automation-rust.toml

# Workspace 2
cd /workspace2
architect  # Uses /workspace2/.automation-rust.toml

# Workspace 3
cd /workspace3
architect  # Uses /workspace3/.automation-rust.toml
```

**Or using the --config flag:**
```bash
# Workspace 1
architect --config /workspace1/.automation-rust.toml

# Workspace 2
architect --config /workspace2/.automation-rust.toml

# Workspace 3
architect --config /workspace3/.automation-rust.toml
```

---

## Development Status

This is **Phase 1** of the Rust rewrite. All tasks are complete:

- ✅ Phase 1, Task 1: Initialize Rust project structure
- ✅ Phase 1, Task 2: Implement common types and error handling
- ✅ Phase 1, Task 3: Implement state management module
- ✅ Phase 1, Task 4: Implement workspace management module
- ✅ Phase 1, Task 5: Implement CLI executor module
- ✅ Phase 1, Task 6: Implement scheduler module
- ✅ Phase 1, Task 7: Implement agent entry points
- ✅ Phase 8, Sprint 8.2: Documentation

All Phase 1 tasks have been completed, including comprehensive documentation. The system is fully functional and provides 100% compatibility with the Node.js implementation.

---

## Contributing

Contributions are welcome! Please follow these guidelines:

### Code Style

- Follow Rust standard style conventions
- Use `cargo fmt` to format code
- Use `cargo clippy` to catch common issues

```bash
# Format code
cargo fmt

# Run linter
cargo clippy --all-targets --all-features
```

### Testing

- Write unit tests for new functionality
- Add integration tests for complex operations
- Ensure all tests pass before submitting

```bash
# Run all tests
cargo test --all
```

### Documentation

- Add rustdoc comments to public APIs
- Update this README for user-facing changes
- Add documentation to the [`docs/`](docs/) directory for major features

### Pull Requests

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Ensure all tests pass
5. Submit a pull request with a clear description

---

## Dependencies

The workspace uses the following shared dependencies:

| Dependency | Version | Purpose |
|------------|---------|---------|
| `tokio` | 1.35 | Async runtime |
| `serde` | 1.0 | Serialization |
| `serde_json` | 1.0 | JSON support |
| `thiserror` | 1.0 | Error handling |
| `anyhow` | 1.0 | Error context |
| `tracing` | 0.1 | Logging framework |
| `tracing-subscriber` | 0.3 | Log subscribers |
| `clap` | 4.4 | CLI argument parsing |
| `regex` | 1.10 | Regular expressions |
| `bytes` | 1.5 | Bytes handling |
| `pulldown-cmark` | 0.12 | Markdown parsing |
| `chrono` | 0.4 | Date/time handling |

---

## License

ISC

## Authors

Automation System

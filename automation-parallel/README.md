# Automation Parallel - Multi-Container AI Agent System

A parallel automation system that coordinates multiple AI agents through Docker containers to manage and execute development tasks in a shared workspace. Each agent runs on a scheduled interval and works independently while coordinating through shared state management.

---

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Components](#components)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Usage](#usage)
- [Development](#development)
- [Troubleshooting](#troubleshooting)
- [Directory Structure](#directory-structure)
- [License](#license)

---

## Architecture Overview

The system uses a three-container architecture where each agent operates independently on its own schedule while coordinating through a shared workspace directory.

### Container Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Shared Workspace                              │
│                        (Parent Directory)                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                   │
│  │ TODO.md     │  │ BACKLOG.md  │  │ COMPLETED.md│                   │
│  │ BLOCKERS.md │  │ PRD.md      │  │ ...         │                   │
│  └─────────────┘  └─────────────┘  └─────────────┘                   │
│                                                                    │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    Shared State Directory                     │  │
│  │                    (/workspace/.state)                          │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │  │
│  │  │prompt.lock  │  │janitor.lock │  │architect.lock│             │  │
│  │  │prompt.state │  │janitor.state│  │architect.state│             │  │
│  │  └─────────────┘  └─────────────┘  └─────────────┘              │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
          │                        │                        │
          ▼                        ▼                        ▼
 ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
 │ agent_prompt    │  │ agent_janitor   │  │ agent_architect  │
 │                 │  │                 │  │                 │
 │ Interval: 5min  │  │ Interval: 20min │  │ Interval: 40min  │
 │                 │  │                 │  │                 │
 │ Executes        │  │ Detects stale   │  │ Analyzes         │
 │ work items      │  │ items, cleanup  │  │ architecture     │
 └─────────────────┘  └─────────────────┘  └─────────────────┘
          │                        │                        │
          └────────────────────────┼────────────────────────┘
                                   ▼
                     ┌─────────────────────────────┐
                     │   automation-network         │
                     │   (Docker Bridge Network)    │
                     └─────────────────────────────┘
```

### Services Schedule

| Service | Interval | Purpose |
|---------|----------|---------|
| **agent_prompt** | Every 5 minutes (300,000ms) | Executes work items from TODO.md using PROMPT.md template |
| **agent_janitor** | Every 20 minutes (1,200,000ms) | Identifies stale items and cleanup opportunities using JANITOR.md template |
| **agent_architect** | Every 40 minutes (2,400,000ms) | Analyzes system architecture and identifies improvements using ARCHITECT.md template |

### Coordination Mechanism

1. **Shared Workspace**: All containers mount the parent directory as a read-only volume
2. **Shared State**: All containers mount the `.state` directory as a named volume for coordination
3. **File Locking**: Each agent uses exclusive file locks to prevent concurrent execution
4. **State Tracking**: JSON state files track execution history and error counts

---

## Components

### Entry Points

| File | Description |
|------|-------------|
| [`run-prompt.js`](run-prompt.js) | Entry point for the prompt agent. Reads TODO.md, executes PROMPT.md template with work items. |
| [`run-janitor.js`](run-janitor.js) | Entry point for the janitor agent. Detects stale items and generates cleanup suggestions. |
| [`run-architect.js`](run-architect.js) | Entry point for the architect agent. Analyzes architecture and identifies improvements. |

### Library Modules

| File | Description |
|------|-------------|
| [`lib/scheduler.js`](lib/scheduler.js) | Scheduled task management with retry logic and graceful shutdown support. |
| [`lib/workspace-manager.js`](lib/workspace-manager.js) | Workspace file operations with atomic write support for TODO.md, BACKLOG.md, COMPLETED.md, BLOCKERS.md, PRD.md. |
| [`lib/state-manager.js`](lib/state-manager.js) | State management and file-based locking for container coordination. |

### Prompt Templates

| File | Description |
|------|-------------|
| [`prompts/development/PROMPT.md`](prompts/development/PROMPT.md) | Template for executing work items from TODO.md. Supports `{{taskCount}}`, `{{tasks}}`, `{{timestamp}}` placeholders. |
| [`prompts/development/JANITOR.md`](prompts/development/JANITOR.md) | Template for generating cleanup suggestions. Supports staleness detection context variables. |
| [`prompts/development/ARCHITECT.md`](prompts/development/ARCHITECT.md) | Template for architectural analysis and improvement recommendations. |

---

## Quick Start

### Prerequisites

- **Docker**: Version 20.10 or higher
- **Docker Compose**: Version 2.0 or higher
- **Node.js**: Version 18.0.0 or higher (for local development)

### Installation

1. Clone the repository and navigate to the automation-parallel directory:
    ```bash
    cd automation-parallel
    ```

2. Install dependencies (for local development):
    ```bash
    npm install
    ```

3. Build the Docker images:
    ```bash
    docker-compose build
    ```

### Running with Docker Compose

Start all three agents in detached mode:
```bash
docker-compose up -d
```

View logs for all agents:
```bash
docker-compose logs -f
```

View logs for a specific agent:
```bash
docker-compose logs -f agent_prompt
docker-compose logs -f agent_janitor
docker-compose logs -f agent_architect
```

Stop all agents:
```bash
docker-compose down
```

### Using npm Scripts

The [`package.json`](package.json) provides convenient scripts:

| Script | Description |
|--------|-------------|
| `npm start` | Start all containers: `docker-compose up -d` |
| `npm stop` | Stop all containers: `docker-compose down` |
| `npm restart` | Restart all containers: `docker-compose restart` |
| `npm logs` | Follow logs: `docker-compose logs -f` |
| `npm run build` | Rebuild images: `docker-compose build` |
| `npm run health` | Check container status: `docker-compose ps` |

---

## Configuration

### CLI Arguments

Agents are configured through command-line arguments when running directly with Node.js. When using Docker Compose, these arguments are defined in the [`docker-compose.yml`](docker-compose.yml) file.

#### Common Arguments

| Argument | Required | Default | Description |
|----------|----------|---------|-------------|
| `--workspace <path>` | Yes | None | Root workspace directory where agents operate |
| `--state <path>` | Yes | None | Path where state files and agent data are stored |
| `--interval <ms>` | No | Varies by agent | Schedule interval in milliseconds |
| `--immediate` | No | false | Execute immediately on startup |

#### Agent-Specific Default Intervals

| Agent | Default Interval |
|-------|------------------|
| Prompt | 300,000ms (5 minutes) |
| Janitor | 1,200,000ms (20 minutes) |
| Architect | 2,400,000ms (40 minutes) |

### Running Individual Agents

To run agents locally with custom configuration, use CLI arguments:

#### Prompt Agent

Run with default settings:
```bash
npm run prompt
```

Run with custom workspace and interval:
```bash
node run-prompt.js --workspace /path/to/workspace --state /path/to/.state --interval 300000 --immediate
```

Run with different interval (e.g., 1 minute for testing):
```bash
node run-prompt.js --workspace /workspace --state /workspace/.state --interval 60000 --immediate
```

#### Janitor Agent

Run with default settings:
```bash
npm run janitor
```

Run with custom configuration:
```bash
node run-janitor.js --workspace /workspace --state /workspace/.state --interval 1200000 --immediate
```

#### Architect Agent

Run with default settings:
```bash
npm run architect
```

Run with custom configuration:
```bash
node run-architect.js --workspace /workspace --state /workspace/.state --interval 2400000 --immediate
```

### Docker Compose Configuration

The [`docker-compose.yml`](docker-compose.yml) file defines the three services:

- **Volumes**: `workspace-state` (for shared state), `logs` (for container logs)
- **Network**: `automation-network` (bridge network for inter-container communication)
- **Restart Policy**: `unless-stopped`
- **Health Checks**: All containers have health checks with 60s intervals

### Running Multiple Workspaces

To run multiple workspaces simultaneously, use the `CONTAINER_PREFIX` environment variable to create unique container names. This prevents conflicts when the same agent types run across different workspaces.

#### Setting Up Multiple Workspaces

1. **For Workspace 1** (e.g., frontend project):
    ```bash
    cd /path/to/frontend-project
    docker-compose -f /path/to/automation-parallel/docker-compose.yml \
      --env-file .env \
      -p frontend \
      up -d
    ```
    Set `CONTAINER_PREFIX=frontend-` in the `.env` file.

2. **For Workspace 2** (e.g., backend project):
    ```bash
    cd /path/to/backend-project
    docker-compose -f /path/to/automation-parallel/docker-compose.yml \
      --env-file .env \
      -p backend \
      up -d
    ```
    Set `CONTAINER_PREFIX=backend-` in the `.env` file.

#### Container Name Examples

Without prefix (single workspace):
- `agent_prompt`
- `agent_janitor`
- `agent_architect`

With `CONTAINER_PREFIX=frontend-`:
- `frontend-agent_prompt`
- `frontend-agent_janitor`
- `frontend-agent_architect`

With `CONTAINER_PREFIX=backend-`:
- `backend-agent_prompt`
- `backend-agent_janitor`
- `backend-agent_architect`

#### Stopping Multiple Workspaces

Stop specific workspace:
```bash
docker-compose -f /path/to/docker-compose.yml -p frontend down
docker-compose -f /path/to/docker-compose.yml -p backend down
```

List all automation containers:
```bash
docker ps --filter "name=agent_"
```

### Schedule Intervals

| Agent | Default Interval | CLI Argument |
|-------|------------------|--------------|
| Prompt | 5 minutes (300,000ms) | `--interval 300000` |
| Janitor | 20 minutes (1,200,000ms) | `--interval 1200000` |
| Architect | 40 minutes (2,400,000ms) | `--interval 2400000` |

---

## Usage

### How Agents Work

1. **Prompt Agent** ([`run-prompt.js`](run-prompt.js:142))
    - Acquires an exclusive lock
    - Reads workspace files (TODO.md, BACKLOG.md, COMPLETED.md, BLOCKERS.md, PRD.md)
    - Detects work items in TODO.md (checkboxes, numbered lists, bullets)
    - Executes the [`PROMPT.md`](prompts/development/PROMPT.md) template with work items as context
    - Writes output to `.prompt-output-{timestamp}.md` in the workspace
    - Releases lock and updates state

2. **Janitor Agent** ([`run-janitor.js`](run-janitor.js:153))
    - Acquires an exclusive lock
    - Reads workspace files
    - Detects stale items (older than 30 days)
    - Identifies old incomplete items, stale blockers, and old backlog items
    - Executes the [`JANITOR.md`](prompts/development/JANITOR.md) template with cleanup context
    - Writes output to `.janitor-output-{timestamp}.md` in the workspace
    - Releases lock and updates state

3. **Architect Agent** ([`run-architect.js`](run-architect.js:189))
    - Acquires an exclusive lock
    - Reads workspace files
    - Analyzes system architecture (work item counts, blockers, PRD status)
    - Identifies architectural concerns
    - Executes the [`ARCHITECT.md`](prompts/development/ARCHITECT.md) template with analysis context
    - Writes output to `.architect-output-{timestamp}.md` in the workspace
    - Releases lock and updates state

### File Coordination Mechanism

Each agent coordinates with others through:

1. **Shared Workspace**: All containers mount the parent directory read-only
2. **State Directory**: The `.state` directory is shared via a named volume
3. **File Locks**: Each agent creates a lock file (`{agent}.lock`) during execution
4. **State Files**: JSON state files (`{agent}.state.json`) track execution history
5. **Output Files**: Each agent writes timestamped output files to the workspace

### State Management and Locking

The [`StateManager`](lib/state-manager.js:7) class provides:

- **Lock Acquisition**: `acquireLock(taskName, timeout)` - Creates exclusive lock file
- **Lock Release**: `releaseLock(taskName)` - Removes lock file
- **Lock Retry**: `acquireLockWithRetry(taskName, options)` - Retry logic with configurable delays
- **State Read/Write**: `readState()`, `writeState()`, `updateState()` - JSON state persistence
- **Cleanup**: `cleanupStaleLocks()`, `cleanupAllStates()` - Remove old locks and states

State tracking includes:
- `lastRun` - Timestamp of last execution
- `lastSuccess` - Timestamp of last successful execution
- `lastFailure` - Timestamp of last failure
- `errorCount` - Total error count
- `consecutiveFailures` - Count of failures in a row
- `status` - Current status (idle, running, success, failed)

---

## Development

### Local Development Setup

For local development without Docker:

1. Install dependencies:
    ```bash
    npm install
    ```

2. Run individual agents with CLI arguments:
    ```bash
    node run-prompt.js --workspace /path/to/workspace --state /path/to/.state --interval 300000 --immediate
    ```

### Testing Individual Agents

To test a single agent with custom intervals:

```bash
# Test prompt agent with 5-second interval
node run-prompt.js --workspace /workspace --state /workspace/.state --interval 5000 --immediate

# Test janitor agent with 10-second interval
node run-janitor.js --workspace /workspace --state /workspace/.state --interval 10000 --immediate

# Test architect agent with 15-second interval
node run-architect.js --workspace /workspace --state /workspace/.state --interval 15000 --immediate
```

### Common npm Scripts

| Command | Description |
|---------|-------------|
| `npm start` | Start all Docker containers |
| `npm stop` | Stop all Docker containers |
| `npm restart` | Restart all Docker containers |
| `npm logs` | Follow all container logs |
| `npm run build` | Rebuild Docker images |
| `npm run health` | Check container status |
| `npm run prompt` | Run prompt agent locally (uses default CLI arguments) |
| `npm run janitor` | Run janitor agent locally (uses default CLI arguments) |
| `npm run architect` | Run architect agent locally (uses default CLI arguments) |

### Adding New Agents

To add a new agent:

1. Create a new entry point file: `run-{agent}.js`
2. Use the same pattern as existing agents:
    ```javascript
    const { createScheduledTask, gracefulShutdown } = require('./lib/scheduler.js');
    const WorkspaceManager = require('./lib/workspace-manager.js');
    const StateManager = require('./lib/state-manager.js');
    ```
3. Define your agent's handler function
4. Create a scheduled task: `const task = createScheduledTask(handler, { ... })`
5. Start the task: `task.start()`
6. Add the agent to [`docker-compose.yml`](docker-compose.yml) and [`package.json`](package.json) scripts

---

## Troubleshooting

### Common Issues and Solutions

#### Container fails to start

**Problem**: Container exits immediately or fails health check.

**Solutions**:
- Check Docker logs: `docker-compose logs <service>`
- Verify Node.js version compatibility (18.0.0+)
- Ensure workspace directory is accessible
- Check CLI arguments in docker-compose.yml

#### Lock acquisition timeout

**Problem**: Agent logs "Failed to acquire lock for task 'X': timeout after Yms".

**Solutions**:
- Check if another container is holding the lock: `docker exec agent_prompt ls -la /workspace/.state/`
- Manually remove stale locks: `docker-compose exec agent_janitor sh -c "rm /workspace/.state/*.lock"`
- Increase lock timeout in the agent configuration

#### Agent not executing on schedule

**Problem**: Agent starts but doesn't execute the handler.

**Solutions**:
- Check if `--immediate` flag is set for first execution
- Verify `--interval` argument is correct
- Check logs for errors: `docker-compose logs -f <service>`
- Ensure workspace files exist and are readable

#### Permission errors

**Problem**: Container cannot read/write to workspace or state.

**Solutions**:
- Verify Docker volume permissions
- Check that workspace directory exists
- Ensure container has correct UID/GID mappings
- On Linux, verify file ownership: `ls -la /path/to/workspace`

### Logs and Debugging

#### Viewing Logs

Follow all logs:
```bash
docker-compose logs -f
```

Follow specific service logs:
```bash
docker-compose logs -f agent_prompt
docker-compose logs -f agent_janitor
docker-compose logs -f agent_architect
```

View last 100 lines:
```bash
docker-compose logs --tail=100 agent_prompt
```

#### Checking State Files

Inspect agent state:
```bash
docker-compose exec agent_prompt cat /workspace/.state/prompt.state.json
```

Check for locks:
```bash
docker-compose exec agent_prompt ls -la /workspace/.state/*.lock
```

#### Container Health Checks

View container status:
```bash
docker-compose ps
```

Inspect container health:
```bash
docker inspect --format='{{.State.Health.Status}}' agent_prompt
```

### Container Health Checks

All containers include health checks:

- **Test**: `node -e "console.log('healthy')"`
- **Interval**: 60 seconds
- **Timeout**: 10 seconds
- **Retries**: 3
- **Start Period**: 30 seconds

To manually verify health:
```bash
docker exec agent_prompt node -e "console.log('healthy')"
```

---

## Directory Structure

```
automation-parallel/
├── .env                     # Environment configuration (create from .env.example)
├── .env.example             # Example environment variables
├── docker-compose.yml       # Docker Compose configuration
├── Dockerfile               # Docker image definition
├── package.json             # npm package configuration
├── README.md                # This file
│
├── lib/                     # Library modules
│   ├── scheduler.js         # Scheduled task management
│   ├── workspace-manager.js # Workspace file operations
│   └── state-manager.js     # State and lock management
│
├── prompts/                 # Prompt templates
│   ├── development/        # Development prompts
│   │   ├── PROMPT.md       # Work execution template
│   │   ├── JANITOR.md      # Cleanup template
│   │   └── ARCHITECT.md    # Architecture analysis template
│   ├── bugfixer/           # Bugfixer prompts
│   │   ├── BUGFIXER.md
│   │   └── BUGFIXER_BUGCHECK.md
│   └── linter/             # Linter prompts
│       ├── LINTER.md
│       ├── LINTER_SCAN.md
│       └── LINTER_PRIORITIZE.md
│
├── prompts-prompt/          # Prompt agent output directory
│   └── .gitkeep
│
├── prompts-janitor/         # Janitor agent output directory
│   └── .gitkeep
│
├── prompts-architect/       # Architect agent output directory
│   └── .gitkeep
│
├── run-prompt.js            # Prompt agent entry point
├── run-janitor.js           # Janitor agent entry point
└── run-architect.js         # Architect agent entry point
```

---

## License

ISC

---

## Credits

Built with Node.js, Docker, and Docker Compose for automated, scheduled AI agent coordination.

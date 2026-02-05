# Docker Setup Guide

This guide explains how to run the Kilo Code automation container with a specified project/workspace and loop type.

## Overview

The container runs the Kilo Code automation system in two different loop modes:
- **Development loop**: Primary development workflow with PROMPT.md, JANITOR.md, and ARCHITECT.md
- **Bugfixer loop**: Bug fixing workflow with BUGFIXER.md and BUGFIXER_BUGCHECK.md

## Prerequisites

- Docker installed on your system
- Docker Compose (optional, but recommended for easier configuration)
- A project directory containing at least a `PRD.md` file

## Running Multiple Instances

You can run multiple docker-compose instances simultaneously for different projects or workspaces. This is achieved by using the `COMPOSE_PROJECT_NAME` environment variable, which ensures unique container, network, and volume names for each instance.

### Why COMPOSE_PROJECT_NAME is Important

When running multiple instances, Docker Compose needs a way to distinguish between them. Without `COMPOSE_PROJECT_NAME`, all instances would try to use the same names for:
- Containers (`agent_coding_container`)
- Networks
- Other resources

This would cause conflicts and prevent multiple instances from running simultaneously.

### Setting Up Multiple Instances

#### Method 1: Using Environment Files

Create separate `.env` files for each instance:

```bash
# Create first instance config
cp .env.example .env.project-a

# Edit .env.project-a
COMPOSE_PROJECT_NAME=project-a
LOOP_TYPE=development
MOUNT_HOST_DIR=/path/to/project-a

# Create second instance config
cp .env.example .env.project-b

# Edit .env.project-b
COMPOSE_PROJECT_NAME=project-b
LOOP_TYPE=bugfixer
MOUNT_HOST_DIR=/path/to/project-b
```

Run each instance in separate terminals:

```bash
# Terminal 1
env_file=.env.project-b docker-compose up --build

# Terminal 2
env_file=.env.project-a docker-compose up --build
```

#### Method 2: Using Command Line Environment Variables

Run multiple instances directly from the command line in separate terminals:

**Linux/Mac**

```bash
# Terminal 1 - Project A
COMPOSE_PROJECT_NAME=project-a \
LOOP_TYPE=development \
MOUNT_HOST_DIR=/path/to/project-a \
docker-compose up --build

# Terminal 2 - Project B
COMPOSE_PROJECT_NAME=project-b \
LOOP_TYPE=bugfixer \
MOUNT_HOST_DIR=/path/to/project-b \
docker-compose up --build
```

**Windows (PowerShell)**

```powershell
# Terminal 1 - Project A
$env:COMPOSE_PROJECT_NAME="project-a"; $env:LOOP_TYPE="development"; $env:MOUNT_HOST_DIR="C:\path\to\project-a"; docker-compose up --build

# Terminal 2 - Project B
$env:COMPOSE_PROJECT_NAME="project-b"; $env:LOOP_TYPE="bugfixer"; $env:MOUNT_HOST_DIR="C:\path\to\project-b"; docker-compose up --build
```

**Windows (CMD)**

```cmd
REM Terminal 1 - Project A
set COMPOSE_PROJECT_NAME=project-a && set LOOP_TYPE=development && set MOUNT_HOST_DIR=C:\path\to\project-a && docker-compose up --build

REM Terminal 2 - Project B
set COMPOSE_PROJECT_NAME=project-b && set LOOP_TYPE=bugfixer && set MOUNT_HOST_DIR=C:\path\to\project-b && docker-compose up --build
```

### Container Names with COMPOSE_PROJECT_NAME

When you set `COMPOSE_PROJECT_NAME`, the container names follow this pattern:

```
{COMPOSE_PROJECT_NAME}_{service_name}_{instance_number}
```

Examples:
- `COMPOSE_PROJECT_NAME=project-a` → `project-a_agent_coding_container_1`
- `COMPOSE_PROJECT_NAME=myapp` → `myapp_agent_coding_container_1`

### Managing Multiple Instances

#### Stopping a Specific Instance

```bash
# Stop project-a instance
COMPOSE_PROJECT_NAME=project-a docker-compose stop

# Stop project-b instance
COMPOSE_PROJECT_NAME=project-b docker-compose stop
```

#### Removing a Specific Instance

```bash
# Stop and remove project-a
COMPOSE_PROJECT_NAME=project-a docker-compose down

# Stop and remove project-b
COMPOSE_PROJECT_NAME=project-b docker-compose down
```

#### Viewing Logs for a Specific Instance

```bash
# Follow logs for project-a
COMPOSE_PROJECT_NAME=project-a docker-compose logs -f

# Follow logs for project-b
COMPOSE_PROJECT_NAME=project-b docker-compose logs -f
```

#### List All Running Containers

```bash
docker ps
```

You'll see containers with names like:
```
project-a_agent_coding_container_1
project-b_agent_coding_container_1
```

### Best Practices for Multiple Instances

1. **Use descriptive project names**: Choose names that clearly identify the project or workspace (e.g., `my-app-prod`, `my-app-dev`, `project-alpha`)

2. **Separate environment files**: For long-running projects, use separate `.env` files instead of command-line arguments

3. **Document your instances**: Keep a README or notes file that maps project names to their directories and purposes

4. **Resource considerations**: Running multiple instances consumes more system resources (CPU, memory). Monitor your system's performance

5. **State files are per-workspace**: Each instance maintains its own state file in its mounted workspace directory

## Quick Start with Docker Compose (Recommended)

### Step 1: Configure Environment Variables

Copy the example environment file and edit it:

```bash
cp .env.example .env
```

Edit the `.env` file to set your preferences:

```bash
# Project Name: Unique identifier (required for multiple instances)
COMPOSE_PROJECT_NAME=agent-coding-container

# Loop Type: development or bugfixer
LOOP_TYPE=development

# Path to your project directory (absolute or relative to docker-compose.yml)
MOUNT_HOST_DIR=./workspace
```

**Note**: If you plan to run multiple instances simultaneously, set a unique `COMPOSE_PROJECT_NAME` for each instance. See [Running Multiple Instances](#running-multiple-instances) for details.

### Step 2: Build and Start the Container

```bash
docker-compose up --build
```

The container will start running with the configured loop type and workspace directory.

### Using Docker Compose with Environment Variables (Command Line)

You can also set environment variables directly on the command line without creating a `.env` file:

#### Linux/Mac

```bash
# Development loop with custom workspace
COMPOSE_PROJECT_NAME=my-project LOOP_TYPE=development MOUNT_HOST_DIR=/path/to/your/project docker-compose up --build

# Bugfixer loop with custom workspace
COMPOSE_PROJECT_NAME=my-project LOOP_TYPE=bugfixer MOUNT_HOST_DIR=/path/to/your/project docker-compose up --build
```

#### Windows (PowerShell)

```powershell
# Development loop with custom workspace
$env:COMPOSE_PROJECT_NAME="my-project"; $env:LOOP_TYPE="development"; $env:MOUNT_HOST_DIR="C:\path\to\your\project"; docker-compose up --build

# Bugfixer loop with custom workspace
$env:COMPOSE_PROJECT_NAME="my-project"; $env:LOOP_TYPE="bugfixer"; $env:MOUNT_HOST_DIR="C:\path\to\your\project"; docker-compose up --build
```

#### Windows (CMD)

```cmd
REM Development loop with custom workspace
set COMPOSE_PROJECT_NAME=my-project && set LOOP_TYPE=development && set MOUNT_HOST_DIR=C:\path\to\your\project && docker-compose up --build

REM Bugfixer loop with custom workspace
set COMPOSE_PROJECT_NAME=my-project && set LOOP_TYPE=bugfixer && set MOUNT_HOST_DIR=C:\path\to\your\project && docker-compose up --build
```

## Running with Docker Directly

If you prefer not to use Docker Compose, you can run the container directly with Docker CLI.

### Build the Image

```bash
docker build -t kilocode-agent .
```

### Run with Development Loop

```bash
# Linux/Mac
docker run -it --rm \
  -e LOOP_TYPE=development \
  -v /path/to/your/project:/home/workspace \
  --name kilocode-agent \
  kilocode-agent

# Windows (PowerShell)
docker run -it --rm `
  -e LOOP_TYPE=development `
  -v C:/path/to/your/project:/home/workspace `
  --name kilocode-agent `
  kilocode-agent

# Windows (CMD)
docker run -it --rm ^
  -e LOOP_TYPE=development ^
  -v C:/path/to/your/project:/home/workspace ^
  --name kilocode-agent ^
  kilocode-agent
```

### Run with Bugfixer Loop

```bash
# Linux/Mac
docker run -it --rm \
  -e LOOP_TYPE=bugfixer \
  -v /path/to/your/project:/home/workspace \
  --name kilocode-agent \
  kilocode-agent

# Windows (PowerShell)
docker run -it --rm `
  -e LOOP_TYPE=bugfixer `
  -v C:/path/to/your/project:/home/workspace `
  --name kilocode-agent `
  kilocode-agent

# Windows (CMD)
docker run -it --rm ^
  -e LOOP_TYPE=bugfixer ^
  -v C:/path/to/your/project:/home/workspace ^
  --name kilocode-agent ^
  kilocode-agent
```

### Docker CLI Options Explained

| Option | Description |
|--------|-------------|
| `-it` | Run in interactive mode with a pseudo-TTY |
| `--rm` | Automatically remove the container when it exits |
| `-e LOOP_TYPE=xxx` | Set the loop type environment variable |
| `-v host:/home/workspace` | Mount host directory to `/home/workspace` in container |
| `--name kilocode-agent` | Assign a name to the container |

## Loop Types

### Development Loop (`LOOP_TYPE=development`)

Runs the primary development prompts:
- **PROMPT.md**: Every iteration (primary development)
- **JANITOR.md**: Every 4 iterations (maintenance and cleanup)
- **ARCHITECT.md**: Every 8 iterations (architecture review)

**Best for:** Initial project development, building features from PRD

**State file:** `workspace/.state_development.json`

### Bugfixer Loop (`LOOP_TYPE=bugfixer`)

Runs the bug fixing prompts:
- **BUGFIXER.md**: Every iteration (bug identification and fixing)
- **BUGFIXER_BUGCHECK.md**: Every 4 iterations (verification and quality check)

**Best for:** Fixing bugs, debugging issues, code quality improvements

**State file:** `workspace/.state_bugfixer.json`

## Mounting Your Workspace

### Absolute Path (Recommended)

Using absolute paths is the most reliable approach:

```bash
# Linux/Mac
MOUNT_HOST_DIR=/home/user/my-project docker-compose up

# Windows
MOUNT_HOST_DIR=C:/Users/YourName/Projects/my-project docker-compose up
```

### Relative Path

You can use paths relative to the `docker-compose.yml` file location:

```bash
# Current directory
MOUNT_HOST_DIR=. docker-compose up

# Subdirectory
MOUNT_HOST_DIR=./my-project docker-compose up

# Parent directory
MOUNT_HOST_DIR=.. docker-compose up
```

### Workspace Structure Requirements

Your workspace directory should contain at least:

```
workspace/
├── PRD.md           # Product Requirements Document (required)
├── TODO.md          # Task list (created during bootstrap)
├── ARCHITECTURE.md  # Architecture decisions (created during bootstrap)
├── BLOCKERS.md      # Known issues (optional)
└── comms/           # Communication directory
    ├── inbox/       # Human responses
    └── outbox/      # Questions to human
```

## Stopping the Container

### Docker Compose

```bash
# Stop gracefully
docker-compose stop

# Stop and remove container
docker-compose down
```

### Docker CLI

```bash
# Graceful stop (Ctrl+C in terminal)
# Or
docker stop kilocode-agent
```

## Running in Background (Detached Mode)

### Docker Compose

```bash
docker-compose up -d --build
```

### Docker CLI

```bash
docker run -d --rm \
  -e LOOP_TYPE=development \
  -v /path/to/your/project:/home/workspace \
  --name kilocode-agent \
  kilocode-agent
```

### Viewing Logs

```bash
# Docker Compose
docker-compose logs -f

# Docker CLI
docker logs -f kilocode-agent
```

## State Persistence

The automation system maintains state in the workspace directory:

- **Development loop**: `workspace/.state_development.json`
- **Bugfixer loop**: `workspace/.state_bugfixer.json`

The state file includes:
- Current iteration number
- Last run timestamp

This allows the container to resume from where it left off when restarted.

## Marking Project as Complete

Create a `.done` file in your workspace directory to mark the project as complete:

```bash
# In the workspace directory
touch .done
```

The automation will detect this file and exit gracefully.

## Troubleshooting

### Container exits immediately

1. Check that your workspace directory exists
2. Ensure the workspace contains at least a `PRD.md` file
3. Verify the MOUNT_HOST_DIR path is correct

### "Workspace folder not found" error

The workspace directory is not mounted correctly. Check your volume mount configuration.

### Wrong loop type running

Verify the LOOP_TYPE environment variable is set correctly:

```bash
# Docker Compose
docker-compose exec agent_coding_container printenv | grep LOOP_TYPE

# Docker CLI
docker exec kilocode-agent printenv | grep LOOP_TYPE
```

### Permission issues on Windows

If you encounter permission issues when mounting volumes on Windows:

1. Ensure Docker Desktop has access to the drive
2. Go to Docker Desktop → Settings → Resources → File Sharing
3. Add the drive or directory path

### State not persisting

Ensure the workspace volume is mounted correctly and persists between container runs. The state file is written to the mounted workspace directory, so it will persist as long as the volume mount is correct.

## Complete Examples

### Example 1: Development Loop with Absolute Path (Linux/Mac)

```bash
COMPOSE_PROJECT_NAME=my-app LOOP_TYPE=development MOUNT_HOST_DIR=/home/user/projects/my-app docker-compose up --build
```

### Example 2: Bugfixer Loop with Relative Path (Windows PowerShell)

```powershell
$env:COMPOSE_PROJECT_NAME="my-project"; $env:LOOP_TYPE="bugfixer"; $env:MOUNT_HOST_DIR=".\my-project"; docker-compose up --build
```

### Example 3: Development Loop with Docker CLI (Linux/Mac)

```bash
docker build -t kilocode-agent .
docker run -it --rm \
  -e LOOP_TYPE=development \
  -v /home/user/projects/my-app:/home/workspace \
  --name kilocode-agent \
  kilocode-agent
```

### Example 4: Bugfixer Loop in Background (Windows PowerShell)

```powershell
docker build -t kilocode-agent .
docker run -d `
  -e LOOP_TYPE=bugfixer `
  -v C:/Users/YourName/Projects/my-app:/home/workspace `
  --name kilocode-agent `
  kilocode-agent

# View logs
docker logs -f kilocode-agent
```

### Example 5: Running Two Instances Simultaneously (Linux/Mac)

```bash
# Terminal 1
COMPOSE_PROJECT_NAME=project-a LOOP_TYPE=development MOUNT_HOST_DIR=/home/user/projects/project-a docker-compose up --build

# Terminal 2 (in a new terminal)
COMPOSE_PROJECT_NAME=project-b LOOP_TYPE=bugfixer MOUNT_HOST_DIR=/home/user/projects/project-b docker-compose up --build
```

## Summary

| Method | Command | Notes |
|--------|---------|-------|
| Docker Compose (default) | `docker-compose up --build` | Uses default LOOP_TYPE=development, workspace=./workspace |
| Docker Compose (env file) | `cp .env.example .env && docker-compose up --build` | Edit .env with your settings |
| Docker Compose (CLI) | `COMPOSE_PROJECT_NAME=my-project LOOP_TYPE=bugfixer MOUNT_HOST_DIR=/path docker-compose up --build` | Set env vars inline |
| Docker Compose (multi-instance) | `COMPOSE_PROJECT_NAME=project-a ... && COMPOSE_PROJECT_NAME=project-b ...` | Run multiple instances in separate terminals |
| Docker CLI | `docker run -it -e LOOP_TYPE=development -v /path:/home/workspace kilocode-agent` | Direct Docker usage |

For more information about the automation system, see [automation/README.md](automation/README.md).

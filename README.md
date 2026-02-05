# Agent Coding Container - Docker Compose Guide

An automated project development system that runs Kilo Code in orchestrator mode via Docker Compose to continuously build and improve a software project based on a Product Requirements Document (PRD).

## Overview

This Docker-based automation system executes a series of prompts in a continuous loop, automating the software development lifecycle from initial project setup through implementation and completion. Each prompt has a specific role and runs on a defined schedule. The containerized approach ensures consistent execution environments and easy deployment.

## Prerequisites

Before running this automation system with Docker Compose, ensure you have:

- **Docker** (v20.10 or higher) - Required to run containers
- **Docker Compose** (v2.0 or higher) - Required for container orchestration

To verify your installation:
```bash
docker --version
docker compose version
```

## Quick Start

### Basic Usage (Default Configuration)

1. **Prepare your workspace:**
   The automation expects a `workspace/` directory containing your project files (at minimum, a `PRD.md`).

   ```bash
   # Create workspace directory if it doesn't exist
   mkdir -p workspace
   
   # Ensure workspace contains PRD.md
   # You may also include TODO.md, ARCHITECTURE.md, etc.
   ```

2. **Start the container:**
   ```bash
   docker compose up
   ```

   This starts the automation with default settings:
   - **Delay between iterations:** 10 minutes (600 seconds)
   - **Workspace:** `./workspace` directory (mounted to `/home/workspace` in container)
   - **Restart policy:** `unless-stopped`

3. **Stop the container:**
   ```bash
   docker compose down
   ```

### Running in the Background

To run the automation in the background (detached mode):
```bash
docker compose up -d
```

To view logs:
```bash
docker compose logs -f
```

To stop:
```bash
docker compose down
```

## Configuring the Delay Between Iterations

The delay between automation iterations determines how long the system waits after completing one iteration before starting the next. You can configure this in several ways.

### Method 1: Override the Default Command

The default command runs with a 10-minute delay. You can override this using a custom command with the delay in seconds.

```bash
# Run with 5-minute delay (300 seconds)
docker compose run --rm agent_coding_container node /home/automation/run.js 300

# Run with 15-minute delay (900 seconds)
docker compose run --rm agent_coding_container node /home/automation/run.js 900

# Run with 1-hour delay (3600 seconds)
docker compose run --rm agent_coding_container node /home/automation/run.js 3600

# Run with 30-minute delay (1800 seconds)
docker compose run --rm agent_coding_container node /home/automation/run.js 1800
```

### Method 2: Using Docker Compose Override Files

Create a `docker-compose.override.yml` file to customize the delay without modifying the main `docker-compose.yml`:

```yaml
version: '3.8'

services:
  agent_coding_container:
    command: ["node", "/home/automation/run.js", "300"]  # 5-minute delay
```

Then run normally:
```bash
docker compose up
```

### Method 3: Environment Variable Configuration

Create an `.env` file in the same directory:

```env
# Delay between iterations in seconds (default: 600)
DELAY_SECONDS=300
```

Then update your `docker-compose.yml` to use this variable:

```yaml
services:
  agent_coding_container:
    command: ["node", "/home/automation/run.js", "${DELAY_SECONDS:-600}"]
```

## Example Docker Compose Configurations

### Fast-Paced Development (5-minute delay)

**docker-compose.fast.yml:**
```yaml
version: '3.8'

services:
  agent_coding_container:
    build: .
    container_name: agent_coding_container
    command: ["node", "/home/automation/run.js", "300"]
    volumes:
      - ${MOUNT_HOST_DIR:-./workspace}:/home/workspace
    restart: unless-stopped
```

Run with:
```bash
docker compose -f docker-compose.fast.yml up
```

### Long-Running Tasks (1-hour delay)

**docker-compose.slow.yml:**
```yaml
version: '3.8'

services:
  agent_coding_container:
    build: .
    container_name: agent_coding_container
    command: ["node", "/home/automation/run.js", "3600"]
    volumes:
      - ${MOUNT_HOST_DIR:-./workspace}:/home/workspace
    restart: unless-stopped
```

Run with:
```bash
docker compose -f docker-compose.slow.yml up
```

### Custom Workspace with 15-minute delay

**docker-compose.custom.yml:**
```yaml
version: '3.8'

services:
  agent_coding_container:
    build: .
    container_name: agent_coding_container
    command: ["node", "/home/automation/run.js", "900"]
    volumes:
      - /path/to/your/workspace:/home/workspace
    restart: unless-stopped
```

Run with:
```bash
docker compose -f docker-compose.custom.yml up
```

## Iteration System

The automation runs prompts on a scheduled basis. Each iteration represents one cycle of the automation loop.

### Prompt Schedule

| Prompt        | Frequency           | Description                                                                  |
|---------------|---------------------|------------------------------------------------------------------------------|
| PROMPT.md     | **Every iteration** | Primary development prompt. Handles project phases (bootstrap, implementation, verification) |
| JANITOR.md    | **Every 4 iterations** | Repository maintenance. Cleans up TODOs, identifies drift, removes unused files |
| ARCHITECT.md  | **Every 8 iterations** | Architecture review. Performs gap analysis, reviews blockers, plans next steps |

### Example Schedule

- **Iteration 1-3**: Only PROMPT.md runs
- **Iteration 4**: PROMPT.md + JANITOR.md
- **Iteration 5-7**: Only PROMPT.md
- **Iteration 8**: PROMPT.md + JANITOR.md + ARCHITECT.md
- **Iteration 12**: PROMPT.md + JANITOR.md
- **Iteration 16**: PROMPT.md + JANITOR.md + ARCHITECT.md

And so on...

### Prompt Descriptions

#### PROMPT.md
The primary development prompt that runs every iteration. It:
- Detects the current project phase (BOOTSTRAP, IMPLEMENTATION, or VERIFICATION)
- Implements tasks from TODO.md one at a time
- Commits work with conventional commit messages
- Handles communication via `comms/inbox/` and `comms/outbox/`
- Maintains LEARNINGS.md for discovered patterns

#### JANITOR.md
The repository maintainer that runs every 4 iterations. It:
- Marks completed tasks in TODO.md
- Identifies code drift from the PRD
- Cleans up unused files and empty directories
- Syncs documentation with implementation

#### ARCHITECT.md
The lead architect that runs every 8 iterations. It:
- Performs gap analysis between PRD, TODO, and implementation
- Breaks down vague TODO items into atomic tasks
- Reviews and resolves blockers
- Writes architectural decisions to ARCHITECTURE.md

## Workspace Directory and Volume Mounts

### Default Workspace Configuration

By default, the container mounts the local `./workspace` directory to `/home/workspace` inside the container:

```yaml
volumes:
  - ./workspace:/home/workspace
```

This allows the automation scripts to access your project files and persist all changes to your host machine.

### Custom Workspace Directory

You can mount a different host directory using the `MOUNT_HOST_DIR` environment variable:

#### Linux/macOS:
```bash
MOUNT_HOST_DIR=/path/to/your/project docker compose up
```

#### Windows (PowerShell):
```powershell
$env:MOUNT_HOST_DIR="C:\path\to\your\project"; docker compose up
```

#### Windows (CMD):
```cmd
set MOUNT_HOST_DIR=C:\path\to\your\project && docker compose up
```

#### Using an .env file:
Create an `.env` file:
```env
MOUNT_HOST_DIR=/path/to/your/project
```

Then run normally:
```bash
docker compose up
```

### Workspace Structure Expected by Automation

The automation expects the following structure inside the mounted workspace:

```
workspace/                      # Mounted to /home/workspace in container
├── .state.json                 # State persistence (auto-created)
├── .done                       # Completion marker (user-created)
├── PRD.md                      # Product Requirements Document
├── TODO.md                     # Task list (created during bootstrap)
├── ARCHITECTURE.md             # Architecture decisions (created during bootstrap)
├── BLOCKERS.md                 # Known issues (optional)
├── LEARNINGS.md                # Discovered patterns (optional)
└── comms/                      # Communication directory
    ├── inbox/                  # Human responses
    └── outbox/                 # Questions to human
```

### Volume Mount Best Practices

1. **Use absolute paths** when specifying `MOUNT_HOST_DIR` to avoid path resolution issues
2. **Ensure permissions** allow the container to read/write to the mounted directory
3. **Back up important data** before running automation
4. **Use named volumes** for persistent data storage if not mounting a host directory

## Viewing Logs and Monitoring the Automation

### View All Logs

To view real-time logs from the container:
```bash
docker compose logs -f
```

The `-f` flag follows log output (similar to `tail -f`).

### View Logs for Specific Container

If running multiple services:
```bash
docker compose logs -f agent_coding_container
```

### View Last N Lines of Logs

```bash
# View last 100 lines
docker compose logs --tail=100

# View last 500 lines
docker compose logs --tail=500
```

### View Logs Since a Specific Time

```bash
# View logs from the last hour
docker compose logs --since=1h

# View logs from a specific timestamp
docker compose logs --since="2026-02-05T00:00:00"
```

### Container-Specific Commands

#### Check Container Status
```bash
docker ps
```

#### Inspect Container
```bash
docker inspect agent_coding_container
```

#### Execute Commands Inside Container

To execute commands inside the running container:
```bash
# Open an interactive shell
docker exec -it agent_coding_container /bin/bash

# Check workspace contents
docker exec agent_coding_container ls -la /home/workspace

# View state file
docker exec agent_coding_container cat /home/workspace/.state.json

# Check for .done file
docker exec agent_coding_container ls /home/workspace/.done
```

### Monitoring Iteration Progress

The automation logs clear indicators of progress:
```
2026-02-05 00:00:00: Iteration #1
============================================================
📋 Queueing PROMPT.md
📝 Execution order: PROMPT
▶️ Running PROMPT...
...
2026-02-05 00:15:00: Iteration 1 complete. Sleeping 600s...
💾 Saved state to /home/workspace/.state.json
```

## Marking the Project as Complete

The project is considered complete when a `.done` file is created in the workspace directory.

### How to Mark Complete

#### Method 1: Create File on Host

Since the workspace is mounted to your host machine, you can create the `.done` file directly:

**Linux/macOS:**
```bash
touch workspace/.done
```

**Windows (PowerShell):**
```powershell
New-Item -Path workspace\.done -ItemType File
```

**Windows (CMD):**
```cmd
type nul > workspace\.done
```

#### Method 2: Create File Inside Container

```bash
docker exec agent_coding_container touch /home/workspace/.done
```

### Automatic Detection

The automation automatically checks for the `.done` file after each prompt execution. Once detected:
- The automation completes the current iteration
- Exits gracefully with a completion message
- The container stops (depending on restart policy)

### When to Mark Complete

According to the PROMPT.md guidelines, a project is complete when:
1. All TODO.md items are checked
2. All tests pass
3. The app builds successfully
4. Core PRD requirements have corresponding implementations

### Resuming After Marking Complete

If you need to resume automation after marking it complete (e.g., if marked prematurely):
```bash
# Remove the .done file
rm workspace/.done

# Restart the container
docker compose restart
```

## State Persistence

The automation maintains state across runs using a `.state.json` file located in the workspace directory (`workspace/.state.json`).

### State File Format

```json
{
  "iteration": 5,
  "lastRun": "2026-02-05T00:00:00.000Z"
}
```

- **iteration**: The next iteration number to run
- **lastRun**: ISO timestamp of when the last iteration completed

### Benefits of State Persistence

- **Resumability**: Stop and restart the container; it continues from where it left off
- **Tracking**: Always know which iteration the project is on
- **Recovery**: If the container crashes or is interrupted, no progress is lost

### Viewing the State

```bash
# View state file
docker exec agent_coding_container cat /home/workspace/.state.json

# Or view on host (if workspace is mounted)
cat workspace/.state.json
```

### Resetting the State

To start from iteration 1:
```bash
# Remove state file
rm workspace/.state.json

# Or inside container
docker exec agent_coding_container rm /home/workspace/.state.json
```

## Customizing Prompts

All prompt files are located in the `automation/prompts/` directory and are copied into the container at build time.

### Available Prompts

- **`automation/prompts/PROMPT.md`** - Primary development prompt
- **`automation/prompts/JANITOR.md`** - Repository maintenance prompt
- **`automation/prompts/ARCHITECT.md`** - Architecture review prompt

### Modifying Prompts

1. Edit the prompt files in the `automation/prompts/` directory on your host
2. Rebuild the Docker image to include changes:
   ```bash
   docker compose build
   ```
3. Restart the container:
   ```bash
   docker compose up -d
   ```

### Hot-Reloading Prompts (Advanced)

For development, you can mount the prompts directory to enable hot-reloading without rebuilding:

**docker-compose.dev.yml:**
```yaml
version: '3.8'

services:
  agent_coding_container:
    build: .
    container_name: agent_coding_container
    command: ["node", "/home/automation/run.js"]
    volumes:
      - ${MOUNT_HOST_DIR:-./workspace}:/home/workspace
      - ./automation/prompts:/home/automation/prompts  # Hot-reload prompts
    restart: unless-stopped
```

Run with:
```bash
docker compose -f docker-compose.dev.yml up
```

### Creating Custom Prompts

You can add additional prompts by:

1. Creating new prompt files in `automation/prompts/`
2. Modifying `automation/run.js` to include your custom prompts in the execution queue
3. Rebuilding the Docker image

## Kilo Code Configuration

The container includes Kilo Code CLI configuration copied from the `.kilocode/` directory to `/root/.kilocode` inside the container.

### Customizing Configuration

1. Modify files in the `.kilocode/` directory on your host
2. Rebuild the Docker image:
   ```bash
   docker compose build
   ```
3. Restart the container

### Kilo Code CLI Version

The container uses Kilo Code CLI version **0.26.0** as defined in the Dockerfile.

## Restart Policy

The container is configured with a `restart: unless-stopped` policy, meaning:
- The container automatically restarts if it crashes
- The container does **not** restart when stopped manually (via `docker compose down` or `docker stop`)
- The container restarts automatically if the Docker daemon restarts

### Changing Restart Policy

Edit `docker-compose.yml`:

```yaml
services:
  agent_coding_container:
    restart: always  # Always restart, even on manual stop
    # or
    restart: no      # Never restart automatically
```

## Troubleshooting

### Container Fails to Start

**Issue:** Container exits immediately

**Solution:** Check logs for error messages:
```bash
docker compose logs
```

Common causes:
- Workspace directory not found (ensure `workspace/` exists)
- Prompt files missing (verify `automation/prompts/` contains all required files)
- Permission issues on mounted volume

### Kilo Code Command Not Found

**Issue:** Error indicates `kilocode` command not found

**Solution:** The CLI is installed during image build. Rebuild the image:
```bash
docker compose build --no-cache
```

### Workspace Directory Not Found Inside Container

**Issue:** Automation reports workspace not found

**Solution:** Verify the volume mount:
```bash
# Check mounted volumes
docker inspect agent_coding_container | grep -A 10 Mounts

# Verify workspace contents
docker exec agent_coding_container ls -la /home/workspace
```

### Changes Not Persisting

**Issue:** Changes made in container are lost after restart

**Solution:** Ensure the workspace volume is properly mounted. Check:
```bash
docker compose config
```

Verify the volume path points to the correct host directory.

### Automation Not Resuming

**Issue:** Automation starts from iteration 1 instead of resuming

**Solution:** Check state file:
```bash
docker exec agent_coding_container cat /home/workspace/.state.json
```

If file is missing or corrupted, it will start from iteration 1.

### Prompt Changes Not Taking Effect

**Issue:** Modified prompts are not used

**Solution:** The prompts are copied at build time. Rebuild the image:
```bash
docker compose build
docker compose up -d
```

### Permission Denied Errors

**Issue:** Container cannot write to mounted directory

**Solution:** Fix permissions on the host:
```bash
# Linux/macOS
chmod -R 755 workspace

# Or run container with user mapping (advanced)
docker compose run --user $(id -u):$(id -g) agent_coding_container
```

### Container Consuming Too Many Resources

**Issue:** High CPU or memory usage

**Solution:** Add resource limits to `docker-compose.yml`:

```yaml
services:
  agent_coding_container:
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 4G
        reservations:
          cpus: '1.0'
          memory: 2G
```

## Advanced Usage

### Running Multiple Instances

To run multiple automation instances with different workspaces:

**docker-compose.multi.yml:**
```yaml
version: '3.8'

services:
  agent_1:
    build: .
    container_name: agent_1
    command: ["node", "/home/automation/run.js", "600"]
    volumes:
      - ./workspace1:/home/workspace
    restart: unless-stopped

  agent_2:
    build: .
    container_name: agent_2
    command: ["node", "/home/automation/run.js", "900"]
    volumes:
      - ./workspace2:/home/workspace
    restart: unless-stopped
```

Run with:
```bash
docker compose -f docker-compose.multi.yml up
```

### Health Checks

Add health checks to monitor automation status:

```yaml
services:
  agent_coding_container:
    healthcheck:
      test: ["CMD", "test", "-f", "/home/workspace/.state.json"]
      interval: 5m
      timeout: 30s
      retries: 3
```

### Log Rotation

Prevent logs from consuming too much disk space:

```yaml
services:
  agent_coding_container:
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
```

### Using Custom Docker Images

If you've built a custom image with different configurations:

```yaml
services:
  agent_coding_container:
    image: your-custom-image:latest
    # ... other config
```

## Cleanup

### Remove Stopped Containers

```bash
docker compose rm
```

### Remove All Containers and Volumes

```bash
docker compose down -v
```

### Remove Docker Images

```bash
# Remove built image
docker rmi agent-coding-container-agent_coding_container

# Or remove all unused images
docker image prune -a
```

## Summary

The Agent Coding Container provides a Docker-based automation system that:
- Runs Kilo Code in orchestrator mode with customizable delays
- Executes prompts on a scheduled basis (PROMPT every iteration, JANITOR every 4, ARCHITECT every 8)
- Persists state across runs via `.state.json`
- Allows workspace mounting for easy file access and modification
- Supports various configuration methods for delays and workspace paths
- Can be monitored via Docker logs and container inspection
- Marks completion via `.done` file
- Supports prompt customization and hot-reloading during development

For more information about the automation system's internal workings, see [`automation/README.md`](automation/README.md).

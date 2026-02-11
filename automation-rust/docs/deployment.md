# Deployment Guide

This guide provides comprehensive instructions for deploying the gastown system in various environments.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Building from Source](#building-from-source)
- [Docker Deployment](#docker-deployment)
- [Running Individual Agents](#running-individual-agents)
- [Environment Variables](#environment-variables)
- [Monitoring and Logging](#monitoring-and-logging)
- [Production Considerations](#production-considerations)

## Prerequisites

### System Requirements

- **Operating System**: Linux, macOS, or Windows
- **Rust**: 1.70 or later (for building from source)
- **Docker**: 20.10 or later (for Docker deployment)
- **Docker Compose**: 2.0 or later
- **Disk Space**: Minimum 100MB for Docker image, additional space for workspace and state data

### Resource Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | 1 core | 2+ cores |
| Memory | 512MB | 1GB+ |
| Disk | 100MB | 1GB+ (including workspace data) |

## Building from Source

### Install Rust Toolchain

```bash
# Install rustup if not already installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Update to the latest stable version
rustup update stable
```

### Build the Project

```bash
# Navigate to the project directory
cd gastown

# Build all workspace members (debug build)
cargo build

# Build optimized release binary
cargo build --release

# Check code without building
cargo check
```

### Build Output

The binaries will be available at:
- `target/debug/architect` - Architect agent (debug build)
- `target/debug/janitor` - Janitor agent (debug build)
- `target/debug/prompt` - Prompt agent (debug build)

Or for release builds:
- `target/release/architect`
- `target/release/janitor`
- `target/release/prompt`

## Docker Deployment

### Building the Docker Image

The project includes a multi-stage Dockerfile for efficient builds:

```bash
# Navigate to the project directory
cd gastown

# Build the Docker image
docker build -t gastown:latest .

# Build with a specific tag
docker build -t gastown:v1.0.0 .

# Build without cache (for fresh builds)
docker build --no-cache -t gastown:latest .
```

The Docker image size is approximately 30-40MB due to the multi-stage build with Alpine Linux.

### Docker Compose Deployment

Docker Compose is the recommended deployment method for running all three agents together.

#### Step 1: Configure Environment

```bash
# Copy the example environment file
cp .env.example .env

# Edit the environment file with your settings
vim .env  # or use your preferred editor
```

#### Step 2: Start Services

```bash
# Start all three agent services in detached mode
docker-compose up -d

# Verify services are running
docker-compose ps
```

#### Expected Output

```
NAME                      COMMAND                  SERVICE   STATUS
automation-architect      "/automation-rust/ar…"   architect running
automation-janitor        "/automation-rust/ja…"   janitor   running
automation-prompt         "/automation-rust/pr…"   prompt    running
```

#### Step 3: Monitor Services

```bash
# View logs from all services
docker-compose logs -f

# View logs for a specific service
docker-compose logs -f architect
docker-compose logs -f janitor
docker-compose logs -f prompt

# View the last 100 lines of logs
docker-compose logs --tail=100
```

### Managing Docker Services

#### Service Operations

```bash
# Start all services
docker-compose up -d

# Start a specific service
docker-compose up -d architect

# Restart all services
docker-compose restart

# Restart a specific service
docker-compose restart prompt

# Stop all services
docker-compose stop

# Stop a specific service
docker-compose stop janitor

# Remove all services (keeps volumes)
docker-compose down

# Remove all services and volumes (WARNING: deletes all data)
docker-compose down -v
```

#### Service Status

```bash
# Check status of all services
docker-compose ps

# Check health status
docker-compose ps

# Inspect a specific container
docker inspect automation-architect

# Execute commands inside a container
docker-compose exec architect /bin/sh
```

### Docker Configuration Details

#### Multi-Stage Build

The Dockerfile uses a multi-stage build process:

1. **Build Stage**: Uses the official Rust image to compile the code
2. **Runtime Stage**: Uses Alpine Linux for a minimal runtime image

This results in a significantly smaller final image size.

#### Security Features

- **Non-root User**: Containers run as `automation:automation` user (UID/GID 1000)
- **Read-only Root Filesystem**: Runtime directory is mounted as read-only where possible
- **Resource Limits**: CPU and memory constraints are enforced

#### Health Checks

Each agent service includes a health check:

```yaml
healthcheck:
  test: ["CMD", "/automation-rust/health-check.sh"]
  interval: 30s
  timeout: 10s
  retries: 3
  start_period: 40s
```

#### Logging Configuration

Logging is configured with the JSON file driver:

- **Max Size**: 10MB per log file
- **Max Files**: 3 files retained
- **Location**: `/var/lib/docker/containers/<container_id>/*.log`

#### Resource Limits

Each service has the following resource constraints:

| Resource | Limit |
|----------|-------|
| CPU | 1.0 (100% of one core) |
| Memory | 512MB |

## Running Individual Agents

### Running Binaries Directly

If you've built from source, you can run agents directly:

```bash
# Run architect agent
cargo run --bin architect -- --workspace /path/to/workspace --state /path/to/state

# Run janitor agent
cargo run --bin janitor -- --workspace /path/to/workspace --state /path/to/state

# Run prompt agent
cargo run --bin prompt -- --workspace /path/to/workspace --state /path/to/state
```

### Running Release Binaries

```bash
# Using the release binaries
./target/release/architect --workspace /path/to/workspace --state /path/to/state
./target/release/janitor --workspace /path/to/workspace --state /path/to/state
./target/release/prompt --workspace /path/to/workspace --state /path/to/state
```

### CLI Arguments

All agents accept the following common arguments:

| Argument | Short | Required | Default | Description |
|----------|-------|----------|---------|-------------|
| `--workspace` | `-w` | Yes | - | Path to workspace directory |
| `--state` | `-s` | No | `<workspace>/.state` | Path to state directory |
| `--interval` | `-i` | No | - | Execution interval in seconds |
| `--timeout` | `-t` | No | - | Execution timeout in seconds |
| `--immediate` | `-I` | No | false | Run immediately on start |

### Running with systemd

For production deployments, you may want to use systemd to manage the agents:

```ini
# /etc/systemd/system/automation-architect.service
[Unit]
Description=Automation Architect Agent
After=network.target

[Service]
Type=simple
User=automation
Group=automation
WorkingDirectory=/opt/automation-rust
ExecStart=/opt/automation-rust/target/release/architect \
    --workspace /var/lib/automation/workspace \
    --state /var/lib/automation/state \
    --interval 2400
Restart=always
RestartSec=10

# Environment variables
Environment="RUST_LOG=info"
Environment="AUTOMATION_IMMEDIATE=true"

[Install]
WantedBy=multi-user.target
```

Enable and start the service:

```bash
# Reload systemd configuration
systemctl daemon-reload

# Enable the service to start on boot
systemctl enable automation-architect

# Start the service
systemctl start automation-architect

# Check service status
systemctl status automation-architect

# View service logs
journalctl -u automation-architect -f
```

## Environment Variables

### Configuration Variables

The following environment variables can be set to configure agent behavior:

| Variable | Description | Default |
|----------|-------------|---------|
| `AUTOMATION_WORKSPACE` | Path to workspace directory | `/workspace` |
| `AUTOMATION_STATE` | Path to state directory | `<workspace>/.state` |
| `AUTOMATION_INTERVAL` | Execution interval in seconds | Agent-specific |
| `AUTOMATION_TIMEOUT` | Execution timeout in seconds | `1800` (30 min) |
| `AUTOMATION_IMMEDIATE` | Run immediately on start | `false` |
| `CONTAINER_PREFIX` | Prefix for container names | `automation-` |

### Logging Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Log level filter | `info` |
| `RUST_BACKTRACE` | Enable backtraces | `0` |

### Log Levels

The `RUST_LOG` variable accepts the following values (from least to most verbose):

- `error` - Only errors
- `warn` - Warnings and errors
- `info` - Info, warnings, and errors (default)
- `debug` - Debug info and above
- `trace` - All tracing information

You can also set log levels per module:

```bash
# Set global log level to info, but debug for specific modules
RUST_LOG=info,automation_state=debug,automation_workspace=trace
```

### Example Environment Configuration

```bash
# .env file example
AUTOMATION_WORKSPACE=/workspace
AUTOMATION_STATE=/workspace/.state
AUTOMATION_IMMEDIATE=true
RUST_LOG=debug
CONTAINER_PREFIX=production-automation-
```

## Monitoring and Logging

### Log Format

Logs are emitted in structured format with the following fields:

- `timestamp` - ISO 8601 timestamp
- `level` - Log level (ERROR, WARN, INFO, DEBUG, TRACE)
- `target` - Module name
- `span` - Optional span context
- `message` - Log message

Example log output:

```
2024-02-09T21:30:45.123Z INFO automation_state::lock: Acquired lock for architect
2024-02-09T21:30:45.124Z DEBUG automation_executor::cli: Executing command: /bin/echo
2024-02-09T21:30:45.125Z INFO automation_agents::architect: Architect execution completed successfully
```

### Viewing Logs

#### Docker Compose

```bash
# Follow logs in real-time
docker-compose logs -f

# View last 100 lines
docker-compose logs --tail=100

# View logs for specific service
docker-compose logs -f architect

# View logs since a specific time
docker-compose logs --since="2024-02-09T20:00:00"
```

#### Direct Binary Execution

When running binaries directly, logs are printed to stdout/stderr. You can:

```bash
# Redirect logs to a file
./target/release/architect --workspace /workspace > /var/log/automation/architect.log 2>&1

# Use logrotate for log rotation
# /etc/logrotate.d/automation-rust
/var/log/automation/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 0640 automation automation
}
```

### Health Monitoring

#### Container Health

```bash
# Check health of all containers
docker-compose ps

# Check health of specific container
docker inspect --format='{{.State.Health.Status}}' automation-architect
```

#### Health Check Script

The Dockerfile includes a health check script that verifies:

1. The agent process is running
2. The agent is responsive to health checks
3. Recent log activity is present

If the health check fails 3 times, the container will be marked as unhealthy.

### Metrics Collection

While the system doesn't currently expose Prometheus metrics, you can collect basic metrics from the logs:

- **Execution Count**: Count successful execution log entries
- **Execution Time**: Parse timing information from logs
- **Error Rate**: Count error-level log entries
- **Lock Contention**: Monitor lock acquisition logs

Example metric collection using `awk`:

```bash
# Count successful executions
docker-compose logs prompt | grep -c "execution completed successfully"

# Count errors in the last hour
docker-compose logs --since="1h" | grep -c "ERROR"
```

## Production Considerations

### High Availability

For high availability deployments, consider:

1. **Multiple Instances**: Run multiple instances of each agent with load balancing
2. **Health Checks**: Configure automated health checks and restart policies
3. **Monitoring**: Integrate with monitoring systems (Prometheus, Grafana)
4. **Alerting**: Set up alerts for failed health checks or error rates

### Backup and Recovery

#### Backup Strategy

1. **Workspace Directory**: Contains all task and plan data
   - Back up regularly (e.g., daily)
   - Use version control for critical files

2. **State Directory**: Contains lock files and process state
   - Less critical but should be backed up
   - Can be regenerated if lost

#### Backup Commands

```bash
# Create a timestamped backup
tar -czf automation-backup-$(date +%Y%m%d-%H%M%S).tar.gz \
    /var/lib/automation/workspace \
    /var/lib/automation/state

# Copy to remote storage
rsync -avz automation-backup-*.tar.gz user@backup-server:/backups/automation/
```

#### Recovery Procedure

```bash
# Stop all services
docker-compose down

# Restore from backup
tar -xzf automation-backup-20240209-150000.tar.gz -C /

# Start services
docker-compose up -d
```

### Security

#### File Permissions

Ensure proper permissions on workspace and state directories:

```bash
# Set ownership to the automation user
chown -R automation:automation /var/lib/automation

# Set appropriate permissions
chmod -R 750 /var/lib/automation/workspace
chmod -R 700 /var/lib/automation/state
```

#### Network Security

If the agents need network access:

1. Use a private network (Docker networks or VPN)
2. Restrict outbound connections
3. Use firewalls to limit access

#### Secrets Management

Avoid storing secrets in the workspace. Use environment variables or a secrets manager:

```bash
# Use Docker secrets
docker secret create api_key ./api_key.txt

# Reference in docker-compose.yml
services:
  prompt:
    secrets:
      - api_key

secrets:
  api_key:
    external: true
```

### Performance Tuning

#### Agent Intervals

Adjust agent intervals based on your workload:

| Agent | Default Interval | Consider Shorter If | Consider Longer If |
|-------|------------------|---------------------|-------------------|
| Architect | 40 min | Frequent planning needed | Planning is expensive |
| Janitor | 20 min | High task churn | Low task churn |
| Prompt | 5 min | Many quick tasks | Few quick tasks |

#### Resource Allocation

Increase resource limits for intensive workloads:

```yaml
# docker-compose.yml
services:
  architect:
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 1G
        reservations:
          cpus: '1.0'
          memory: 512M
```

## Troubleshooting

For detailed troubleshooting information, see [Troubleshooting Guide](troubleshooting.md).

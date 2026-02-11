# Troubleshooting Guide

This guide helps you diagnose and resolve common issues with the gastown system.

## Table of Contents

- [Quick Diagnostics](#quick-diagnostics)
- [Debug Mode](#debug-mode)
- [Common Issues](#common-issues)
- [Lock File Issues](#lock-file-issues)
- [Performance Problems](#performance-problems)
- [Docker Issues](#docker-issues)
- [Getting Help](#getting-help)

## Quick Diagnostics

Before diving into specific issues, perform these quick diagnostic checks:

```bash
# Check if containers are running
docker-compose ps

# Check container health status
docker-compose ps

# View recent logs for all services
docker-compose logs --tail=50

# Check disk space
df -h

# Check if state directory exists and is accessible
ls -la /path/to/state
```

Expected output for healthy system:

```
NAME                      COMMAND                  SERVICE   STATUS
automation-architect      "/automation-rust/ar…"   architect running (healthy)
automation-janitor        "/automation-rust/ja…"   janitor   running (healthy)
automation-prompt         "/automation-rust/pr…"   prompt    running (healthy)
```

## Debug Mode

Enabling debug mode provides detailed logging to help diagnose issues.

### Enabling Debug Logging

Set the `RUST_LOG` environment variable to `debug` or `trace`:

```bash
# Enable debug logging
RUST_LOG=debug

# Enable trace logging (most verbose)
RUST_LOG=trace

# Set for specific modules only
RUST_LOG=automation_state=debug,automation_workspace=info
```

### Using Debug Mode with Docker Compose

Edit the `.env` file:

```bash
# .env
RUST_LOG=debug
```

Then restart the services:

```bash
docker-compose down
docker-compose up -d
docker-compose logs -f
```

### Using Debug Mode with Direct Binary Execution

```bash
# Run with debug logging
RUST_LOG=debug ./target/release/architect --workspace /workspace --state /workspace/.state

# Run with trace logging
RUST_LOG=trace ./target/release/architect --workspace /workspace --state /workspace/.state
```

### Enable Backtraces

For detailed error information, enable Rust backtraces:

```bash
# Enable full backtraces
RUST_BACKTRACE=1 ./target/release/architect --workspace /workspace --state /workspace/.state

# Enable full backtraces with source code locations
RUST_BACKTRACE=full ./target/release/architect --workspace /workspace --state /workspace/.state
```

### Log Levels Reference

| Level | Description | When to Use |
|-------|-------------|-------------|
| `error` | Only errors | Production (default) |
| `warn` | Warnings and errors | Production with warnings |
| `info` | Info, warnings, and errors | Normal operation (default) |
| `debug` | Debug information | Troubleshooting |
| `trace` | All tracing information | Deep debugging |

## Common Issues

### Issue: Container fails to start

**Symptoms:**
- `docker-compose up -d` shows errors
- `docker-compose ps` shows services as "Exit" status
- Logs show "Cannot start service" errors

**Possible Causes:**
1. Port conflicts
2. Invalid environment variables
3. Missing volumes
4. Permission issues

**Solutions:**

```bash
# 1. Check for port conflicts
netstat -tuln | grep -E ':(3000|3001|3002)'
# or on macOS/Linux
lsof -i :3000

# 2. Verify environment variables
cat .env

# 3. Check if volumes exist
docker volume ls

# 4. Check container logs for specific errors
docker-compose logs architect

# 5. Try recreating containers
docker-compose down
docker-compose up -d --force-recreate
```

### Issue: Container exits immediately after starting

**Symptoms:**
- `docker-compose ps` shows services with "Exit" status
- Logs show the process exited quickly

**Possible Causes:**
1. Missing required arguments
2. Invalid workspace path
3. Invalid state path
4. Missing dependencies

**Solutions:**

```bash
# 1. Check container logs for specific errors
docker-compose logs architect

# 2. Verify workspace directory exists
ls -la /path/to/workspace

# 3. Verify state directory exists
ls -la /path/to/state

# 4. Run container in foreground to see errors
docker-compose run --rm architect

# 5. Check .env configuration
cat .env
```

### Issue: Agents not executing tasks

**Symptoms:**
- Containers are running and healthy
- No new tasks are being processed
- Workspace files not being updated

**Possible Causes:**
1. Agent interval is too long
2. No tasks in TODO.md
3. Lock file stuck
4. Agent disabled

**Solutions:**

```bash
# 1. Check agent interval in .env
cat .env | grep INTERVAL

# 2. Verify TODO.md has tasks
cat /path/to/workspace/TODO.md

# 3. Check for stuck lock files
ls -la /path/to/state/locks/

# 4. Force immediate execution
# Add to .env:
AUTOMATION_IMMEDIATE=true
docker-compose restart

# 5. View agent logs
docker-compose logs -f prompt
```

### Issue: "Permission denied" errors

**Symptoms:**
- Logs show permission errors
- Agents cannot read/write files

**Possible Causes:**
1. Incorrect file permissions
2. Wrong user context in container
3. SELinux/AppArmor restrictions

**Solutions:**

```bash
# 1. Check file permissions
ls -la /path/to/workspace
ls -la /path/to/state

# 2. Fix permissions
chmod -R 755 /path/to/workspace
chmod -R 700 /path/to/state
chown -R 1000:1000 /path/to/workspace
chown -R 1000:1000 /path/to/state

# 3. Check container user context
docker-compose exec architect id

# 4. If using SELinux, set appropriate context
chcon -Rt svirt_sandbox_file_t /path/to/workspace
```

### Issue: Health checks failing

**Symptoms:**
- `docker-compose ps` shows "unhealthy" status
- Containers restart frequently

**Possible Causes:**
1. Agent process crashed
2. Health check script not working
3. Long-running operations
4. Resource exhaustion

**Solutions:**

```bash
# 1. Check container logs for crashes
docker-compose logs --tail=100

# 2. Inspect health check status
docker inspect --format='{{json .State.Health}}' automation-architect | jq

# 3. Check resource usage
docker stats

# 4. Manually run health check
docker-compose exec architect /automation-rust/health-check.sh

# 5. Increase health check timeout if needed
# Edit docker-compose.yml:
healthcheck:
  timeout: 30s  # Increase from 10s
```

### Issue: High memory usage

**Symptoms:**
- Containers using more memory than expected
- System memory exhaustion

**Possible Causes:**
1. Large workspace
2. Many concurrent operations
3. Memory leaks
4. Insufficient system memory

**Solutions:**

```bash
# 1. Check container memory usage
docker stats

# 2. View system memory
free -h

# 3. Check for memory leaks in logs
docker-compose logs | grep -i memory

# 4. Increase memory limit in docker-compose.yml
services:
  architect:
    deploy:
      resources:
        limits:
          memory: 1G  # Increase from 512M

# 5. Restart containers
docker-compose restart
```

### Issue: Agents not responding

**Symptoms:**
- Containers are running but not processing
- No log output
- Tasks stuck

**Possible Causes:**
1. Deadlock
2. Hanging process
3. Network issue
4. Resource contention

**Solutions:**

```bash
# 1. Check if process is running inside container
docker-compose exec architect ps aux

# 2. View current logs
docker-compose logs --tail=50

# 3. Restart the service
docker-compose restart architect

# 4. Check for network issues
docker-compose exec architect ping -c 3 google.com

# 5. If unresponsive, force recreate
docker-compose up -d --force-recreate architect
```

### Issue: JSON parsing errors

**Symptoms:**
- Logs show JSON parsing errors
- State files corrupted
- Workspace state inconsistent

**Possible Causes:**
1. Corrupted state file
2. Concurrent writes
3. Invalid JSON format
4. File system errors

**Solutions:**

```bash
# 1. Validate state file JSON
cat /path/to/state/prompt.json | jq

# 2. If corrupted, restore from backup
tar -xzf automation-backup-*.tar.gz

# 3. Check for concurrent write issues
docker-compose logs | grep -i concurrent

# 4. Verify file system integrity
fsck /dev/sda1  # Replace with your device

# 5. Clear corrupted state (last resort)
# WARNING: This will lose state data
rm /path/to/state/prompt.json
```

## Lock File Issues

### Issue: Stale lock file preventing execution

**Symptoms:**
- Agent cannot acquire lock
- Logs show "Lock already held" error
- No process actually holding the lock

**Diagnosis:**

```bash
# Check lock files
ls -la /path/to/state/locks/

# View lock file contents
cat /path/to/state/locks/architect.lock

# Check if process is running
PID=$(cat /path/to/state/locks/architect.lock | jq -r '.pid')
ps -p $PID
```

**Solutions:**

```bash
# Option 1: Automatic cleanup (recommended)
# The Rust implementation automatically cleans stale locks
# Restart the agent to trigger cleanup
docker-compose restart architect

# Option 2: Manual cleanup
# Verify process is not running first
PID=$(cat /path/to/state/locks/architect.lock | jq -r '.pid')
if ! ps -p $PID > /dev/null 2>&1; then
  echo "Process $PID is not running, removing lock"
  rm /path/to/state/locks/architect.lock
fi

# Option 3: Force cleanup (use with caution)
# WARNING: Only use if you're certain no process is holding the lock
rm /path/to/state/locks/*.lock
```

### Issue: Lock timeout errors

**Symptoms:**
- Logs show "Lock acquisition timeout"
- Agents fail to start or execute

**Diagnosis:**

```bash
# Check lock file timestamps
ls -la /path/to/state/locks/

# View lock file details
stat /path/to/state/locks/architect.lock
```

**Solutions:**

```bash
# 1. Check if another instance is running
docker-compose ps

# 2. Check if process is still alive
PID=$(cat /path/to/state/locks/architect.lock | jq -r '.pid')
ps -p $PID

# 3. If process is dead but lock exists
docker-compose restart architect

# 4. Increase lock timeout (if needed)
# This requires code modification to change the timeout value
# Default is 30 seconds

# 5. Check for multiple instances
ps aux | grep architect
```

### Issue: Multiple agents holding same lock

**Symptoms:**
- Concurrent lock conflicts
- Agents interfering with each other
- Race conditions

**Diagnosis:**

```bash
# Check for multiple running instances
docker-compose ps

# Check processes on host
ps aux | grep -E "architect|janitor|prompt"

# Check lock file count
ls -la /path/to/state/locks/
```

**Solutions:**

```bash
# 1. Ensure only one instance of each agent is running
docker-compose down
docker-compose up -d

# 2. Check .env for duplicate configurations
cat .env

# 3. Check for other Docker containers
docker ps | grep automation

# 4. Remove old containers if needed
docker container prune
```

## Performance Problems

### Issue: Slow execution

**Symptoms:**
- Tasks taking longer than expected
- High CPU usage
- System sluggish

**Diagnosis:**

```bash
# Check CPU usage
docker stats

# Check execution time in logs
docker-compose logs prompt | grep "execution completed"

# Check system load
uptime
top
```

**Solutions:**

```bash
# 1. Check for resource contention
docker stats

# 2. Review agent configuration
cat .env | grep -E "INTERVAL|TIMEOUT"

# 3. Increase agent intervals if workload is high
# Edit .env:
AUTOMATION_INTERVAL=600  # Increase from 300

# 4. Optimize workspace size
# Archive old tasks
mv /path/to/workspace/COMPLETED.md /path/to/workspace/COMPLETED-archive.md
echo "# COMPLETED" > /path/to/workspace/COMPLETED.md

# 5. Use release build for better performance
# Already default in Docker image

# 6. Check for external dependencies
# If agents depend on external APIs, check their performance
```

### Issue: High disk I/O

**Symptoms:**
- Slow file operations
- High disk usage
- System disk thrashing

**Diagnosis:**

```bash
# Check disk I/O
iostat -x 1

# Check disk usage
df -h

# Check file counts
find /path/to/workspace -type f | wc -l
find /path/to/state -type f | wc -l
```

**Solutions:**

```bash
# 1. Move workspace to faster disk
# Ensure workspace is on SSD if possible

# 2. Archive old data
tar -czf workspace-archive.tar.gz /path/to/workspace

# 3. Clean up old state files
find /path/to/state -name "*.json" -mtime +7 -delete

# 4. Increase Docker I/O limits if needed
# Edit docker-compose.yml:
services:
  architect:
    deploy:
      resources:
        limits:
          # Add I/O limits if your Docker supports it
```

### Issue: Startup delay

**Symptoms:**
- Agents take long time to start
- Initial lock acquisition is slow
- First execution delayed

**Diagnosis:**

```bash
# Measure startup time
time docker-compose up -d

# Check image size
docker images | grep automation-rust

# Check disk space
df -h
```

**Solutions:**

```bash
# 1. Ensure using the latest image
docker pull automation-rust:latest

# 2. Prune old images and containers
docker system prune -a

# 3. Use --no-cache for fresh build
docker build --no-cache -t automation-rust .

# 4. Pre-pull image to avoid download delay
docker pull automation-rust:latest

# 5. Use release build (already default)
# The Dockerfile uses release builds
```

## Docker Issues

### Issue: Cannot pull Docker image

**Symptoms:**
- `docker pull` fails
- Image not found errors
- Network timeout

**Solutions:**

```bash
# 1. Check Docker daemon status
docker info

# 2. Check network connectivity
ping -c 3 docker.io

# 3. Try building from source
docker build -t automation-rust .

# 4. Check Docker Hub status
curl -s https://status.docker.com/

# 5. Use a mirror if available
# Edit /etc/docker/daemon.json:
{
  "registry-mirrors": ["https://mirror.gcr.io"]
}
```

### Issue: Docker Compose not found

**Symptoms:**
- `docker-compose` command not found
- Permission denied errors

**Solutions:**

```bash
# 1. Check if docker-compose is installed
docker-compose --version

# 2. If not installed, install it
# Linux:
sudo curl -L "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
sudo chmod +x /usr/local/bin/docker-compose

# 3. Use Docker Compose V2 (if available)
docker compose version

# 4. Check PATH
echo $PATH | grep docker
```

### Issue: Volume mounting issues

**Symptoms:**
- Containers cannot access mounted volumes
- "Permission denied" errors
- "No such file or directory" errors

**Solutions:**

```bash
# 1. Check if volumes exist
docker volume ls

# 2. Check volume details
docker volume inspect automation-workspace

# 3. Verify volume mounts in docker-compose.yml
cat docker-compose.yml | grep -A 10 "volumes:"

# 4. Check SELinux/AppArmor
# On systems with SELinux:
getenforce
# If enforcing, set context:
chcon -Rt svirt_sandbox_file_t /path/to/workspace

# 5. Recreate volumes
docker-compose down -v
docker-compose up -d
```

### Issue: Container networking issues

**Symptoms:**
- Containers cannot communicate
- DNS resolution failures
- Connection timeouts

**Solutions:**

```bash
# 1. Check Docker network
docker network ls

# 2. Inspect network details
docker network inspect automation-rust_default

# 3. Test DNS resolution
docker-compose exec architect ping -c 3 janitor

# 4. Recreate network
docker-compose down
docker-compose up -d

# 5. Check firewall rules
# Ensure Docker traffic is allowed
sudo iptables -L -n -v | grep docker
```

## Getting Help

### Before Asking for Help

Before seeking help, gather the following information:

```bash
# 1. System information
uname -a
docker --version
docker-compose --version

# 2. Container status
docker-compose ps

# 3. Recent logs (last 100 lines)
docker-compose logs --tail=100 > logs.txt

# 4. Configuration
cat .env

# 5. Lock file status
ls -la /path/to/state/locks/

# 6. Disk and memory
df -h
free -h
```

### Collect Diagnostic Information

Create a diagnostic bundle:

```bash
#!/bin/bash
# create-diagnostics.sh

DIAGNOSTICS_DIR="diagnostics-$(date +%Y%m%d-%H%M%S)"
mkdir -p $DIAGNOSTICS_DIR

# System information
uname -a > $DIAGNOSTICS_DIR/system.txt
docker --version >> $DIAGNOSTICS_DIR/system.txt
docker-compose --version >> $DIAGNOSTICS_DIR/system.txt

# Container status
docker-compose ps > $DIAGNOSTICS_DIR/containers.txt

# Logs
docker-compose logs --tail=500 > $DIAGNOSTICS_DIR/logs.txt

# Configuration
cp .env $DIAGNOSTICS_DIR/

# Docker Compose file
cp docker-compose.yml $DIAGNOSTICS_DIR/

# Lock files
cp -r /path/to/state/locks $DIAGNOSTICS_DIR/

# Create archive
tar -czf $DIAGNOSTICS_DIR.tar.gz $DIAGNOSTICS_DIR

echo "Diagnostics created: $DIAGNOSTICS_DIR.tar.gz"
```

### Common Channels for Help

1. **Documentation**: Check the [README](../README.md) and other documentation files
2. **Troubleshooting Guide**: You're reading it! Check for similar issues
3. **Issue Tracker**: File an issue on the project repository with:
   - Clear description of the problem
   - Steps to reproduce
   - Expected vs actual behavior
   - Diagnostic bundle
4. **Community Forums**: Search for similar issues in community discussions
5. **Support Team**: Contact support if available

### Useful Commands for Debugging

```bash
# View real-time logs with filtering
docker-compose logs -f | grep -E "ERROR|WARN"

# Count errors in logs
docker-compose logs | grep -c ERROR

# Find recent errors
docker-compose logs --since="1h" | grep ERROR

# View resource usage
docker stats --no-stream

# Inspect container configuration
docker inspect automation-architect

# Execute shell in container
docker-compose exec architect /bin/sh

# Check environment variables in container
docker-compose exec architect env

# View file system in container
docker-compose exec architect ls -la /workspace

# Check network connectivity
docker-compose exec architect ping -c 3 google.com

# View process list in container
docker-compose exec architect ps aux

# Check disk usage in container
docker-compose exec architect df -h

# View memory usage in container
docker-compose exec architect free -h
```

### Escalation Procedure

If you cannot resolve the issue:

1. **Document everything**: Take notes of what you tried and the results
2. **Create backup**: Before any destructive actions
3. **Rollback if necessary**: Use the [Migration Guide](migration.md) rollback procedure
4. **Contact support**: Provide the diagnostic bundle
5. **Consider fallback**: Use the Node.js implementation if available

### Preventive Measures

To minimize issues:

1. **Regular monitoring**: Set up log monitoring and alerts
2. **Backup frequently**: Backup workspace and state regularly
3. **Update regularly**: Keep Docker images and system updated
4. **Resource monitoring**: Monitor CPU, memory, and disk usage
5. **Log rotation**: Implement log rotation to prevent disk exhaustion

## Additional Resources

- [Deployment Guide](deployment.md) - Detailed deployment instructions
- [Migration Guide](migration.md) - Migrating from Node.js to Rust
- [Architecture Documentation](architecture.md) - System architecture and design decisions
- [API Documentation](api/) - Module and API reference

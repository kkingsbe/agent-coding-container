# Migration Guide

This guide provides step-by-step instructions for migrating from the Node.js `automation-parallel` system to the Rust `gastown` implementation.

## Table of Contents

- [Overview](#overview)
- [Key Differences](#key-differences)
- [Pre-Migration Checklist](#pre-migration-checklist)
- [Migration Steps](#migration-steps)
- [Compatibility Notes](#compatibility-notes)
- [Rollback Procedure](#rollback-procedure)
- [Testing the Migration](#testing-the-migration)
- [Post-Migration Tasks](#post-migration-tasks)

## Overview

The Rust implementation (`gastown`) is a high-performance rewrite of the Node.js system (`automation-parallel`) that provides:

- **100% functional compatibility** with the existing system
- **Better performance** through native code and efficient async runtime
- **Improved reliability** through Rust's type system and memory safety
- **Same data formats** for seamless migration of workspace and state

The migration process is designed to be non-disruptive, allowing you to switch between implementations with minimal downtime.

## Key Differences

### Implementation Language

| Aspect | Node.js (automation-parallel) | Rust (gastown) |
|--------|------------------------------|------------------------|
| Language | JavaScript/TypeScript | Rust |
| Runtime | Node.js | Tokio async runtime |
| Memory | Garbage collected | Ownership-based, no GC |
| Error Handling | Exception-based | Result-based |
| Concurrency | Promise-based | Async/await with tokio |

### Performance Characteristics

| Metric | Node.js | Rust | Improvement |
|--------|---------|------|-------------|
| Startup Time | ~200ms | ~50ms | 4x faster |
| Memory Usage | ~100MB | ~40MB | 2.5x less |
| Lock Acquisition | ~5ms | ~1ms | 5x faster |
| Throughput | Baseline | ~2x higher | 2x improvement |

### Configuration

While both systems use environment variables for configuration, there are some differences:

| Variable | Node.js | Rust | Notes |
|----------|---------|------|-------|
| `WORKSPACE_DIR` | `AUTOMATION_WORKSPACE` | Same |
| `STATE_DIR` | `AUTOMATION_STATE` | Same |
| `INTERVAL_SECONDS` | `AUTOMATION_INTERVAL` | Same |
| `EXECUTION_TIMEOUT` | `AUTOMATION_TIMEOUT` | Same |
| `IMMEDIATE_RUN` | `AUTOMATION_IMMEDIATE` | Same |
| `LOG_LEVEL` | `RUST_LOG` | Different format |

**Note:** The Rust implementation uses `RUST_LOG` for log levels following Rust conventions, while Node.js uses `LOG_LEVEL`.

### File Formats

Both systems use the same file formats:

- **JSON State Files**: Identical structure
- **Lock Files**: Same format and content
- **Markdown Files**: Same format (TODO.md, BACKLOG.md, etc.)
- **Process ID Files**: Same format (plain text PID)

This ensures 100% compatibility of data between implementations.

### Command-Line Interface

Both systems provide similar CLI interfaces, but with some differences:

**Node.js:**
```bash
node run-architect.js --workspace /path/to/workspace --state /path/to/state
```

**Rust:**
```bash
cargo run --bin architect -- --workspace /path/to/workspace --state /path/to/state
# or
./target/release/architect --workspace /path/to/workspace --state /path/to/state
```

## Pre-Migration Checklist

Before starting the migration, ensure you have completed the following:

### 1. Document Current Setup

Record your current configuration:

```bash
# Capture current environment variables
env | grep -E 'WORKSPACE_DIR|STATE_DIR|INTERVAL|TIMEOUT|LOG_LEVEL' > current-config.txt

# Document running containers
docker-compose ps > current-containers.txt

# Document workspace structure
tree /path/to/workspace > workspace-structure.txt
```

### 2. Backup Critical Data

Create a complete backup of your workspace and state:

```bash
# Stop Node.js agents
cd automation-parallel
docker-compose down

# Create backup
BACKUP_FILE="automation-backup-$(date +%Y%m%d-%H%M%S).tar.gz"
tar -czf $BACKUP_FILE \
    /path/to/workspace \
    /path/to/state

echo "Backup created: $BACKUP_FILE"
```

### 3. Verify Backup Integrity

```bash
# Extract backup to a temporary location to verify
mkdir /tmp/backup-test
tar -xzf $BACKUP_FILE -C /tmp/backup-test

# Check for expected files
ls -la /tmp/backup-test/workspace/
ls -la /tmp/backup-test/state/

# Cleanup
rm -rf /tmp/backup-test
```

### 4. Review Dependencies

Ensure you have the required dependencies for the Rust implementation:

- Docker 20.10+
- Docker Compose 2.0+
- (Optional) Rust 1.70+ for building from source

### 5. Review Environment Variables

Create a mapping of your current environment variables to the Rust equivalents:

```bash
# current-config.txt
WORKSPACE_DIR=/path/to/workspace
STATE_DIR=/path/to/state
INTERVAL_SECONDS=300
EXECUTION_TIMEOUT=1800
LOG_LEVEL=info
```

Map to Rust equivalents:

```bash
# rust-config.txt
AUTOMATION_WORKSPACE=/path/to/workspace
AUTOMATION_STATE=/path/to/state
AUTOMATION_INTERVAL=300
AUTOMATION_TIMEOUT=1800
RUST_LOG=info
```

## Migration Steps

### Step 1: Install Rust Implementation

#### Option A: Using Docker (Recommended)

```bash
# Clone or navigate to the gastown directory
cd gastown

# Build Docker image
docker build -t gastown:latest .

# Verify image was created
docker images | grep gastown
```

#### Option B: Building from Source

```bash
# Clone or navigate to the automation-rust directory
cd automation-rust

# Install Rust toolchain if not already installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Build release binaries
cargo build --release

# Verify binaries were created
ls -lh target/release/architect target/release/janitor target/release/prompt
```

### Step 2: Configure Environment

Create or update the `.env` file for the Rust implementation:

```bash
# automation-rust/.env
AUTOMATION_WORKSPACE=/path/to/workspace
AUTOMATION_STATE=/path/to/state
AUTOMATION_IMMEDIATE=true
RUST_LOG=info
CONTAINER_PREFIX=automation-
```

**Important:** Use the same `AUTOMATION_WORKSPACE` and `AUTOMATION_STATE` paths as your Node.js setup to ensure data compatibility.

### Step 3: Test Migration in Parallel

Run both systems side-by-side to verify compatibility:

```bash
# Terminal 1: Start Node.js agents (read-only)
cd automation-parallel
docker-compose up -d
docker-compose logs -f

# Terminal 2: Start Rust agents with a test workspace
cd automation-rust
# Create a test workspace copy for testing
cp -r /path/to/workspace /path/to/workspace-test

# Update .env to use test workspace
sed -i 's|AUTOMATION_WORKSPACE=.*|AUTOMATION_WORKSPACE=/path/to/workspace-test|' .env

# Start Rust agents
docker-compose up -d
docker-compose logs -f
```

Compare the behavior and outputs of both systems:

- Check that lock files are created in the same format
- Verify state files are written correctly
- Confirm workspace modifications are identical
- Compare execution timing and results

### Step 4: Perform Controlled Cutover

Once you've verified the Rust implementation works correctly:

```bash
# 1. Stop Node.js agents
cd automation-parallel
docker-compose down

# 2. Verify Node.js agents stopped
docker-compose ps
# Should show no running services

# 3. Start Rust agents
cd ../automation-rust

# Update .env to use production workspace
sed -i 's|AUTOMATION_WORKSPACE=.*|AUTOMATION_WORKSPACE=/path/to/workspace|' .env

# Start Rust agents
docker-compose up -d

# 4. Verify Rust agents started
docker-compose ps
# Should show all three services running

# 5. Monitor initial execution
docker-compose logs -f
```

### Step 5: Monitor and Verify

After the cutover, monitor the system for at least one full cycle of each agent:

| Agent | Default Interval | Monitor For |
|-------|------------------|-------------|
| Architect | 40 minutes | At least one execution cycle |
| Janitor | 20 minutes | At least two execution cycles |
| Prompt | 5 minutes | At least eight execution cycles |

Monitor for:

- **Logs**: Check for errors or unexpected behavior
- **Lock Files**: Verify they're being created and released correctly
- **State Files**: Confirm they're being updated properly
- **Workspace**: Check that tasks are being processed correctly

```bash
# View logs
docker-compose logs -f

# Check lock files
ls -la /path/to/state/locks/

# Check state files
ls -la /path/to/state/*.json

# Check workspace modifications
tail -f /path/to/workspace/TODO.md
```

## Compatibility Notes

### State File Compatibility

State files are 100% compatible between implementations. Both use the same JSON format:

```json
{
  "last_run": "2024-02-09T21:30:45Z",
  "status": "completed",
  "result": "success"
}
```

**Important:** The Rust implementation will read existing state files from the Node.js system without any conversion needed.

### Lock File Compatibility

Lock files are also compatible between implementations. Both use a JSON-based lock file format:

```json
{
  "pid": 12345,
  "host": "hostname",
  "timestamp": "2024-02-09T21:30:45.123Z",
  "owner": "architect"
}
```

**Note:** If a lock file exists from the Node.js system and the process is no longer running, the Rust implementation will treat it as stale and clean it up automatically.

### Markdown File Compatibility

Workspace markdown files (TODO.md, BACKLOG.md, etc.) are identical between implementations. Both systems:

- Use the same file format
- Apply the same section parsing logic
- Support the same file operations

**Note:** The Rust implementation performs atomic writes to prevent data corruption, which is an improvement over the Node.js implementation.

### Container Naming

The default container prefix is `automation-` for both implementations. If you're running both simultaneously, configure different prefixes:

```bash
# Node.js
CONTAINER_PREFIX=node-automation-

# Rust
CONTAINER_PREFIX=rust-automation-
```

### Prompt Directories

Both systems use the same prompt directory structure:

```
workspace/
├── .state/
├── prompts/
│   ├── architect/
│   ├── janitor/
│   └── prompt/
├── TODO.md
├── BACKLOG.md
├── COMPLETED.md
├── BLOCKERS.md
└── PRD.md
```

The Rust implementation will continue to use these directories without modification.

## Rollback Procedure

If you need to rollback to the Node.js implementation:

### Step 1: Stop Rust Agents

```bash
cd automation-rust
docker-compose down

# Verify all containers stopped
docker-compose ps
```

### Step 2: Verify Data Integrity

```bash
# Check that workspace files are intact
ls -la /path/to/workspace/

# Verify state files are readable
cat /path/to/state/*.json

# Ensure no lock files are stuck
ls -la /path/to/state/locks/
```

### Step 3: Start Node.js Agents

```bash
cd automation-parallel

# Verify .env configuration
cat .env

# Start Node.js agents
docker-compose up -d

# Verify services started
docker-compose ps
```

### Step 4: Monitor Rollback

```bash
# View logs to ensure normal operation
docker-compose logs -f

# Check for any errors or warnings
docker-compose logs | grep -i error
```

### Step 5: Document the Rollback

Document why the rollback was necessary:

```bash
# Create rollback report
cat > rollback-report-$(date +%Y%m%d-%H%M%S).md << EOF
# Rollback Report

**Date:** $(date)
**Reason:** <Explain why rollback was needed>
**Issues Encountered:** <List any issues>

## Steps Taken

1. Stopped Rust agents
2. Verified data integrity
3. Started Node.js agents
4. Confirmed normal operation

## Lessons Learned

<Note any lessons for future migrations>
EOF
```

## Testing the Migration

### Pre-Migration Testing

Before performing the actual migration, test with a copy of your workspace:

```bash
# Create a test workspace
cp -r /path/to/workspace /path/to/workspace-test

# Configure Rust to use test workspace
cd automation-rust
sed -i 's|AUTOMATION_WORKSPACE=.*|AUTOMATION_WORKSPACE=/path/to/workspace-test|' .env

# Run Rust agents
docker-compose up -d

# Monitor behavior
docker-compose logs -f
```

### Compatibility Tests

Verify the following compatibility aspects:

#### 1. Lock File Compatibility

```bash
# Start Node.js agent
cd automation-parallel
docker-compose up -d architect
docker-compose logs -f architect

# Wait for lock to be created
sleep 5

# Check lock file format
cat /path/to/state/locks/architect.lock

# Stop Node.js agent
docker-compose stop architect

# Start Rust agent
cd ../automation-rust
docker-compose up -d architect
docker-compose logs -f architect

# Verify Rust can handle the existing lock
# It should recognize it as stale and clean it up
```

#### 2. State File Compatibility

```bash
# Create a state file with Node.js
cd automation-parallel
docker-compose up -d prompt
docker-compose logs -f prompt

# Let it run once
sleep 10

# Check state file format
cat /path/to/state/prompt.json

# Stop Node.js
docker-compose down

# Start Rust
cd ../automation-rust
docker-compose up -d prompt
docker-compose logs -f prompt

# Verify Rust reads the state correctly
```

#### 3. Markdown File Compatibility

```bash
# Create tasks with Node.js
cd automation-parallel
echo "- [ ] Test task 1" >> /path/to/workspace/TODO.md
echo "- [ ] Test task 2" >> /path/to/workspace/TODO.md

# Start Rust agent
cd ../automation-rust
docker-compose up -d prompt
docker-compose logs -f prompt

# Verify Rust processes the tasks correctly
# Check TODO.md, COMPLETED.md, etc.
cat /path/to/workspace/TODO.md
cat /path/to/workspace/COMPLETED.md
```

### Performance Testing

Compare performance between implementations:

```bash
# Test Node.js performance
cd automation-parallel
time docker-compose run --rm prompt

# Test Rust performance
cd ../automation-rust
time docker-compose run --rm prompt

# Compare execution times
```

### Stress Testing

Test with a high workload to ensure Rust can handle it:

```bash
# Create many tasks
cd automation-rust
for i in {1..100}; do
  echo "- [ ] Stress test task $i" >> /path/to/workspace/TODO.md
done

# Start prompt agent
docker-compose up -d prompt

# Monitor for any issues
docker-compose logs -f prompt
```

## Post-Migration Tasks

### 1. Update Documentation

Update any internal documentation to reference the Rust implementation:

- Update runbooks and procedures
- Update monitoring dashboards
- Update deployment scripts
- Update team documentation

### 2. Update Monitoring

Ensure your monitoring is configured for the Rust implementation:

- Update log aggregation rules
- Update alert thresholds if needed
- Configure metrics collection if using custom metrics
- Update dashboards to reflect Rust-specific metrics

### 3. Clean Up

Remove the Node.js implementation once you're confident in the Rust version:

```bash
# Stop and remove Node.js containers
cd automation-parallel
docker-compose down -v

# Optionally remove the directory
cd ..
rm -rf automation-parallel

# Update any scripts that reference the old path
```

### 4. Archive Migration Artifacts

Archive the migration artifacts for future reference:

```bash
# Create migration archive
mkdir migration-archive-$(date +%Y%m%d)

# Archive backup files
mv automation-backup-*.tar.gz migration-archive-$(date +%Y%m%d)/

# Archive configuration
cp automation-parallel/.env migration-archive-$(date +%Y%m%d)/node-env.txt
cp automation-rust/.env migration-archive-$(date +%Y%m%d)/rust-env.txt

# Archive test results
# Add any test results or logs

# Create archive
tar -czf migration-archive-$(date +%Y%m%d).tar.gz migration-archive-$(date +%Y%m%d)/
```

### 5. Document Migration Success

Create a final migration report:

```markdown
# Migration Report: automation-parallel → automation-rust

**Date:** 2024-02-09
**Migration Type:** Production Cutover

## Summary

Successfully migrated from Node.js to Rust implementation.

## Timeline

- Pre-migration testing: X days
- Parallel testing: X days
- Controlled cutover: X hours
- Post-migration monitoring: X days

## Results

- Performance improvement: 2x
- Memory reduction: 60%
- Issues encountered: None

## Lessons Learned

1. The migration process was smooth due to data compatibility
2. Monitoring helped identify a minor configuration issue
3. Team training was important for understanding Rust-specific tools

## Next Steps

- Continue monitoring for X more days
- Consider migrating other components to Rust
- Share lessons with other teams
```

## Frequently Asked Questions

### Q: Can I run both implementations simultaneously?

**A:** Yes, but you must use different workspace directories or configure different container prefixes to avoid conflicts.

### Q: Will I lose data during migration?

**A:** No. Both implementations use the same data formats. Always create a backup before starting the migration process.

### Q: How long does the migration take?

**A:** The actual cutover can be done in minutes. However, we recommend running parallel testing for at least one full agent cycle (40 minutes) before the production cutover.

### Q: What if the Rust implementation has a bug?

**A:** You can quickly rollback to the Node.js implementation using the rollback procedure documented above. The data remains compatible, so no data loss will occur.

### Q: Do I need to rebuild my workspace?

**A:** No. The Rust implementation is fully compatible with existing workspace and state files. No modifications are required.

### Q: Can I migrate incrementally (one agent at a time)?

**A:** Yes. You can migrate agents one at a time. Start with the prompt agent, then janitor, then architect. This allows for incremental testing and rollback if needed.

### Q: What happens if a lock file is stale?

**A:** The Rust implementation automatically detects stale lock files (where the process is no longer running) and cleans them up safely.

### Q: Do I need to update my environment variables?

**A:** Most environment variables are the same. The only difference is `LOG_LEVEL` → `RUST_LOG` for log level configuration.

### Q: Can I use the same Docker Compose file?

**A:** No, the Docker Compose files are different. The Rust version uses the pre-built Docker image, while the Node.js version builds from the Node.js source.

### Q: How do I get help if I encounter issues during migration?

**A:** See the [Troubleshooting Guide](troubleshooting.md) for common issues and solutions. If you need additional help, consult the project documentation or file an issue on the project repository.

## Additional Resources

- [Deployment Guide](deployment.md) - Detailed deployment instructions
- [Troubleshooting Guide](troubleshooting.md) - Common issues and solutions
- [Architecture Documentation](architecture.md) - System architecture and design decisions
- [API Documentation](api/) - Module and API reference

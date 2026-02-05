# Agent Coding Container

**Autonomous software development using AI coding agents in a loop.**

A Docker-based system that runs [Kilo Code CLI](https://kilo.ai/cli) in continuous loops to build software from a Product Requirements Document (PRD). Inspired by [Geoffrey Huntley's Ralph Loop](https://ghuntley.com/loop/) and the [BMAD Method](https://github.com/bmad-code-org/BMAD-METHOD).

## How It Works

You provide a PRD. The system runs specialized AI agents in a loop until the project is complete.

```
┌─────────────────────────────────────────────────────────────────┐
│                         THE LOOP                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐          │
│   │   WORKER    │   │   JANITOR   │   │  ARCHITECT  │          │
│   │  (PROMPT)   │   │             │   │             │          │
│   ├─────────────┤   ├─────────────┤   ├─────────────┤          │
│   │ Every tick  │   │ Every 4     │   │ Every 8     │          │
│   │             │   │ ticks       │   │ ticks       │          │
│   ├─────────────┤   ├─────────────┤   ├─────────────┤          │
│   │ Implements  │   │ Cleans up   │   │ Reviews     │          │
│   │ one task    │   │ tech debt   │   │ architecture│          │
│   │ from TODO   │   │ and drift   │   │ and planning│          │
│   └─────────────┘   └─────────────┘   └─────────────┘          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Key insight:** Each agent runs with fresh context. Memory persists via git history and markdown files (TODO.md, ARCHITECTURE.md, LEARNINGS.md), not the LLM's context window. This prevents context pollution and allows indefinite operation.

## Results

We've tested this system extensively:

| Metric | Result |
|--------|--------|
| **Longest stable run** | 10+ hours without divergence |
| **Tasks completed per hour** | ~4-6 (depends on complexity) |
| **Context pollution** | None (fresh context each tick) |
| **Human intervention required** | Minimal (via comms/ system) |

The system successfully bootstrapped projects from just a PRD, created architecture docs, generated task lists, and implemented features—all autonomously.

## Quick Start

### Prerequisites

- Docker (v20.10+)
- Docker Compose (v2.0+)
- Kilo Code account (for API access)

### 1. Clone and Setup

```bash
git clone https://github.com/your-repo/agent-coding-container.git
cd agent-coding-container

# Create workspace with your PRD
mkdir -p workspace
cp your-prd.md workspace/PRD.md
```

### 2. Configure Kilo Code

Copy your Kilo Code config:
```bash
cp -r ~/.kilocode .kilocode/
```

### 3. Run

```bash
docker compose up
```

That's it. The system will:
1. Read your PRD
2. Bootstrap the project structure
3. Generate TODO.md with tasks
4. Implement tasks one at a time
5. Continue until `.done` file is created

## The Agent System

### Three-Prompt Architecture

The system uses three specialized prompts that run at different intervals:

| Agent | File | Frequency | Role |
|-------|------|-----------|------|
| **Worker** | `PROMPT.md` | Every tick | Implements one task from TODO.md |
| **Janitor** | `JANITOR.md` | Every 4 ticks | Cleans up drift, prunes completed TODOs |
| **Architect** | `ARCHITECT.md` | Every 8 ticks | Gap analysis, breaks down vague tasks |

This separation of concerns prevents any single agent from both planning and executing, which reduces drift and maintains focus.

### Why This Works

1. **Fresh context each iteration** — No context pollution from accumulated conversation
2. **Git as memory** — All progress persists in files and commits
3. **Single task enforcement** — Each session completes ONE task, then stops
4. **Specialized roles** — Planning, execution, and cleanup are separate concerns
5. **Human-in-the-loop option** — The `comms/` system allows async communication

## Project Structure

After bootstrap, your workspace will look like:

```
workspace/
├── PRD.md              # Your requirements (input)
├── TODO.md             # Task list (auto-generated, auto-maintained)
├── ARCHITECTURE.md     # Key decisions (auto-generated)
├── LEARNINGS.md        # Patterns discovered (auto-updated)
├── BLOCKERS.md         # Issues preventing progress
├── .state.json         # Loop state persistence
├── .done               # Completion marker
├── comms/
│   ├── inbox/          # Human → Agent messages
│   ├── outbox/         # Agent → Human questions
│   └── archive/        # Processed messages
└── src/                # Your actual code
```

## Configuration

### Tick Interval

Default is 10 minutes (600 seconds). Adjust via command:

```bash
# 5-minute ticks
docker compose run --rm agent_coding_container node /home/automation/run.js 300

# 15-minute ticks
docker compose run --rm agent_coding_container node /home/automation/run.js 900
```

Or via `.env`:
```env
DELAY_SECONDS=300
```

### Custom Workspace

```bash
MOUNT_HOST_DIR=/path/to/your/project docker compose up
```

## Human Communication

The agents can ask questions when blocked. Check `workspace/comms/outbox/` for RFIs (Requests for Information).

To respond:
1. Read the question in `comms/outbox/`
2. Create your response in `comms/inbox/`
3. The next iteration will pick it up

## Marking Complete

The loop runs until a `.done` file exists:

```bash
touch workspace/.done
```

The system considers itself complete when:
- All TODO.md items are checked
- All tests pass
- The app builds successfully

## Additional Loops

Beyond the build loop, you can run specialized loops for code quality:

### Bug Fixing Loop

Uses `BUGFIXER.md` and `BUGFIXER_BUGCHECK.md`:
- Discovers bugs via static analysis, test failures, code smells
- Fixes one bug per session with regression tests
- Tracks progress in `BUGS.md`

### Coverage Loop (Coming Soon)

- Identifies untested code paths
- Adds tests systematically
- Targets 80%+ coverage

### Type Safety Loop (Coming Soon)

- Finds `any` types and unsafe assertions
- Adds proper typing incrementally
- Escalates tsconfig strictness

## Advanced Usage

### Running Multiple Loops in Parallel

```yaml
# docker-compose.multi.yml
services:
  build_loop:
    build: .
    volumes:
      - ./workspace:/home/workspace
    command: ["node", "/home/automation/run.js", "600"]

  bugfix_loop:
    build: .
    volumes:
      - ./workspace:/home/workspace
    command: ["node", "/home/automation/run-bugfix.js", "900"]
```

### Hot-Reload Prompts (Development)

Mount the prompts directory for live editing:

```yaml
volumes:
  - ./workspace:/home/workspace
  - ./automation/prompts:/home/automation/prompts
```

### Resource Limits

```yaml
services:
  agent_coding_container:
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 4G
```

## Monitoring

### View Logs

```bash
docker compose logs -f
```

### Check Progress

```bash
# View current state
cat workspace/.state.json

# Count completed tasks
grep -c "^\- \[x\]" workspace/TODO.md

# Count remaining tasks  
grep -c "^\- \[ \]" workspace/TODO.md
```

### Inside the Container

```bash
docker exec -it agent_coding_container /bin/bash
```

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Container exits immediately | Check logs: `docker compose logs` |
| Changes not persisting | Verify volume mount: `docker compose config` |
| Automation stuck | Check `BLOCKERS.md` and `comms/outbox/` |
| Starting from iteration 1 | State file missing—check `.state.json` |
| Prompts not updating | Rebuild image: `docker compose build` |

## How It Compares

| Approach | Planning | Execution | Memory | Human Involvement |
|----------|----------|-----------|--------|-------------------|
| **This System** | Architect agent | Worker agent | Git + files | Optional (comms/) |
| **BMAD Method** | Heavy upfront | In-session | Agent handoffs | High |
| **Ralph Loop** | Minimal | Fresh each loop | Git only | Low (AFK) |
| **Claude Code** | Interactive | Interactive | Session | High |

## Contributing

PRs welcome. Key areas:
- Additional specialized loops (performance, security, accessibility)
- Better progress reporting and dashboards
- Integration with issue trackers (GitHub Issues, Linear)
- Multi-repo orchestration

## License

MIT

## Acknowledgments

- [Geoffrey Huntley](https://ghuntley.com/loop/) for the Ralph Loop concept
- [BMAD Method](https://github.com/bmad-code-org/BMAD-METHOD) for multi-agent architecture patterns
- [Kilo Code](https://kilo.ai) for the CLI that makes this possible
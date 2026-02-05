# Automation System

An automated project development system that runs Kilo Code in orchestrator mode to continuously build and improve a software project based on a Product Requirements Document (PRD).

## Overview

The automation system executes a series of prompts in a continuous loop, automating the software development lifecycle from initial project setup through implementation and completion. Each prompt has a specific role and runs on a defined schedule.

## Prerequisites

- **Node.js** (v14 or higher) - Required to run the automation script
- **Kilo Code CLI** - Must be installed and available in your system PATH
- **A workspace directory** - The automation expects a `workspace/` directory (at `../workspace/` relative to this directory) containing your project

## Installation

The automation script requires no additional dependencies beyond Node.js. Simply ensure:

1. Node.js is installed
2. The `kilocode` CLI command is available
3. The `workspace/` directory exists and contains your project files (at minimum, a `PRD.md`)

## How to Run

### Basic Usage (Default 10-minute delay)

```bash
cd automation
node run.js
```

This starts the automation with a 10-minute (600 second) delay between iterations.

### Custom Delay

You can specify a custom delay (in seconds) as the first argument:

```bash
# Run with 5-minute delay
node run.js 300

# Run with 15-minute delay
node run.js 900

# Run with 1-hour delay
node run.js 3600
```

## Iteration System

The automation runs prompts on a scheduled basis. Each iteration represents one cycle of the automation loop.

### Prompt Schedule

| Prompt  | Frequency       | Description                                                                 |
|---------|-----------------|-----------------------------------------------------------------------------|
| PROMPT.md | **Every iteration** | Primary development prompt. Handles project phases (bootstrap, implementation, verification) |
| JANITOR.md | **Every 4 iterations** | Repository maintenance. Cleans up TODOs, identifies drift, removes unused files |
| ARCHITECT.md | **Every 8 iterations** | Architecture review. Performs gap analysis, reviews blockers, plans next steps |

### Example Schedule

- **Iteration 1-3**: Only PROMPT.md runs
- **Iteration 4**: PROMPT.md + JANITOR.md
- **Iteration 5-7**: Only PROMPT.md
- **Iteration 8**: PROMPT.md + JANITOR.md + ARCHITECT.md
- **Iteration 12**: PROMPT.md + JANITOR.md
- **Iteration 16**: PROMPT.md + JANITOR.md + ARCHITECT.md

And so on...

## Prompt Descriptions

### PROMPT.md
The primary development prompt that runs every iteration. It:
- Detects the current project phase (BOOTSTRAP, IMPLEMENTATION, or VERIFICATION)
- Implements tasks from TODO.md one at a time
- Commits work with conventional commit messages
- Handles communication via `comms/inbox/` and `comms/outbox/`
- Maintains LEARNINGS.md for discovered patterns

### JANITOR.md
The repository maintainer that runs every 4 iterations. It:
- Marks completed tasks in TODO.md
- Identifies code drift from the PRD
- Cleans up unused files and empty directories
- Syncs documentation with implementation

### ARCHITECT.md
The lead architect that runs every 8 iterations. It:
- Performs gap analysis between PRD, TODO, and implementation
- Breaks down vague TODO items into atomic tasks
- Reviews and resolves blockers
- Writes architectural decisions to ARCHITECTURE.md

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

If the state file doesn't exist, the automation starts from iteration 1.

### Benefits of State Persistence

- **Resumability**: You can stop the automation (Ctrl+C) and restart later; it will continue from where it left off
- **Tracking**: Always know which iteration the project is on
- **Recovery**: If the script crashes or is interrupted, no progress is lost

## Marking Project as Complete

The project is considered complete when a `.done` file is created in the workspace directory.

### How to Mark Complete

The automation automatically checks for the `.done` file after each prompt execution. To mark a project complete:

1. **Manually create the file:**
   ```bash
   # In the workspace directory
   touch .done
   # Or on Windows
   type nul > .done
   ```

2. **Let the automation detect it:** During the next iteration, after a prompt completes successfully, the automation will detect the `.done` file and exit gracefully with a completion message.

### When to Mark Complete

According to the PROMPT.md guidelines, a project is complete when:
1. All TODO.md items are checked
2. All tests pass
3. The app builds successfully
4. Core PRD requirements have corresponding implementations

## Workspace Structure

The automation expects the following structure:

```
agent-coding-container/
├── automation/           # This directory
│   ├── README.md        # This file
│   ├── run.js           # Main automation script
│   ├── package.json
│   └── prompts/
│       ├── PROMPT.md    # Primary development prompt
│       ├── JANITOR.md   # Maintenance prompt
│       └── ARCHITECT.md # Architecture prompt
└── workspace/           # Target project directory
    ├── .state.json      # State persistence (auto-created)
    ├── .done            # Completion marker (user-created)
    ├── PRD.md           # Product Requirements Document
    ├── TODO.md          # Task list (created during bootstrap)
    ├── ARCHITECTURE.md  # Architecture decisions (created during bootstrap)
    ├── BLOCKERS.md      # Known issues (optional)
    └── comms/           # Communication directory
        ├── inbox/       # Human responses
        └── outbox/      # Questions to human
```

## Stopping the Automation

Press `Ctrl+C` to gracefully stop the automation. The current state will be saved before exit, allowing you to resume later.

## Example Commands

```bash
# Start automation with default 10-minute delay
cd automation
node run.js

# Start with 5-minute delay
node run.js 300

# Start with 30-minute delay for long-running tasks
node run.js 1800

# Start with 1-hour delay
node run.js 3600
```

## Troubleshooting

### Kilo Code command not found
Ensure the Kilo Code CLI is installed and available in your system PATH. Run `kilocode --help` to verify.

### Workspace directory not found
Create a `workspace/` directory at `../workspace/` relative to the automation directory, and ensure it contains at least a `PRD.md` file.

### Prompts not found
Verify that all prompt files exist:
- `automation/prompts/PROMPT.md`
- `automation/prompts/JANITOR.md`
- `automation/prompts/ARCHITECT.md`

### Automation not resuming
Check that the `workspace/.state.json` file exists and is valid JSON. If corrupted, delete it to start from iteration 1.

## How It Works

1. **Initialization**: The script checks for required prompts and workspace existence
2. **State Loading**: Loads saved state from `.state.json` or starts fresh
3. **Iteration Loop**:
   - Builds a queue of prompts to run based on the current iteration number
   - Executes prompts sequentially (not in parallel)
   - After each prompt, checks for `.done` file to determine completion
   - Saves state and waits for the configured delay before the next iteration
4. **Completion**: If `.done` is found, the automation exits gracefully

## Notes

- The automation runs Kilo Code with a 15-minute timeout per prompt
- All prompts are executed in orchestrator mode with auto-confirmation
- The workspace path is relative to the automation directory (one level up)
- The automation is designed to run continuously but can be stopped and resumed at any time

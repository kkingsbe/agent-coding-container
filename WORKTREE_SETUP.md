# Git Worktree Setup for AI Bugfixer

## One-Time Setup

```bash
# From your main project directory
cd /path/to/your/project

# Create the bugfix branch (if it doesn't exist)
git branch ai/bugfixes

# Create a worktree in a sibling directory
git worktree add ../project-ai-bugfix ai/bugfixes
```

This creates:
```
/path/to/
├── your/project/          ← You work here (main)
└── project-ai-bugfix/     ← AI works here (ai/bugfixes)
```

Both directories share the same `.git` history - commits in one are visible in the other.

## Running the Bugfixer

```bash
# Point the container at the worktree
MOUNT_HOST_DIR=/path/to/project-ai-bugfix LOOP_TYPE=bugfixer docker compose up
```

## Merging Fixes

```bash
# From your main project directory
cd /path/to/your/project
git merge ai/bugfixes --no-ff -m "fix: merge AI bugfixes"
```

Or cherry-pick specific fixes:
```bash
git log ai/bugfixes --oneline
git cherry-pick <commit-hash>
```

## Keeping the AI Branch Updated

When you've made significant changes to main that the AI should see:

```bash
# Go to the worktree
cd /path/to/project-ai-bugfix

# Rebase onto main
git rebase main

# Or merge main in
git merge main
```

## Cleanup

When you're done with the bugfixer loop:

```bash
cd /path/to/your/project
git worktree remove ../project-ai-bugfix
git branch -d ai/bugfixes  # Optional: delete the branch too
```

## Tips

- The worktree shares `node_modules`, `.git`, etc. with main - no extra disk space for those
- You can have multiple worktrees for different AI tasks (bugfix, coverage, etc.)
- Use `git worktree list` to see all active worktrees
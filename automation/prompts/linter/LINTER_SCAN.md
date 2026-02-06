# LINTER_SCAN.md

You are a code quality analyst. Your only job is to identify and prioritize
files that need linter compliance work. You do NOT fix anything. You analyze
and create a prioritized work plan.

## Rules

- **STRICT: Analysis Only.** Do not modify any source code files.
- **Document Everything.** Every file with lint issues gets an entry in LINT_TODO.md.
- **Be Specific.** Include file paths, error counts, and error categories.
- **Prioritize Strategically.** Consider file importance, error severity, and dependencies.
- **Commit your findings** before the session ends.

## Discovery Methods

### 1. Run the Linter

Determine the project type and run the appropriate linter:

```bash
# TypeScript/JavaScript projects
npm run lint 2>&1 | tee /tmp/lint-output.txt

# If no lint script, try direct:
npx eslint . --ext .ts,.tsx,.js,.jsx 2>&1 | tee /tmp/lint-output.txt

# Python projects
ruff check . 2>&1 | tee /tmp/lint-output.txt
# or
pylint **/*.py 2>&1 | tee /tmp/lint-output.txt

# Go projects
golangci-lint run 2>&1 | tee /tmp/lint-output.txt
```

### 2. Parse and Aggregate Errors

Count errors per file:
```bash
# For ESLint output
grep -E "^\s*[0-9]+:[0-9]+" /tmp/lint-output.txt | \
  sed 's/:.*//g' | sort | uniq -c | sort -rn

# For file-based output
grep -E "^(src/|lib/|app/)" /tmp/lint-output.txt | \
  cut -d: -f1 | sort | uniq -c | sort -rn
```

### 3. Categorize Error Types

```bash
# Extract unique rule violations
grep -oE "\b[a-z-]+/[a-z-]+\b|@typescript-eslint/[a-z-]+" /tmp/lint-output.txt | \
  sort | uniq -c | sort -rn

# Common high-impact categories:
# - @typescript-eslint/no-explicit-any    (type safety)
# - @typescript-eslint/no-unused-vars     (dead code)
# - no-console                            (debug artifacts)
# - prefer-const                          (immutability)
# - @typescript-eslint/explicit-function-return-type (documentation)
```

### 4. Assess File Importance

Check which files are:
- **Core modules:** Entry points, main business logic
- **Shared utilities:** Used by many other files
- **High-traffic:** Frequently imported
- **Test files:** Lower priority, but still important

```bash
# Find most-imported files
grep -rh "from ['\"]" src/ | grep -oE "from ['\"][^'\"]+['\"]" | \
  sort | uniq -c | sort -rn | head -20
```

## Prioritization Matrix

Score each file (higher = fix first):

| Factor | Weight | Scoring |
|--------|--------|---------|
| Error Count | 30% | 1-10 errors: 1pt, 11-30: 2pts, 31+: 3pts |
| Error Severity | 30% | Mostly errors: 3pts, Mixed: 2pts, Mostly warnings: 1pt |
| File Importance | 25% | Core: 3pts, Shared: 2pts, Feature: 1pt |
| Fix Complexity | 15% | Simple (auto-fixable): 3pts, Medium: 2pts, Complex: 1pt |

## LINT_TODO.md Format

Create/update LINT_TODO.md with this structure:

```markdown
# LINT_TODO.md

> Generated: [timestamp]
> Total Files: [count]
> Total Errors: [count]
> Total Warnings: [count]

## Summary by Rule

| Rule | Count | Auto-fixable | Priority |
|------|-------|--------------|----------|
| @typescript-eslint/no-explicit-any | 45 | No | HIGH |
| no-unused-vars | 32 | No | MEDIUM |
| prefer-const | 28 | Yes | LOW |

## Files to Fix (Prioritized)

### Priority 1: Critical Path
Files that block other fixes or are core infrastructure.

- [ ] `src/core/auth.ts` (12 errors, 3 warnings)
  - **Errors:** 8x no-explicit-any, 4x no-unused-vars
  - **Importance:** Core authentication module
  - **Estimated effort:** 15 min
  - **Notes:** Fix any types first, may reveal more issues

- [ ] `src/utils/helpers.ts` (8 errors, 5 warnings)
  - **Errors:** 5x no-explicit-any, 3x prefer-const
  - **Importance:** Shared utility, imported by 15 files
  - **Estimated effort:** 10 min
  - **Notes:** Auto-fix prefer-const first

### Priority 2: High Impact
Core business logic files with significant issues.

- [ ] `src/services/user.service.ts` (15 errors, 2 warnings)
  - **Errors:** ...
  - **Importance:** ...
  - **Estimated effort:** 20 min

### Priority 3: Medium Impact
Feature files with moderate issues.

- [ ] ...

### Priority 4: Low Impact
Test files, configs, or files with few issues.

- [ ] ...

## Deferred / Blocked

Files that cannot be fixed yet due to dependencies:

- `src/legacy/old-module.ts` - Requires refactor, tracked in TODO.md
- `src/generated/api-types.ts` - Auto-generated, fix generator instead

## Auto-fix Candidates

Files where `--fix` can resolve most issues:

```bash
npx eslint --fix src/utils/format.ts
npx eslint --fix src/components/Button.tsx
```

## Configuration Issues

Problems with the linter config itself:

- [ ] Rule X is too strict for this codebase
- [ ] Missing parser for .vue files
- [ ] Conflicting rules: A vs B

## Detection of Existing State

1. **If LINT_TODO.md doesn't exist:**
   - Full discovery pass
   - Create LINT_TODO.md from scratch

2. **If LINT_TODO.md exists:**
   - Re-run linter to get current state
   - Update counts and priorities
   - Mark newly-clean files as complete
   - Add any new files with issues
   - Preserve manual notes and blocked items

## Commit Convention

Use: `docs: update lint analysis findings`

## Session Flow

1. Detect project type and linter
2. Run full lint analysis
3. Parse and aggregate results
4. Assess file importance via import analysis
5. Score and prioritize files
6. Create/update LINT_TODO.md
7. Commit findings
8. STOP

## Important

You have ~15 minutes. Focus on creating an accurate, actionable work plan.
The LINTER loop will fix files one at a time in subsequent sessions.

Quality > Speed. A well-prioritized list saves hours of fixing work.
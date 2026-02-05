# LINTER.md

You are a code quality engineer working on this project. Your job is to
systematically bring the codebase into linter compliance, one file at a time.

## Phase Detection

First, examine the repository to determine what phase you're in:

1. **If LINT_TODO.md doesn't exist:**
    - You're in DISCOVERY phase
    - Run the linter and create LINT_TODO.md
    - Prioritize files by error count, severity, and importance
    - Do NOT fix anything yet—just document

2. **If LINT_TODO.md exists with unchecked items:**
    - You're in FIXING phase
    - Pick the highest-priority unchecked file from LINT_TODO.md
    - Fix ALL lint issues in that single file
    - Run linter on that file to verify
    - Mark it complete in LINT_TODO.md
    - Commit your changes

3. **If all LINT_TODO.md items are checked:**
    - You're in VERIFICATION phase
    - Run full linter across the codebase
    - If new issues found, add them to LINT_TODO.md and continue
    - If clean, create `.linter-done` with summary

## Rules

- **STRICT: Single File Enforcement.** Fix exactly ONE file per session.
- **Session Termination:** Once you have committed fixes for a file and checked it off in LINT_TODO.md, you MUST STOP.
- **Complete the File.** Don't partially fix a file. Fix ALL lint issues in it.
- **Always commit your work** before the session ends.
- **Update LINT_TODO.md** to reflect current state.
- **If blocked**, document in LINT_BLOCKERS.md and move to next file.
- **Preserve behavior.** Lint fixes must not change functionality.
- **Run tests** after fixing to ensure nothing broke.

## Files to Check

- `LINT_TODO.md` - Current prioritized file list (you create/maintain this)
- `LINT_BLOCKERS.md` - Files that can't be fixed and why
- `LINT_LEARNINGS.md` - Patterns discovered during fixing

## Fixing Strategy

### Step 1: Auto-fix First

Always try auto-fix before manual work:

```bash
# TypeScript/JavaScript
npx eslint --fix src/path/to/file.ts

# Python
ruff check --fix src/path/to/file.py

# Go
gofmt -w src/path/to/file.go
```

### Step 2: Manual Fixes

For issues that can't be auto-fixed:

**`no-explicit-any` / Type Safety:**
```typescript
// Bad
function process(data: any): any { ... }

// Good - Infer or define proper types
interface ProcessInput { id: string; value: number; }
interface ProcessOutput { result: string; }
function process(data: ProcessInput): ProcessOutput { ... }
```

**`no-unused-vars`:**
```typescript
// Bad - Remove or use the variable
const unused = 'delete me';

// Good - If intentionally unused, prefix with underscore
function handler(_event: Event, data: Data) { ... }
```

**`prefer-const`:**
```typescript
// Bad
let x = 5;  // never reassigned

// Good
const x = 5;
```

**`no-console`:**
```typescript
// Bad
console.log('debug');

// Good - Use proper logger or remove
logger.debug('debug');
```

**`explicit-function-return-type`:**
```typescript
// Bad
function add(a: number, b: number) { return a + b; }

// Good
function add(a: number, b: number): number { return a + b; }
```

### Step 3: Verify Fix

```bash
# Lint the specific file
npx eslint src/path/to/file.ts

# Run related tests
npm test -- --grep "file-related-tests"

# Or run full test suite if unsure
npm test
```

## Handling Complex Cases

### When Types Are Unclear

If you can't determine the correct type:

1. Check how the value is used downstream
2. Look for similar patterns in the codebase
3. Use a named type alias with TODO:
   ```typescript
   // TODO: Refine this type once API contract is finalized
   type ApiResponse = Record<string, unknown>;
   ```
4. Document in LINT_LEARNINGS.md for future reference

### When Fixing Would Change Behavior

If a lint fix might alter functionality:

1. Do NOT make the fix
2. Add to LINT_BLOCKERS.md:
   ```markdown
   - `src/legacy/parser.ts:45` - no-explicit-any
     - **Reason:** Complex union type, fixing may break runtime
     - **Recommendation:** Requires dedicated refactor task
   ```
3. Add eslint-disable comment with explanation:
   ```typescript
   // eslint-disable-next-line @typescript-eslint/no-explicit-any -- Legacy API compatibility
   ```

### When Rule Seems Wrong

If a rule produces many false positives:

1. Document in LINT_LEARNINGS.md
2. Consider if rule config should change
3. Add to `comms/outbox/` for human decision

## LINT_TODO.md Entry Format

```markdown
- [ ] `src/services/auth.service.ts` (12 errors, 3 warnings)
  - **Errors:** 8x no-explicit-any, 4x no-unused-vars
  - **Importance:** Core authentication module
  - **Estimated effort:** 15 min
  - **Notes:** Fix any types first, may reveal more issues
```

When complete:
```markdown
- [x] `src/services/auth.service.ts` ~~(12 errors, 3 warnings)~~ ✅
  - **Fixed:** 2024-01-15
  - **Actual effort:** 12 min
  - **Changes:** Replaced 8 any with proper interfaces, removed 4 unused imports
```

## Communication

### Check Inbox (Start of Session)

Before taking action, check `comms/inbox/` for any files.

* **Read** guidance on type decisions or rule exceptions
* **Move** processed files to `comms/archive/`
* **Update** LINT_TODO.md if new information changes priorities

### Issuing an RFI

If you need clarification (e.g., "Should we disable this rule globally?"):

* **Create** a file in `comms/outbox/` named `YYYY-MM-DD_lint-question.md`
* **Format:** Context / Options / Impact
* **Pivot:** Work on a different file that doesn't require this decision

## Learnings

After each session, append to LINT_LEARNINGS.md:
```markdown
### [date] - File: src/path/to/file.ts
- **Pattern:** How to type Express request handlers
- **Solution:** Use `Request<Params, ResBody, ReqBody, Query>`
- **Gotcha:** Don't forget to import from 'express'
```

## Commit Convention

Use conventional commits:
- `style: fix lint errors in [filename]`
- `refactor: add types to [module]`

Format: `style: [scope] fix [N] lint errors`

Example: `style: auth.service fix 12 lint errors`

## Completion Check

The linter loop is complete when:
1. All LINT_TODO.md items are checked
2. Full lint run produces zero errors
3. All tests still pass
4. No new lint issues from fixed files

If complete, create `.linter-done` with:
- Total files fixed
- Total errors resolved
- Time spent
- Any rules that should be reconsidered

## Quick Reference: Common ESLint Rules

| Rule | Auto-fixable | Strategy |
|------|--------------|----------|
| prefer-const | ✅ | Auto-fix |
| no-unused-vars | ❌ | Delete or prefix with _ |
| no-explicit-any | ❌ | Define interface or use unknown |
| no-console | ❌ | Remove or use logger |
| eqeqeq | ✅ | Auto-fix (=== instead of ==) |
| quotes | ✅ | Auto-fix |
| semi | ✅ | Auto-fix |
| indent | ✅ | Auto-fix |
| @typescript-eslint/explicit-function-return-type | ❌ | Add return type annotation |
| @typescript-eslint/no-floating-promises | ❌ | Add await or void |

## Important

You have ~15 minutes. Focus on completely fixing ONE file.
Quality over speed—a partially-fixed file creates confusion.
The next iteration will continue with the next prioritized file.
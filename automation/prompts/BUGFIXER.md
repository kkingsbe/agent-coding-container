# BUGFIXER.md

You are a bug-fixing engineer working on this project. Your job is to
systematically identify and fix defects in the codebase.

## Phase Detection

First, examine the repository to determine what phase you're in:

1. **If BUGS.md doesn't exist:**
   - You're in DISCOVERY phase
   - Run static analysis, linting, and type checking
   - Execute the test suite and note any failures
   - Search for code smells: empty catch blocks, TODO/FIXME/HACK comments, unsafe type assertions
   - Create BUGS.md with findings, prioritized by severity

2. **If BUGS.md exists with unchecked items:**
   - You're in FIXING phase
   - Pick the highest-severity unchecked bug from BUGS.md
   - Fix it completely
   - Add a regression test that would have caught it
   - Mark it complete in BUGS.md
   - Commit your changes

3. **If all BUGS.md items are checked:**
   - You're in VERIFICATION phase
   - Run full test suite and static analysis
   - If new issues found, add them to BUGS.md and continue
   - If clean, create `.bugfixer-done` with summary

## Rules

- **STRICT: Single Bug Enforcement.** Fix exactly ONE bug per session.
- **Session Termination:** Once you have committed a fix and checked a box in BUGS.md, you MUST STOP.
- **Always commit your work** before the session ends.
- **Update BUGS.md** to reflect current state.
- **If blocked**, document in BLOCKERS.md and move to next bug.
- **Regression test required.** Every fix must include a test that fails without the fix.
- **No features.** If you find missing functionality, ignore it—that's not a bug.
- **No refactoring.** If code is ugly but correct, leave it.

## Files to Check

- `BUGS.md` - Current bug list (you create/maintain this)
- `BLOCKERS.md` - Known issues preventing progress
- `LEARNINGS.md` - Patterns and gotchas discovered

## Bug Entry Format

When creating BUGS.md, use this format:

```
- [ ] **[SEVERITY]** Short title `file.ts:line`
  - **What:** Description of incorrect behavior
  - **Why:** Root cause if known
  - **Repro:** Steps or test case that triggers it
```

Severity levels: CRITICAL > HIGH > MEDIUM > LOW

Example:
```
- [ ] **[HIGH]** Expired JWT tokens accepted `src/middleware/auth.ts:15`
  - **What:** Protected routes accept expired tokens
  - **Why:** Token expiration check uses `<` instead of `<=`
  - **Repro:** `npm test -- --grep "expired token"`
```

## Bug Discovery Methods

When in DISCOVERY phase, use these techniques:

1. **Static Analysis**
   ```bash
   npm run lint          # ESLint errors
   npm run typecheck     # TypeScript errors (tsc --noEmit)
   ```

2. **Test Failures**
   ```bash
   npm test              # Note any failing tests
   ```

3. **Code Smell Search**
   ```bash
   grep -rn "// TODO\|// FIXME\|// HACK\|// BUG" src/
   grep -rn "catch.*{}" src/                        # Empty catch blocks
   grep -rn "as any\|: any" src/                    # Unsafe types
   grep -rn "@ts-ignore\|@ts-expect-error" src/     # Type bypasses
   ```

4. **Edge Case Audit**
   - What happens with null/undefined inputs?
   - What happens with empty arrays/strings?
   - What happens at boundary values (0, -1, MAX_INT)?

## Task Sizing

Each bug in BUGS.md must be:
- Fixable in under 15 minutes
- Independently testable
- Has clear reproduction steps

If a bug is too large, break it into smaller bugs:

Bad: `- [ ] **[HIGH]** Authentication system is broken`

Good:
```
- [ ] **[HIGH]** Expired tokens accepted `auth.ts:15`
- [ ] **[HIGH]** Missing rate limiting on login `auth.ts:42`
- [ ] **[MEDIUM]** Password reset tokens don't expire `reset.ts:8`
```

## Communication

### Check Inbox (Start of Session)

Before taking action, check `comms/inbox/` for any files.

* **Read** and integrate into your fix plan
* **Move** processed files to `comms/archive/`
* **Update** BUGS.md if new information unblocks a fix

### Issuing an RFI

If you need clarification to proceed:

* **Create** a file in `comms/outbox/` named `YYYY-MM-DD_short-description.md`
* **Format:** Context / Question / Impact
* **Pivot:** Work on a different bug that doesn't require this info

## Learnings

After each session, append to LEARNINGS.md:
```
### [date] - Bug Fix
- **Fixed:** [bug title]
- **Root Cause:** [why it happened]
- **Pattern:** [broader pattern to watch for]
- **Prevention:** [how to avoid similar bugs]
```

## Commit Convention

Use conventional commits: `fix:` for bug fixes, `test:` for test additions

Format: `fix: [component] short description`

Example: `fix: auth middleware reject expired JWT tokens`

## Completion Check

The bugfixer loop is complete when:
1. All BUGS.md items are checked
2. All tests pass
3. Static analysis is clean
4. A fresh discovery pass finds no new issues

If complete, create `.bugfixer-done` with:
- Total bugs fixed
- Categories of bugs found
- Patterns identified for prevention
# Fix Workflow — /aris:fix

Autonomous fix loop that takes a broken state and iteratively repairs it until everything
passes. One fix per iteration. Atomic, committed, verified, auto-reverted on failure.

**Core idea:** Detect -> Prioritize -> Fix ONE thing -> Verify -> Keep/Revert -> Repeat
until zero errors.

## Trigger

- User invokes `/aris:fix`
- User says "fix all errors", "make tests pass", "fix the build", "clean up all warnings"
- User has output from `/aris:debug` and wants to fix the findings

## Loop Support

```
# Unlimited -- keep fixing until everything passes
/aris:fix

# Bounded -- exactly N fix iterations
/aris:fix
Iterations: 30

# With explicit target
/aris:fix
Target: make all tests pass
Scope: src/**/*.ts
Guard: npm run typecheck
```

## PREREQUISITE: Interactive Setup (when invoked without flags)

**CRITICAL -- BLOCKING PREREQUISITE:** If `/aris:fix` is invoked without explicit
`--target`, `--guard`, or `--scope`, you MUST first auto-detect all failures, then use
`AskUserQuestion` to gather user input BEFORE proceeding to ANY phase. DO NOT skip
this step.

**Pre-scan:** Run test suite, type checker, linter, and build to detect failures.

**Single batched call -- all 4 questions at once:**

You MUST call `AskUserQuestion` with all 4 questions in ONE call:

| # | Header | Question | Options (from auto-detection) |
|---|--------|----------|-------------------------------|
| 1 | `Fix What` | "Found [N] test failures, [M] type errors, [K] lint errors. What should I fix?" | "Fix everything (recommended)", "Only tests", "Only type errors", "Only lint" |
| 2 | `Guard` | "What command must ALWAYS pass? (prevents fixes from breaking other things)" | "npm test", "tsc --noEmit", "npm run build", "Skip -- no guard" |
| 3 | `Scope` | "Which files can I modify?" | Suggested globs from error locations + "All project files" |
| 4 | `Launch` | "Ready to fix?" | "Fix until zero errors", "Fix with iteration limit", "Edit config", "Cancel" |

**IMPORTANT:** Always ask all 4 questions in a single call -- never one at a time.

If the user provides `--target`, `--guard`, `--scope`, or `--from-debug` flags, skip
the interactive setup and proceed directly to Phase 1.

## Architecture

```
/aris:fix
  |-- Phase 1: Detect (what's broken?)
  |-- Phase 2: Prioritize (fix order)
  |-- Phase 3: Fix ONE thing (atomic change)
  |-- Phase 4: Commit (before verification)
  |-- Phase 5: Verify (did error count decrease?)
  |-- Phase 6: Guard (did anything else break?)
  |-- Phase 7: Decide (keep / revert / rework)
  +-- Phase 8: Log & Repeat
```

## Phase 1: Detect -- What's Broken?

**STOP: Have you completed the Interactive Setup above?** If invoked without flags,
you MUST complete the `AskUserQuestion` call above BEFORE entering this phase.

Auto-detect the failure domain from context, or accept explicit target.

**Detection algorithm:**
```
FUNCTION detectFailures(context):
  failures = []

  # Run test suite
  IF test runner detected (jest, pytest, vitest, cargo test):
    result = run_tests()
    IF failures -> ADD {type: "test", count: N, details: [...]}

  # Run type checker
  IF typescript detected:
    result = run("tsc --noEmit")
    IF errors -> ADD {type: "type", count: N, details: [...]}

  # Run linter
  IF linter detected (eslint, ruff, clippy):
    result = run_lint()
    IF errors -> ADD {type: "lint", count: N, details: [...]}

  # Run build
  IF build script detected:
    result = run_build()
    IF fails -> ADD {type: "build", count: 1, details: [...]}

  # Check for debug findings
  IF debug/{latest}/findings.md exists:
    bugs = parse_findings()
    ADD {type: "bug", count: N, details: [...]}

  RETURN failures sorted by severity
```

**Detection priority:** Run build first -- if build fails, type/test/lint results are
unreliable.

**Output:** `Phase 1: Detected -- [N] test failures, [M] type errors, [K] lint errors`

## Phase 2: Prioritize -- Fix Order

Fix in this order (blockers first, polish last):

| Priority | Category | Why First |
|----------|----------|-----------|
| 1 | **Build failures** | Nothing works if it doesn't compile |
| 2 | **Critical/High bugs** | From debug findings -- data loss, security |
| 3 | **Type errors** | Type safety prevents cascading bugs |
| 4 | **Test failures** | Tests verify correctness |
| 5 | **Medium/Low bugs** | From debug findings |
| 6 | **Lint errors** | Code quality |
| 7 | **Warnings** | Polish |

**Within a category, prioritize by:**
1. Cascading impact (fixing one may fix others downstream)
2. Simplicity (quick wins first -- build momentum)
3. File locality (fixes in same file grouped)

**Output:** `Phase 2: Prioritized -- fixing [category] first ([N] items)`

## Phase 3: Fix ONE Thing -- Atomic Change

Pick the highest-priority unfixed item and make ONE focused change.

**Fix strategies by category:**

| Category | Strategy |
|----------|----------|
| Build failure | Read error, fix the exact line/import/config |
| Type error | Add proper types, fix signatures, handle null cases |
| Test failure | Read test + implementation, find mismatch, fix implementation (not test) |
| Lint error | Apply the rule -- auto-fix where possible |
| Bug (from debug) | Apply the suggested fix from findings.md |
| Warning | Resolve the underlying issue, don't suppress |

**Fix Strategies by Language:**

| Language | Never Do | Correct Pattern |
|----------|----------|-----------------|
| TypeScript | `any`, `@ts-ignore`, type assertions to bypass | Proper interfaces, generics, discriminated unions |
| Python | Bare `except:`, missing type hints | `except SpecificError:`, full type hints |
| Go | Ignoring errors with `_`, `panic` in library code | Explicit error wrapping with `fmt.Errorf` |
| Rust | `.unwrap()` in production, `#[allow(unused)]` | `Result<T, E>` propagation with `?`, `thiserror` |

**Rules:**
- ONE fix per iteration. Not two. Not "while I'm here."
- Fix the IMPLEMENTATION, not the test (unless the test is genuinely wrong)
- Never add `@ts-ignore`, `eslint-disable`, `# type: ignore` to suppress errors
- Never use `any` to escape type errors -- use proper types or generics
- Never delete tests -- fix the implementation to satisfy them
- Prefer minimal changes -- smallest diff that fixes the issue

## Phase 4: Commit -- Before Verification

```bash
git add <modified-files>
git commit -m "[aris] fix: [what was fixed] -- [file:line]"
```

Commit BEFORE running verification. This enables clean rollback if the fix breaks
something else.

## Phase 5: Verify -- Did It Help?

Re-run the detection from Phase 1 and compare:

```
previous_errors = error_count_before
current_errors = error_count_after
delta = previous_errors - current_errors
```

**Expected:** `delta > 0` (fewer errors than before)

## Phase 6: Guard -- Did Anything Else Break?

If a guard command is specified, run it:

```bash
guard_result = run(guard_command)  # e.g., "npm test"
```

**Guard prevents regressions.** Fixing a type error shouldn't break a test. Fixing a
test shouldn't break the build.

## Phase 7: Decide -- Keep, Revert, or Rework

| Condition | Action |
|-----------|--------|
| `delta > 0` AND guard passes | **KEEP** -- commit stays, log "fixed" |
| `delta > 0` AND guard fails | **REWORK** -- revert, try different approach (max 2) |
| `delta == 0` | **DISCARD** -- revert, fix didn't help |
| `delta < 0` (more errors!) | **DISCARD** -- revert immediately |
| Crash during fix | **RECOVER** -- revert, try simpler approach (max 3) |

**Rework strategy (when guard fails):**
1. Read the guard failure -- understand what regressed
2. Revert: `git revert HEAD --no-edit`
3. Understand why the fix broke something else
4. Find an approach that fixes the target WITHOUT breaking the guard
5. If 2 rework attempts fail -> skip this item, add to `blocked.md`, move to next

**Extended decision matrix:**

| Condition | delta | Guard | Action | Status |
|-----------|-------|-------|--------|--------|
| Perfect fix | > 0 | pass | KEEP | fixed |
| Partial fix | > 0 | pass | KEEP + continue | fixed |
| Regression | > 0 | fail | REWORK | rework |
| No effect | == 0 | - | DISCARD | discarded |
| Made worse | < 0 | - | DISCARD immediately | discarded |
| Crash | any | fail | RECOVER (simpler) | crash |
| 3rd attempt | any | any | SKIP to blocked | blocked |

## Phase 8: Log & Repeat

**With ARIS CLI:**
```bash
# After a successful fix
aris record --metric $REMAINING_ERRORS --status kept --summary "fix: add return type in auth.ts:42"

# After a failed fix attempt
aris record --metric $REMAINING_ERRORS --status discarded --summary "fix attempt: wrong approach for auth.ts"
```

**Fallback -- append to fix-results.tsv:**
```tsv
iteration	category	target	delta	guard	status	description
0	-	-	-	pass	baseline	47 test failures, 12 type errors, 3 lint errors
1	type	auth.ts:42	-2	pass	fixed	add return type annotation
2	type	db.ts:15	-1	pass	fixed	handle nullable column
3	test	api.test.ts	-3	pass	fixed	fix expected status code (was 200, should be 201)
4	test	auth.test.ts	0	-	discarded	wrong approach -- test expectation was correct
5	test	auth.test.ts	-1	pass	fixed	missing await on async handler
```

**Every 5 iterations, print progress:**
```
=== Fix Progress (iteration 15) ===
Baseline: 62 errors -> Current: 23 errors (-39, -63%)
Category breakdown:
  Tests:  31/47 fixed
  Types:  8/12 fixed
  Lint:   0/3 fixed (not yet started -- lower priority)
Keeps: 11 | Discards: 3 | Reworks: 1
```

**Completion detection:**
```
IF current_errors == 0:
  PRINT "=== All Clear -- Zero Errors ==="
  STOP (even in unbounded mode)
```

---

## Flags

| Flag | Purpose |
|------|---------|
| `--target <command>` | Explicit verify command (overrides auto-detection) |
| `--guard <command>` | Safety command that must always pass |
| `--scope <glob>` | Limit fixes to specific files |
| `--category <type>` | Only fix specific category (test, type, lint, build, bug) |
| `--skip-lint` | Don't fix lint errors (focus on functional issues) |
| `--from-debug` | Read findings from latest debug/ session |

---

## Fix Session State Machine

```
States: DETECTING -> PRIORITIZING -> FIXING -> VERIFYING -> DECIDING -> [DONE | LOOP]

DETECTING:
  -> Run all error detection commands
  -> If zero errors found -> DONE (nothing to fix)
  -> If errors found -> PRIORITIZING

PRIORITIZING:
  -> Sort errors by priority table
  -> Group cascading errors (fixing one fixes others)
  -> Pick first unfixed item -> FIXING

FIXING:
  -> Read error details + surrounding code
  -> Assess blast radius (impact assessment)
  -> Check git history for prior attempts on this file
  -> Apply minimal change
  -> Commit -> VERIFYING

VERIFYING:
  -> Re-run error detection
  -> Compute delta (previous - current)
  -> Run guard command -> DECIDING

DECIDING:
  -> delta > 0 AND guard passes -> KEEP -> log "fixed" -> LOOP
  -> delta > 0 AND guard fails -> REWORK (max 2) -> FIXING
  -> delta == 0 -> DISCARD -> revert -> PRIORITIZING (next item)
  -> delta < 0 -> DISCARD -> revert immediately -> PRIORITIZING
  -> 3 failed attempts on same item -> SKIP -> blocked list -> PRIORITIZING
  -> All items fixed or skipped -> DONE

DONE:
  -> Generate summary.md
  -> Print fix_score
  -> Suggest /aris:debug for blocked items
```

---

## What NOT to Do -- Anti-Patterns

| Anti-Pattern | Why It's Wrong | Do This Instead |
|--------------|----------------|-----------------|
| Add `@ts-ignore` / `eslint-disable` | Hides the problem | Fix the root cause |
| Use `any` type to silence TypeScript | Defeats type safety | Use proper types or `unknown` |
| Delete or skip failing tests | Removes the safety net | Fix the implementation |
| Suppress lint with inline comments | Accumulates tech debt | Apply the lint rule correctly |
| `catch (e) {}` empty catch blocks | Swallows errors | Log at minimum; handle or re-throw |
| Comment out broken code | It will never be uncommented | Fix it or delete it entirely |
| Hardcode values to pass tests | Test passes, feature is broken | Fix the logic, not the values |
| Increase test timeouts | Masks slow code or deadlocks | Profile and fix the underlying issue |

---

## Composite Metric

For bounded loops:

```
fix_score = reduction_score + quality_score + guard_score + bonus_score

reduction_score = ((baseline_errors - current_errors) / baseline_errors) * 60

quality_score = 0
  quality_score -= (suppression_count * 5)   # @ts-ignore, eslint-disable used
  quality_score -= (skipped_test_count * 10)  # tests deleted/commented out
  quality_score -= (any_type_count * 3)       # `any` type introduced
  quality_score = max(quality_score, -20)     # floor

guard_score = (guard_always_passed ? 25 : 0)

bonus_score = 0
  bonus_score += (zero_errors ? 10 : 0)       # all clear bonus
  bonus_score += (no_discards ? 5 : 0)        # every fix worked first try
```

**Interpretation:**
- **100+** = perfect: all errors fixed, no regressions, no anti-patterns
- **80-99** = good: significant progress, guards held
- **60-79** = acceptable: meaningful reduction, some issues
- **<60** = needs work: too many discards or anti-patterns

---

## Fix Impact Assessment

Before applying a fix, estimate the blast radius:

```
FUNCTION assessImpact(target_file, fix_type):
  dependents = grep -r "import.*{target_file}" src/
  is_critical = target_file in [auth, payments, database, api-gateway]
  test_coverage = count tests that import target_file

  RETURN {
    dependents: N,
    is_critical: bool,
    test_coverage: N,
    risk_level: HIGH if (dependents > 10 OR is_critical) else MEDIUM if dependents > 3 else LOW
  }
```

| Risk Level | Action |
|------------|--------|
| LOW | Fix and verify with unit tests |
| MEDIUM | Fix with unit + integration tests as guard |
| HIGH | Fix in isolation, verify against full suite |

---

## Compound Fix Detection

When fixing one error reveals another:

```
FUNCTION detectCompound(before_errors, after_errors):
  new_errors = after_errors - before_errors  # errors that didn't exist before

  IF new_errors > 0:
    LOG "Compound fix: {N} new errors surfaced"
    # Pre-existing errors that were masked -- add to fix queue
    CONTINUE (do not treat as regression)

  IF delta == 0 AND error_details_changed:
    LOG "Error transformed -- not fixed, just moved"
    REVERT and try different approach
```

**Common compound patterns:**
- Fixing a type error reveals a logic error the wrong type was hiding
- Fixing a null check reveals a missing initialization
- Upgrading a dependency reveals tests were relying on a bug
- Fixing test A reveals test B shared mutable state

---

## Rollback Protocol

When a fix makes things worse:

```
STEP 1: git revert HEAD --no-edit
  # OR for harder cases:
  git revert --abort 2>/dev/null
  git reset --hard HEAD~1

STEP 2: Verify rollback succeeded
  Run detection -- should return to pre-fix error count

STEP 3: Log the failed approach
  aris record --status discarded --summary "fix attempt failed: <why>"

STEP 4: Analyze before retrying
  - What assumption was wrong?
  - What did the fix break?
  - Is there a smaller, safer change?
```

---

## The Fix Didn't Work -- Escalation Path

When 3 attempts at the same error fail:

```
Attempt 1: FAIL -> Log approach, try different strategy
Attempt 2: FAIL -> Log approach, read git history for prior attempts
Attempt 3: FAIL -> Escalate:

  1. DOCUMENT: What was tried (3 approaches), why each failed
  2. ISOLATE: Create minimal reproduction case
  3. SKIP: Move this error to "blocked" list
  4. FLAG: Note in summary.md
  5. SUGGEST: /aris:debug on the specific error for root cause analysis
```

**Never loop on the same failing approach.** Each attempt must use a materially
different strategy.

---

## Auto-Detection Reference

| Signal | Detected Type | Verify Command |
|--------|---------------|----------------|
| `package.json` has `test` script | test | `npm test` |
| `tsconfig.json` exists | type | `tsc --noEmit` |
| `.eslintrc*` or `eslint.config.*` | lint | `npx eslint .` |
| `pyproject.toml` has `pytest` | test | `pytest` |
| `Cargo.toml` exists | test + lint | `cargo test`, `cargo clippy` |
| `go.mod` exists | test + lint | `go test ./...` |
| `build` script in package.json | build | `npm run build` |
| `debug/*/findings.md` exists | bug | Parse findings |

---

## Chaining Patterns

```bash
# Debug first, then fix what was found
/aris:debug
Iterations: 15

/aris:fix --from-debug
Iterations: 30

# Fix with guard
/aris:fix
Target: npm run typecheck
Guard: npm test

# Fix only tests, guard with types
/aris:fix --category test --guard "tsc --noEmit"

# Fix everything -- iterate until clean
/aris:fix

# Bounded sprint
/aris:fix
Iterations: 20
```

---

## Output Directory

Creates `fix/{YYMMDD}-{HHMM}-{fix-slug}/` with:
- `fix-results.tsv` -- iteration log
- `summary.md` -- what was fixed, what remains, stats
- `blocked.md` -- errors that needed 3+ attempts and were escalated
- `impact-assessment.md` -- blast radius analysis for each fix applied

---

## Summary Report Format

```markdown
# Fix Session Summary

## Stats
- Session: fix/260423-1805-auth-fixes/
- Duration: 23 iterations
- Baseline: 47 errors (31 test, 12 type, 4 lint)
- Final: 3 errors (2 test, 1 type, 0 lint)
- Reduction: 93.6% (-44 errors)

## Fix Score
fix_score: 97/100
- Reduction: 58/60 (93.6%)
- Guard: 25/25 (no regressions)
- Bonus: +10 (zero lint errors)
- Anti-patterns used: 0

## Fixed
- auth.ts:42 -- add return type annotation (type)
- db.ts:15 -- handle nullable column (type)
- api.test.ts -- fix expected status code 200->201 (test)
[... more ...]

## Blocked (requires investigation)
- auth/token-refresh.ts -- circular dependency blocks type resolution
  -> Suggested: /aris:debug --scope auth/token-refresh.ts

## Remaining
- user.test.ts:88 -- flaky timing test (not a code bug)
- config.ts:12 -- type error requires breaking API change
```

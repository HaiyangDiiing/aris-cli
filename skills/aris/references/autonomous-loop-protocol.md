# ARIS Autonomous Loop Protocol

> The 8-phase experiment loop. This is the core protocol — read it fully before starting,
> then follow it phase-by-phase for every iteration.
>
> **Principle**: One change per iteration. Commit before verify. Revert on failure.
> Git is your memory — read your own diffs to learn.

---

## Phase 0: Precondition Checks

Run these ONCE before the first iteration. Do not skip.

### With CLI (preferred)

```bash
aris doctor
```

This validates 14+ items: git repo, config, target file, eval command, metric parseability,
branch state, stale locks, program.md, clean worktree, guard command, time budget.
Fix every failing check before proceeding.

### Without CLI (manual fallback)

```bash
# 1. Git repo
git rev-parse --is-inside-work-tree || { echo "FAIL: not a git repo"; exit 1; }

# 2. Config exists
test -f autoresearch.toml || { echo "FAIL: no autoresearch.toml"; exit 1; }

# 3. Target file exists
TARGET=$(grep target_file autoresearch.toml | cut -d'"' -f2)
test -f "$TARGET" || { echo "FAIL: target file $TARGET not found"; exit 1; }

# 4. Eval command runs and outputs a number
EVAL_CMD=$(grep eval_command autoresearch.toml | cut -d'"' -f2)
METRIC_OUTPUT=$($EVAL_CMD 2>&1 | tail -1 | grep -oE '\-?[0-9]+\.?[0-9]*' | tail -1)
test -n "$METRIC_OUTPUT" || { echo "FAIL: eval command did not output a number"; exit 1; }

# 5. Working tree clean
test -z "$(git status --porcelain)" || echo "WARNING: uncommitted changes — commit or stash first"

# 6. Remove stale locks
rm -f .autoresearch/loop.lock
```

### CLI Detection

At the start, detect which mode to use for the entire session:

```bash
if command -v aris >/dev/null 2>&1; then
    # CLI-enhanced mode: structured data, validation, reward-hacking detection
    CLI_MODE="enhanced"
else
    echo "⚠ aris CLI not found. Running in fallback mode (no validation, no TUI)."
    echo "  Install with: cargo install aris-cli"
    CLI_MODE="fallback"
fi
```

### Read Context

Before entering the loop, read everything:

1. `autoresearch.toml` — your configuration (target file, eval command, metric, direction)
2. `program.md` — research direction, ideas, constraints from the human researcher
3. Experiment history:
   - CLI mode: `aris log -n 20 --json`
   - Fallback: `cat .autoresearch/experiments.jsonl 2>/dev/null | tail -20`
4. Git history: `git log --oneline -20`

### Establish Baseline

```bash
# Create experiment branch
git checkout -b <branch> 2>/dev/null || git checkout <branch>

# Create lock file
mkdir -p .autoresearch
echo '{"started_at": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'", "iteration": 0}' > .autoresearch/loop.lock

# Run eval to get baseline metric
BASELINE=$(<eval_command> 2>&1 | <extract_metric>)

# Record baseline
aris record --metric "$BASELINE" --status baseline --summary "Initial baseline"
# Fallback: echo -e "0\t$(git rev-parse --short HEAD)\t$BASELINE\t0.0\t-\t-\tbaseline\tInitial baseline" >> .autoresearch/results.tsv
```

---

## Phase 1: Review (EVERY iteration)

**This is how you learn.** Do not skip any step.

### Step 1.1: Read experiment history

```bash
# CLI mode
aris log -n 20 --json

# Fallback
tail -20 .autoresearch/experiments.jsonl 2>/dev/null || tail -20 .autoresearch/results.tsv
```

### Step 1.2: Read git history

```bash
git log --oneline -20
```

### Step 1.3: Read the last kept change's diff (CRITICAL)

```bash
git diff HEAD~1
```

**Why this matters:** The experiment log tells you THAT a change worked. The diff tells you
WHY it worked — what specific code was added, removed, or modified. Without reading diffs,
you will repeat failed approaches and miss patterns in successful ones.

If the last experiment was discarded (and reverted), look at the last KEPT change:
```bash
# Find last kept commit
LAST_KEPT=$(git log --oneline --all | head -20 | grep -i "keep\|kept" | head -1 | cut -d' ' -f1)
git show "$LAST_KEPT" --stat --patch
```

### Step 1.4: Analyze patterns

Ask yourself:
- What has been tried? What worked? What failed?
- Are there repeated failure themes? (same approach, different params = diminishing returns)
- What did the DIFFS of successful experiments have in common?
- Am I stuck? (5+ consecutive discards → see Stuck Protocol below)
- Is the metric plateauing? (15+ iterations without new best → see Plateau Detection)

### Step 1.5: Check iteration count

```bash
# Read current iteration from lock file
ITERATION=$(cat .autoresearch/loop.lock 2>/dev/null | grep -oE '"iteration":\s*[0-9]+' | grep -oE '[0-9]+')

# Bounded mode: check if done
if [ -n "$MAX_ITERATIONS" ] && [ "$ITERATION" -ge "$MAX_ITERATIONS" ]; then
    echo "=== Reached iteration limit ($MAX_ITERATIONS). Generating final report. ==="
    aris report  # or manual summary
    rm -f .autoresearch/loop.lock
    exit 0
fi
```

---

## Phase 2: Ideate (Strategic Next Change)

Pick ONE atomic change based on what you learned in Phase 1.

### Priority ordering (follow this order)

1. **Fix crashes first** — If the last experiment crashed, fix the crash before trying anything new
2. **Exploit successful patterns** — If last kept experiment improved metric by changing X, try a similar change to Y
3. **Explore untried directions** — Check `program.md` for ideas you haven't attempted
4. **Combine near-misses** — If experiments A and B each almost improved the metric, try combining them
5. **Simplify** — Remove code/complexity. The best optimization is often removal.
6. **Radical changes** — Fundamentally different approach. Only try after exhausting incremental changes.

### Write hypothesis BEFORE modifying

"I predict that [specific change] will [improve/reduce] [metric] by [estimated amount]
because [reasoning based on what I learned from diffs and history]."

This forces clarity. If you can't write a clear hypothesis, you don't understand the change well enough.

---

## Phase 3: Modify (ONE Atomic Change)

### Rules

1. **One change per experiment.** If you change 3 things and it works, you don't know which
   helped. If it fails, you don't know which hurt.

2. **One-sentence test:** Write your change description as one sentence. If it contains "and"
   linking two separate actions (e.g., "increased LR AND changed model architecture"), SPLIT
   into two experiments.

3. **Scope check:** Only modify files within the configured scope (typically `target_file`).
   ```bash
   git diff --name-only
   ```
   If more than 5 files are modified → reconsider. This is probably not atomic.

4. **Multi-file changes are OK** if they serve the same logical intent (e.g., renaming a
   function across 3 files that import it).

---

## Phase 4: Commit (BEFORE Verify)

**Always commit before running eval.** This ensures clean revert on failure.

### Step 4.1: Stage specific files

```bash
git add <target_file>
# NEVER use git add -A (risks staging .env, node_modules, user's work)
# NEVER use git add . (same risk)
```

### Step 4.2: Check for actual changes

```bash
if git diff --cached --quiet; then
    # No changes to commit — this is a no-op
    aris record --status no-op --summary "attempted change produced no diff"
    # Continue to next iteration — don't waste an eval run
    goto Phase 8
fi
```

### Step 4.3: Commit with descriptive message

```bash
git commit -m "[aris] experiment #$ITERATION: <brief description of the change>"
```

### Step 4.4: Handle pre-commit hook failures

```
IF git commit fails due to pre-commit hook (lint, format, type check):

  FOR attempt IN 1..2:
    1. Read the hook error output carefully
    2. Fix the specific issue (auto-format, fix lint error, fix type error)
    3. Re-stage: git add <files>
    4. Re-commit: git commit -m "[aris] experiment #$ITERATION: <description>"
    5. IF commit succeeds → proceed to Phase 5

  IF still failing after 2 attempts:
    aris record --status hook-blocked --summary "pre-commit hook rejected: <error summary>"
    # Fallback: echo -e "$ITERATION\t-\t-\t-\t-\t-\thook-blocked\tpre-commit: <error>" >> .autoresearch/results.tsv
    safe_revert  (see Safe Revert Protocol below)
    goto Phase 8

  NEVER use --no-verify. NEVER skip hooks.
```

---

## Phase 5: Verify (Mechanical Metric Only)

### Step 5.1: Run eval with timeout

```bash
timeout <time_budget> <eval_command>
# macOS alternative (no timeout command):
# perl -e 'alarm(<seconds>); exec @ARGV' <eval_command>
```

### Step 5.2: Extract metric

The eval command must output a number. Extract it:

```bash
METRIC=$(<eval_command> 2>&1 | <extraction_pipeline>)
```

Common extraction patterns:
- Python pytest coverage: `pytest --cov=src 2>&1 | grep TOTAL | awk '{print $4}' | tr -d '%'`
- Node jest coverage: `npx jest --coverage 2>&1 | grep 'All files' | awk '{print $4}'`
- Custom script: `python eval.py 2>&1 | tail -1 | grep -oE '\-?[0-9]+\.?[0-9]*' | tail -1`

### Step 5.3: Validate metric is numeric

```bash
METRIC=$(echo "$METRIC" | xargs)  # strip whitespace
if ! echo "$METRIC" | grep -qE '^-?[0-9]+\.?[0-9]*$'; then
    # Non-numeric output — metric extraction is broken
    aris record --status metric-error --summary "eval output was not numeric: '$METRIC'"
    # Fallback: echo -e "$ITERATION\t-\t-\t-\t-\t-\tmetric-error\tnon-numeric: $METRIC" >> .autoresearch/results.tsv
    safe_revert
    
    # CRITICAL: If this is the second consecutive metric-error, STOP THE LOOP
    # The eval pipeline itself is broken — no point continuing
    if [ "$LAST_STATUS" = "metric-error" ]; then
        echo "FATAL: 2 consecutive metric-errors. Eval pipeline is broken. Fix it before continuing."
        rm -f .autoresearch/loop.lock
        exit 1
    fi
    
    goto Phase 8
fi
```

### Step 5.4: Handle timeout/crash

```
IF eval command times out or exits non-zero:
    → Go to Phase 5.7 (Crash Recovery)
```

---

## Phase 5.5: Guard Check (Optional)

Only runs if `guard_command` is configured in `autoresearch.toml`.

```bash
GUARD_CMD=$(grep guard_command autoresearch.toml | cut -d'"' -f2)
if [ -n "$GUARD_CMD" ]; then
    $GUARD_CMD
    GUARD_EXIT=$?
fi
```

### Guard passed (exit 0)

Continue to Phase 6 — the experiment is valid.

### Guard failed (exit non-zero) BUT metric improved

**Do NOT immediately discard.** The improvement is real but something else broke.
Attempt to REWORK — preserve the metric improvement intent while fixing the guard issue.

```
FOR rework_attempt IN 1..2:
    1. safe_revert  (roll back the experiment)
    2. Read guard error output — what specifically broke?
       (e.g., "3 tests failed in test_auth.py", "type error in line 42")
    3. Rework the implementation:
       - Keep the same optimization intent
       - Fix the specific issue the guard caught
       - This is NOT a new experiment — it's a refinement
    4. git add <files> && git commit -m "[aris] rework #$ITERATION: fix guard (<what you fixed>)"
    5. Re-run verify (eval_command)
    6. IF metric STILL improved:
       7. Re-run guard
       8. IF guard passes → KEEP (reworked). Go to Phase 7 with status "kept" and summary "[REWORKED] <description>"
       9. IF guard fails again → try next rework attempt

IF guard still fails after 2 rework attempts:
    safe_revert
    aris record --metric $METRIC --status discarded --summary "metric improved but guard failed after 2 reworks: <guard error>"
    goto Phase 8
```

### Guard failed AND metric did not improve

Normal discard — no rework needed. Go to Phase 6 (it will discard).

---

## Phase 5.7: Crash Recovery

When the eval command crashes (non-zero exit or timeout).

```
FOR fix_attempt IN 1..3:
    1. Read the error output carefully
       - Syntax error? (missing bracket, wrong indent)
       - Import error? (missing module, wrong path)
       - Runtime error? (null reference, type mismatch, OOM)
       - Timeout? (infinite loop, excessive computation)
    
    2. Diagnose the specific issue
    
    3. Apply a targeted fix (NOT a new experiment — just fixing the crash)
    
    4. git add <files> && git commit -m "[aris] fix crash #$ITERATION: <what you fixed>"
    
    5. Re-run eval with timeout
    
    6. IF eval succeeds (exit 0 and produces numeric metric):
       → Continue to Phase 5.5 (guard check) or Phase 6 (decide)
    
    7. IF eval still crashes:
       → Try next fix attempt

IF still crashing after 3 fix attempts:
    safe_revert  (revert ALL crash fix commits back to pre-experiment state)
    aris record --status crash --summary "eval crashed: <error>. Fix attempts failed."
    # Fallback: echo -e "$ITERATION\t-\t0.0\t0.0\t-\t-\tcrash\t<error>" >> .autoresearch/results.tsv
    goto Phase 8
```

---

## Phase 6: Decide (No Ambiguity)

This is a mechanical decision. Do not deliberate — follow the table.

| Condition | Action | Record Status |
|-----------|--------|---------------|
| Metric improved AND (no guard OR guard passed) | **KEEP** — commit stays | `--status kept` |
| Metric improved AND guard failed | **REWORK** — see Phase 5.5 | `--status kept` if rework succeeds |
| Metric same or worse | **DISCARD** — safe_revert | `--status discarded` |
| Eval crashed | **AUTO-FIX** — see Phase 5.7 | `--status crash` if unfixable |
| No diff produced (no-op) | **SKIP** — no revert needed | `--status no-op` |
| Pre-commit hook blocked | **REVERT** — see Phase 4.4 | `--status hook-blocked` |
| Metric non-numeric | **REVERT** — see Phase 5.3 | `--status metric-error` |
| 2 consecutive metric-errors | **STOP LOOP** | Fatal: eval is broken |

### How to determine "improved"

Read `metric_direction` from `autoresearch.toml`:

```
IF metric_direction = "higher":
    improved = (new_metric > best_metric)
    
IF metric_direction = "lower":
    improved = (new_metric < best_metric)
```

Where `best_metric` is the best value from ALL kept/baseline experiments (not just the previous one).

### Discard procedure

```bash
# Record the result BEFORE reverting
aris record --metric $METRIC --status discarded --summary "<what you tried and why it failed>"

# Then revert
safe_revert
```

### Keep procedure

```bash
# Record the result (commit already exists from Phase 4)
aris record --metric $METRIC --status kept --summary "<what you changed and why it worked>"
# No revert — the commit stays
```

---

## Phase 7: Log Results

### With CLI (preferred)

```bash
# For keep/discard with metric:
aris record --metric $METRIC --status <kept|discarded> --summary "<description>"

# For error statuses (no metric):
aris record --status <crash|no-op|hook-blocked|metric-error> --summary "<description>"
```

### Without CLI (fallback — TSV format)

```bash
echo -e "$ITERATION\t$(git rev-parse --short HEAD 2>/dev/null || echo '-')\t${METRIC:-'-'}\t${DELTA:-'-'}\t${GUARD_STATUS:-'-'}\t${GUARD_METRIC:-'-'}\t$STATUS\t$DESCRIPTION" \
  >> .autoresearch/results.tsv
```

### Progress summary (every 10 iterations)

```
IF iteration % 10 == 0:
    Print:
    === ARIS Progress (iteration $ITERATION) ===
    Baseline: <baseline_metric> → Current best: <best_metric> (<improvement>%)
    Keeps: N | Discards: N | Crashes: N | Errors: N
    Last 5: keep, discard, discard, keep, crash
    =============================================
```

---

## Phase 8: Loop Control

### Update lock file

```bash
echo '{"started_at": "<original_start>", "iteration": '$((ITERATION + 1))'}' > .autoresearch/loop.lock
```

### Bounded mode (Iterations: N)

```
IF current_iteration >= max_iterations:
    Print final summary
    aris report  (or manual summary)
    rm -f .autoresearch/loop.lock
    STOP
ELSE:
    → Go to Phase 1
```

### Unbounded mode (default)

```
ALWAYS go to Phase 1. NEVER ask "should I keep going?" — just loop.

EXCEPT:
- Plateau detected (see below)
- User interrupts (Ctrl+C)
- Fatal error (2 consecutive metric-errors)
```

### Stuck detection (>5 consecutive discards)

You are in a local minimum. Do NOT keep tweaking the same thing.

```
IF consecutive_discards >= 7:
    STUCK LEVEL 3: "You are deeply stuck. Escalating."
    1. Re-read ALL in-scope files from scratch (not just the changed parts)
    2. Re-read program.md for ideas you haven't tried
    3. Re-read entire experiment log for patterns
    4. Run: aris review (generates cross-model review prompt)
    5. Try the OPPOSITE of what hasn't been working
    6. Try REMOVING code instead of adding
    7. Try a fundamentally different approach

IF consecutive_discards >= 5:
    STUCK LEVEL 2: "Consider changing strategy."
    1. Run: aris review
    2. Combine 2-3 previously successful changes
    3. Try a different CATEGORY of change
       (if tuning hyperparameters → try architecture; if architecture → try hyperparameters)

IF consecutive_discards >= 3:
    STUCK LEVEL 1: "Re-read program.md for fresh ideas."
```

### Plateau detection (15 iterations without new best)

```
IF iterations_since_best_improved >= 15:
    echo "PLATEAU: No improvement in 15 iterations. Current best at run #$BEST_RUN."
    echo "Options:"
    echo "  1. Stop and review: aris report"
    echo "  2. Fundamentally change approach"
    echo "  3. Adjust the metric or eval command"
    echo "  4. Fork into parallel explorations: aris fork"
    # In unbounded mode, ask the user what to do
    # In bounded mode, just continue until limit
```

---

## Safe Revert Protocol

Used whenever an experiment needs to be rolled back.

```bash
safe_revert() {
    # Attempt 1: git revert (preserves failed experiment in git history)
    if git revert HEAD --no-edit 2>/dev/null; then
        return 0  # Clean revert
    fi
    
    # Attempt 2: revert had conflicts — abort and force reset
    git revert --abort 2>/dev/null
    git reset --hard HEAD~1
    
    # Verify clean state
    if [ -n "$(git status --porcelain)" ]; then
        echo "WARNING: working tree not clean after revert. Manual cleanup may be needed."
        git checkout -- .  # Last resort
    fi
}
```

**Why `git revert` before `git reset`:** `revert` creates a NEW commit that undoes the change,
preserving the failed experiment in git history. This lets the Agent (and you) see what was
tried. `reset --hard` erases the commit entirely — use only when revert conflicts.

---

## Noise Handling

For metrics with inherent noise (benchmarks, latency tests), use these techniques:

### Multi-run median

Run eval multiple times, use the median:

```bash
METRICS=()
for i in 1 2 3; do
    M=$(<eval_command> 2>&1 | <extract>)
    METRICS+=("$M")
done
# Sort and take middle value
MEDIAN=$(printf '%s\n' "${METRICS[@]}" | sort -n | sed -n '2p')
```

With CLI (multi-sample support):
```bash
aris record --metric $M1 --metric $M2 --metric $M3 --status kept --summary "..."
# CLI computes median, MAD, and confidence automatically
```

### Minimum delta threshold

Ignore improvements smaller than the noise floor:

```
IF abs(new_metric - best_metric) < min_delta:
    Treat as "same" → DISCARD
    (The improvement is within noise range)
```

### Confirmation run

For suspicious improvements, re-run eval to confirm:

```
IF improvement_pct > 20%:  # suspiciously large
    Re-run eval
    IF second_run confirms improvement (within 5% of first):
        KEEP (confirmed)
    ELSE:
        DISCARD (noise artifact)
```

---

## Quick Reference

### Statuses

| Status | Meaning | Has Metric? |
|--------|---------|:-----------:|
| `baseline` | Initial measurement | Yes |
| `kept` | Experiment improved metric | Yes |
| `discarded` | Experiment did not improve | Yes |
| `crash` | Eval command failed, unfixable after 3 attempts | Optional |
| `no-op` | No code changes were produced | No |
| `hook-blocked` | Pre-commit hook rejected commit | No |
| `metric-error` | Eval output was non-numeric | No |

### Key commands

| Command | When to use |
|---------|-------------|
| `aris doctor` | Phase 0: validate environment |
| `aris log -n 20` | Phase 1: read history |
| `aris record --metric X --status Y --summary "Z"` | Phase 7: log results |
| `aris best` | Anytime: find best experiment |
| `aris review` | When stuck: generate review prompt |
| `aris report` | End of loop: generate summary |
| `aris watch` | Separate terminal: live dashboard |
| `aris fork` | When stuck: parallel exploration |
| `aris export --format csv` | Analysis: export data |

### The Loop In One Sentence

**Review your history → pick ONE atomic change → commit → run eval → keep or revert → repeat.**

---

## Appendix: Git as Memory

Your most powerful tool is `git diff HEAD~1`. Read it EVERY iteration.

- The experiment log tells you THAT a change worked.
- The diff tells you WHY it worked — what code was added/removed/modified.
- Without reading diffs, you are experimenting blind.

```bash
# What did the last kept change actually do?
git diff HEAD~1

# What patterns do successful experiments share?
git log --oneline | grep -i "keep\|kept" | head -10

# What does the winning commit look like?
git show <winning_hash> --stat --patch
```

**Learn from your own history. The diff is the teacher.**

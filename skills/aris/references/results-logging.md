# Results Logging Protocol

Track every iteration in a structured log. Enables pattern recognition and prevents
repeating failed experiments.

## Dual Logging System

ARIS supports two logging backends:

| Mode | Format | File | Managed By |
|------|--------|------|-----------|
| CLI-enhanced | JSONL | `.autoresearch/experiments.jsonl` | `aris record` |
| Fallback | TSV | `.autoresearch/results.tsv` | Agent (bash) |

**Always prefer CLI-enhanced mode.** It provides validation, reward-hacking detection,
and structured data. Use fallback only when the `aris` binary is not available.

---

## CLI-Enhanced Mode (Primary)

### Setup & Initialization

```bash
# CLI creates the data directory and initializes the log automatically
mkdir -p .autoresearch

# Record baseline as iteration 0
aris record --metric "$BASELINE" --status baseline --summary "Initial baseline"
```

The CLI handles:
- JSONL format with validated fields
- Automatic run numbering
- Git hash capture
- Delta computation from previous best
- Reward-hacking detection (flags implausible improvements)
- File locking (fs2) for concurrent access safety

### Recording Results

```bash
# After a kept experiment:
aris record --metric 87.1 --status kept --summary "add tests for auth middleware"

# After a discarded experiment:
aris record --metric 86.5 --status discarded --summary "refactor test helpers (broke 2 tests)"

# After a crash (no metric available):
aris record --status crash --summary "eval crashed: DB connection failed after 3 fix attempts"

# After a no-op (no code changes produced):
aris record --status no-op --summary "attempted change produced no diff"

# After a hook-blocked commit:
aris record --status hook-blocked --summary "pre-commit lint rejected formatting"

# After a metric-error (eval output was non-numeric):
aris record --status metric-error --summary "eval output was 'PASS' -- not a number"

# Multi-sample recording (for noisy metrics):
aris record --metric 87.1 --metric 86.9 --metric 87.3 --status kept --summary "..."
# CLI computes median, MAD, and confidence automatically
```

### JSONL Record Format

Each line in `experiments.jsonl` is a JSON object:

```json
{
  "run_number": 1,
  "timestamp": "2026-04-23T10:30:00Z",
  "git_hash": "b2c3d4e",
  "metric": 87.1,
  "delta": 1.9,
  "status": "kept",
  "summary": "add tests for auth middleware",
  "samples": null,
  "mad": null,
  "confidence": null
}
```

For error statuses (crash, no-op, hook-blocked, metric-error), `metric` is `null`.

For multi-sample records:
```json
{
  "run_number": 5,
  "metric": 87.1,
  "samples": [87.1, 86.9, 87.3],
  "mad": 0.2,
  "confidence": 0.95,
  "status": "kept",
  "summary": "..."
}
```

### Reading the Log

```bash
# Phase 1 (Review): Read recent entries
aris log -n 20

# Machine-readable for analysis
aris log -n 20 --json

# Find the best experiment
aris best

# Compare two experiments
aris diff 3 7

# Export for external analysis
aris export --format csv
aris export --format json
```

---

## Fallback Mode (TSV)

When `aris` CLI is not available, use direct TSV logging.

### Setup & Initialization

```bash
# 1. Create data directory and log file with metric direction header
mkdir -p .autoresearch
echo "# metric_direction: higher_is_better" > .autoresearch/results.tsv
echo -e "iteration\tcommit\tmetric\tdelta\tguard\tguard-metric\tstatus\tdescription" >> .autoresearch/results.tsv

# 2. Add to .gitignore (log stays local, not committed)
echo ".autoresearch/results.tsv" >> .gitignore

# 3. Run verify command to establish baseline metric
BASELINE=$(<eval_command> 2>&1 | <extract_metric>)

# 4. Record baseline as iteration 0
COMMIT=$(git rev-parse --short HEAD)
echo -e "0\t${COMMIT}\t${BASELINE}\t0.0\tpass\t-\tbaseline\tinitial state" >> .autoresearch/results.tsv
```

### TSV Columns

| Column | Type | Description |
|--------|------|-------------|
| iteration | int | Sequential counter starting at 0 (baseline) |
| commit | string | Short git hash (7 chars), "-" if reverted |
| metric | float | Measured value from verification, "-" for error statuses |
| delta | float | Change from previous best, "-" for error statuses |
| guard | enum | `pass`, `fail`, or `-` (no guard configured) |
| guard-metric | float or `-` | Guard metric value (metric-valued guards only) |
| status | enum | `baseline`, `kept`, `discarded`, `crash`, `no-op`, `hook-blocked`, `metric-error` |
| description | string | One-sentence description of what was tried |

### Logging Function

```bash
log_iteration() {
  local iteration=$1 commit=$2 metric=$3 delta=$4 guard=$5 guard_metric=$6 status=$7 description=$8
  echo -e "${iteration}\t${commit}\t${metric}\t${delta}\t${guard}\t${guard_metric}\t${status}\t${description}" \
    >> .autoresearch/results.tsv
}

# Usage examples:
log_iteration 1 "b2c3d4e" "87.1" "+1.9" "pass" "-" "kept" "add tests for auth middleware"
log_iteration 2 "-" "86.5" "-0.6" "-" "-" "discarded" "refactor test helpers (broke 2 tests)"
log_iteration 3 "-" "-" "-" "-" "-" "crash" "eval crashed: DB connection failed"
log_iteration 4 "-" "-" "-" "-" "-" "no-op" "attempted change produced no diff"
log_iteration 5 "-" "-" "-" "-" "-" "hook-blocked" "pre-commit lint rejected formatting"
log_iteration 6 "-" "-" "-" "-" "-" "metric-error" "eval output was 'PASS' -- not a number"
```

### Example (pass/fail guard)

```tsv
iteration	commit	metric	delta	guard	guard-metric	status	description
0	a1b2c3d	85.2	0.0	pass	-	baseline	initial state -- test coverage 85.2%
1	b2c3d4e	87.1	+1.9	pass	-	kept	add tests for auth middleware edge cases
2	-	86.5	-0.6	-	-	discarded	refactor test helpers (broke 2 tests)
3	-	-	-	-	-	crash	eval crashed: DB connection failed after 3 fix attempts
4	-	88.9	+1.8	fail	-	discarded	inline hot-path (guard: 3 tests broke)
5	c3d4e5f	88.3	+1.2	pass	-	kept	add tests for error handling in API routes
```

### Example (metric-valued guard -- bundle size with 5% threshold)

```tsv
iteration	commit	metric	delta	guard	guard-metric	status	description
0	a1b2c3d	85.2	0.0	pass	48200	baseline	coverage 85.2%, bundle 48200 bytes
1	b2c3d4e	87.1	+1.9	pass	48500	kept	add auth tests (bundle +300, within 5%)
2	-	88.0	+0.9	fail	51500	discarded	add integration tests (bundle exceeds 5% threshold)
3	c3d4e5f	87.8	+0.7	pass	47900	kept	add unit tests (bundle decreased)
```

### Reading the TSV

```bash
# Phase 1 (Review): Read recent entries
tail -20 .autoresearch/results.tsv

# Count outcomes
KEEPS=$(grep -c 'kept' .autoresearch/results.tsv || echo 0)
DISCARDS=$(grep -c 'discarded' .autoresearch/results.tsv || echo 0)
CRASHES=$(grep -c 'crash' .autoresearch/results.tsv || echo 0)

# Detect stuck state
LAST_5=$(tail -5 .autoresearch/results.tsv | awk -F'\t' '{print $7}')
# If all 5 are "discarded" -> trigger stuck recovery protocol

# Pattern recognition: what changes succeed?
grep 'kept' .autoresearch/results.tsv | awk -F'\t' '{print $8}'
```

---

## Integration with the Loop Protocol

Where logging fits in the loop lifecycle:

```
Phase 0 (Setup):    -> CREATE log, record baseline (iteration 0)
Phase 1 (Review):   -> READ last 10-20 entries for pattern recognition
Phase 3-6 (Loop):   -> Modify, Commit, Verify, Decide
Phase 7 (Log):      -> APPEND new row after keep/discard/crash decision
Phase 8 (Repeat):   -> Back to Phase 1 (reads updated log)
```

Complete end-to-end example:

```
/aris
Goal: Increase test coverage from 72% to 90%
Scope: src/**/*.ts
Verify: npx jest --coverage 2>&1 | grep 'All files' | awk '{print $4}'
Guard: npm run typecheck

# Internal lifecycle:
# 1. Agent runs aris record --metric 72.0 --status baseline --summary "Initial baseline"
# 2. Agent reads log (aris log -n 20) -> decides first experiment
# 3. Agent modifies code, commits, runs verify -> gets 74.5
# 4. Agent runs aris record --metric 74.5 --status kept --summary "add auth tests"
# 5. Next iteration: agent reads log, sees auth tests worked -> tries similar pattern
# 6. Continues until coverage reaches 90% or iterations exhausted
```

---

## Summary Reporting

Every 10 iterations (or at loop completion in bounded mode), print a brief summary:

```
=== ARIS Progress (iteration 20) ===
Baseline: 85.2% -> Current best: 92.1% (+6.9%)
Keeps: 8 | Discards: 10 | Crashes: 2
Last 5: kept, discarded, discarded, kept, kept
============================================
```

With CLI:
```bash
aris report  # Full markdown report
aris best    # Quick best-experiment summary
```

---

## Log Management

- Create at setup (iteration 0 = baseline)
- Append after EVERY iteration (including crashes, no-ops, errors)
- Do NOT commit experiment logs to git (add to .gitignore)
- Read last 10-20 entries at start of each iteration for context
- Use to detect patterns: what kind of changes tend to succeed?

## Metric Direction

Clarify at setup whether lower or higher is better:
- **Lower is better:** val_bpb, response time (ms), bundle size (KB), error count
- **Higher is better:** test coverage (%), lighthouse score, throughput (req/s)

The direction is stored in `autoresearch.toml` as `metric_direction`.
In fallback TSV mode, record it as a comment in the first line:
```
# metric_direction: higher_is_better
```

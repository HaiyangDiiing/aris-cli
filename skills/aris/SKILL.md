---
name: aris
description: >
  Use when user types /aris, /aris:plan, /aris:debug, or /aris:fix,
  or mentions "aris", "autoresearch", "experiment loop", "optimize this metric",
  "run experiments", or wants autonomous iteration on any measurable goal.
  ARIS (Autonomous Research Iteration System) — iteratively improve any measurable
  metric by modifying code, evaluating results, and keeping improvements.
  Reads autoresearch.toml for config. Run `aris init` first.
argument-hint: "[goal or metric to optimize]"
user-invocable: true
metadata:
  version: "0.3.3"
  author: 199-biotechnologies
---

# ARIS — Autonomous Research Iteration System

> "Research while you sleep"

Inspired by [Karpathy's autoresearch](https://github.com/karpathy/autoresearch). Applies
constraint-driven autonomous iteration to ANY work — not just ML research.

**Core idea:** You are an autonomous agent. Modify -> Verify -> Keep/Discard -> Repeat.

**Architecture:** This SKILL.md is the index and router. The full protocols live in
`references/` files, loaded on demand when the relevant subcommand executes. The `aris`
CLI provides structured data operations (record, log, best, export, watch). The protocol
(Markdown) is the control plane (WHAT to do); the CLI (Rust) is the data plane (HOW to
store/query/visualize).

---

## MANDATORY: Interactive Setup Gate

**CRITICAL -- READ THIS FIRST BEFORE ANY ACTION:**

For ALL commands (`/aris`, `/aris:plan`, `/aris:debug`, `/aris:fix`):

1. **Check if the user provided ALL required context inline** (Goal, Scope, Metric, flags, etc.)
2. **If ANY required context is missing -> you MUST use `AskUserQuestion` to collect it
   BEFORE proceeding to any execution phase.** DO NOT skip this step. DO NOT proceed
   without user input.
3. Each subcommand's reference file has an "Interactive Setup" section -- follow it exactly
   when context is missing.

| Command | Required Context | If Missing -> Ask |
|---------|-----------------|------------------|
| `/aris` | Goal, Scope, Metric, Direction, Verify | Batch 1 (4 questions) + Batch 2 (3 questions) from Setup Phase below |
| `/aris:plan` | Goal | Ask via `AskUserQuestion` per `references/plan-workflow.md` |
| `/aris:debug` | Issue/Symptom, Scope | 4 batched questions per `references/debug-workflow.md` |
| `/aris:fix` | Target, Scope | 4 batched questions per `references/fix-workflow.md` |

**YOU MUST NOT start any loop, phase, or execution without completing interactive setup
when context is missing. This is a BLOCKING prerequisite.**

---

## Subcommands

| Subcommand | Purpose | Reference File |
|------------|---------|----------------|
| `/aris` | Run the autonomous experiment loop (default) | `references/autonomous-loop-protocol.md` |
| `/aris:plan` | Interactive wizard to build Scope, Metric, Direction & Verify from a Goal | `references/plan-workflow.md` |
| `/aris:debug` | Autonomous bug-hunting loop: scientific method + iterative investigation | `references/debug-workflow.md` |
| `/aris:fix` | Autonomous fix loop: iteratively repair errors until zero remain | `references/fix-workflow.md` |

### /aris -- Autonomous Experiment Loop (default)

The core workflow. Iteratively improves a measurable metric by modifying code, running
experiments, and keeping what works. Runs hundreds of experiments. Most will fail.
That's expected. The wins compound.

Load: `references/autonomous-loop-protocol.md` for the full 8-phase protocol.

**Quick summary of the 8 phases:**

```
Phase 0: Precondition  -- aris doctor (or manual checks)
Phase 1: Review        -- aris log + git log + git diff (EVERY iteration)
Phase 2: Ideate        -- Pick ONE atomic change based on history
Phase 3: Modify        -- Implement the change (one change per experiment)
Phase 4: Commit        -- git commit BEFORE verify (enables clean revert)
Phase 5: Verify        -- Run eval, extract metric, validate numeric
Phase 5.5: Guard       -- If guard_command configured, check it (rework if needed)
Phase 5.7: Crash       -- If eval crashed, auto-fix (max 3 attempts)
Phase 6: Decide        -- Mechanical decision: keep, discard, rework, crash, no-op
Phase 7: Log           -- aris record (or TSV fallback)
Phase 8: Loop          -- Back to Phase 1 (stuck/plateau detection)
```

**Usage:**
```
# Unlimited -- keep improving until interrupted or plateau
/aris
Goal: Increase test coverage to 90%
Scope: src/**/*.ts
Metric: coverage %
Direction: higher
Verify: npx jest --coverage 2>&1 | grep 'All files' | awk '{print $4}'

# Bounded -- exactly 25 iterations
/aris
Goal: Reduce bundle size below 200KB
Iterations: 25

# With guard (regression prevention)
/aris
Goal: Improve API response time
Verify: node bench.js 2>&1 | tail -1
Guard: npm test
Direction: lower
```

### /aris:plan -- Goal -> Configuration Wizard

Converts a plain-language goal into a validated, ready-to-execute aris configuration.

Load: `references/plan-workflow.md` for the full 7-step protocol.

**Quick summary:**

1. **Capture Goal** -- ask what the user wants to improve (or accept inline text)
2. **Analyze Context** -- scan codebase for tooling, test runners, build scripts
3. **Define Scope** -- suggest file globs, validate they resolve to real files
4. **Define Metric** -- suggest mechanical metrics, validate they output a number
5. **Define Direction** -- higher or lower is better
6. **Define Verify** -- construct the shell command, **dry-run it**, confirm it works
7. **Confirm & Launch** -- present the complete config, offer to launch immediately

**Critical gates:**
- Metric MUST be mechanical (outputs a parseable number, not subjective)
- Verify command MUST pass a dry run on the current codebase before accepting
- Scope MUST resolve to >= 1 file

**Usage:**
```
/aris:plan
Goal: Make the API respond faster

/aris:plan Increase test coverage to 95%

/aris:plan Reduce bundle size below 200KB
```

After the wizard completes, the user gets a ready-to-paste `/aris` invocation -- or can
launch it directly.

### /aris:debug -- Autonomous Bug-Hunting Loop

Scientific bug hunting protocol. Uses the autoresearch loop pattern to systematically
find and fix bugs through observation, hypothesis, and verification.

Load: `references/debug-workflow.md` for the full protocol.

**What it does:**

1. **Reproduce** -- confirm the bug exists with a failing test or observable symptom
2. **Observe** -- gather evidence: error logs, stack traces, `aris log` for regression points
3. **Hypothesize** -- form a specific, testable hypothesis about the root cause
4. **Test** -- make ONE targeted change to test the hypothesis
5. **Verify** -- run the failing test/scenario -- does it pass now?
6. **Record** -- log the result with `aris record`
7. **Iterate** -- if not fixed, use what you learned to form a better hypothesis

**Usage:**
```
# Interactive (will ask for symptom and scope)
/aris:debug

# With context
/aris:debug
Issue: API returns 500 on POST /users with special characters
Scope: src/api/**/*.ts
Iterations: 10
```

### /aris:fix -- Autonomous Fix Loop

Iteratively repairs errors (tests, types, lint, build) until zero remain. Each iteration
picks the highest-priority error, fixes it, verifies the fix, and logs the result.

Load: `references/fix-workflow.md` for the full protocol.

**What it does:**

1. **Scan** -- run the verification command to enumerate all current errors
2. **Prioritize** -- rank errors: build > type > test > lint
3. **Fix** -- apply ONE targeted fix for the highest-priority error
4. **Verify** -- re-run verification -- did the error count decrease?
5. **Record** -- log result with `aris record`
6. **Iterate** -- repeat until zero errors or iteration limit

**Usage:**
```
# Fix all test failures
/aris:fix
Target: npm test
Scope: src/**/*.ts

# Fix build errors
/aris:fix
Target: cargo build 2>&1
Iterations: 10

# Fix lint issues
/aris:fix
Target: npx eslint src/ --format json
Scope: src/**/*.ts
```

---

## When to Activate

- User invokes `/aris` -> run the loop
- User invokes `/aris:plan` -> run the planning wizard
- User invokes `/aris:debug` -> run the debug loop
- User invokes `/aris:fix` -> run the fix loop
- User says "help me set up aris", "plan an experiment run" -> run the planning wizard
- User says "find all bugs", "hunt bugs", "debug this", "why is this failing" -> run the debug loop
- User says "fix all errors", "make tests pass", "fix the build" -> run the fix loop
- User says "work autonomously", "iterate until done", "keep improving", "run overnight" -> run the loop
- User mentions "autoresearch" with a goal -> run the loop (backward compatibility)
- Any task requiring repeated iteration cycles with measurable outcomes -> run the loop

---

## Bounded Iterations

By default, aris loops until the metric plateaus (no improvement to the best metric for
15 consecutive measured iterations), then asks the user whether to stop, continue, or
change strategy. To run exactly N iterations instead, add `Iterations: N` to your
inline config.

**Unlimited (default):**
```
/aris
Goal: Increase test coverage to 90%
```

**Bounded (N iterations):**
```
/aris
Goal: Increase test coverage to 90%
Iterations: 25
```

After N iterations, print a final summary with baseline -> current best, keeps/discards/
crashes. If the goal is achieved before N iterations, print early completion and stop.

### When to Use Bounded Iterations

| Scenario | Recommendation |
|----------|---------------|
| Run overnight, review in morning | Unlimited + `Plateau-Patience: off` |
| Quick 30-min improvement session | `Iterations: 10` |
| Targeted fix with known scope | `Iterations: 5` |
| Exploratory -- see if approach works | `Iterations: 15` |
| CI/CD pipeline integration | `--iterations N` flag |
| Long run with safety net (default) | Unlimited (plateau detection after 15 iterations) |

### Plateau Detection

In unlimited mode, aris tracks whether the best metric is still improving. If 15
consecutive measured iterations pass without a new best, the loop pauses and asks the
user to decide: stop, continue, or change strategy. Configure with `Plateau-Patience: N`
(default 15), or disable with `Plateau-Patience: off`. Bounded mode ignores this setting.

```
/aris
Goal: Reduce bundle size below 200KB
Verify: npx esbuild src/index.ts --bundle --minify | wc -c
Plateau-Patience: 20
```

### Metric-Valued Guards

By default, guards are pass/fail (exit code 0 = pass). For guards that measure a number
(bundle size, response time, coverage), you can set a regression threshold:

```
/aris
Goal: Increase test coverage to 95%
Verify: npx jest --coverage 2>&1 | grep 'All files' | awk '{print $4}'
Guard: npx esbuild src/index.ts --bundle --minify | wc -c
Guard-Direction: lower is better
Guard-Threshold: 5%
```

This means: "optimize coverage, but reject any change that grows bundle size more than
5% from baseline." The primary metric still drives keep/discard. The guard-metric is
tracked in the results log for visibility into drift over time.

| Parameter | Required | Description |
|-----------|----------|-------------|
| `Guard` | Yes | Command that outputs a number (metric-valued) or exits 0/1 (pass/fail) |
| `Guard-Direction` | Only for metric-valued | `higher is better` or `lower is better` |
| `Guard-Threshold` | Only for metric-valued | Max allowed regression as % of baseline (e.g., `5%`, `0%` for strict) |

Without `Guard-Direction` and `Guard-Threshold`, the guard operates in pass/fail mode.

---

## Setup Phase (Do Once)

**If the user provides Goal, Scope, Metric, and Verify inline** -> extract them and
proceed to step 5.

**CRITICAL: If ANY critical field is missing (Goal, Scope, Metric, Direction, or Verify),
you MUST use `AskUserQuestion` to collect them interactively. DO NOT proceed to The Loop
or any execution phase without completing this setup. This is a BLOCKING prerequisite.**

### Interactive Setup (when invoked without full config)

Scan the codebase first for smart defaults, then ask ALL questions in batched
`AskUserQuestion` calls (max 4 per call). This gives users full clarity upfront.

**Batch 1 -- Core config (4 questions in one call):**

Use a SINGLE `AskUserQuestion` call with these 4 questions:

| # | Header | Question | Options (smart defaults from codebase scan) |
|---|--------|----------|----------------------------------------------|
| 1 | `Goal` | "What do you want to improve?" | "Test coverage (higher)", "Bundle size (lower)", "Performance (faster)", "Code quality (fewer errors)" |
| 2 | `Scope` | "Which files can aris modify?" | Suggested globs from project structure (e.g. "src/**/*.ts", "content/**/*.md") |
| 3 | `Metric` | "What number tells you if it got better? (must be a command output, not subjective)" | Detected options: "coverage % (higher)", "bundle size KB (lower)", "error count (lower)", "test pass count (higher)" |
| 4 | `Direction` | "Higher or lower is better?" | "Higher is better", "Lower is better" |

**Batch 2 -- Verify + Guard + Launch (3 questions in one call):**

| # | Header | Question | Options |
|---|--------|----------|---------|
| 5 | `Verify` | "What command produces the metric? (I'll dry-run it to confirm)" | Suggested commands from detected tooling |
| 6 | `Guard` | "Any command that must ALWAYS pass? (prevents regressions)" | "npm test", "tsc --noEmit", "npm run build", "Skip -- no guard" |
| 7 | `Launch` | "Ready to go?" | "Launch (unlimited)", "Launch with iteration limit", "Edit config", "Cancel" |

**After Batch 2:** Dry-run the verify command. If it fails, ask user to fix or choose a
different command. If it passes, proceed with launch choice.

**IMPORTANT:** You MUST call `AskUserQuestion` with batched questions -- never ask one at
a time, and never skip this step. Users should see all config choices together for full
context. DO NOT proceed to Setup Steps or The Loop without completing interactive setup.

### Setup Steps (after config is complete)

1. **Read all in-scope files** for full context before any modification
2. **Define the goal** -- extracted from user input or inline config
3. **Define scope constraints** -- validated file globs
4. **Define guard (optional)** -- regression prevention command
5. **Create a results log** -- Track every iteration (see `references/results-logging.md`)
6. **Establish baseline** -- Run verification on current state AND guard (if set).
   Record as iteration #0
7. **Confirm and go** -- Show user the setup, get confirmation, then BEGIN THE LOOP

---

## The Loop

Read `references/autonomous-loop-protocol.md` for the full 8-phase protocol.

```
LOOP (FOREVER or N times):
  1. Review: Read current state + git history + results log
  2. Ideate: Pick next change based on goal, past results, what hasn't been tried
  3. Modify: Make ONE focused change to in-scope files
  4. Commit: Git commit the change (before verification)
  5. Verify: Run the mechanical metric (tests, build, benchmark, etc.)
  6. Guard: If guard is set, run the guard command
  7. Decide:
     - IMPROVED + guard passed (or no guard) -> Keep commit, log "keep", advance
     - IMPROVED + guard FAILED -> Revert, then try to rework the optimization
       (max 2 attempts) so it improves the metric WITHOUT breaking the guard.
       Never modify guard/test files -- adapt the implementation instead.
       If still failing -> log "discard (guard failed)" and move on
     - SAME/WORSE -> Git revert, log "discard"
     - CRASHED -> Try to fix (max 3 attempts), else log "crash" and move on
  8. Log: Record result in results log
  9. Repeat: Go to step 1.
     - If unbounded: NEVER STOP. NEVER ASK "should I continue?"
     - If bounded (N): Stop after N iterations, print final summary
```

### Decision Matrix (Quick Reference)

| Condition | Action | Record Status |
|-----------|--------|---------------|
| Metric improved AND (no guard OR guard passed) | **KEEP** | `--status kept` |
| Metric improved AND guard failed | **REWORK** (max 2 attempts) | `--status kept` if rework succeeds |
| Metric same or worse | **DISCARD** + safe_revert | `--status discarded` |
| Eval crashed | **AUTO-FIX** (max 3 attempts) | `--status crash` if unfixable |
| No diff produced (no-op) | **SKIP** | `--status no-op` |
| Pre-commit hook blocked | **REVERT** | `--status hook-blocked` |
| Metric non-numeric | **REVERT** | `--status metric-error` |
| 2 consecutive metric-errors | **STOP LOOP** | Fatal: eval is broken |
| 15 iterations no improvement | **PLATEAU** alert | Ask user or change strategy |

---

## Critical Rules

1. **Loop until done** -- Unbounded: loop until interrupted. Bounded: loop N times then
   summarize.
2. **Read before write** -- Always understand full context before modifying
3. **One change per iteration** -- Atomic changes. If it breaks, you know exactly why
4. **Mechanical verification only** -- No subjective "looks good". Use metrics
5. **Automatic rollback** -- Failed changes revert instantly. No debates
6. **Simplicity wins** -- Equal results + less code = KEEP. Tiny improvement + ugly
   complexity = DISCARD
7. **Git is memory** -- Every experiment committed with `[aris]` prefix. Use
   `git revert` (not `git reset --hard`) for rollbacks so failed experiments remain
   visible in history. Agent MUST read `git log` and `git diff` of kept commits to
   learn patterns before each iteration
8. **When stuck, think harder** -- Re-read files, re-read goal, combine near-misses,
   try radical changes. Don't ask for help unless truly blocked by missing
   access/permissions

---

## Core Principles

See `references/core-principles.md` for the full treatment. Summary:

### 1. Constraint as Enabler

Tight constraints (one file, one metric, one change per iteration) don't limit you --
they focus you. The narrower the search space, the faster you converge. A 126-experiment
overnight session works because each experiment is atomic and fast.

### 2. Strategy/Tactics Separation

**Strategy** = what to try next (human decides via `program.md`, agent adjusts via
Phase 2 priority ordering). **Tactics** = how to implement it (agent handles fully).
Never let tactical complexity contaminate strategic decisions.

### 3. Mechanical Metrics

If a human must judge it, you can't run it overnight. The metric MUST be:
- A number (not "looks better")
- Extracted from a command (not eye-balled)
- Reproducible (same input -> same output, within noise bounds)
- Fast (seconds, not minutes)

Use `aris record --metric X` for structured tracking with validation.

### 4. Fast Verification

The eval command is the bottleneck. Every second it takes multiplies across hundreds of
iterations. Optimize the eval first:
- Use focused test suites (not full CI)
- Cache expensive setup
- Run only relevant benchmarks
- `aris doctor` validates the eval command works before the loop starts

### 5. Cost Shapes Behavior

Cheap experiments encourage exploration. Expensive experiments encourage premature
convergence. Design your setup so that each iteration is:
- Fast (< 60 seconds ideal)
- Cheap (no cloud costs per run)
- Reversible (git revert, not database migrations)

### 6. Git as Memory

Every experiment is a git commit. Every discard is a git revert. The full history of
what you tried -- including failures -- is in `git log`. Read it. Learn from it.

- `git diff HEAD~1` tells you WHY a change worked (not just THAT it worked)
- `git log --oneline` shows patterns across experiments
- `aris log` provides structured experiment data alongside git history

### 7. Honest Limits

Know when to stop:
- Plateau detected (15 iterations, no improvement) -> escalate to user
- 2 consecutive metric-errors -> eval is broken, stop
- 7 consecutive discards -> deeply stuck, change strategy fundamentally
- The metric is a proxy, not the goal. If the proxy is saturated, change the proxy

---

## CLI Command Reference

ARIS provides a Rust CLI (`aris`) for structured data operations. All commands support
`--json` flag and `AUTORESEARCH_FORMAT=json` environment variable for machine-readable
output. ALL state management goes through the CLI. NEVER write to experiments.jsonl
directly.

### CLI Detection

At the start, detect which mode to use:

```bash
if command -v aris >/dev/null 2>&1; then
    CLI_MODE="enhanced"  # Structured data, validation, reward-hacking detection
else
    echo "Warning: aris CLI not found. Running in fallback mode."
    echo "  Install with: cargo install aris-cli"
    CLI_MODE="fallback"  # Bash commands only, no validation
fi
```

### Command Table

| Command | Purpose | When to Use |
|---------|---------|-------------|
| `aris doctor` | Pre-flight check (14+ validations) | Phase 0: before first iteration |
| `aris init --target-file F --eval-command C` | Initialize project config | Once: project setup |
| `aris record --metric X --status Y --summary "Z"` | Record experiment result | Phase 7: after every iteration |
| `aris log [-n N]` | View experiment history | Phase 1: every iteration |
| `aris best` | Best result + diff from baseline | Anytime: check progress |
| `aris diff <a> <b>` | Compare two experiments | Analysis: compare approaches |
| `aris status` | Current loop state | Anytime: check if loop is running |
| `aris review` | Generate cross-model review prompt | When stuck: get fresh perspective |
| `aris fork <names...>` | Create parallel exploration branches | When stuck: try multiple directions |
| `aris merge-best` | Compare forks, find the winner | After forking: pick the best |
| `aris report` | Generate markdown summary | End of loop: final report |
| `aris export --format csv` | Export data for analysis | Analysis: external tools |
| `aris watch` | Live terminal dashboard | Separate terminal: monitor progress |
| `aris guide` | Print full methodology | Reference: works without skill installed |
| `aris agent-info` | Machine-readable CLI capabilities | Agent discovery: what commands exist |

### CLI Fallback (Without Binary)

Every CLI command has a bash fallback for environments where the binary isn't installed.
The protocol in `references/autonomous-loop-protocol.md` documents both modes. Key
fallbacks:

| CLI Command | Bash Fallback |
|-------------|---------------|
| `aris doctor` | Manual checks (git repo, config, target file, eval command) |
| `aris record --metric X --status Y --summary "Z"` | `echo -e "N\t$(git rev-parse --short HEAD)\tX\t...\tY\tZ" >> .autoresearch/results.tsv` |
| `aris log -n 20` | `tail -20 .autoresearch/experiments.jsonl` or `tail -20 .autoresearch/results.tsv` |
| `aris best` | `sort -t'\t' -k3 -n .autoresearch/results.tsv \| tail -1` |

The CLI adds validation, reward-hacking detection, and structured data -- but the loop
doesn't REQUIRE it. Fallback mode records data but skips validation.

### Record Statuses

| Status | Meaning | Has Metric? |
|--------|---------|:-----------:|
| `baseline` | Initial measurement | Yes |
| `kept` | Experiment improved metric | Yes |
| `discarded` | Experiment did not improve | Yes |
| `crash` | Eval command failed, unfixable after 3 attempts | Optional |
| `no-op` | No code changes were produced | No |
| `hook-blocked` | Pre-commit hook rejected commit | No |
| `metric-error` | Eval output was non-numeric | No |

---

## Adapting to Different Domains

The PRINCIPLES are universal; the METRICS are domain-specific. Adapt the loop to your
domain:

| Domain | Metric | Scope | Verify Command | Guard |
|--------|--------|-------|----------------|-------|
| Backend code | Tests pass + coverage % | `src/**/*.ts` | `npm test` | -- |
| Frontend UI | Lighthouse score | `src/components/**` | `npx lighthouse` | `npm test` |
| ML training | val_bpb / loss | `train.py` | `uv run train.py` | -- |
| Blog/content | Word count + readability | `content/*.md` | Custom script | -- |
| Performance | Benchmark time (ms) | Target files | `npm run bench` | `npm test` |
| Refactoring | Tests pass + LOC reduced | Target module | `npm test && wc -l` | `npm run typecheck` |
| Bug hunting | Bugs found + test coverage | Target files | `/aris:debug` | -- |
| Error fixing | Error count (lower) | Target files | `/aris:fix` | `npm test` |

---

## Experiment Ordering (Research Strategy)

These strategies come from Karpathy's 126-experiment overnight sessions, community results
(chess engines, Sudoku solvers, trading bots), and triple-audit hardening. Follow them.

### Priority Order

1. **Hyperparameters first** -- Learning rate, batch size, weight decay, warmup steps.
   Lowest risk, highest signal. Karpathy's agent found that "AdamW betas were all messed
   up" -- a simple hyperparameter fix that a human missed for years.

2. **Regularization second** -- Dropout, weight decay schedules, gradient clipping, label
   smoothing. Karpathy's agent found "Value Embeddings really like regularization and I
   wasn't applying any."

3. **Architecture changes third** -- Only after hyperparameters are tuned. Architecture
   changes are high-variance: they either work great or catastrophically. The community
   found that most architecture experiments fail (Sudoku: 263 runs, most "failed
   catastrophically," but the winner beat the paper by 5%).

4. **Exotic ideas last** -- Novel approaches, paper reproductions, unconventional techniques.
   Save these for when standard approaches plateau.

### Where ARIS Works Best

ARIS is most effective for tasks where:
1. The objective is a single scalar with clear better/worse signal
2. Quality degradation is cheap to evaluate (simple pass/fail gate)
3. The search space is micro-optimizations: each change is small, independent, and
   instantly benchmarkable

### Reward Hacking Awareness

The metric can lie. Watch for:
- **Overfitting the eval** -- If your eval has limited test cases, the agent might optimize
  for those specific cases without generalizing. This is the #1 community complaint.
- **Secondary costs** -- A change that improves loss but doubles training time is not a
  real win.
- **Goodhart's Law** -- When the metric becomes the target, it ceases to be a good metric.
  After 108 legit experiments (2.6x speedup ceiling), one agent started caching I/O to
  fake a 101,687x speedup by memorizing the small test set.
- **The CLI warns you** -- `aris record` has built-in reward-hacking detection. If you
  record a `kept` with implausible improvement, it will flag it.

---

## Noise Handling

For metrics with inherent noise (benchmarks, latency tests):

### Multi-run median

Run eval multiple times, use the median:

```bash
# With CLI (multi-sample support):
aris record --metric $M1 --metric $M2 --metric $M3 --status kept --summary "..."
# CLI computes median, MAD, and confidence automatically
```

### Minimum delta threshold

Ignore improvements smaller than the noise floor:

```
IF abs(new_metric - best_metric) < min_delta:
    Treat as "same" -> DISCARD (within noise range)
```

### Confirmation run

For suspicious improvements (> 20% gain), re-run eval to confirm:

```
IF second_run confirms improvement (within 5% of first):
    KEEP (confirmed)
ELSE:
    DISCARD (noise artifact)
```

---

## Config & Data Files

| File | Purpose |
|------|---------|
| `autoresearch.toml` | Project configuration (target file, eval command, metric, direction) |
| `program.md` | Research direction, ideas, constraints from the human researcher |
| `.autoresearch/experiments.jsonl` | Structured experiment log (managed by CLI) |
| `.autoresearch/results.tsv` | Fallback experiment log (when CLI unavailable) |
| `.autoresearch/loop.lock` | Lock file indicating active loop (iteration count, start time) |

**Note:** Config and data files use the `autoresearch` name for backward compatibility.
The `aris` CLI reads and writes these files transparently.

---

## Parallel Exploration (Forking)

When stuck with genuinely different hypotheses, fork into parallel branches:

```bash
aris fork approach-a approach-b approach-c
```

This creates parallel branches (`autoresearch-fork-approach-a`, etc.). Assign each to a
different agent or session. After all finish:

```bash
aris merge-best
```

Compares fork branches and merges the winner back.

Use forks when you have **genuinely different hypotheses** -- e.g., "try transformers vs
convolutions vs linear models." Don't fork for small parameter variations.

---

## Works With

- **`/aris:plan`** -- Set up the loop configuration interactively
- **`/aris:debug`** -- Scientific bug hunting with the same loop pattern
- **`/aris:fix`** -- Error fixing loop for build/test/lint failures
- **`aris review`** -- Pipe to a second model for cross-model second opinions
- **`aris fork`** -- Parallel exploration when stuck

---

## Post-Completion: Support Prompt (Once Per Project)

After the FIRST successful completion of any aris command in a project (loop finishes,
summary printed, or bounded iterations complete), check if the file `.autoresearch-starred`
exists in the project root.

**If it does NOT exist:**

Ask the user: *"If aris helped, would you like to star the repo on GitHub to support the
project? I can run `gh api -X PUT /user/starred/HaiyangDiiing/aris-cli` for you."*

- If yes -> run the command, then create `.autoresearch-starred` marker file
- If no -> create `.autoresearch-starred` marker file (never ask again)

**If it exists:** skip entirely. Never ask twice.

---

## Version

Installed by aris-cli v0.3.3

Protocol files: `skills/aris/SKILL.md` (this file) + `skills/aris/references/*.md`
Command dispatchers: `commands/aris.md` + `commands/aris/*.md`

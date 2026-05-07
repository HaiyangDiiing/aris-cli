<div align="center">

![ARIS banner](assets/banner.png)

# ARIS

**Autonomous Research Iteration System**

*Autonomous experiment loops for AI coding agents*

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

Turn any measurable engineering goal into a guarded loop: your agent edits code, runs verification, records the metric, keeps improvements, and reverts failures.

Inspired by [Karpathy's autoresearch](https://github.com/karpathy/autoresearch).

[Quick Start](#quick-start) · [Why ARIS](#why-aris) · [How It Works](#how-it-works) · [Skill Commands](#skill-commands) · [CLI](#cli)

[中文文档](README.md)

</div>

---

## What Is This?

ARIS is a **portable skill protocol** plus an optional Rust CLI for AI coding agents. It gives the agent a disciplined way to optimize measurable targets without drifting into vague "try things until it feels better" work.

Use the skill alone if you want a prompt-level workflow. Add the CLI when you want one-command installation, pre-flight checks, structured experiment logs, best-result lookup, reports, and a live TUI.

**Supported agents:** Claude Code, OpenAI Codex, Cursor, Windsurf, OpenCode, Gemini CLI, GitHub Copilot, and universal `.agents/skills/` workflows.

**What it does:**

```
You: /aris
     Goal: Increase test coverage to 90%
     Verify: pytest --cov | grep TOTAL | awk '{print $NF}' | tr -d '%'

Agent: (runs 50+ experiments autonomously)
       Baseline: 72% → Best: 91% (23 keeps, 27 discards, 0 crashes)
```

**Describe tasks in natural language — no structured fields required:**

```
You: /aris
     I need to test whether my data preprocessing pipeline is fully ok.
     Validate every output item using vllm. All 133 items passing is our goal.

Agent: (understands the objective, breaks down verification, iterates)
       Baseline: 98/133 passing → Final: 133/133 passing (12 keeps, 5 discards)
```

Describe your goal like you'd tell a colleague. The agent infers the metric, verification method, and completion criteria automatically.

You set the goal. The agent does the work. You review the results.

---

## Quick Start

### 1. Install ARIS

```bash
cargo install aris-cli
aris install claude-code
```

Use `aris install codex`, `cursor`, `windsurf`, `opencode`, `gemini`, `copilot`, `agents`, or `all` for other agent targets.

### 2. Configure a measurable goal

```bash
aris init \
  --target-file src/api/routes.ts \
  --eval-command "npm run bench:api | grep p99 | awk '{print $NF}'" \
  --metric-name p99_latency \
  --metric-direction lower

aris doctor
```

`aris doctor` checks that the project, git state, eval command, and logging setup are ready before the agent starts iterating.

### 3. Start the loop

In your AI coding agent, type:

```
/aris
Goal: Reduce API response time below 100ms
Scope: src/api/**/*.ts
Metric: p99 latency (ms)
Direction: lower
Verify: npm run bench:api | grep "p99" | awk '{print $NF}'
Guard: npm test
```

That's it. The agent enters an autonomous loop — modifying, verifying, keeping or reverting — until the goal is reached or you interrupt.

Prefer manual installation? Copy `skills/aris/`, `commands/aris/`, and `commands/aris.md` into your agent's skill/command directories. The protocol works without the CLI; the CLI just makes installation, validation, and reporting easier.

---

## Why ARIS?

AI coding agents are good at making changes. They are less reliable at running long, disciplined optimization campaigns unless the workflow is explicit. ARIS supplies that workflow.

| Problem | ARIS behavior |
|---------|---------------|
| Agents make broad, hard-to-debug changes | One atomic change per iteration |
| "Looks better" replaces real measurement | Mechanical metric verification only |
| Failed attempts pollute the working tree | Automatic rollback through git |
| Experiment history disappears from context | Structured logs and commit history |
| Metrics get gamed after many iterations | Reward-hacking detection flags suspicious jumps |
| Different agents need different prompts | One portable skill protocol across platforms |

---

## How It Works

### The 8-Phase Loop

```
Phase 0: Precondition  — validate environment (git repo, config, eval command)
Phase 1: Review        — read experiment history + git log (EVERY iteration)
Phase 2: Ideate        — pick ONE atomic change based on past results
Phase 3: Modify        — implement the change
Phase 4: Commit        — git commit BEFORE verify (enables clean revert)
Phase 5: Verify        — run eval, extract metric
Phase 6: Decide        — mechanical decision: keep / discard / rework / crash
Phase 7: Log           — record result
Phase 8: Loop          — back to Phase 1
```

### Decision Rules

| Condition | Action |
|-----------|--------|
| Metric improved, guard passed | **KEEP** — advance |
| Metric improved, guard failed | **REWORK** (max 2 attempts) |
| Metric same or worse | **DISCARD** — git revert |
| Eval crashed | **AUTO-FIX** (max 3 attempts) |
| No code change produced | **SKIP** |
| 15 iterations without improvement | **PLATEAU** — pause and ask |

### Core Principles

1. **One change per iteration** — atomic. If it breaks, you know why.
2. **Mechanical verification only** — no subjective judgment. Numbers only.
3. **Automatic rollback** — failed changes revert via `git revert`.
4. **Git is memory** — every experiment committed, failures visible in history.
5. **Hyperparameters first** — lowest risk, highest signal. Architecture last.
6. **Reward hacking detection** — flags implausible improvements.

---

## Skill Commands

| Command | What the agent does |
|---------|---------------------|
| `/aris` | Run the autonomous experiment loop |
| `/aris:plan` | Interactive wizard: Goal → Scope, Metric, Direction, Verify |
| `/aris:debug` | Autonomous bug-hunting loop (scientific method) |
| `/aris:fix` | Iteratively repair errors until zero remain |

### Usage Examples

```
# Unlimited — loop until interrupted or plateau
/aris
Goal: Increase test coverage to 90%
Scope: src/**/*.ts
Verify: npx jest --coverage | grep 'All files' | awk '{print $4}'

# Bounded — exactly 25 iterations
/aris
Goal: Reduce bundle size below 200KB
Iterations: 25

# With guard (regression prevention)
/aris
Goal: Improve API response time
Verify: node bench.js | tail -1
Guard: npm test
Direction: lower
```

### Natural Language Works Too

No need to memorize structured fields — just describe what you want in plain language:

```
/aris
I need to test my data preprocessing pipeline end-to-end. Validate every
output item using vllm. All 133 items passing is our goal.

/aris
Get this model's inference latency under 50ms. Use wrk for benchmarking.
Don't break existing unit tests.

/aris
My ETL script currently hangs on 17 edge cases. Fix them one by one.
cargo test all green means done.
```

The agent automatically infers the target metric, verification command, scope, and completion criteria from your description.

### Don't Know What Metric to Use?

```
/aris:plan
Goal: Make the API faster
```

The plan wizard analyzes your codebase, suggests metrics, and dry-runs the verify command before launching.

---

## CLI

ARIS works without any binary — the skill protocol handles everything through normal shell and git commands. The CLI adds the parts that make longer runs easier to trust: installation, validation, structured logs, best-result lookup, reports, export, parallel exploration, and a live dashboard.

```bash
cargo install aris-cli
```

### What the CLI Adds

| Without CLI (bash fallback) | With CLI |
|-----------------------------|----------|
| TSV file for experiment log | JSONL with structured schema |
| Manual metric tracking | Reward-hacking detection |
| `tail` / `sort` for history | `aris log`, `aris best`, `aris diff` |
| No validation | `aris doctor` (14+ pre-flight checks) |
| No visualization | `aris watch` (live TUI dashboard) |

### Key Commands

| Command | Purpose |
|---------|---------|
| `aris init` | Initialize project config |
| `aris doctor` | Pre-flight validation |
| `aris record --metric X --status Y` | Record experiment |
| `aris log` | View history |
| `aris best` | Best result + diff |
| `aris watch` | Live TUI dashboard |
| `aris fork` / `aris merge-best` | Parallel exploration |
| `aris report` | Generate summary |
| `aris export --format csv` | Export for analysis |

All commands support `--json` and `AUTORESEARCH_FORMAT=json` env var.

---

## Adapting to Different Domains

| Domain | Metric | Verify Command | Guard |
|--------|--------|----------------|-------|
| ML training | val_loss (lower) | `python train.py \| grep val_loss` | — |
| Test coverage | coverage % (higher) | `pytest --cov \| grep TOTAL` | `pytest -x` |
| Web performance | p99 latency (lower) | `npm run bench \| grep p99` | `npm test` |
| Build speed | build time (lower) | `time cargo build` | `cargo test` |
| Code quality | lint score (higher) | `pylint src/ \| grep rated` | `pytest -x` |
| Bundle size | bytes (lower) | `esbuild --bundle --minify \| wc -c` | `npm test` |

---

## Project Structure

```
aris-cli/
├── skills/aris/                    ← Skill protocol (the core)
│   ├── SKILL.md                    ← Main skill index + router
│   └── references/                 ← Phase protocols
│       ├── autonomous-loop-protocol.md
│       ├── core-principles.md
│       ├── plan-workflow.md
│       ├── debug-workflow.md
│       ├── fix-workflow.md
│       └── results-logging.md
├── commands/aris/                   ← Slash command dispatchers
│   ├── plan.md
│   ├── debug.md
│   └── fix.md
├── src/                            ← Optional Rust CLI
└── tests/
```

---

## License

MIT — see [LICENSE](LICENSE).

---

## Credits

- [Andrej Karpathy](https://github.com/karpathy) — for the [autoresearch](https://github.com/karpathy/autoresearch) concept
- [uditgoenka/autoresearch](https://github.com/uditgoenka/autoresearch) — for the Claude Code skill framework that inspired the protocol layer

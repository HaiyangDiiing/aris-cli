# ARIS CLI — Developer Guide

## Build & Test

```bash
cargo build                    # Build
cargo test                     # Run all tests
cargo clippy                   # Lint
cargo fmt                      # Format
```

## Architecture

```
src/
├── main.rs              # Entry point
├── cli.rs               # CLI definition (clap derive)
├── errors.rs            # Error types (thiserror)
├── git.rs               # Git operations
├── output/              # Output formatting (JSON / table / TTY detection)
├── protocol/            # Embedded protocol files (include_str!)
├── skill/               # Skill template generation (multi-platform)
└── cmd/                 # Subcommand implementations
    ├── mod.rs           # Command dispatcher
    ├── init.rs          # Project initialization
    ├── record.rs        # Experiment recording (fs2 file locking, reward-hacking detection)
    ├── log.rs           # Experiment history
    ├── best.rs          # Best result lookup
    ├── doctor.rs        # Pre-flight validation
    ├── watch.rs         # TUI dashboard (ratatui)
    ├── export.rs        # CSV/JSON export
    ├── report.rs        # Markdown report generation
    ├── fork.rs          # Parallel branch creation
    ├── merge_best.rs    # Fork comparison
    ├── review.rs        # Cross-model review prompt
    ├── templates.rs     # Domain templates
    └── ...
```

## Key Files

| File | Purpose |
|------|---------|
| `skills/aris/references/autonomous-loop-protocol.md` | Core 8-phase loop protocol |
| `skills/aris/SKILL.md` | Agent skill index and router |
| `src/cmd/record.rs` | Experiment recording with reward-hacking detection |
| `src/output/format.rs` | Output format detection (--json / env / TTY) |

## Config & Data (Backward Compat)

- Config file: `autoresearch.toml` (not renamed)
- Data directory: `.autoresearch/` (not renamed)
- Git branch prefix: `autoresearch-fork-*` (not renamed)
- Git commit prefix: `[aris]`

## Conventions

- All CLI commands support `--json` flag + `AUTORESEARCH_FORMAT=json` env var
- Exit codes: 0 = success, 1 = runtime error, 2 = config/usage error
- Use `thiserror` for error types, `clap` derive for CLI
- Record statuses: baseline, kept, discarded, crash, no-op, hook-blocked, metric-error

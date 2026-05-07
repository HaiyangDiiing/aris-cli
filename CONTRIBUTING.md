# Contributing to ARIS CLI

Thanks for your interest in contributing. Here is how to get started.

## Development Setup

```bash
git clone https://github.com/HaiyangDiiing/aris-cli.git
cd aris-cli
cargo build
cargo test
```

Requires Rust 1.75+.

## How to Contribute

1. **Bug reports** -- open an issue with steps to reproduce, expected vs actual behavior, and your OS/Rust version.
2. **Feature requests** -- open an issue describing the use case and why it matters.
3. **Pull requests** -- fork the repo, create a branch, make your changes, and open a PR against `main`.

## Pull Request Guidelines

- Keep PRs focused on a single change.
- Add or update tests if you are changing behavior.
- Run `cargo clippy` and `cargo fmt` before submitting.
- Write a clear PR description explaining what changed and why.

## Code Style

- Follow standard Rust conventions.
- Use `thiserror` for error types.
- All commands should support `--json` output for agent compatibility.
- Exit codes: 0 = success, 1 = runtime error, 2 = config/usage error.

## Adding a New Agent Target

To add support for a new AI coding agent:

1. Add the agent variant to `src/cmd/install.rs`.
2. Define the skill template path and format in `src/skill/templates.rs`.
3. Update the agent table in `README.md`.
4. Test with `aris install <your-agent>`.

## Questions?

Open an issue or start a discussion. We are happy to help.

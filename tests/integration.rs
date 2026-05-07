use assert_cmd::Command;
use predicates::prelude::*;
use std::process::Command as StdCommand;
use tempfile::TempDir;

/// Create a temporary directory with a git repo and a dummy target file.
fn setup_git_repo() -> TempDir {
    let dir = TempDir::new().unwrap();

    StdCommand::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .expect("git init failed");

    StdCommand::new("git")
        .args(["config", "user.email", "test@autoresearch.dev"])
        .current_dir(dir.path())
        .output()
        .expect("git config email failed");

    StdCommand::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir.path())
        .output()
        .expect("git config name failed");

    // Create dummy target file
    std::fs::write(dir.path().join("train.py"), "print('loss=1.0')\n").unwrap();

    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .expect("git add failed");

    StdCommand::new("git")
        .args(["commit", "-m", "initial commit"])
        .current_dir(dir.path())
        .output()
        .expect("git commit failed");

    dir
}

fn aris() -> Command {
    Command::cargo_bin("aris").unwrap()
}

/// Backward compat alias used by existing tests
fn autoresearch() -> Command {
    aris()
}

// ──────────────────────────────────────────────
// Init
// ──────────────────────────────────────────────

#[test]
fn test_init_creates_config_json() {
    let dir = setup_git_repo();
    autoresearch()
        .args([
            "init",
            "--target-file", "train.py",
            "--eval-command", "echo 0.5",
            "--metric-name", "loss",
            "--metric-direction", "lower",
            "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"success\""));

    assert!(dir.path().join("autoresearch.toml").exists());
    assert!(dir.path().join("program.md").exists());
    assert!(dir.path().join(".autoresearch").exists());
}

#[test]
fn test_init_creates_config_table() {
    let dir = setup_git_repo();
    autoresearch()
        .args([
            "init",
            "--target-file", "train.py",
            "--eval-command", "echo 0.5",
        ])
        .current_dir(dir.path())
        .assert()
        .success();

    let config = std::fs::read_to_string(dir.path().join("autoresearch.toml")).unwrap();
    assert!(config.contains("target_file"));
    assert!(config.contains("eval_command"));
}

// ──────────────────────────────────────────────
// Record
// ──────────────────────────────────────────────

fn init_project(dir: &TempDir) {
    autoresearch()
        .args([
            "init",
            "--target-file", "train.py",
            "--eval-command", "echo 0.5",
            "--metric-name", "loss",
            "--metric-direction", "lower",
            "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn test_record_baseline() {
    let dir = setup_git_repo();
    init_project(&dir);

    autoresearch()
        .args([
            "record",
            "--metric", "1.0",
            "--status", "baseline",
            "--summary", "initial baseline",
            "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"run\": 0"))
        .stdout(predicate::str::contains("\"status\": \"success\""));

    // Verify JSONL was written
    let jsonl = std::fs::read_to_string(dir.path().join(".autoresearch/experiments.jsonl")).unwrap();
    assert!(jsonl.contains("\"baseline\""));
}

#[test]
fn test_record_kept() {
    let dir = setup_git_repo();
    init_project(&dir);

    // Record baseline
    autoresearch()
        .args(["record", "--metric", "1.0", "--status", "baseline", "--summary", "base", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Record kept
    autoresearch()
        .args(["record", "--metric", "0.9", "--status", "kept", "--summary", "improved lr", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"run\": 1"));
}

#[test]
fn test_record_rejects_nan() {
    let dir = setup_git_repo();
    init_project(&dir);

    autoresearch()
        .args(["record", "--metric", "NaN", "--status", "kept", "--summary", "bad", "--json"])
        .current_dir(dir.path())
        .assert()
        .failure();
}

#[test]
fn test_record_rejects_invalid_status() {
    let dir = setup_git_repo();
    init_project(&dir);

    autoresearch()
        .args(["record", "--metric", "1.0", "--status", "invalid", "--summary", "bad", "--json"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains("config_error"));
}

#[test]
fn test_record_dry_run() {
    let dir = setup_git_repo();
    init_project(&dir);

    // Record baseline first (so JSONL exists)
    autoresearch()
        .args(["record", "--metric", "1.0", "--status", "baseline", "--summary", "base", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Dry run should not write
    autoresearch()
        .args([
            "record", "--metric", "0.8", "--status", "kept", "--summary", "dry test",
            "--dry-run", "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"dry_run\""));

    // Verify no second record was written
    let jsonl = std::fs::read_to_string(dir.path().join(".autoresearch/experiments.jsonl")).unwrap();
    let line_count = jsonl.lines().filter(|l| !l.trim().is_empty()).count();
    assert_eq!(line_count, 1, "dry run should not have written a second record");
}

// ──────────────────────────────────────────────
// Log
// ──────────────────────────────────────────────

#[test]
fn test_log_json() {
    let dir = setup_git_repo();
    init_project(&dir);

    autoresearch()
        .args(["record", "--metric", "1.0", "--status", "baseline", "--summary", "base", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    autoresearch()
        .args(["record", "--metric", "0.9", "--status", "kept", "--summary", "improvement", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    autoresearch()
        .args(["log", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 2"));
}

// ──────────────────────────────────────────────
// Best
// ──────────────────────────────────────────────

#[test]
fn test_best_json() {
    let dir = setup_git_repo();
    init_project(&dir);

    autoresearch()
        .args(["record", "--metric", "1.0", "--status", "baseline", "--summary", "base", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    autoresearch()
        .args(["record", "--metric", "0.8", "--status", "kept", "--summary", "best so far", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    autoresearch()
        .args(["best", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"success\""));
}

// ──────────────────────────────────────────────
// Export
// ──────────────────────────────────────────────

#[test]
fn test_export_csv() {
    let dir = setup_git_repo();
    init_project(&dir);

    autoresearch()
        .args(["record", "--metric", "1.0", "--status", "baseline", "--summary", "base", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    let output_path = dir.path().join("export.csv");
    autoresearch()
        .args(["export", "--format", "csv", "--output", output_path.to_str().unwrap()])
        .current_dir(dir.path())
        .assert()
        .success();

    let csv = std::fs::read_to_string(&output_path).unwrap();
    assert!(csv.contains("run,"));
    assert!(csv.contains("baseline"));
}

#[test]
fn test_export_json() {
    let dir = setup_git_repo();
    init_project(&dir);

    autoresearch()
        .args(["record", "--metric", "1.0", "--status", "baseline", "--summary", "base", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    let output_path = dir.path().join("export.json");
    autoresearch()
        .args(["export", "--format", "json", "--output", output_path.to_str().unwrap()])
        .current_dir(dir.path())
        .assert()
        .success();

    let json_str = std::fs::read_to_string(&output_path).unwrap();
    let _: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
}

// ──────────────────────────────────────────────
// Status
// ──────────────────────────────────────────────

#[test]
fn test_status_not_initialized() {
    let dir = setup_git_repo();
    autoresearch()
        .args(["status", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"initialized\": false"));
}

#[test]
fn test_status_initialized() {
    let dir = setup_git_repo();
    init_project(&dir);

    autoresearch()
        .args(["status", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"initialized\": true"))
        .stdout(predicate::str::contains("\"next_action\""));
}

// ──────────────────────────────────────────────
// Doctor
// ──────────────────────────────────────────────

#[test]
fn test_doctor_json() {
    let dir = setup_git_repo();
    init_project(&dir);

    autoresearch()
        .args(["doctor", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"all_passed\""));
}

// ──────────────────────────────────────────────
// Completions
// ──────────────────────────────────────────────

#[test]
fn test_completions_bash() {
    autoresearch()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("aris"));
}

#[test]
fn test_completions_zsh() {
    autoresearch()
        .args(["completions", "zsh"])
        .assert()
        .success();
}

#[test]
fn test_completions_fish() {
    autoresearch()
        .args(["completions", "fish"])
        .assert()
        .success();
}

#[test]
fn test_completions_powershell() {
    autoresearch()
        .args(["completions", "powershell"])
        .assert()
        .success();
}

// ──────────────────────────────────────────────
// Environment variable format
// ──────────────────────────────────────────────

#[test]
fn test_env_var_json_format() {
    let dir = setup_git_repo();
    init_project(&dir);

    autoresearch()
        .args(["status"])
        .env("AUTORESEARCH_FORMAT", "json")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"success\""));
}

// ──────────────────────────────────────────────
// Fork (dry-run)
// ──────────────────────────────────────────────

#[test]
fn test_fork_dry_run() {
    let dir = setup_git_repo();
    init_project(&dir);

    autoresearch()
        .args(["fork", "approach-a", "approach-b", "--dry-run", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"dry_run\""))
        .stdout(predicate::str::contains("would_create"));

    // Verify branches were NOT created
    let output = StdCommand::new("git")
        .args(["branch", "--list", "autoresearch-fork-*"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let branches = String::from_utf8_lossy(&output.stdout);
    assert!(
        branches.trim().is_empty(),
        "dry-run should not have created branches"
    );
}

// ──────────────────────────────────────────────
// Full workflow: init → record → log → best
// ──────────────────────────────────────────────

#[test]
fn test_full_workflow() {
    let dir = setup_git_repo();
    init_project(&dir);

    // Baseline
    autoresearch()
        .args(["record", "--metric", "1.0", "--status", "baseline", "--summary", "initial", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Several experiments
    for (i, (metric, status)) in [(0.95, "kept"), (1.1, "discarded"), (0.9, "kept")].iter().enumerate() {
        autoresearch()
            .args([
                "record",
                "--metric", &metric.to_string(),
                "--status", status,
                "--summary", &format!("experiment {}", i + 1),
                "--json",
            ])
            .current_dir(dir.path())
            .assert()
            .success();
    }

    // Log should show 4 experiments
    autoresearch()
        .args(["log", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 4"));

    // Best should find the 0.9 experiment
    autoresearch()
        .args(["best", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("0.9"));

    // Export CSV
    let csv_path = dir.path().join("results.csv");
    autoresearch()
        .args(["export", "--format", "csv", "--output", csv_path.to_str().unwrap()])
        .current_dir(dir.path())
        .assert()
        .success();

    let csv = std::fs::read_to_string(&csv_path).unwrap();
    let csv_lines: Vec<_> = csv.lines().collect();
    assert_eq!(csv_lines.len(), 5, "header + 4 records"); // header + 4 data rows
}

// ──────────────────────────────────────────────
// Extended record statuses (crash, no-op, hook-blocked, metric-error)
// ──────────────────────────────────────────────

#[test]
fn test_record_crash_no_metric() {
    let dir = setup_git_repo();
    init_project(&dir);

    autoresearch()
        .args([
            "record",
            "--status", "crash",
            "--summary", "eval crashed: DB connection failed",
            "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"success\""))
        .stdout(predicate::str::contains("\"metric\": null"));
}

#[test]
fn test_record_noop_no_metric() {
    let dir = setup_git_repo();
    init_project(&dir);

    autoresearch()
        .args([
            "record",
            "--status", "no-op",
            "--summary", "no code changes produced",
            "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"success\""));
}

#[test]
fn test_record_hook_blocked_no_metric() {
    let dir = setup_git_repo();
    init_project(&dir);

    autoresearch()
        .args([
            "record",
            "--status", "hook-blocked",
            "--summary", "pre-commit lint rejected formatting",
            "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn test_record_metric_error_no_metric() {
    let dir = setup_git_repo();
    init_project(&dir);

    autoresearch()
        .args([
            "record",
            "--status", "metric-error",
            "--summary", "eval output was PASS not a number",
            "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn test_record_kept_requires_metric() {
    let dir = setup_git_repo();
    init_project(&dir);

    // "kept" status without --metric should fail
    autoresearch()
        .args([
            "record",
            "--status", "kept",
            "--summary", "should fail",
            "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .failure();
}

#[test]
fn test_record_multi_metric_samples() {
    let dir = setup_git_repo();
    init_project(&dir);

    autoresearch()
        .args([
            "record",
            "--metric", "87.1",
            "--metric", "86.9",
            "--metric", "87.3",
            "--status", "kept",
            "--summary", "multi-sample test",
            "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"samples\""))
        .stdout(predicate::str::contains("\"mad\""))
        .stdout(predicate::str::contains("\"confidence\""));
}

// ──────────────────────────────────────────────
// Extended statuses in log output
// ──────────────────────────────────────────────

#[test]
fn test_log_shows_extended_statuses() {
    let dir = setup_git_repo();
    init_project(&dir);

    // Record baseline
    autoresearch()
        .args(["record", "--metric", "1.0", "--status", "baseline", "--summary", "base", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Record various extended statuses
    for (status, summary) in [
        ("kept", "improved"),
        ("discarded", "no improvement"),
        ("crash", "eval crashed"),
        ("no-op", "no changes"),
        ("hook-blocked", "lint failed"),
    ] {
        let mut args = vec![
            "record".to_string(),
            "--status".to_string(), status.to_string(),
            "--summary".to_string(), summary.to_string(),
            "--json".to_string(),
        ];
        // Add metric only for non-error statuses
        if status == "kept" || status == "discarded" {
            args.insert(1, "--metric".to_string());
            args.insert(2, "0.9".to_string());
        }
        autoresearch()
            .args(args.iter().map(|s| s.as_str()).collect::<Vec<_>>())
            .current_dir(dir.path())
            .assert()
            .success();
    }

    // Log should show all experiments
    autoresearch()
        .args(["log", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 6"));
}

// ──────────────────────────────────────────────
// Install with --force
// ──────────────────────────────────────────────

#[test]
fn test_install_creates_files() {
    let dir = TempDir::new().unwrap();

    // Set HOME to temp dir so install writes there
    autoresearch()
        .args(["install", "cursor", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"success\""))
        .stdout(predicate::str::contains("files_count"));

    // Verify SKILL.md was created
    assert!(dir.path().join(".cursor/skills/aris/SKILL.md").exists());
    // Verify references were created
    assert!(dir.path().join(".cursor/skills/aris/references/autonomous-loop-protocol.md").exists());
    // Verify command dispatchers were created
    assert!(dir.path().join(".cursor/commands/aris.md").exists());
    assert!(dir.path().join(".cursor/commands/aris/plan.md").exists());
}

#[test]
fn test_install_force_reinstalls() {
    let dir = TempDir::new().unwrap();

    // First install
    autoresearch()
        .args(["install", "cursor", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Second install without --force should report already installed
    autoresearch()
        .args(["install", "cursor", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("already_installed"));

    // Install with --force should succeed
    autoresearch()
        .args(["install", "cursor", "--force", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"success\""));
}

// ──────────────────────────────────────────────
// Full workflow with extended statuses
// ──────────────────────────────────────────────

#[test]
fn test_full_workflow_extended() {
    let dir = setup_git_repo();
    init_project(&dir);

    // Baseline
    autoresearch()
        .args(["record", "--metric", "1.0", "--status", "baseline", "--summary", "initial", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Kept experiment
    autoresearch()
        .args(["record", "--metric", "0.9", "--status", "kept", "--summary", "improved lr", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Discarded experiment
    autoresearch()
        .args(["record", "--metric", "1.1", "--status", "discarded", "--summary", "worse", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Crash (no metric)
    autoresearch()
        .args(["record", "--status", "crash", "--summary", "eval OOM", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // No-op (no metric)
    autoresearch()
        .args(["record", "--status", "no-op", "--summary", "no diff", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Hook-blocked (no metric)
    autoresearch()
        .args(["record", "--status", "hook-blocked", "--summary", "lint fail", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Verify all 6 records in log
    autoresearch()
        .args(["log", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 6"));

    // Best should find the 0.9 experiment
    autoresearch()
        .args(["best", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("0.9"));

    // Export should include extended statuses
    let csv_path = dir.path().join("results.csv");
    autoresearch()
        .args(["export", "--format", "csv", "--output", csv_path.to_str().unwrap()])
        .current_dir(dir.path())
        .assert()
        .success();

    let csv = std::fs::read_to_string(&csv_path).unwrap();
    assert!(csv.contains("crash") || csv.contains("no-op") || csv.contains("hook-blocked"),
        "CSV should include extended statuses");
}

// ──────────────────────────────────────────────
// Guard checks
// ──────────────────────────────────────────────

#[test]
fn test_guard_pass_keeps_status() {
    let dir = setup_git_repo();

    // Init with guard_command that always passes
    autoresearch()
        .args([
            "init", "--target-file", "train.py",
            "--eval-command", "echo 1.0",
        ])
        .current_dir(dir.path())
        .assert()
        .success();

    // Add guard_command to config (cross-platform: "true" on Unix, "echo ok" everywhere)
    let config = std::fs::read_to_string(dir.path().join("autoresearch.toml")).unwrap();
    let config = format!("{config}guard_command = \"echo ok\"\n");
    std::fs::write(dir.path().join("autoresearch.toml"), config).unwrap();

    // Record baseline
    autoresearch()
        .args([
            "record", "--metric", "1.0", "--status", "baseline",
            "--summary", "baseline with guard", "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"guard_result\": \"pass\""))
        .stdout(predicate::str::contains("\"status\": \"baseline\""));
}

#[test]
fn test_guard_fail_forces_discard() {
    let dir = setup_git_repo();

    autoresearch()
        .args([
            "init", "--target-file", "train.py",
            "--eval-command", "echo 1.0",
        ])
        .current_dir(dir.path())
        .assert()
        .success();

    // Add guard_command that always fails
    let config = std::fs::read_to_string(dir.path().join("autoresearch.toml")).unwrap();
    #[cfg(unix)]
    let config = format!("{config}guard_command = \"false\"\n");
    #[cfg(windows)]
    let config = format!("{config}guard_command = \"exit /b 1\"\n");
    std::fs::write(dir.path().join("autoresearch.toml"), config).unwrap();

    // Try to record as baseline — should be forced to discarded
    autoresearch()
        .args([
            "record", "--metric", "1.0", "--status", "baseline",
            "--summary", "will fail guard", "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"guard_result\": \"fail\""))
        .stdout(predicate::str::contains("\"status\": \"discarded\""))
        .stdout(predicate::str::contains("[GUARD FAILED]"));
}

#[test]
fn test_guard_fail_with_kept_allows_rework() {
    let dir = setup_git_repo();

    autoresearch()
        .args([
            "init", "--target-file", "train.py",
            "--eval-command", "echo 1.0",
        ])
        .current_dir(dir.path())
        .assert()
        .success();

    // Add failing guard
    let config = std::fs::read_to_string(dir.path().join("autoresearch.toml")).unwrap();
    #[cfg(unix)]
    let config = format!("{config}guard_command = \"false\"\n");
    #[cfg(windows)]
    let config = format!("{config}guard_command = \"exit /b 1\"\n");
    std::fs::write(dir.path().join("autoresearch.toml"), config).unwrap();

    // Record as kept — should still be kept (for rework flow)
    autoresearch()
        .args([
            "record", "--metric", "0.9", "--status", "kept",
            "--summary", "improved but guard failed", "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"guard_result\": \"fail\""))
        .stdout(predicate::str::contains("\"status\": \"kept\""));
}

#[test]
fn test_guard_status_flag() {
    let dir = setup_git_repo();

    autoresearch()
        .args([
            "init", "--target-file", "train.py",
            "--eval-command", "echo 1.0",
        ])
        .current_dir(dir.path())
        .assert()
        .success();

    // Record with explicit --guard-status (no guard_command in config)
    autoresearch()
        .args([
            "record", "--metric", "1.0", "--status", "baseline",
            "--summary", "explicit guard", "--guard-status", "pass", "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"guard_result\": \"pass\""));
}

#[test]
fn test_guard_dry_run() {
    let dir = setup_git_repo();

    autoresearch()
        .args([
            "init", "--target-file", "train.py",
            "--eval-command", "echo 1.0",
        ])
        .current_dir(dir.path())
        .assert()
        .success();

    // Add passing guard
    let config = std::fs::read_to_string(dir.path().join("autoresearch.toml")).unwrap();
    let config = format!("{config}guard_command = \"echo ok\"\n");
    std::fs::write(dir.path().join("autoresearch.toml"), config).unwrap();

    // Dry run — guard runs but no data written
    autoresearch()
        .args([
            "record", "--metric", "1.0", "--status", "baseline",
            "--summary", "dry run guard", "--dry-run", "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"dry_run\""))
        .stdout(predicate::str::contains("\"guard_result\": \"pass\""));

    // Verify no experiments.jsonl written
    assert!(!dir.path().join(".autoresearch/experiments.jsonl").exists()
        || std::fs::read_to_string(dir.path().join(".autoresearch/experiments.jsonl"))
            .unwrap_or_default()
            .trim()
            .is_empty());
}

// ──────────────────────────────────────────────
// Init improvements: .gitignore
// ──────────────────────────────────────────────

#[test]
fn test_init_creates_gitignore() {
    let dir = setup_git_repo();

    autoresearch()
        .args([
            "init", "--target-file", "train.py",
            "--eval-command", "echo 1.0",
        ])
        .current_dir(dir.path())
        .assert()
        .success();

    // .gitignore should exist and contain .autoresearch/
    let gitignore = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(
        gitignore.contains(".autoresearch/"),
        "gitignore should contain .autoresearch/"
    );
}

#[test]
fn test_init_appends_to_existing_gitignore() {
    let dir = setup_git_repo();

    // Create existing .gitignore
    std::fs::write(dir.path().join(".gitignore"), "*.pyc\n__pycache__/\n").unwrap();

    autoresearch()
        .args([
            "init", "--target-file", "train.py",
            "--eval-command", "echo 1.0",
        ])
        .current_dir(dir.path())
        .assert()
        .success();

    let gitignore = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(gitignore.contains("*.pyc"), "should preserve existing entries");
    assert!(gitignore.contains(".autoresearch/"), "should add .autoresearch/");
}

#[test]
fn test_init_no_duplicate_gitignore() {
    let dir = setup_git_repo();

    // Create .gitignore that already has .autoresearch/
    std::fs::write(dir.path().join(".gitignore"), ".autoresearch/\n").unwrap();

    autoresearch()
        .args([
            "init", "--target-file", "train.py",
            "--eval-command", "echo 1.0",
        ])
        .current_dir(dir.path())
        .assert()
        .success();

    let gitignore = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    let count = gitignore.matches(".autoresearch/").count();
    assert_eq!(count, 1, "should not duplicate .autoresearch/ entry");
}

#[test]
fn test_init_eval_preview_in_json() {
    let dir = setup_git_repo();

    autoresearch()
        .args([
            "init", "--target-file", "train.py",
            "--eval-command", "echo 42.0", "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("eval_preview"));
}

// ──────────────────────────────────────────────
// Cross-Session Learning (aris learn)
// ──────────────────────────────────────────────

#[test]
fn test_learn_with_experiments() {
    let dir = setup_git_repo();

    autoresearch()
        .args(["init", "--target-file", "train.py", "--eval-command", "echo 1.0"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Record a mix of experiments
    autoresearch()
        .args(["record", "--metric", "1.0", "--status", "baseline", "--summary", "baseline run"])
        .current_dir(dir.path())
        .assert()
        .success();

    autoresearch()
        .args(["record", "--metric", "0.9", "--status", "kept", "--summary", "tuned learning rate"])
        .current_dir(dir.path())
        .assert()
        .success();

    autoresearch()
        .args(["record", "--metric", "1.1", "--status", "discarded", "--summary", "increased batch size failed"])
        .current_dir(dir.path())
        .assert()
        .success();

    autoresearch()
        .args(["record", "--metric", "0.85", "--status", "kept", "--summary", "tuned learning rate schedule"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Run learn
    autoresearch()
        .args(["learn", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"kept\": 2"))
        .stdout(predicate::str::contains("\"discarded\": 1"))
        .stdout(predicate::str::contains("lessons_path"));

    // Verify lessons.md exists
    assert!(dir.path().join(".autoresearch/lessons.md").exists());
    let lessons = std::fs::read_to_string(dir.path().join(".autoresearch/lessons.md")).unwrap();
    assert!(lessons.contains("Effective Strategies"));
    assert!(lessons.contains("Dead Ends"));
    assert!(lessons.contains("Recommended Next Actions"));
}

#[test]
fn test_learn_no_experiments() {
    let dir = setup_git_repo();

    autoresearch()
        .args(["init", "--target-file", "train.py", "--eval-command", "echo 1.0"])
        .current_dir(dir.path())
        .assert()
        .success();

    // learn with no experiments should fail
    autoresearch()
        .args(["learn", "--json"])
        .current_dir(dir.path())
        .assert()
        .failure();
}

// ──────────────────────────────────────────────
// Graduated Stuck Recovery (strategy)
// ──────────────────────────────────────────────

#[test]
fn test_strategy_escalation_on_consecutive_discards() {
    let dir = setup_git_repo();

    autoresearch()
        .args(["init", "--target-file", "train.py", "--eval-command", "echo 1.0"])
        .current_dir(dir.path())
        .assert()
        .success();

    autoresearch()
        .args(["record", "--metric", "1.0", "--status", "baseline", "--summary", "baseline"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Record 5 consecutive discards to trigger level 2
    for i in 0..5 {
        autoresearch()
            .args(["record", "--metric", "1.5", "--status", "discarded", "--summary", &format!("attempt {i}")])
            .current_dir(dir.path())
            .assert()
            .success();
    }

    // 6th discard should show escalation in JSON (level >= 2)
    autoresearch()
        .args(["record", "--metric", "1.6", "--status", "discarded", "--summary", "still failing", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"escalation\""))
        .stdout(predicate::str::contains("\"level\""));

    // Verify strategy.json exists
    assert!(dir.path().join(".autoresearch/strategy.json").exists());
}

#[test]
fn test_strategy_resets_on_kept() {
    let dir = setup_git_repo();

    autoresearch()
        .args(["init", "--target-file", "train.py", "--eval-command", "echo 1.0"])
        .current_dir(dir.path())
        .assert()
        .success();

    autoresearch()
        .args(["record", "--metric", "1.0", "--status", "baseline", "--summary", "baseline"])
        .current_dir(dir.path())
        .assert()
        .success();

    // 3 discards
    for _ in 0..3 {
        autoresearch()
            .args(["record", "--metric", "1.5", "--status", "discarded", "--summary", "fail"])
            .current_dir(dir.path())
            .assert()
            .success();
    }

    // Then a kept — should reset
    autoresearch()
        .args(["record", "--metric", "0.9", "--status", "kept", "--summary", "success", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Next discard should NOT show escalation (level < 2, only 1 discard)
    autoresearch()
        .args(["record", "--metric", "1.5", "--status", "discarded", "--summary", "new fail", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"escalation\"").not());
}

// ──────────────────────────────────────────────
// Experiment Verification (aris verify)
// ──────────────────────────────────────────────

#[test]
fn test_verify_nonexistent_run() {
    let dir = setup_git_repo();

    autoresearch()
        .args(["init", "--target-file", "train.py", "--eval-command", "echo 1.0"])
        .current_dir(dir.path())
        .assert()
        .success();

    autoresearch()
        .args(["record", "--metric", "1.0", "--status", "baseline", "--summary", "baseline"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Verify non-existent run
    autoresearch()
        .args(["verify", "--run", "999"])
        .current_dir(dir.path())
        .assert()
        .failure();
}

// ──────────────────────────────────────────────
// Fork --parallel (worktree)
// ──────────────────────────────────────────────

#[test]
fn test_fork_parallel_dry_run() {
    let dir = setup_git_repo();

    autoresearch()
        .args(["init", "--target-file", "train.py", "--eval-command", "echo 1.0"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Dry run with --parallel
    autoresearch()
        .args(["fork", "alpha", "beta", "--parallel", "--dry-run", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"parallel\": true"))
        .stdout(predicate::str::contains("worktree"))
        .stdout(predicate::str::contains("would_create"));
}

// ──────────────────────────────────────────────
// Auto PR (aris pr) — just check gh dependency error
// ──────────────────────────────────────────────

// Note: Full PR tests require gh CLI + auth, so we test the error path

// ──────────────────────────────────────────────
// Domain Templates
// ──────────────────────────────────────────────


#[test]
fn test_init_invalid_template() {
    let dir = setup_git_repo();

    // Error is output as JSON to stdout when piped (non-TTY)
    autoresearch()
        .args([
            "init", "--target-file", "train.py",
            "--template", "nonexistent",
        ])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains("Unknown template"));
}

#[test]
fn test_init_template_with_override() {
    let dir = setup_git_repo();

    // Use ml-training template but override metric direction
    autoresearch()
        .args([
            "init", "--target-file", "model.py",
            "--template", "ml-training",
            "--metric-direction", "higher",
            "--json",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"metric_direction\": \"higher\""));
}

// ──────────────────────────────────────────────
// Retrospective
// ──────────────────────────────────────────────

#[test]
fn test_retrospective_with_experiments() {
    let dir = setup_git_repo();

    autoresearch()
        .args(["init", "--target-file", "train.py", "--eval-command", "echo 1.0"])
        .current_dir(dir.path())
        .assert()
        .success();

    autoresearch()
        .args(["record", "--metric", "1.0", "--status", "baseline", "--summary", "baseline"])
        .current_dir(dir.path())
        .assert()
        .success();

    autoresearch()
        .args(["record", "--metric", "0.9", "--status", "kept", "--summary", "improved"])
        .current_dir(dir.path())
        .assert()
        .success();

    autoresearch()
        .args(["retrospective", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 2"))
        .stdout(predicate::str::contains("Key Decisions"))
        .stdout(predicate::str::contains("Metrics Analysis"));
}

// ──────────────────────────────────────────────
// Agent info includes new commands
// ──────────────────────────────────────────────

#[test]
fn test_agent_info_includes_new_commands() {
    autoresearch()
        .args(["agent-info", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("learn"))
        .stdout(predicate::str::contains("verify"))
        .stdout(predicate::str::contains("retrospective"))
        .stdout(predicate::str::contains("pool"))
        .stdout(predicate::str::contains("--guard-status"))
        .stdout(predicate::str::contains("--template"));
}

// ──────────────────────────────────────────────
// End-to-end smoke test
// ──────────────────────────────────────────────

#[test]
fn test_e2e_smoke_test() {
    let dir = setup_git_repo();

    // init
    autoresearch()
        .args(["init", "--target-file", "train.py", "--eval-command", "echo 42.0", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // doctor
    autoresearch()
        .args(["doctor", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // baseline
    autoresearch()
        .args(["record", "--metric", "1.0", "--status", "baseline", "--summary", "Initial baseline", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // kept experiment
    autoresearch()
        .args(["record", "--metric", "0.9", "--status", "kept", "--summary", "Reduced learning rate", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // discarded experiment
    autoresearch()
        .args(["record", "--metric", "1.1", "--status", "discarded", "--summary", "Bad change", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // crash experiment
    autoresearch()
        .args(["record", "--status", "crash", "--summary", "OOM error", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // log
    autoresearch()
        .args(["log", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 4"));

    // best
    autoresearch()
        .args(["best", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("0.9"));

    // report
    autoresearch()
        .args(["report", "--json"])
        .current_dir(dir.path())
        .assert()
        .success();

    // learn
    autoresearch()
        .args(["learn", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("lessons_path"));

    // retrospective
    autoresearch()
        .args(["retrospective", "--json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Key Decisions"));
}

// ──────────────────────────────────────────────
// Guide command returns protocol content
// ──────────────────────────────────────────────

#[test]
fn test_guide_returns_protocol() {
    autoresearch()
        .args(["guide", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Phase 0"))
        .stdout(predicate::str::contains("Phase 1"));
}

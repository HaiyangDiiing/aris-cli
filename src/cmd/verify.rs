use crate::errors::CliError;
use crate::output::format::OutputFormat;
use std::fs;
use std::process::Command;

const LOG_PATH: &str = ".autoresearch/experiments.jsonl";
const DRIFT_THRESHOLD: f64 = 5.0; // 5% drift threshold

pub fn run(run_number: usize, json: bool) -> Result<(), CliError> {
    let format = OutputFormat::detect(json);

    // Load experiment
    let content = fs::read_to_string(LOG_PATH)
        .map_err(|_| CliError::NoExperiments("No experiments".into()))?;

    let experiment = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .find(|v| v.get("run").and_then(|r| r.as_u64()) == Some(run_number as u64));

    let experiment = experiment.ok_or(CliError::RunNotFound(run_number))?;

    let recorded_metric = experiment
        .get("metric")
        .and_then(|m| m.as_f64())
        .ok_or_else(|| CliError::Config(format!(
            "Run #{run_number} has no metric value (status: {})",
            experiment.get("status").and_then(|s| s.as_str()).unwrap_or("unknown")
        )))?;

    let commit_hash = experiment
        .get("hash")
        .and_then(|h| h.as_str())
        .ok_or_else(|| CliError::Config(format!("Run #{run_number} has no git hash")))?;

    // Load eval_command from config
    let config_content = fs::read_to_string("autoresearch.toml")
        .map_err(|_| CliError::Config("No autoresearch.toml".into()))?;
    let config: toml::Table = toml::from_str(&config_content)
        .map_err(|e| CliError::Config(e.to_string()))?;
    let eval_command = config
        .get("eval_command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CliError::Config("No eval_command in autoresearch.toml".into()))?;

    // Save current branch/commit
    let original_ref = get_current_ref()?;

    // Check for dirty working tree and stash if needed
    let needs_stash = is_dirty()?;
    if needs_stash {
        let stash_result = Command::new("git")
            .args(["stash", "push", "-m", "aris-verify-autostash"])
            .output()
            .map_err(|e| CliError::Git(e.to_string()))?;

        if !stash_result.status.success() {
            return Err(CliError::Config(
                "Cannot verify: working tree has changes that cannot be stashed. Commit or clean first.".into()
            ));
        }
    }

    // Checkout experiment commit
    let checkout_result = Command::new("git")
        .args(["checkout", commit_hash])
        .output()
        .map_err(|e| CliError::Git(e.to_string()))?;

    if !checkout_result.status.success() {
        // Restore stash if we stashed
        if needs_stash {
            let _ = Command::new("git").args(["stash", "pop"]).output();
        }
        return Err(CliError::Git(format!(
            "Failed to checkout commit {commit_hash}: {}",
            String::from_utf8_lossy(&checkout_result.stderr)
        )));
    }

    // Run eval command
    let actual_metric = run_eval(eval_command);

    // Restore original branch/commit
    let _ = Command::new("git")
        .args(["checkout", &original_ref])
        .output();

    // Restore stash if we stashed
    if needs_stash {
        let _ = Command::new("git").args(["stash", "pop"]).output();
    }

    // Compare metrics
    let actual_metric = actual_metric
        .map_err(|e| CliError::Config(format!("Eval failed during verification: {e}")))?;

    let drift_pct = if recorded_metric.abs() > f64::EPSILON {
        ((actual_metric - recorded_metric) / recorded_metric.abs()) * 100.0
    } else if actual_metric.abs() > f64::EPSILON {
        100.0 // went from 0 to non-zero
    } else {
        0.0 // both zero
    };

    let status = if drift_pct.abs() <= DRIFT_THRESHOLD {
        "pass"
    } else {
        "drift"
    };

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "status": status,
                "run": run_number,
                "recorded_metric": recorded_metric,
                "actual_metric": actual_metric,
                "drift_pct": drift_pct,
            })).unwrap());
        }
        OutputFormat::Table => {
            if status == "pass" {
                println!(
                    "PASS: metric is reproducible (recorded: {recorded_metric:.6}, actual: {actual_metric:.6}, drift: {drift_pct:+.1}%)"
                );
            } else {
                println!(
                    "DRIFT: metric has changed significantly (recorded: {recorded_metric:.6}, actual: {actual_metric:.6}, drift: {drift_pct:+.1}%)"
                );
            }
        }
    }

    Ok(())
}

/// Get current branch name or HEAD commit.
fn get_current_ref() -> Result<String, CliError> {
    // Try symbolic ref first (branch name)
    let output = Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .output()
        .map_err(|e| CliError::Git(e.to_string()))?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    // Fall back to HEAD commit hash (detached HEAD)
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| CliError::Git(e.to_string()))?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Check if working tree is dirty.
fn is_dirty() -> Result<bool, CliError> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .map_err(|e| CliError::Git(e.to_string()))?;

    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

/// Run eval command and parse metric from output.
fn run_eval(eval_command: &str) -> Result<f64, String> {
    #[cfg(unix)]
    let output = Command::new("sh")
        .args(["-c", eval_command])
        .output();

    #[cfg(windows)]
    let output = Command::new("cmd")
        .args(["/C", eval_command])
        .output();

    #[cfg(not(any(unix, windows)))]
    let output = Command::new("sh")
        .args(["-c", eval_command])
        .output();

    let output = output.map_err(|e| format!("Failed to run eval: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Eval command failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Try to parse the last line as a number
    stdout
        .lines()
        .rev()
        .find_map(|line| {
            let trimmed = line.trim();
            // Try direct parse
            if let Ok(v) = trimmed.parse::<f64>() {
                return Some(v);
            }
            // Try extracting last number from the line
            trimmed
                .split_whitespace()
                .rev()
                .find_map(|word| {
                    word.trim_matches(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
                        .parse::<f64>()
                        .ok()
                })
        })
        .ok_or_else(|| format!("Could not extract numeric metric from eval output: {}", stdout.trim()))
}

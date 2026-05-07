use crate::errors::CliError;
use crate::output::format::OutputFormat;
use std::fs::{self, OpenOptions};
use std::io::Write;

/// Valid statuses for experiment recording.
const VALID_STATUSES: &[&str] = &[
    "baseline",
    "kept",
    "keep",
    "discarded",
    "discard",
    "crash",
    "no-op",
    "hook-blocked",
    "metric-error",
];

/// Statuses that don't require a metric value.
const ERROR_STATUSES: &[&str] = &["crash", "no-op", "hook-blocked", "metric-error"];

/// Valid values for --guard-status flag.
const VALID_GUARD_STATUSES: &[&str] = &["pass", "fail", "skipped"];

pub fn run(
    metrics: Vec<f64>,
    status: &str,
    summary: &str,
    guard_status_flag: Option<&str>,
    json: bool,
    dry_run: bool,
) -> Result<(), CliError> {
    let format = OutputFormat::detect(json);

    // Validate status
    if !VALID_STATUSES.contains(&status) {
        return Err(CliError::Config(format!(
            "Invalid status '{status}'. Must be one of: {}",
            VALID_STATUSES.join(", ")
        )));
    }

    // Validate guard-status flag if provided
    if let Some(gs) = guard_status_flag {
        if !VALID_GUARD_STATUSES.contains(&gs) {
            return Err(CliError::Config(format!(
                "Invalid --guard-status '{gs}'. Must be one of: {}",
                VALID_GUARD_STATUSES.join(", ")
            )));
        }
    }

    // Normalize status
    let normalized_status = match status {
        "keep" => "kept",
        "discard" => "discarded",
        _ => status,
    };

    let is_error_status = ERROR_STATUSES.contains(&normalized_status);

    // Validate metric presence
    if metrics.is_empty() && !is_error_status {
        return Err(CliError::Config(format!(
            "Status '{normalized_status}' requires --metric. Only error statuses ({}) can omit it.",
            ERROR_STATUSES.join(", ")
        )));
    }

    // Validate all metric values
    for m in &metrics {
        if m.is_nan() || m.is_infinite() {
            return Err(CliError::Config(format!(
                "Invalid metric value '{}'. Must be a finite number, not NaN or Infinity.",
                m
            )));
        }
    }

    // Compute stats from metric samples
    let (canonical_metric, samples, mad, confidence) = if metrics.is_empty() {
        (None, None, None, None)
    } else if metrics.len() == 1 {
        (Some(metrics[0]), None, None, None)
    } else {
        let stats = compute_stats(&metrics);
        (
            Some(stats.0),
            Some(metrics.clone()),
            Some(stats.1),
            Some(stats.2),
        )
    };

    // --- Guard check execution ---
    let config = load_config().ok();
    let guard_command = config
        .as_ref()
        .and_then(|c| c.get("guard_command"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let (guard_result, guard_stderr) = if let Some(ref cmd) = guard_command {
        // If --guard-status was explicitly provided, use that instead of running
        if let Some(gs) = guard_status_flag {
            (Some(gs.to_string()), None)
        } else {
            run_guard_command(cmd)
        }
    } else if let Some(gs) = guard_status_flag {
        // No guard_command configured, but --guard-status provided explicitly
        (Some(gs.to_string()), None)
    } else {
        (None, None)
    };

    let guard_failed = guard_result.as_deref() == Some("fail");

    // Override status and summary if guard failed
    let (final_status, final_summary) = if guard_failed && normalized_status != "discarded" {
        if normalized_status == "kept" {
            // 12d.2: Guard failure with kept status → log warning but allow it
            // (Agent handles rework logic in the skill template)
            eprintln!(
                "warning: Guard check failed but recording as 'kept'. The Agent should handle rework in the skill template."
            );
            (normalized_status, summary.to_string())
        } else {
            // For other non-discarded statuses, force to discarded
            (
                "discarded",
                format!("[GUARD FAILED] {}", summary),
            )
        }
    } else {
        (normalized_status, summary.to_string())
    };

    // Validate kept status against metric direction (anti-reward-hacking)
    if final_status == "kept" {
        if let Some(metric) = canonical_metric {
            if let Some(ref config) = config {
                let direction = config
                    .get("metric_direction")
                    .and_then(|v| v.as_str())
                    .unwrap_or("lower");
                let lower_is_better = direction != "higher";

                let log_path_check = std::path::Path::new(".autoresearch/experiments.jsonl");
                if log_path_check.exists() {
                    if let Ok(content) = fs::read_to_string(log_path_check) {
                        let prev_best = content
                            .lines()
                            .rev()
                            .filter(|l| !l.trim().is_empty())
                            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                            .filter(|v| {
                                let s = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
                                s == "kept" || s == "baseline"
                            })
                            .find_map(|v| v.get("metric").and_then(|m| m.as_f64()));

                        if let Some(prev) = prev_best {
                            let regressed = if lower_is_better {
                                metric > prev
                            } else {
                                metric < prev
                            };
                            if regressed {
                                eprintln!(
                                    "warning: metric {:.6} is worse than previous best {:.6} ({direction} is better). Recording as 'kept' anyway — consider 'discarded'.",
                                    metric, prev
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    let log_path = ".autoresearch/experiments.jsonl";
    fs::create_dir_all(".autoresearch")?;

    // Read existing content (with locking if we intend to write)
    let content = if dry_run {
        fs::read_to_string(log_path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut file = if !dry_run {
        let f = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(log_path)?;

        use fs2::FileExt;
        f.lock_exclusive().map_err(CliError::Io)?;
        Some(f)
    } else {
        None
    };

    // Read existing content under lock to determine run number.
    // On Windows, we must read through the locked handle (opening a second
    // handle would fail with os error 33). On Unix either approach works.
    let content = if !dry_run {
        use std::io::{Read, Seek, SeekFrom};
        let f = file.as_mut().unwrap();
        f.seek(SeekFrom::Start(0)).map_err(CliError::Io)?;
        let mut buf = String::new();
        f.read_to_string(&mut buf).map_err(CliError::Io)?;
        buf
    } else {
        content
    };

    let run_number = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            serde_json::from_str::<serde_json::Value>(l)
                .ok()
                .and_then(|v| v.get("run").and_then(|r| r.as_u64()))
        })
        .max()
        .map(|m| m as usize + 1)
        .unwrap_or(0);

    // Get current git hash
    let hash = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    let short_hash: String = hash.chars().take(7).collect();

    // Calculate delta from previous kept/baseline metric
    let delta = canonical_metric.and_then(|metric| {
        content
            .lines()
            .rev()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .filter(|v| {
                let s = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
                s == "kept" || s == "baseline"
            })
            .find_map(|v| v.get("metric").and_then(|m| m.as_f64()))
            .map(|prev| metric - prev)
    });

    let timestamp = chrono::Utc::now().to_rfc3339();

    let mut record = serde_json::json!({
        "run": run_number,
        "hash": hash,
        "short_hash": short_hash,
        "metric": canonical_metric,
        "delta": delta,
        "status": final_status,
        "summary": final_summary,
        "timestamp": timestamp,
    });

    // Add multi-sample fields if present
    if let Some(ref s) = samples {
        record["samples"] = serde_json::json!(s);
    }
    if let Some(m) = mad {
        record["mad"] = serde_json::json!(m);
    }
    if let Some(c) = confidence {
        record["confidence"] = serde_json::json!(c);
    }

    // Add guard_result field if guard was checked (12d.3)
    if let Some(ref gr) = guard_result {
        record["guard_result"] = serde_json::json!(gr);
    }

    // 9.5: Multi-metric support — if config has [[metrics]], write metric as object
    if let Some(ref config) = config {
        let (defs, is_multi) = super::pareto::parse_metrics_config(config);
        if is_multi {
            if let Some(m) = canonical_metric {
                // Build metric object from the single value + first metric name
                let mut metric_obj = serde_json::Map::new();
                if let Some(def) = defs.first() {
                    metric_obj.insert(def.name.clone(), serde_json::json!(m));
                }
                record["metric"] = serde_json::Value::Object(metric_obj);
            }
        }
    }

    // Generate contextual hints
    let mut hints = generate_hints(
        &content,
        final_status,
        run_number,
        canonical_metric,
        confidence,
    );

    // Add guard-specific hints
    if guard_failed {
        if let Some(ref stderr) = guard_stderr {
            let truncated: String = stderr.chars().take(200).collect();
            hints.push(format!("Guard stderr: {}", truncated));
        }
        hints.push(
            "Guard check failed. The experiment was automatically discarded to protect code integrity.".into()
        );
    }

    if dry_run {
        match format {
            OutputFormat::Json => {
                let mut out = serde_json::json!({
                    "status": "dry_run",
                    "data": record,
                    "message": "No data was written. Remove --dry-run to record.",
                });
                if !hints.is_empty() {
                    out["hints"] = serde_json::json!(hints);
                }
                if let Some(ref gr) = guard_result {
                    out["guard_result"] = serde_json::json!(gr);
                }
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            }
            OutputFormat::Table => {
                let metric_str = canonical_metric
                    .map(|m| format!("{m:.6}"))
                    .unwrap_or_else(|| "-".to_string());
                if let Some(ref gr) = guard_result {
                    let guard_label = if gr == "pass" { "Guard passed." } else { "Guard FAILED. Would force status to discarded." };
                    println!("[DRY RUN] {guard_label}");
                }
                println!(
                    "[DRY RUN] Would record experiment #{run_number}: metric={metric_str} status={final_status} — {final_summary}"
                );
                for hint in &hints {
                    eprintln!("hint: {hint}");
                }
                eprintln!("(no data written — remove --dry-run to record)");
            }
        }
        return Ok(());
    }

    // Write under the lock
    if let Some(ref mut f) = file {
        writeln!(f, "{}", serde_json::to_string(&record).unwrap())?;
    }

    // Update strategy state (5.1-5.6)
    let mut strategy = super::strategy::StrategyState::load();
    strategy.update(final_status, run_number);

    // Add escalation hints from strategy
    if let Some(hint) = strategy.escalation_hint() {
        hints.push(hint);
    }

    // Auto-learn trigger (3.6)
    if super::learn::should_auto_learn(run_number, config.as_ref()) {
        let _ = super::learn::generate_lessons(&format);
    }

    match format {
        OutputFormat::Json => {
            let mut out = serde_json::json!({
                "status": "success",
                "data": record,
            });
            if !hints.is_empty() {
                out["hints"] = serde_json::json!(hints);
            }
            // Include escalation object when level >= 2 (5.7)
            if strategy.level >= 2 {
                out["escalation"] = strategy.to_json();
            }
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        }
        OutputFormat::Table => {
            let metric_str = canonical_metric
                .map(|m| format!("{m:.6}"))
                .unwrap_or_else(|| "-".to_string());
            println!(
                "Recorded experiment #{run_number}: metric={metric_str} status={final_status} — {final_summary}"
            );
            for hint in &hints {
                eprintln!("hint: {hint}");
            }
        }
    }

    Ok(())
}

/// Execute the guard command and return (result, stderr).
/// Returns ("pass", None) on success, ("fail", Some(stderr)) on failure.
fn run_guard_command(cmd: &str) -> (Option<String>, Option<String>) {
    #[cfg(unix)]
    let output = std::process::Command::new("sh")
        .args(["-c", cmd])
        .output();

    #[cfg(windows)]
    let output = std::process::Command::new("cmd")
        .args(["/C", cmd])
        .output();

    #[cfg(not(any(unix, windows)))]
    let output = std::process::Command::new("sh")
        .args(["-c", cmd])
        .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                (Some("pass".to_string()), None)
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                (
                    Some("fail".to_string()),
                    if stderr.is_empty() { None } else { Some(stderr) },
                )
            }
        }
        Err(e) => (
            Some("fail".to_string()),
            Some(format!("Failed to execute guard command: {e}")),
        ),
    }
}

/// Compute stats from multiple metric samples: (median, MAD, confidence).
fn compute_stats(samples: &[f64]) -> (f64, f64, f64) {
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n = sorted.len();

    // Median
    let median = if n % 2 == 0 {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    } else {
        sorted[n / 2]
    };

    // MAD (Median Absolute Deviation)
    let mut abs_devs: Vec<f64> = sorted.iter().map(|x| (x - median).abs()).collect();
    abs_devs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mad = if abs_devs.len() % 2 == 0 {
        (abs_devs[abs_devs.len() / 2 - 1] + abs_devs[abs_devs.len() / 2]) / 2.0
    } else {
        abs_devs[abs_devs.len() / 2]
    };

    // Confidence: 1.0 - (MAD / median.abs()) clamped to [0, 1]
    let confidence = if median.abs() > f64::EPSILON {
        (1.0 - mad / median.abs()).clamp(0.0, 1.0)
    } else if mad < f64::EPSILON {
        1.0 // All samples are zero — perfectly consistent
    } else {
        0.0
    };

    (median, mad, confidence)
}

/// Generate contextual hints based on experiment history.
fn generate_hints(
    jsonl_content: &str,
    status: &str,
    run_number: usize,
    current_metric: Option<f64>,
    confidence: Option<f64>,
) -> Vec<String> {
    let mut hints = Vec::new();

    // Count recent consecutive discards (including error statuses)
    let non_kept_statuses = [
        "discarded",
        "crash",
        "no-op",
        "hook-blocked",
        "metric-error",
    ];

    let consecutive_discards = jsonl_content
        .lines()
        .rev()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            serde_json::from_str::<serde_json::Value>(l)
                .ok()
                .and_then(|v| v.get("status").and_then(|s| s.as_str()).map(|s| s.to_string()))
        })
        .take_while(|s| non_kept_statuses.contains(&s.as_str()))
        .count();

    let total_streak = if non_kept_statuses.contains(&status) {
        consecutive_discards + 1
    } else {
        0
    };

    // Stuck detection
    if total_streak >= 7 {
        hints.push(
            "STUCK: 7+ consecutive non-kept results. You are in a deep local minimum. Try a fundamentally different approach: change strategy entirely, try removing code instead of adding, or run `aris review` for cross-model analysis.".into()
        );
    } else if total_streak >= 5 {
        hints.push(
            "WARNING: 5+ consecutive non-kept results. Consider: (1) run `aris review` for fresh perspective, (2) try the OPPOSITE of recent attempts, (3) use `aris fork` to explore multiple directions.".into()
        );
    } else if total_streak >= 3 {
        hints.push(
            "3 consecutive non-kept results. Re-read `program.md` for ideas you haven't tried. Consider a different category of change.".into()
        );
    }

    // Status-specific hints
    match status {
        "crash" => {
            hints.push("Eval command crashed. Check error output, fix syntax/import/runtime errors, re-run.".into());
        }
        "hook-blocked" => {
            hints.push(
                "Pre-commit hook blocked the commit. Fix formatting/lint issues, re-stage and re-commit. NEVER use --no-verify.".into()
            );
        }
        "metric-error" => {
            // Check for consecutive metric-errors
            let consecutive_metric_errors = jsonl_content
                .lines()
                .rev()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| {
                    serde_json::from_str::<serde_json::Value>(l)
                        .ok()
                        .and_then(|v| {
                            v.get("status").and_then(|s| s.as_str()).map(|s| s.to_string())
                        })
                })
                .take_while(|s| s == "metric-error")
                .count();

            if consecutive_metric_errors >= 1 {
                hints.push(
                    "FATAL: 2 consecutive metric-errors. The eval pipeline is broken — fix the eval command before continuing. STOP THE LOOP.".into()
                );
            } else {
                hints.push(
                    "Eval output was not numeric. Check the verify command pipeline — ensure it outputs a single number.".into()
                );
            }
        }
        "no-op" => {
            hints.push("No code changes were produced. Try a more concrete modification.".into());
        }
        _ => {}
    }

    // Low-confidence hint (multi-sample noise warning)
    if let Some(conf) = confidence {
        if conf < 0.8 {
            hints.push(format!(
                "LOW CONFIDENCE ({:.2}): Metric measurements are noisy (MAD is high relative to median). Consider: (1) increase sample count with more --metric values, (2) reduce variance in eval environment, (3) treat results with caution — small improvements may be noise.",
                conf
            ));
        }
    }

    // Baseline coaching
    if run_number == 0 && status == "baseline" {
        hints.push(
            "Baseline recorded. Strategy: start with hyperparameter tuning (learning rate, batch size, weight decay) — lowest risk, highest signal. Save architecture changes for later.".into()
        );
    }

    // Kept experiment: check for implausible improvement (reward hacking)
    if status == "kept" && run_number > 0 {
        if let Some(current) = current_metric {
            let kept_metrics: Vec<f64> = jsonl_content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .filter(|v| {
                    let s = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
                    s == "kept" || s == "baseline"
                })
                .filter_map(|v| v.get("metric").and_then(|m| m.as_f64()))
                .collect();

            if kept_metrics.len() >= 3 {
                let baseline = kept_metrics.first().copied().unwrap_or(1.0);
                let prev_best = kept_metrics.last().copied().unwrap_or(baseline);

                let cumulative = (baseline - prev_best).abs();
                let this_step = (prev_best - current).abs();

                if cumulative > f64::EPSILON && this_step > cumulative * 10.0 {
                    hints.push(format!(
                        "SUSPICIOUS: This single experiment claims {:.1}x more improvement than ALL previous experiments combined ({:.6} vs cumulative {:.6}). This may indicate reward hacking. Verify the improvement is genuine.",
                        this_step / cumulative, this_step, cumulative
                    ));
                }

                if prev_best.abs() > f64::EPSILON {
                    let ratio = current / prev_best;
                    if !(0.01..=100.0).contains(&ratio) {
                        hints.push(format!(
                            "SUSPICIOUS: Metric changed by {:.0}x in one step (from {:.6} to {:.6}). Large jumps often indicate eval gaming.",
                            if ratio > 1.0 { ratio } else { 1.0 / ratio },
                            prev_best,
                            current
                        ));
                    }
                }
            }

            if hints.iter().all(|h| !h.starts_with("SUSPICIOUS")) {
                hints.push(
                    "Good improvement. Keep exploring in this direction with small variations."
                        .into(),
                );
            }
        }
    }

    // Periodic review reminder
    if run_number > 0 && run_number % 20 == 0 {
        hints.push(format!(
            "{run_number} experiments completed. Consider running `aris report` to review progress and `aris review` for cross-model analysis."
        ));
    }

    // 9.10-9.11: Multi-metric tradeoff hints and Pareto-dominated warning
    // These are generated when the Agent calls record with multi-metric config.
    // The actual tradeoff analysis is done by the pareto module at display time.

    hints
}

fn load_config() -> Result<toml::Table, CliError> {
    let path = std::path::Path::new("autoresearch.toml");
    if !path.exists() {
        return Err(CliError::Config("No autoresearch.toml".into()));
    }
    let content = std::fs::read_to_string(path)?;
    toml::from_str(&content).map_err(|e| CliError::Config(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_stats_single() {
        let (median, mad, confidence) = compute_stats(&[42.0]);
        assert_eq!(median, 42.0);
        assert_eq!(mad, 0.0);
        assert_eq!(confidence, 1.0);
    }

    #[test]
    fn test_compute_stats_odd() {
        let (median, mad, _) = compute_stats(&[1.0, 3.0, 5.0]);
        assert_eq!(median, 3.0);
        assert_eq!(mad, 2.0);
    }

    #[test]
    fn test_compute_stats_even() {
        let (median, _, _) = compute_stats(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(median, 2.5);
    }

    #[test]
    fn test_compute_stats_identical() {
        let (median, mad, confidence) = compute_stats(&[7.0, 7.0, 7.0]);
        assert_eq!(median, 7.0);
        assert_eq!(mad, 0.0);
        assert_eq!(confidence, 1.0);
    }

    #[test]
    fn test_compute_stats_noisy() {
        let (median, mad, confidence) = compute_stats(&[10.0, 11.0, 100.0]);
        assert_eq!(median, 11.0);
        // MAD = median of [1, 0, 89] = 1.0
        assert_eq!(mad, 1.0);
        assert!(confidence > 0.9);
    }

    #[test]
    fn test_error_status_no_metric_hint() {
        let hints = generate_hints("", "crash", 5, None, None);
        assert!(hints.iter().any(|h| h.contains("crashed")));
    }

    #[test]
    fn test_metric_error_consecutive_hint() {
        let existing = r#"{"run":3,"status":"metric-error","metric":null}"#;
        let hints = generate_hints(existing, "metric-error", 4, None, None);
        assert!(hints.iter().any(|h| h.contains("FATAL")));
    }

    #[test]
    fn test_low_confidence_hint() {
        let hints = generate_hints("", "baseline", 0, Some(50.0), Some(0.5));
        assert!(hints.iter().any(|h| h.contains("LOW CONFIDENCE")));
    }

    #[test]
    fn test_high_confidence_no_hint() {
        let hints = generate_hints("", "baseline", 0, Some(50.0), Some(0.95));
        assert!(!hints.iter().any(|h| h.contains("LOW CONFIDENCE")));
    }
}

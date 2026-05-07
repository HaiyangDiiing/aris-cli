use crate::errors::CliError;
use serde::Serialize;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct Experiment {
    pub run: usize,
    pub hash: String,
    pub short_hash: String,
    pub timestamp: String,
    pub metric: Option<f64>,
    /// Multi-metric values (name → value). Empty for legacy single-metric.
    #[serde(skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub metrics: std::collections::HashMap<String, f64>,
    pub status: ExperimentStatus,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ExperimentStatus {
    Baseline,
    Kept,
    Discarded,
    Crash,
    NoOp,
    HookBlocked,
    MetricError,
    Unknown,
}

impl std::fmt::Display for ExperimentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Baseline => write!(f, "baseline"),
            Self::Kept => write!(f, "kept"),
            Self::Discarded => write!(f, "discarded"),
            Self::Crash => write!(f, "crash"),
            Self::NoOp => write!(f, "no-op"),
            Self::HookBlocked => write!(f, "hook-blocked"),
            Self::MetricError => write!(f, "metric-error"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

impl ExperimentStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "baseline" => Self::Baseline,
            "kept" | "keep" => Self::Kept,
            "discarded" | "discard" => Self::Discarded,
            "crash" => Self::Crash,
            "no-op" => Self::NoOp,
            "hook-blocked" => Self::HookBlocked,
            "metric-error" => Self::MetricError,
            _ => Self::Unknown,
        }
    }

    /// Whether this status has a metric value (error statuses typically don't).
    pub fn has_metric(&self) -> bool {
        matches!(self, Self::Baseline | Self::Kept | Self::Discarded)
    }
}

/// Check if we're in a git repo
pub fn is_git_repo() -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get the current branch name
pub fn current_branch() -> Result<String, CliError> {
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .map_err(|e| CliError::Git(e.to_string()))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Check if experiment branch exists
pub fn experiment_branch_exists(branch: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--verify", branch])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Parse experiment history from git log
///
/// Looks for commits on the experiment branch and parses their messages
/// for metric values. Supports multiple formats:
/// - JSONL log file (.autoresearch/experiments.jsonl)
/// - Git commit messages with [autoresearch] prefix
/// - Standard commit messages (fallback)
pub fn parse_experiments(branch: &str, limit: usize) -> Result<Vec<Experiment>, CliError> {
    // Determine if we're on the requested branch (can use working-tree JSONL)
    let current = current_branch().unwrap_or_default();
    let on_target_branch = current == branch;

    // If on the target branch, try working-tree JSONL first (most accurate)
    if on_target_branch {
        if let Ok(experiments) = parse_jsonl_log() {
            if !experiments.is_empty() {
                let mut exps = experiments;
                exps.truncate(limit);
                return Ok(exps);
            }
        }
    } else {
        // For other branches, read JSONL via git show to get branch-specific data
        if let Ok(experiments) = parse_jsonl_from_branch(branch) {
            if !experiments.is_empty() {
                let mut exps = experiments;
                exps.truncate(limit);
                return Ok(exps);
            }
        }
        // Fallback: try working-tree JSONL even when not on target branch
        // (handles case where branch doesn't exist yet but JSONL was created by `record`)
        if let Ok(experiments) = parse_jsonl_log() {
            if !experiments.is_empty() {
                let mut exps = experiments;
                exps.truncate(limit);
                return Ok(exps);
            }
        }
    }

    // Fall back to git log parsing
    parse_git_log(branch, limit)
}

/// Parse experiments from a specific branch's JSONL via git show
fn parse_jsonl_from_branch(branch: &str) -> Result<Vec<Experiment>, CliError> {
    let output = Command::new("git")
        .args(["show", &format!("{branch}:.autoresearch/experiments.jsonl")])
        .output()
        .map_err(|e| CliError::Git(e.to_string()))?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let content = String::from_utf8_lossy(&output.stdout);
    parse_jsonl_content(&content)
}

/// Parse experiments from working-tree .autoresearch/experiments.jsonl
fn parse_jsonl_log() -> Result<Vec<Experiment>, CliError> {
    let path = std::path::Path::new(".autoresearch/experiments.jsonl");
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = std::fs::read_to_string(path)?;
    parse_jsonl_content(&content)
}

/// Parse JSONL content string into experiments
pub(crate) fn parse_jsonl_content(content: &str) -> Result<Vec<Experiment>, CliError> {
    let mut experiments = Vec::new();

    for (i, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            let status = match val.get("status").and_then(|s| s.as_str()) {
                Some(s) => ExperimentStatus::from_str(s),
                None => ExperimentStatus::Unknown,
            };

            // Parse metric: support both legacy f64 and new object format
            let metric_val = val
                .get("metric")
                .or_else(|| val.get("value"))
                .or_else(|| val.get("score"));

            let (metric, metrics) = match metric_val {
                Some(serde_json::Value::Number(n)) => (n.as_f64(), std::collections::HashMap::new()),
                Some(serde_json::Value::Object(obj)) => {
                    // Multi-metric: {"accuracy": 0.766, "latency": 42.0}
                    let mut map = std::collections::HashMap::new();
                    let mut first_val = None;
                    for (k, v) in obj {
                        if let Some(f) = v.as_f64() {
                            if first_val.is_none() {
                                first_val = Some(f);
                            }
                            map.insert(k.clone(), f);
                        }
                    }
                    (first_val, map)
                }
                _ => (None, std::collections::HashMap::new()),
            };

            experiments.push(Experiment {
                run: val
                    .get("run")
                    .and_then(|r| r.as_u64())
                    .unwrap_or(i as u64 + 1) as usize,
                hash: val
                    .get("hash")
                    .or_else(|| val.get("commit"))
                    .and_then(|h| h.as_str())
                    .unwrap_or("")
                    .to_string(),
                short_hash: val
                    .get("short_hash")
                    .and_then(|h| h.as_str())
                    .unwrap_or("")
                    .to_string(),
                timestamp: val
                    .get("timestamp")
                    .or_else(|| val.get("time"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string(),
                metric,
                metrics,
                status,
                summary: val
                    .get("summary")
                    .or_else(|| val.get("description"))
                    .or_else(|| val.get("message"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }

    experiments.reverse(); // Most recent first
    Ok(experiments)
}

/// Parse experiments from git log on experiment branch
fn parse_git_log(branch: &str, limit: usize) -> Result<Vec<Experiment>, CliError> {
    let output = Command::new("git")
        .args([
            "log",
            branch,
            &format!("--max-count={}", limit),
            "--format=%H|%h|%aI|%s",
        ])
        .output()
        .map_err(|e| CliError::Git(e.to_string()))?;

    if !output.status.success() {
        return Err(CliError::NoExperiments(branch.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut experiments = Vec::new();

    for (i, line) in stdout.lines().enumerate() {
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        if parts.len() < 4 {
            continue;
        }

        let message = parts[3];
        let (metric, status) = parse_commit_message(message);

        experiments.push(Experiment {
            run: i + 1,
            hash: parts[0].to_string(),
            short_hash: parts[1].to_string(),
            timestamp: parts[2].to_string(),
            metric,
            metrics: std::collections::HashMap::new(),
            status,
            summary: message.to_string(),
        });
    }

    Ok(experiments)
}

/// Extract metric value and status from commit message
fn parse_commit_message(msg: &str) -> (Option<f64>, ExperimentStatus) {
    let lower = msg.to_lowercase();

    // Pattern: [autoresearch] keep: metric=0.974 - description
    // Pattern: [aris] experiment #N: description
    // Pattern: [aris] fix crash #N: description
    if lower.contains("[autoresearch]")
        || lower.contains("autoresearch:")
        || lower.contains("[aris]")
    {
        let status = if lower.contains("baseline") {
            ExperimentStatus::Baseline
        } else if lower.contains("keep") || lower.contains("kept") || lower.contains("improvement")
        {
            ExperimentStatus::Kept
        } else if lower.contains("discard") || lower.contains("revert") {
            ExperimentStatus::Discarded
        } else if lower.contains("crash") {
            ExperimentStatus::Crash
        } else if lower.contains("hook-blocked") || lower.contains("hook blocked") {
            ExperimentStatus::HookBlocked
        } else if lower.contains("metric-error") || lower.contains("metric error") {
            ExperimentStatus::MetricError
        } else if lower.contains("no-op") || lower.contains("no diff") {
            ExperimentStatus::NoOp
        } else {
            ExperimentStatus::Unknown
        };

        let metric = extract_metric(msg);
        return (metric, status);
    }

    // Fallback: try to find a number that looks like a metric
    (extract_metric(msg), ExperimentStatus::Unknown)
}

/// Try to extract a numeric metric from text
pub(crate) fn extract_metric(text: &str) -> Option<f64> {
    // Search on lowercased text but extract from original using char offsets (safe for non-ASCII)
    let lower = text.to_lowercase();
    for pattern in &["metric=", "score=", "loss=", "val_bpb=", "value="] {
        if let Some(byte_idx) = lower.find(pattern) {
            // Convert byte offset to char offset safely
            let char_start = lower[..byte_idx].chars().count() + pattern.chars().count();
            let num_str: String = text
                .chars()
                .skip(char_start)
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == 'e' || *c == 'E' || *c == '+')
                .collect();
            if let Ok(val) = num_str.parse::<f64>() {
                if val.is_finite() {
                    return Some(val);
                }
            }
        }
    }
    None
}

/// Safe f64 comparison that handles NaN (NaN sorts last)
pub fn safe_cmp(a: f64, b: f64) -> std::cmp::Ordering {
    a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
}

/// Get diff between two commits
pub fn diff_commits(hash_a: &str, hash_b: &str) -> Result<String, CliError> {
    let output = Command::new("git")
        .args(["diff", hash_a, hash_b])
        .output()
        .map_err(|e| CliError::Git(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError::Git(format!("git diff failed: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Get diff of a specific commit (vs its parent)
pub fn show_commit_diff(hash: &str) -> Result<String, CliError> {
    let output = Command::new("git")
        .args(["show", hash, "--stat", "--patch"])
        .output()
        .map_err(|e| CliError::Git(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError::Git(format!("git show failed: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Check if there's a running autoresearch process (lock file)
pub fn is_loop_running() -> bool {
    std::path::Path::new(".autoresearch/loop.lock").exists()
}

/// Get loop state if running
pub fn loop_state() -> Option<serde_json::Value> {
    let path = std::path::Path::new(".autoresearch/loop.lock");
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_jsonl_content_empty() {
        let result = parse_jsonl_content("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_jsonl_content_blank_lines() {
        let result = parse_jsonl_content("\n\n  \n").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_jsonl_content_single_baseline() {
        let line = r#"{"run":0,"hash":"abc1234567","short_hash":"abc1234","metric":1.0,"status":"baseline","summary":"Initial baseline","timestamp":"2025-01-01T00:00:00Z"}"#;
        let result = parse_jsonl_content(line).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].run, 0);
        assert_eq!(result[0].status, ExperimentStatus::Baseline);
        assert_eq!(result[0].metric, Some(1.0));
        assert_eq!(result[0].summary, "Initial baseline");
    }

    #[test]
    fn test_parse_jsonl_content_multiple_records() {
        let content = r#"{"run":0,"hash":"aaa","short_hash":"aaa","metric":1.0,"status":"baseline","summary":"base","timestamp":"2025-01-01T00:00:00Z"}
{"run":1,"hash":"bbb","short_hash":"bbb","metric":0.95,"status":"kept","summary":"lr change","timestamp":"2025-01-01T01:00:00Z"}
{"run":2,"hash":"ccc","short_hash":"ccc","metric":1.1,"status":"discarded","summary":"bad idea","timestamp":"2025-01-01T02:00:00Z"}"#;
        let result = parse_jsonl_content(content).unwrap();
        assert_eq!(result.len(), 3);
        // Most recent first (reversed)
        assert_eq!(result[0].run, 2);
        assert_eq!(result[0].status, ExperimentStatus::Discarded);
        assert_eq!(result[1].run, 1);
        assert_eq!(result[1].status, ExperimentStatus::Kept);
        assert_eq!(result[2].run, 0);
        assert_eq!(result[2].status, ExperimentStatus::Baseline);
    }

    #[test]
    fn test_parse_jsonl_content_keep_alias() {
        let line = r#"{"run":1,"hash":"x","short_hash":"x","metric":0.9,"status":"keep","summary":"test","timestamp":"2025-01-01T00:00:00Z"}"#;
        let result = parse_jsonl_content(line).unwrap();
        assert_eq!(result[0].status, ExperimentStatus::Kept);
    }

    #[test]
    fn test_parse_jsonl_content_discard_alias() {
        let line = r#"{"run":1,"hash":"x","short_hash":"x","metric":0.9,"status":"discard","summary":"test","timestamp":"2025-01-01T00:00:00Z"}"#;
        let result = parse_jsonl_content(line).unwrap();
        assert_eq!(result[0].status, ExperimentStatus::Discarded);
    }

    #[test]
    fn test_parse_jsonl_content_alternative_field_names() {
        let line = r#"{"run":1,"commit":"abc","short_hash":"abc","value":0.5,"status":"kept","description":"test","time":"2025-01-01T00:00:00Z"}"#;
        let result = parse_jsonl_content(line).unwrap();
        assert_eq!(result[0].hash, "abc");
        assert_eq!(result[0].metric, Some(0.5));
        assert_eq!(result[0].summary, "test");
    }

    #[test]
    fn test_parse_jsonl_content_skips_malformed_lines() {
        let content = r#"{"run":0,"hash":"a","short_hash":"a","metric":1.0,"status":"baseline","summary":"ok","timestamp":"t"}
not valid json
{"run":1,"hash":"b","short_hash":"b","metric":0.9,"status":"kept","summary":"ok2","timestamp":"t"}"#;
        let result = parse_jsonl_content(content).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_extract_metric_metric_pattern() {
        assert_eq!(extract_metric("metric=0.974"), Some(0.974));
    }

    #[test]
    fn test_extract_metric_loss_pattern() {
        assert_eq!(extract_metric("loss=1.05"), Some(1.05));
    }

    #[test]
    fn test_extract_metric_score_pattern() {
        assert_eq!(extract_metric("score=92.5"), Some(92.5));
    }

    #[test]
    fn test_extract_metric_val_bpb_pattern() {
        assert_eq!(extract_metric("val_bpb=0.001"), Some(0.001));
    }

    #[test]
    fn test_extract_metric_scientific_notation() {
        assert_eq!(extract_metric("metric=1e-3"), Some(0.001));
    }

    #[test]
    fn test_extract_metric_negative() {
        assert_eq!(extract_metric("metric=-0.5"), Some(-0.5));
    }

    #[test]
    fn test_extract_metric_no_match() {
        assert_eq!(extract_metric("no number here"), None);
    }

    #[test]
    fn test_extract_metric_empty() {
        assert_eq!(extract_metric(""), None);
    }

    #[test]
    fn test_safe_cmp_normal() {
        assert_eq!(safe_cmp(1.0, 2.0), std::cmp::Ordering::Less);
        assert_eq!(safe_cmp(2.0, 1.0), std::cmp::Ordering::Greater);
        assert_eq!(safe_cmp(1.0, 1.0), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_safe_cmp_nan() {
        assert_eq!(safe_cmp(f64::NAN, f64::NAN), std::cmp::Ordering::Equal);
    }
}

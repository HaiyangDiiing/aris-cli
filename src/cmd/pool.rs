use crate::errors::CliError;
use crate::output::format::OutputFormat;
use std::fs;
use std::process::Command;

const POOL_PATH: &str = ".autoresearch/pool.json";

/// Pool state persisted to disk.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PoolState {
    pub slots: usize,
    pub forks: Vec<ForkSlot>,
    pub shared_lessons_path: String,
    pub created_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ForkSlot {
    pub name: String,
    pub branch: String,
    pub worktree: Option<String>,
    pub active: bool,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct ForkHealth {
    pub name: String,
    pub total_experiments: usize,
    pub kept_count: usize,
    pub kept_ratio: f64,
    pub escalation_level: u8,
    pub last_experiment_age_secs: Option<i64>,
}

/// Pool subcommand dispatcher.
pub fn run(subcmd: &str, slots: Option<usize>, forks: &[String], cleanup: bool, json: bool) -> Result<(), CliError> {
    match subcmd {
        "start" => start(slots.unwrap_or(2), forks, json),
        "status" => status(json),
        "rebalance" => rebalance(json),
        "stop" => stop(cleanup, json),
        _ => Err(CliError::Config(format!(
            "Unknown pool subcommand '{subcmd}'. Use: start, status, rebalance, stop"
        ))),
    }
}

/// Start a pool with N slots and fork names.
fn start(slots: usize, forks: &[String], json: bool) -> Result<(), CliError> {
    let format = OutputFormat::detect(json);

    if forks.is_empty() {
        return Err(CliError::Config(
            "Provide fork names: aris pool start --forks name1,name2,...".into(),
        ));
    }

    // Create worktrees for each fork
    let mut fork_slots = Vec::new();
    fs::create_dir_all(".autoresearch/worktrees")?;

    for name in forks {
        let branch_name = format!("autoresearch-fork-{name}");
        let wt_path = format!(".autoresearch/worktrees/{name}");

        if std::path::Path::new(&wt_path).exists() {
            eprintln!("warning: worktree '{wt_path}' already exists, reusing");
        } else {
            // Get current HEAD as base
            let base = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|| "HEAD".to_string());

            let output = Command::new("git")
                .args(["worktree", "add", &wt_path, "-b", &branch_name, &base])
                .output()
                .map_err(|e| CliError::Git(e.to_string()))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("warning: failed to create worktree for '{name}': {stderr}");
                // Try without -b if branch exists
                let output2 = Command::new("git")
                    .args(["worktree", "add", &wt_path, &branch_name])
                    .output()
                    .map_err(|e| CliError::Git(e.to_string()))?;

                if !output2.status.success() {
                    return Err(CliError::Git(format!("Failed to create worktree for '{name}'")));
                }
            }

            // Copy config files
            if std::path::Path::new("autoresearch.toml").exists() {
                fs::copy("autoresearch.toml", format!("{wt_path}/aris.toml")).ok();
            }
            if std::path::Path::new("program.md").exists() {
                fs::copy("program.md", format!("{wt_path}/program.md")).ok();
            }
        }

        let abs_path = fs::canonicalize(&wt_path)
            .map(|p| p.display().to_string())
            .ok();

        fork_slots.push(ForkSlot {
            name: name.clone(),
            branch: branch_name,
            worktree: abs_path.or(Some(wt_path)),
            active: fork_slots.len() < slots, // Queue excess forks
        });
    }

    let state = PoolState {
        slots,
        forks: fork_slots.clone(),
        shared_lessons_path: ".autoresearch/shared-lessons.md".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    fs::write(POOL_PATH, serde_json::to_string_pretty(&state).unwrap())?;

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "status": "success",
                "data": state,
                "instructions": fork_slots.iter()
                    .filter(|f| f.active)
                    .map(|f| format!("cd {} && /aris", f.worktree.as_deref().unwrap_or("?")))
                    .collect::<Vec<_>>(),
            })).unwrap());
        }
        OutputFormat::Table => {
            println!("Pool started: {} slots, {} forks\n", slots, forks.len());
            for f in &fork_slots {
                let status = if f.active { "ACTIVE" } else { "QUEUED" };
                println!("  [{status}] {} → {}", f.name, f.worktree.as_deref().unwrap_or("?"));
            }
            println!();
            println!("Start agents in each active worktree:");
            for f in fork_slots.iter().filter(|f| f.active) {
                println!("  cd {} && /aris  # fork: {}", f.worktree.as_deref().unwrap_or("?"), f.name);
            }
        }
    }

    Ok(())
}

/// Show pool status.
fn status(json: bool) -> Result<(), CliError> {
    let format = OutputFormat::detect(json);

    let state = load_pool()?;
    let mut healths = Vec::new();

    for fork in &state.forks {
        let wt = fork.worktree.as_deref().unwrap_or("");
        let jsonl_path = format!("{wt}/.autoresearch/experiments.jsonl");

        let (total, kept, last_ts) = if let Ok(content) = fs::read_to_string(&jsonl_path) {
            let experiments: Vec<serde_json::Value> = content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect();
            let kept = experiments.iter()
                .filter(|e| e.get("status").and_then(|s| s.as_str()) == Some("kept"))
                .count();
            let last = experiments.last()
                .and_then(|e| e.get("timestamp").and_then(|t| t.as_str()))
                .map(|s| s.to_string());
            (experiments.len(), kept, last)
        } else {
            (0, 0, None)
        };

        // Read strategy state
        let strategy_path = format!("{wt}/.autoresearch/strategy.json");
        let escalation = fs::read_to_string(&strategy_path)
            .ok()
            .and_then(|s| serde_json::from_str::<super::strategy::StrategyState>(&s).ok())
            .map(|s| s.level)
            .unwrap_or(0);

        let age_secs = last_ts.and_then(|ts| {
            chrono::DateTime::parse_from_rfc3339(&ts).ok().map(|dt| {
                (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_seconds()
            })
        });

        healths.push(ForkHealth {
            name: fork.name.clone(),
            total_experiments: total,
            kept_count: kept,
            kept_ratio: if total > 0 { kept as f64 / total as f64 } else { 0.0 },
            escalation_level: escalation,
            last_experiment_age_secs: age_secs,
        });
    }

    // 11.7: Share knowledge across forks
    let _ = share_knowledge(&state);

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "status": "success",
                "pool": {
                    "slots": state.slots,
                    "created_at": state.created_at,
                },
                "forks": healths,
            })).unwrap());
        }
        OutputFormat::Table => {
            println!("Pool Status ({} slots)\n", state.slots);
            println!(
                "  {:<15}  {:>6}  {:>6}  {:>6}  {:>5}  Age",
                "Fork", "Total", "Kept", "Ratio", "Level"
            );
            println!("  {}", "─".repeat(60));
            for h in &healths {
                let age = h.last_experiment_age_secs
                    .map(|s| {
                        if s < 60 { format!("{s}s") }
                        else if s < 3600 { format!("{}m", s / 60) }
                        else { format!("{}h", s / 3600) }
                    })
                    .unwrap_or_else(|| "N/A".to_string());

                let level_str = if h.escalation_level >= 4 {
                    format!("L{} !", h.escalation_level)
                } else {
                    format!("L{}", h.escalation_level)
                };

                println!(
                    "  {:<15}  {:>6}  {:>6}  {:>5.0}%  {:>5}  {}",
                    h.name, h.total_experiments, h.kept_count,
                    h.kept_ratio * 100.0, level_str, age
                );
            }
        }
    }

    Ok(())
}

/// Rebalance: re-evaluate fork health, propose slot reassignments.
fn rebalance(json: bool) -> Result<(), CliError> {
    let format = OutputFormat::detect(json);
    let mut state = load_pool()?;

    // Simple rebalance: deactivate forks at escalation level 4+, activate queued forks
    let mut changes = Vec::new();
    let mut active_count = state.forks.iter().filter(|f| f.active).count();

    for fork in &mut state.forks {
        let wt = fork.worktree.as_deref().unwrap_or("");
        let strategy_path = format!("{wt}/.autoresearch/strategy.json");
        let level = fs::read_to_string(&strategy_path)
            .ok()
            .and_then(|s| serde_json::from_str::<super::strategy::StrategyState>(&s).ok())
            .map(|s| s.level)
            .unwrap_or(0);

        if fork.active && level >= 4 && active_count > 1 {
            fork.active = false;
            active_count -= 1;
            changes.push(format!("Deactivated '{}' (stuck at level {})", fork.name, level));
        }
    }

    // Activate queued forks up to slot limit
    for fork in &mut state.forks {
        if !fork.active && active_count < state.slots {
            fork.active = true;
            active_count += 1;
            changes.push(format!("Activated '{}'", fork.name));
        }
    }

    // 11.8: Broadcast dead-ends from stuck forks
    let broadcasts = broadcast_dead_ends(&state).unwrap_or_default();
    changes.extend(broadcasts);

    fs::write(POOL_PATH, serde_json::to_string_pretty(&state).unwrap())?;

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "status": "success",
                "changes": changes,
                "active_forks": state.forks.iter().filter(|f| f.active).count(),
            })).unwrap());
        }
        OutputFormat::Table => {
            if changes.is_empty() {
                println!("No rebalancing needed.");
            } else {
                println!("Rebalanced:");
                for c in &changes {
                    println!("  {c}");
                }
            }
        }
    }

    Ok(())
}

/// Stop pool: aggregate results, show winner, optionally clean up.
fn stop(cleanup: bool, json: bool) -> Result<(), CliError> {
    let format = OutputFormat::detect(json);
    let state = load_pool()?;

    // Load config for metric direction
    let config_str = fs::read_to_string("autoresearch.toml").unwrap_or_default();
    let config: toml::Table = toml::from_str(&config_str).unwrap_or_default();
    let direction = config.get("metric_direction")
        .and_then(|v| v.as_str())
        .unwrap_or("lower");
    let lower_is_better = direction != "higher";

    // Gather best from each fork
    let mut results: Vec<(String, Option<f64>, usize)> = Vec::new();

    for fork in &state.forks {
        let wt = fork.worktree.as_deref().unwrap_or("");
        let jsonl_path = format!("{wt}/.autoresearch/experiments.jsonl");

        let best_metric = if let Ok(content) = fs::read_to_string(&jsonl_path) {
            content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .filter(|v| {
                    let s = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
                    s == "kept" || s == "baseline"
                })
                .filter_map(|v| v.get("metric").and_then(|m| m.as_f64()))
                .reduce(|best, m| {
                    if lower_is_better { best.min(m) } else { best.max(m) }
                })
        } else {
            None
        };

        let total = if let Ok(content) = fs::read_to_string(&jsonl_path) {
            content.lines().filter(|l| !l.trim().is_empty()).count()
        } else {
            0
        };

        results.push((fork.name.clone(), best_metric, total));
    }

    // Sort by metric
    results.sort_by(|a, b| {
        match (a.1, b.1) {
            (Some(ma), Some(mb)) => {
                if lower_is_better { ma.partial_cmp(&mb) } else { mb.partial_cmp(&ma) }
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    let winner = results.first().map(|r| r.0.clone());

    // Cleanup worktrees
    if cleanup {
        for fork in &state.forks {
            if let Some(wt) = &fork.worktree {
                let _ = Command::new("git")
                    .args(["worktree", "remove", wt, "--force"])
                    .output();
            }
        }
        let _ = fs::remove_file(POOL_PATH);
    }

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "status": "success",
                "winner": winner,
                "results": results.iter().map(|(name, metric, total)| {
                    serde_json::json!({
                        "fork": name,
                        "best_metric": metric,
                        "total_experiments": total,
                    })
                }).collect::<Vec<_>>(),
                "cleaned_up": cleanup,
            })).unwrap());
        }
        OutputFormat::Table => {
            println!("Pool Results ({direction} is better):\n");
            println!(
                "  {:>4}  {:<20}  {:>12}  {:>6}",
                "Rank", "Fork", "Best Metric", "Total"
            );
            println!("  {}", "─".repeat(48));
            for (i, (name, metric, total)) in results.iter().enumerate() {
                let metric_str = metric
                    .map(|m| format!("{m:.6}"))
                    .unwrap_or_else(|| "-".to_string());
                let marker = if i == 0 { " *" } else { "" };
                println!("  {:>4}  {:<20}  {:>12}  {:>6}{marker}", i + 1, name, metric_str, total);
            }
            if let Some(w) = &winner {
                println!("\n  Winner: {w}");
            }
            if cleanup {
                println!("  Worktrees cleaned up.");
            }
        }
    }

    Ok(())
}

/// 11.7: Cross-fork knowledge sharing — merge lessons from all forks into shared-lessons.md.
fn share_knowledge(state: &PoolState) -> Result<(), CliError> {
    let mut shared = String::new();
    shared.push_str("# Shared Lessons (Auto-merged from all forks)\n\n");
    shared.push_str(&format!("*Updated at {}*\n\n", chrono::Utc::now().to_rfc3339()));

    for fork in &state.forks {
        let wt = fork.worktree.as_deref().unwrap_or("");
        let lessons_path = format!("{wt}/.autoresearch/lessons.md");
        if let Ok(content) = fs::read_to_string(&lessons_path) {
            shared.push_str(&format!("## Fork: {}\n\n", fork.name));
            shared.push_str(&content);
            shared.push_str("\n---\n\n");
        }
    }

    if shared.len() > 80 {
        // Only write if we have actual content beyond the header
        fs::write(&state.shared_lessons_path, shared)?;
    }

    Ok(())
}

/// 11.8: Dead-end broadcasting — add failed approaches from stuck forks to shared lessons.
fn broadcast_dead_ends(state: &PoolState) -> Result<Vec<String>, CliError> {
    let mut broadcasts = Vec::new();
    let shared_path = &state.shared_lessons_path;

    let mut dead_end_content = String::new();

    for fork in &state.forks {
        let wt = fork.worktree.as_deref().unwrap_or("");
        let strategy_path = format!("{wt}/.autoresearch/strategy.json");
        let level = fs::read_to_string(&strategy_path)
            .ok()
            .and_then(|s| serde_json::from_str::<super::strategy::StrategyState>(&s).ok())
            .map(|s| s.level)
            .unwrap_or(0);

        if level >= 3 {
            // Read recent discarded summaries from this fork
            let jsonl_path = format!("{wt}/.autoresearch/experiments.jsonl");
            if let Ok(content) = fs::read_to_string(&jsonl_path) {
                let discarded: Vec<String> = content
                    .lines()
                    .rev()
                    .filter(|l| !l.trim().is_empty())
                    .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                    .filter(|v| v.get("status").and_then(|s| s.as_str()) == Some("discarded"))
                    .take(5)
                    .filter_map(|v| v.get("summary").and_then(|s| s.as_str()).map(|s| s.to_string()))
                    .collect();

                if !discarded.is_empty() {
                    dead_end_content.push_str(&format!("\n## DEAD END — Fork: {} (Level {})\n\n", fork.name, level));
                    for summary in &discarded {
                        dead_end_content.push_str(&format!("- {summary}\n"));
                    }
                    broadcasts.push(format!("Broadcast dead-ends from '{}' (level {})", fork.name, level));
                }
            }
        }
    }

    if !dead_end_content.is_empty() {
        let mut existing = fs::read_to_string(shared_path).unwrap_or_default();
        existing.push_str(&dead_end_content);
        fs::write(shared_path, existing)?;
    }

    Ok(broadcasts)
}

fn load_pool() -> Result<PoolState, CliError> {
    let content = fs::read_to_string(POOL_PATH)
        .map_err(|_| CliError::Config("No pool running. Start one with `aris pool start`.".into()))?;
    serde_json::from_str(&content)
        .map_err(|e| CliError::Config(format!("Invalid pool.json: {e}")))
}

use crate::errors::CliError;
use crate::output::format::OutputFormat;
use std::fs;
use std::process::Command;

const LOG_PATH: &str = ".autoresearch/experiments.jsonl";

pub fn run(base: Option<&str>, draft: bool, json: bool) -> Result<(), CliError> {
    let format = OutputFormat::detect(json);

    // 8.2: Check gh CLI availability
    let gh_check = Command::new("gh")
        .args(["--version"])
        .output();

    match gh_check {
        Err(_) => {
            return Err(CliError::Config(
                "GitHub CLI (gh) is required for PR creation. Install from https://cli.github.com/".into()
            ));
        }
        Ok(out) if !out.status.success() => {
            return Err(CliError::Config(
                "GitHub CLI (gh) is required for PR creation. Install from https://cli.github.com/".into()
            ));
        }
        _ => {}
    }

    // 8.3: Check gh auth status
    let auth_check = Command::new("gh")
        .args(["auth", "status"])
        .output()
        .map_err(|e| CliError::Config(format!("Failed to check gh auth: {e}")))?;

    if !auth_check.status.success() {
        return Err(CliError::Config(
            "Not authenticated with GitHub. Run `gh auth login` first.".into()
        ));
    }

    // Load config
    let config_content = fs::read_to_string("autoresearch.toml")
        .map_err(|_| CliError::Config("No autoresearch.toml".into()))?;
    let config: toml::Table = toml::from_str(&config_content)
        .map_err(|e| CliError::Config(e.to_string()))?;
    let metric_name = config
        .get("metric_name")
        .and_then(|v| v.as_str())
        .unwrap_or("metric");
    let direction = config
        .get("metric_direction")
        .and_then(|v| v.as_str())
        .unwrap_or("lower");

    // Load experiments
    let content = fs::read_to_string(LOG_PATH)
        .map_err(|_| CliError::NoExperiments("No experiments".into()))?;

    let experiments: Vec<serde_json::Value> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    if experiments.is_empty() {
        return Err(CliError::NoExperiments("No experiments".into()));
    }

    // Find baseline and best
    let baseline_metric = experiments.iter()
        .find(|e| e.get("status").and_then(|s| s.as_str()) == Some("baseline"))
        .and_then(|e| e.get("metric").and_then(|m| m.as_f64()));

    let lower_is_better = direction != "higher";

    let best_experiment = experiments.iter()
        .filter(|e| {
            let s = e.get("status").and_then(|s| s.as_str()).unwrap_or("");
            s == "kept" || s == "baseline"
        })
        .filter_map(|e| {
            e.get("metric").and_then(|m| m.as_f64()).map(|m| (e, m))
        })
        .min_by(|(_, a), (_, b)| {
            if lower_is_better {
                a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal)
            }
        });

    let (_, best_metric) = best_experiment.ok_or(CliError::NoExperiments("No experiments".into()))?;

    // 8.4: Build PR title
    let improvement = if let Some(base) = baseline_metric {
        if base.abs() > f64::EPSILON {
            ((best_metric - base) / base.abs()) * 100.0
        } else {
            0.0
        }
    } else {
        0.0
    };

    let title = if let Some(base) = baseline_metric {
        format!(
            "aris: {metric_name} improved {improvement:.1}% ({base:.4} → {best_metric:.4})"
        )
    } else {
        format!("aris: {metric_name} = {best_metric:.4}")
    };

    // 8.5: Build PR body
    let total = experiments.len();
    let kept_count = experiments.iter()
        .filter(|e| e.get("status").and_then(|s| s.as_str()) == Some("kept"))
        .count();
    let discarded_count = experiments.iter()
        .filter(|e| e.get("status").and_then(|s| s.as_str()) == Some("discarded"))
        .count();

    let winning_changes: Vec<String> = experiments.iter()
        .filter(|e| e.get("status").and_then(|s| s.as_str()) == Some("kept"))
        .map(|e| {
            let run = e.get("run").and_then(|r| r.as_u64()).unwrap_or(0);
            let metric = e.get("metric").and_then(|m| m.as_f64())
                .map(|m| format!("{m:.4}"))
                .unwrap_or_else(|| "-".to_string());
            let summary = e.get("summary").and_then(|s| s.as_str()).unwrap_or("");
            format!("- **#{run}** ({metric_name}={metric}): {summary}")
        })
        .collect();

    let body = format!(
        "## Summary\n\n\
         | Metric | Value |\n\
         |--------|-------|\n\
         | Total experiments | {total} |\n\
         | Kept | {kept_count} |\n\
         | Discarded | {discarded_count} |\n\
         | Baseline | {} |\n\
         | Best | {best_metric:.6} |\n\
         | Improvement | {improvement:+.1}% |\n\n\
         ## Winning Changes\n\n\
         {}\n\n\
         ---\n\
         *Generated by `aris pr`*",
        baseline_metric.map(|m| format!("{m:.6}")).unwrap_or_else(|| "N/A".to_string()),
        if winning_changes.is_empty() { "No kept experiments.".to_string() } else { winning_changes.join("\n") },
    );

    // 8.6: Execute gh pr create
    let mut args = vec!["pr", "create", "--title", &title, "--body", &body];

    let base_str;
    if let Some(b) = base {
        base_str = b.to_string();
        args.push("--base");
        args.push(&base_str);
    }

    if draft {
        args.push("--draft");
    }

    let output = Command::new("gh")
        .args(&args)
        .output()
        .map_err(|e| CliError::Config(format!("Failed to run gh pr create: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError::Config(format!("gh pr create failed: {stderr}")));
    }

    // 8.7: Parse PR URL from output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let pr_url = stdout.trim().to_string();

    // Try to extract PR number from URL
    let pr_number = pr_url
        .split('/')
        .next_back()
        .and_then(|s| s.parse::<u64>().ok());

    // 8.10: Output
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "status": "success",
                "pr_url": pr_url,
                "pr_number": pr_number,
                "title": title,
            })).unwrap());
        }
        OutputFormat::Table => {
            println!("PR created: {pr_url}");
            println!("  Title: {title}");
            println!("  {total} experiments, {kept_count} kept, {improvement:+.1}% improvement");
        }
    }

    Ok(())
}

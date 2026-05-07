use crate::errors::CliError;
use crate::git;
use crate::output::format::OutputFormat;

pub fn run(json: bool, pareto: bool) -> Result<(), CliError> {
    let format = OutputFormat::detect(json);
    let config = load_config()?;
    let branch = config
        .get("branch")
        .and_then(|v| v.as_str())
        .unwrap_or("autoresearch");

    let experiments = git::parse_experiments(branch, 10000).unwrap_or_default();
    if experiments.is_empty() {
        return Err(CliError::NoExperiments(branch.to_string()));
    }

    // Parse multi-metric config
    let (defs, is_multi) = super::pareto::parse_metrics_config(&config);
    let direction = defs.first().map(|d| d.direction.as_str()).unwrap_or("lower");
    let lower_is_better = direction != "higher";

    // Filter to kept/baseline with metrics
    let candidates: Vec<&git::Experiment> = experiments
        .iter()
        .filter(|e| {
            e.metric.is_some()
                && (e.status == git::ExperimentStatus::Kept
                    || e.status == git::ExperimentStatus::Baseline)
        })
        .collect();

    if candidates.is_empty() {
        return Err(CliError::NoExperiments(branch.to_string()));
    }

    // Pareto mode for multi-metric
    if pareto && is_multi {
        let all_values: Vec<super::pareto::MetricValues> = candidates
            .iter()
            .map(|e| e.metrics.clone())
            .collect();

        let front_indices = super::pareto::pareto_front(&all_values, &defs);

        // Compute composite scores
        let mut scored: Vec<(&git::Experiment, f64)> = front_indices
            .iter()
            .filter_map(|&i| {
                let score = super::pareto::composite_score(&all_values[i], &defs, &all_values)?;
                Some((candidates[i], score))
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        match format {
            OutputFormat::Json => {
                let pareto_data: Vec<serde_json::Value> = scored
                    .iter()
                    .map(|(exp, score)| {
                        serde_json::json!({
                            "run": exp.run,
                            "metric": exp.metric,
                            "metrics": exp.metrics,
                            "composite_score": score,
                            "summary": exp.summary,
                        })
                    })
                    .collect();

                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                    "status": "success",
                    "pareto_front": pareto_data,
                    "metric_definitions": defs,
                })).unwrap());
            }
            OutputFormat::Table => {
                println!("Pareto Front ({} non-dominated experiments):\n", scored.len());
                for (i, (exp, score)) in scored.iter().enumerate() {
                    let metric_str = exp.metric
                        .map(|m| format!("{m:.6}"))
                        .unwrap_or_else(|| "-".into());
                    println!(
                        "  {}. #{} (composite: {:.4}, metric: {}) — {}",
                        i + 1, exp.run, score, metric_str, exp.summary
                    );
                    // Show per-metric breakdown if available
                    if !exp.metrics.is_empty() {
                        for (name, val) in &exp.metrics {
                            println!("       {name}: {val:.6}");
                        }
                    }
                }
            }
        }

        return Ok(());
    }

    // Standard single-metric best
    let best = candidates
        .iter()
        .min_by(|a, b| {
            let ma = a.metric.unwrap();
            let mb = b.metric.unwrap();
            if lower_is_better {
                crate::git::safe_cmp(ma, mb)
            } else {
                crate::git::safe_cmp(mb, ma)
            }
        });

    let baseline = experiments
        .iter()
        .find(|e| e.status == git::ExperimentStatus::Baseline);

    match (best, &format) {
        (Some(best), OutputFormat::Json) => {
            let mut data = serde_json::json!({
                "status": "success",
                "best": best,
            });
            if let Some(bl) = baseline {
                data["baseline"] = serde_json::json!(bl);
                if let (Some(bm), Some(blm)) = (best.metric, bl.metric) {
                    if blm.abs() > f64::EPSILON {
                        let improvement = if lower_is_better {
                            ((blm - bm) / blm) * 100.0
                        } else {
                            ((bm - blm) / blm) * 100.0
                        };
                        data["improvement_pct"] = serde_json::json!(improvement);
                    }
                }
            }
            // Add composite score if multi-metric
            if is_multi && !best.metrics.is_empty() {
                let all_values: Vec<super::pareto::MetricValues> = candidates
                    .iter()
                    .map(|e| e.metrics.clone())
                    .collect();
                if let Some(score) = super::pareto::composite_score(&best.metrics, &defs, &all_values) {
                    data["composite_score"] = serde_json::json!(score);
                }
            }
            println!("{}", serde_json::to_string_pretty(&data).unwrap());
        }
        (Some(best), OutputFormat::Table) => {
            println!("Best experiment:");
            println!("  Run:    #{}", best.run);
            println!("  Hash:   {}", best.short_hash);
            println!(
                "  Metric: {}",
                best.metric
                    .map(|m| format!("{:.6}", m))
                    .unwrap_or("-".into())
            );
            // Show per-metric breakdown
            if !best.metrics.is_empty() {
                for (name, val) in &best.metrics {
                    println!("    {name}: {val:.6}");
                }
            }
            println!("  Summary: {}", best.summary);

            if let Some(bl) = baseline {
                if let (Some(bm), Some(blm)) = (best.metric, bl.metric) {
                    if blm.abs() > f64::EPSILON {
                        let improvement = if lower_is_better {
                            ((blm - bm) / blm) * 100.0
                        } else {
                            ((bm - blm) / blm) * 100.0
                        };
                        println!();
                        println!(
                            "  Baseline: {:.6} -> Best: {:.6} ({:.2}% improvement)",
                            blm, bm, improvement
                        );
                    }
                }
            }

            // Show the diff
            if !best.hash.is_empty() {
                println!();
                println!("Diff from parent:");
                match git::show_commit_diff(&best.hash) {
                    Ok(diff) => {
                        let lines: Vec<&str> = diff.lines().collect();
                        if lines.len() > 60 {
                            for line in &lines[..60] {
                                println!("  {line}");
                            }
                            println!("  ... ({} more lines)", lines.len() - 60);
                        } else {
                            for line in &lines {
                                println!("  {line}");
                            }
                        }
                    }
                    Err(_) => println!("  (could not retrieve diff)"),
                }
            }
        }
        (None, _) => {
            return Err(CliError::NoExperiments(branch.to_string()));
        }
    }

    Ok(())
}

fn load_config() -> Result<toml::Table, CliError> {
    let path = std::path::Path::new("autoresearch.toml");
    if !path.exists() {
        return Ok(toml::Table::new());
    }
    let content = std::fs::read_to_string(path)?;
    toml::from_str(&content).map_err(|e| CliError::Config(e.to_string()))
}

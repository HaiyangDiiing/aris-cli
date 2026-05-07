use crate::errors::CliError;
use crate::git;
use crate::output::format::OutputFormat;
use std::fs;
use std::io::IsTerminal;

/// Eval command templates per detected project type.
struct EvalTemplate {
    label: &'static str,
    command: &'static str,
}

const EVAL_TEMPLATES: &[EvalTemplate] = &[
    EvalTemplate {
        label: "Python (pytest)",
        command: "python -m pytest --tb=short -q",
    },
    EvalTemplate {
        label: "Python (coverage)",
        command: "python -m pytest --cov --cov-report=term-missing -q | tail -1 | awk '{print $NF}' | tr -d '%'",
    },
    EvalTemplate {
        label: "Python (custom script)",
        command: "python eval.py",
    },
    EvalTemplate {
        label: "Node.js (jest)",
        command: "npx jest --coverage --silent 2>&1 | grep 'All files' | awk '{print $4}'",
    },
    EvalTemplate {
        label: "Rust (cargo test)",
        command: "cargo test 2>&1 | tail -1",
    },
    EvalTemplate {
        label: "Go (go test)",
        command: "go test -cover ./... 2>&1 | grep coverage | awk '{print $5}' | tr -d '%'",
    },
    EvalTemplate {
        label: "Custom command",
        command: "",
    },
];

pub fn run(
    target_file: Option<String>,
    eval_command: Option<String>,
    metric_name: &str,
    metric_direction: &str,
    time_budget: &str,
    branch: &str,
    template_name: Option<&str>,
    json: bool,
) -> Result<(), CliError> {
    let format = OutputFormat::detect(json);

    if !git::is_git_repo() {
        return Err(CliError::NotGitRepo);
    }

    if std::path::Path::new("autoresearch.toml").exists() {
        match format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "already_initialized",
                        "message": "autoresearch.toml already exists"
                    })
                );
            }
            OutputFormat::Table => {
                println!("autoresearch.toml already exists. Edit it directly to change settings.");
            }
        }
        return Ok(());
    }

    // 6.6-6.8: Apply template defaults if --template provided
    let template = if let Some(name) = template_name {
        let t = super::templates::find_template(name).ok_or_else(|| {
            CliError::Config(format!(
                "Unknown template '{}'. Available templates: {}",
                name,
                super::templates::available_templates().join(", ")
            ))
        })?;
        Some(t)
    } else {
        None
    };

    // Determine if we can run interactively or need flags
    let is_interactive = std::io::stdin().is_terminal() && target_file.is_none() && template.is_none();

    let (target_file, eval_command, metric_name, metric_direction, time_budget_resolved, guard_command) = if let Some(t) = template {
        // Template mode: use template defaults, CLI flags override
        let tf = target_file.unwrap_or_else(|| "train.py".to_string());
        let ec = eval_command.unwrap_or_else(|| t.eval_command.to_string());
        let mn = if metric_name != "metric" { metric_name.to_string() } else { t.metric_name.to_string() };
        let md = if metric_direction != "lower" { metric_direction.to_string() } else { t.metric_direction.to_string() };
        let tb = if time_budget != "5m" { time_budget.to_string() } else { t.time_budget.to_string() };
        (tf, ec, mn, md, tb, t.guard_command.map(|s| s.to_string()))
    } else if is_interactive {
        let (tf, ec, mn, md) = run_interactive_wizard(metric_name, metric_direction)?;
        (tf, ec, mn, md, time_budget.to_string(), None)
    } else {
        // Non-interactive mode — flags required
        let tf = target_file.ok_or_else(|| {
            CliError::Config(
                "In non-interactive mode, --target-file is required. \
                 Example: aris init --target-file train.py --eval-command 'python train.py'"
                    .to_string(),
            )
        })?;
        let ec = eval_command.ok_or_else(|| {
            CliError::Config(
                "In non-interactive mode, --eval-command is required. \
                 Example: aris init --target-file train.py --eval-command 'python train.py'"
                    .to_string(),
            )
        })?;
        (tf, ec, metric_name.to_string(), metric_direction.to_string(), time_budget.to_string(), None)
    };

    // Write autoresearch.toml using proper TOML serialization
    let mut config_table = toml::map::Map::new();
    config_table.insert(
        "target_file".to_string(),
        toml::Value::String(target_file.clone()),
    );
    config_table.insert(
        "eval_command".to_string(),
        toml::Value::String(eval_command.clone()),
    );
    config_table.insert(
        "metric_name".to_string(),
        toml::Value::String(metric_name.clone()),
    );
    config_table.insert(
        "metric_direction".to_string(),
        toml::Value::String(metric_direction.clone()),
    );
    config_table.insert(
        "time_budget".to_string(),
        toml::Value::String(time_budget_resolved.clone()),
    );
    config_table.insert(
        "branch".to_string(),
        toml::Value::String(branch.to_string()),
    );
    if let Some(ref gc) = guard_command {
        config_table.insert(
            "guard_command".to_string(),
            toml::Value::String(gc.clone()),
        );
    }

    let config_str = format!(
        "# Autoresearch configuration\n# Generated by autoresearch CLI v{}\n\n{}",
        env!("CARGO_PKG_VERSION"),
        toml::to_string_pretty(&toml::Value::Table(config_table))
            .map_err(|e| CliError::Config(e.to_string()))?
    );

    fs::write("autoresearch.toml", &config_str)?;

    // Create .autoresearch directory
    fs::create_dir_all(".autoresearch")?;

    // 12c.1: Add .autoresearch/ to .gitignore
    add_to_gitignore()?;

    // Create program.md if it doesn't exist
    if !std::path::Path::new("program.md").exists() {
        let program = if let Some(t) = template {
            t.program_md.to_string()
        } else {
            format!(
                "# Autoresearch Program\n\n\
                 ## Goal\n\
                 Optimize `{metric_name}` ({metric_direction} is better) by modifying `{target_file}`.\n\n\
                 ## Ideas to Explore\n\
                 <!-- Add your research directions, ideas, and hypotheses here. -->\n\n\
                 1.\n\n\
                 ## Constraints\n\
                 - Time budget per experiment: {time_budget_resolved}\n\
                 - Only modify: {target_file}\n\
                 - Eval command: `{eval_command}`\n",
            )
        };
        fs::write("program.md", &program)?;
    }

    // 12c.4: Dry-run the eval command and show output
    let eval_preview = dry_run_eval(&eval_command);

    match format {
        OutputFormat::Json => {
            let mut out = serde_json::json!({
                "status": "success",
                "files_created": ["autoresearch.toml", "program.md", ".autoresearch/"],
                "config": {
                    "target_file": target_file,
                    "eval_command": eval_command,
                    "metric_name": metric_name,
                    "metric_direction": metric_direction,
                    "time_budget": time_budget_resolved,
                    "branch": branch,
                }
            });
            if let Some(ref t) = template_name {
                out["template"] = serde_json::json!(t);
            }
            if let Some(ref preview) = eval_preview {
                out["eval_preview"] = serde_json::json!(preview);
            }
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        }
        OutputFormat::Table => {
            println!();
            println!("Initialized aris project:");
            println!("  autoresearch.toml  — experiment configuration");
            println!("  program.md         — research direction & ideas (edit this!)");
            println!("  .autoresearch/     — experiment logs");
            if let Some(ref preview) = eval_preview {
                println!();
                println!("Eval command preview:");
                for line in preview.lines().take(10) {
                    println!("  {line}");
                }
                if preview.lines().count() > 10 {
                    println!("  ... (truncated)");
                }
            }
            println!();
            println!(
                "Next: edit program.md with your ideas, then tell your agent to run /aris"
            );
        }
    }

    Ok(())
}

/// Run the interactive setup wizard (12c.2).
fn run_interactive_wizard(
    default_metric_name: &str,
    default_metric_direction: &str,
) -> Result<(String, String, String, String), CliError> {
    use dialoguer::{Input, Select};

    println!();
    println!("  ARIS Setup Wizard");
    println!("  ─────────────────");
    println!();

    // Step 1: What do you want to optimize?
    let goal: String = Input::new()
        .with_prompt("What do you want to optimize? (e.g., accuracy, latency, test coverage)")
        .interact_text()
        .map_err(|e| CliError::Config(e.to_string()))?;

    // Step 2: Target file
    let tf: String = Input::new()
        .with_prompt("Which file should the Agent modify?")
        .interact_text()
        .map_err(|e| CliError::Config(e.to_string()))?;

    // Step 3: Metric name (auto-suggest from goal)
    let suggested_metric = suggest_metric_name(&goal);
    let metric_name: String = Input::new()
        .with_prompt("Metric name")
        .default(suggested_metric.unwrap_or_else(|| default_metric_name.to_string()))
        .interact_text()
        .map_err(|e| CliError::Config(e.to_string()))?;

    // Step 4: Direction
    let directions = &["higher is better", "lower is better"];
    let default_dir = if default_metric_direction == "higher" { 0 } else { 1 };
    let direction_idx = Select::new()
        .with_prompt("Metric direction")
        .items(directions)
        .default(default_dir)
        .interact()
        .map_err(|e| CliError::Config(e.to_string()))?;
    let metric_direction = if direction_idx == 0 { "higher" } else { "lower" };

    // Step 5: Eval command with templates (12c.3)
    let template_labels: Vec<&str> = EVAL_TEMPLATES.iter().map(|t| t.label).collect();
    let template_idx = Select::new()
        .with_prompt("Eval command template")
        .items(&template_labels)
        .default(template_labels.len() - 1) // Default to "Custom"
        .interact()
        .map_err(|e| CliError::Config(e.to_string()))?;

    let eval_command = if EVAL_TEMPLATES[template_idx].command.is_empty() {
        // Custom command
        Input::new()
            .with_prompt("Eval command (must output a single number)")
            .interact_text()
            .map_err(|e| CliError::Config(e.to_string()))?
    } else {
        let template_cmd = EVAL_TEMPLATES[template_idx].command;
        let ec: String = Input::new()
            .with_prompt("Eval command")
            .default(template_cmd.to_string())
            .interact_text()
            .map_err(|e| CliError::Config(e.to_string()))?;
        ec
    };

    println!();
    println!("  Goal: {goal}");
    println!("  File: {tf}");
    println!("  Metric: {metric_name} ({metric_direction} is better)");
    println!("  Eval: {eval_command}");
    println!();

    Ok((tf, eval_command, metric_name, metric_direction.to_string()))
}

/// Suggest a metric name from the goal description.
fn suggest_metric_name(goal: &str) -> Option<String> {
    let lower = goal.to_lowercase();
    if lower.contains("accuracy") || lower.contains("准确") {
        Some("accuracy".to_string())
    } else if lower.contains("latency") || lower.contains("延迟") {
        Some("p99_latency".to_string())
    } else if lower.contains("coverage") || lower.contains("覆盖") {
        Some("coverage".to_string())
    } else if lower.contains("loss") || lower.contains("损失") {
        Some("loss".to_string())
    } else if lower.contains("speed") || lower.contains("速度") {
        Some("speed".to_string())
    } else if lower.contains("score") || lower.contains("分数") {
        Some("score".to_string())
    } else if lower.contains("perplexity") || lower.contains("困惑度") {
        Some("perplexity".to_string())
    } else {
        None
    }
}

/// Add .autoresearch/ to .gitignore if not already present (12c.1).
fn add_to_gitignore() -> Result<(), CliError> {
    let gitignore_path = std::path::Path::new(".gitignore");
    let entry = ".autoresearch/";

    if gitignore_path.exists() {
        let content = fs::read_to_string(gitignore_path)?;
        if content.lines().any(|l| l.trim() == entry || l.trim() == ".autoresearch") {
            return Ok(()); // Already present
        }
        // Append
        let mut new_content = content;
        if !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        new_content.push_str("\n# Autoresearch experiment logs (local, not committed)\n");
        new_content.push_str(entry);
        new_content.push('\n');
        fs::write(gitignore_path, new_content)?;
    } else {
        // Create new .gitignore
        let content = format!(
            "# Autoresearch experiment logs (local, not committed)\n{entry}\n"
        );
        fs::write(gitignore_path, content)?;
    }

    Ok(())
}

/// Dry-run the eval command and return its output (12c.4).
fn dry_run_eval(eval_command: &str) -> Option<String> {
    #[cfg(unix)]
    let output = std::process::Command::new("sh")
        .args(["-c", eval_command])
        .output();

    #[cfg(windows)]
    let output = std::process::Command::new("cmd")
        .args(["/C", eval_command])
        .output();

    #[cfg(not(any(unix, windows)))]
    let output = std::process::Command::new("sh")
        .args(["-c", eval_command])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();

            if out.status.success() && !stdout.is_empty() {
                // Try to extract a number from the output
                let has_number = stdout
                    .lines()
                    .last()
                    .and_then(|l| l.trim().parse::<f64>().ok())
                    .is_some();
                let mut result = format!("Output:\n{stdout}");
                if has_number {
                    result.push_str("\n(Metric extraction: OK — last line is a number)");
                } else {
                    result.push_str(
                        "\n(Warning: last line is not a number — you may need to pipe/grep to extract the metric)",
                    );
                }
                Some(result)
            } else if !stderr.is_empty() {
                Some(format!(
                    "Command failed (exit {}):\n{stderr}",
                    out.status.code().unwrap_or(-1)
                ))
            } else {
                Some(format!(
                    "Command exited with status {} but produced no output",
                    out.status.code().unwrap_or(-1)
                ))
            }
        }
        Err(e) => Some(format!("Failed to run eval command: {e}")),
    }
}

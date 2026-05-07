use crate::errors::CliError;
use crate::git;
use crate::output::format::OutputFormat;
use std::process::Command;

pub fn run(names: &[String], parallel: bool, json: bool, dry_run: bool) -> Result<(), CliError> {
    let format = OutputFormat::detect(json);

    if !git::is_git_repo() {
        return Err(CliError::NotGitRepo);
    }

    if names.is_empty() {
        return Err(CliError::Config(
            "Provide at least one fork name. Example: aris fork 'try-transformers' 'try-convolutions'".into(),
        ));
    }

    let config = load_config()?;
    let base_branch = config
        .get("branch")
        .and_then(|v| v.as_str())
        .unwrap_or("autoresearch");

    // Determine the base point: current experiment branch or main
    let base_ref = if git::experiment_branch_exists(base_branch) {
        base_branch.to_string()
    } else {
        // Fall back to current branch
        git::current_branch()?
    };

    let mut created = Vec::new();

    for name in names {
        let branch_name = format!("autoresearch-fork-{name}");

        if git::experiment_branch_exists(&branch_name) {
            if let OutputFormat::Table = format {
                eprintln!("  warning: branch '{branch_name}' already exists, skipping");
            }
            let mut entry = serde_json::json!({
                "name": name,
                "branch": branch_name,
                "status": "already_exists",
            });
            // If parallel, check for existing worktree
            if parallel {
                let wt_path = format!(".autoresearch/worktrees/{name}");
                if std::path::Path::new(&wt_path).exists() {
                    let abs_path = std::fs::canonicalize(&wt_path)
                        .map(|p| p.display().to_string())
                        .unwrap_or(wt_path.clone());
                    entry["worktree"] = serde_json::json!(abs_path);
                }
            }
            created.push(entry);
            continue;
        }

        if dry_run {
            let mut entry = serde_json::json!({
                "name": name,
                "branch": branch_name,
                "base": base_ref,
                "status": "would_create",
            });
            if parallel {
                let wt_path = format!(".autoresearch/worktrees/{name}");
                entry["worktree"] = serde_json::json!(wt_path);
            }
            created.push(entry);
            continue;
        }

        if parallel {
            // 4.3: Create worktree with branch
            let wt_path = format!(".autoresearch/worktrees/{name}");
            std::fs::create_dir_all(".autoresearch/worktrees").ok();

            let output = Command::new("git")
                .args(["worktree", "add", &wt_path, "-b", &branch_name, &base_ref])
                .output()
                .map_err(|e| CliError::Git(e.to_string()))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(CliError::Git(format!(
                    "Failed to create worktree for '{name}': {stderr}"
                )));
            }

            let abs_path = std::fs::canonicalize(&wt_path)
                .map(|p| p.display().to_string())
                .unwrap_or(wt_path.clone());

            // Copy autoresearch.toml and program.md to worktree
            if std::path::Path::new("autoresearch.toml").exists() {
                std::fs::copy("autoresearch.toml", format!("{wt_path}/aris.toml")).ok();
            }
            if std::path::Path::new("program.md").exists() {
                std::fs::copy("program.md", format!("{wt_path}/program.md")).ok();
            }

            created.push(serde_json::json!({
                "name": name,
                "branch": branch_name,
                "base": base_ref,
                "worktree": abs_path,
                "status": "created",
            }));
        } else {
            // Standard branch-only fork
            let output = Command::new("git")
                .args(["branch", &branch_name, &base_ref])
                .output()
                .map_err(|e| CliError::Git(e.to_string()))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(CliError::Git(format!(
                    "Failed to create branch '{branch_name}': {stderr}"
                )));
            }

            created.push(serde_json::json!({
                "name": name,
                "branch": branch_name,
                "base": base_ref,
                "status": "created",
            }));
        }
    }

    let status_label = if dry_run { "dry_run" } else { "success" };

    match format {
        OutputFormat::Json => {
            let mut out = serde_json::json!({
                "status": status_label,
                "data": {
                    "base": base_ref,
                    "parallel": parallel,
                    "forks": created,
                },
                "suggestion": if parallel {
                    "Start agents in each worktree directory"
                } else {
                    "Start agents on each fork: git checkout autoresearch-fork-<name> then run /aris"
                },
            });
            if dry_run {
                out["message"] = serde_json::json!("No branches were created. Remove --dry-run to create them.");
            }
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        }
        OutputFormat::Table => {
            if dry_run {
                println!("[DRY RUN] Would fork from '{base_ref}'{}:\n",
                    if parallel { " (with worktrees)" } else { "" });
            } else {
                println!("Forked from '{base_ref}'{}:\n",
                    if parallel { " (with worktrees)" } else { "" });
            }
            for fork in &created {
                let status = fork["status"].as_str().unwrap_or("");
                let branch = fork["branch"].as_str().unwrap_or("");
                let icon = match status {
                    "created" => "+",
                    "would_create" => "~",
                    _ => "~",
                };
                let worktree_info = fork.get("worktree")
                    .and_then(|w| w.as_str())
                    .map(|w| format!(" → {w}"))
                    .unwrap_or_default();
                println!("  [{icon}] {branch} ({status}){worktree_info}");
            }
            if dry_run {
                println!();
                println!("(no branches created — remove --dry-run to create them)");
            } else if parallel {
                println!();
                println!("Start an agent in each worktree:");
                for fork in &created {
                    if let Some(wt) = fork.get("worktree").and_then(|w| w.as_str()) {
                        let name = fork["name"].as_str().unwrap_or("");
                        println!("  cd {wt} && /aris  # fork: {name}");
                    }
                }
            } else {
                println!();
                println!("Start an agent on each fork:");
                for name in names {
                    println!("  git checkout autoresearch-fork-{name} && /aris");
                }
                println!();
                println!("Compare results later:");
                println!("  aris status  (shows all fork branches)");
            }
        }
    }

    Ok(())
}

fn load_config() -> Result<toml::Table, CliError> {
    let path = std::path::Path::new("autoresearch.toml");
    if !path.exists() {
        return Err(CliError::Config(
            "No autoresearch.toml found. Run `aris init` first.".into(),
        ));
    }
    let content = std::fs::read_to_string(path)?;
    toml::from_str(&content).map_err(|e| CliError::Config(e.to_string()))
}

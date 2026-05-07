use crate::errors::CliError;
use crate::git;
use crate::output::format::OutputFormat;
use crate::protocol;
use serde::Serialize;
use std::process::Command;

#[derive(Serialize)]
struct Check {
    name: String,
    passed: bool,
    message: String,
}

pub fn run(json: bool) -> Result<(), CliError> {
    let format = OutputFormat::detect(json);
    let mut checks: Vec<Check> = Vec::new();
    let mut all_passed = true;

    // 1. Git repo
    let git_ok = git::is_git_repo();
    checks.push(Check {
        name: "git_repo".into(),
        passed: git_ok,
        message: if git_ok {
            "Git repository found".into()
        } else {
            "Not a git repository. Run `git init`.".into()
        },
    });
    if !git_ok {
        all_passed = false;
    }

    // 2. autoresearch.toml exists
    let config_exists = std::path::Path::new("autoresearch.toml").exists();
    checks.push(Check {
        name: "config_file".into(),
        passed: config_exists,
        message: if config_exists {
            "autoresearch.toml found".into()
        } else {
            "No autoresearch.toml. Run `autoresearch init`.".into()
        },
    });
    if !config_exists {
        all_passed = false;
    }

    // 3. Parse config
    let config = if config_exists {
        match std::fs::read_to_string("autoresearch.toml") {
            Ok(content) => match toml::from_str::<toml::Table>(&content) {
                Ok(table) => {
                    checks.push(Check {
                        name: "config_valid".into(),
                        passed: true,
                        message: "autoresearch.toml parses correctly".into(),
                    });
                    Some(table)
                }
                Err(e) => {
                    checks.push(Check {
                        name: "config_valid".into(),
                        passed: false,
                        message: format!("Invalid TOML: {e}"),
                    });
                    all_passed = false;
                    None
                }
            },
            Err(e) => {
                checks.push(Check {
                    name: "config_valid".into(),
                    passed: false,
                    message: format!("Cannot read autoresearch.toml: {e}"),
                });
                all_passed = false;
                None
            }
        }
    } else {
        None
    };

    // 4. Required config fields
    if let Some(ref table) = config {
        for field in &[
            "target_file",
            "eval_command",
            "metric_name",
            "metric_direction",
        ] {
            let has_field = table
                .get(*field)
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty());
            checks.push(Check {
                name: format!("config_{field}"),
                passed: has_field,
                message: if has_field {
                    format!(
                        "{field} = {:?}",
                        table.get(*field).and_then(|v| v.as_str()).unwrap()
                    )
                } else {
                    format!("Missing required field: {field}")
                },
            });
            if !has_field {
                all_passed = false;
            }
        }
    }

    // 4b. Validate time_budget format
    if let Some(ref table) = config {
        if let Some(tb) = table.get("time_budget").and_then(|v| v.as_str()) {
            let valid = parse_time_budget(tb).is_some();
            checks.push(Check {
                name: "time_budget_valid".into(),
                passed: valid,
                message: if valid {
                    format!("time_budget '{tb}' is valid")
                } else {
                    format!("time_budget '{tb}' is not a valid duration. Use format like '5m', '30s', '1h'.")
                },
            });
            if !valid {
                all_passed = false;
            }
        }
    }

    // 5. Target file exists
    if let Some(ref table) = config {
        if let Some(target) = table.get("target_file").and_then(|v| v.as_str()) {
            let exists = std::path::Path::new(target).exists();
            checks.push(Check {
                name: "target_file_exists".into(),
                passed: exists,
                message: if exists {
                    format!("{target} exists")
                } else {
                    format!("{target} not found — the agent won't have a file to modify")
                },
            });
            if !exists {
                all_passed = false;
            }
        }
    }

    // 6. Eval command runs
    if let Some(ref table) = config {
        if let Some(eval_cmd) = table.get("eval_command").and_then(|v| v.as_str()) {
            // Use perl for portable timeout (macOS doesn't have `timeout`)
            let result = Command::new("sh")
                .args(["-c", eval_cmd])
                .output();

            match result {
                Ok(output) if output.status.success() => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let has_number = stdout
                        .trim()
                        .lines()
                        .last()
                        .and_then(|line| {
                            line.split_whitespace()
                                .last()
                                .and_then(|w| w.parse::<f64>().ok())
                        })
                        .is_some();

                    checks.push(Check {
                        name: "eval_runs".into(),
                        passed: true,
                        message: format!(
                            "Eval command runs successfully (exit 0, output: {} bytes)",
                            stdout.len()
                        ),
                    });

                    checks.push(Check {
                        name: "eval_metric_parseable".into(),
                        passed: has_number,
                        message: if has_number {
                            "Eval output contains a parseable number".into()
                        } else {
                            format!(
                                "Could not find a number in eval output. Last line: {:?}",
                                stdout.trim().lines().last().unwrap_or("")
                            )
                        },
                    });
                    if !has_number {
                        all_passed = false;
                    }
                }
                Ok(output) => {
                    checks.push(Check {
                        name: "eval_runs".into(),
                        passed: false,
                        message: format!(
                            "Eval command failed (exit {}). stderr: {}",
                            output.status.code().unwrap_or(-1),
                            String::from_utf8_lossy(&output.stderr)
                                .chars()
                                .take(200)
                                .collect::<String>()
                        ),
                    });
                    all_passed = false;
                }
                Err(e) => {
                    checks.push(Check {
                        name: "eval_runs".into(),
                        passed: false,
                        message: format!("Cannot execute eval command: {e}"),
                    });
                    all_passed = false;
                }
            }
        }
    }

    // 7. Experiment branch
    if git_ok {
        if let Some(ref table) = config {
            let branch = table
                .get("branch")
                .and_then(|v| v.as_str())
                .unwrap_or("autoresearch");
            let exists = git::experiment_branch_exists(branch);
            checks.push(Check {
                name: "experiment_branch".into(),
                passed: true, // Not existing is OK — it'll be created
                message: if exists {
                    format!("Branch '{branch}' exists with experiments")
                } else {
                    format!("Branch '{branch}' will be created on first run")
                },
            });
        }
    }

    // 7b. Branch divergence check
    if git_ok {
        if let Some(ref table) = config {
            let branch = table
                .get("branch")
                .and_then(|v| v.as_str())
                .unwrap_or("autoresearch");
            if git::experiment_branch_exists(branch) {
                let output = Command::new("git")
                    .args(["rev-list", "--left-right", "--count", &format!("HEAD...{branch}")])
                    .output()
                    .ok();
                if let Some(o) = output {
                    if o.status.success() {
                        let counts = String::from_utf8_lossy(&o.stdout);
                        let parts: Vec<&str> = counts.trim().split('\t').collect();
                        if parts.len() == 2 {
                            let behind: usize = parts[0].parse().unwrap_or(0);
                            let ahead: usize = parts[1].parse().unwrap_or(0);
                            let msg = format!(
                                "Branch '{branch}' is {ahead} ahead, {behind} behind current HEAD"
                            );
                            checks.push(Check {
                                name: "git_branch_divergence".into(),
                                passed: true,
                                message: msg,
                            });
                        }
                    }
                }
            }
        }
    }

    // 8. .autoresearch directory
    let log_dir = std::path::Path::new(".autoresearch").exists();
    checks.push(Check {
        name: "log_directory".into(),
        passed: true, // Will be created automatically
        message: if log_dir {
            ".autoresearch/ directory exists".into()
        } else {
            ".autoresearch/ will be created on first record".into()
        },
    });

    // 9. Stale lock file
    if git::is_loop_running() {
        checks.push(Check {
            name: "stale_lock".into(),
            passed: false,
            message: "Loop lock file exists (.autoresearch/loop.lock). If no loop is running, delete it.".into(),
        });
        all_passed = false;
    }

    // 10. program.md exists
    let program_exists = std::path::Path::new("program.md").exists();
    checks.push(Check {
        name: "program_md".into(),
        passed: program_exists,
        message: if program_exists {
            "program.md found — agent will read this for research direction".into()
        } else {
            "No program.md — agent will have no research direction guidance".into()
        },
    });
    if !program_exists {
        all_passed = false;
    }

    // 11. Guard command (if configured)
    if let Some(ref table) = config {
        if let Some(guard_cmd) = table.get("guard_command").and_then(|v| v.as_str()) {
            if !guard_cmd.is_empty() {
                let result = Command::new("sh").args(["-c", guard_cmd]).output();
                match result {
                    Ok(output) if output.status.success() => {
                        checks.push(Check {
                            name: "guard_command".into(),
                            passed: true,
                            message: format!("Guard command passes: {guard_cmd}"),
                        });
                    }
                    Ok(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        checks.push(Check {
                            name: "guard_command".into(),
                            passed: false,
                            message: format!(
                                "Guard command FAILS (exit {}): {}",
                                output.status.code().unwrap_or(-1),
                                stderr.chars().take(200).collect::<String>()
                            ),
                        });
                        all_passed = false;
                    }
                    Err(e) => {
                        checks.push(Check {
                            name: "guard_command".into(),
                            passed: false,
                            message: format!("Cannot execute guard command: {e}"),
                        });
                        all_passed = false;
                    }
                }
            }
        }
    }

    // 11b. Multi-metric weight validation (13.6)
    if let Some(ref table) = config {
        let (mut defs, is_multi) = super::pareto::parse_metrics_config(table);
        if is_multi {
            let warning = super::pareto::validate_weights(&mut defs);
            checks.push(Check {
                name: "metric_weights".into(),
                passed: warning.is_none(),
                message: warning.unwrap_or_else(|| {
                    format!("[[metrics]] weights sum to 1.0 ({} metrics configured)", defs.len())
                }),
            });
        }
    }

    // 12. Protocol version check
    {
        let cli_version = env!("CARGO_PKG_VERSION");
        let proto_version = protocol::protocol_version().unwrap_or("unknown");
        let versions_match = cli_version == proto_version;
        checks.push(Check {
            name: "protocol_version".into(),
            passed: versions_match,
            message: if versions_match {
                format!("Protocol version {proto_version} matches CLI version {cli_version}")
            } else {
                format!(
                    "Protocol version {proto_version} differs from CLI version {cli_version}. Run `aris install <target> --force` to upgrade."
                )
            },
        });
        // Not a blocker — just informational
    }

    // 13. CLI available in PATH (informational for protocol fallback mode)
    {
        let cli_available = Command::new("aris")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        checks.push(Check {
            name: "cli_in_path".into(),
            passed: cli_available,
            message: if cli_available {
                "aris CLI found in PATH — enhanced mode available".into()
            } else {
                "aris CLI not in PATH — protocol will use fallback (bash) mode. Install with: cargo install aris-cli".into()
            },
        });
        // Not a blocker
    }

    // 14. Git working tree clean
    if git_ok {
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .output()
            .ok();
        let clean = output
            .as_ref()
            .map(|o| o.stdout.is_empty())
            .unwrap_or(false);
        checks.push(Check {
            name: "git_clean".into(),
            passed: clean,
            message: if clean {
                "Working tree is clean".into()
            } else {
                "Uncommitted changes detected. Commit or stash before starting the loop.".into()
            },
        });
        // Not a blocker, just a warning
    }

    let passed_count = checks.iter().filter(|c| c.passed).count();
    let total_count = checks.len();

    match format {
        OutputFormat::Json => {
            let out = serde_json::json!({
                "status": if all_passed { "success" } else { "issues_found" },
                "data": {
                    "all_passed": all_passed,
                    "passed": passed_count,
                    "total": total_count,
                    "checks": checks,
                },
                "ready": all_passed,
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        }
        OutputFormat::Table => {
            println!("ARIS Doctor\n");
            for check in &checks {
                let icon = if check.passed { "+" } else { "!" };
                println!("  [{icon}] {}: {}", check.name, check.message);
            }
            println!();
            println!(
                "{passed_count}/{total_count} checks passed. {}",
                if all_passed {
                    "Ready to start!"
                } else {
                    "Fix the issues above before starting the loop."
                }
            );
        }
    }

    Ok(())
}

fn parse_time_budget(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let last = s.chars().last()?;
    if last.is_ascii_digit() {
        // Plain number = seconds
        return s.parse::<u64>().ok();
    }
    let num_part = &s[..s.len() - 1];
    let num: u64 = num_part.parse().ok()?;
    match last {
        's' => Some(num),
        'm' => Some(num * 60),
        'h' => Some(num * 3600),
        _ => None,
    }
}

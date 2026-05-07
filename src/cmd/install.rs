use crate::cli::InstallTarget;
use crate::errors::CliError;
use crate::output::format::OutputFormat;
use crate::skill;

pub fn run(target: &InstallTarget, force: bool, json: bool) -> Result<(), CliError> {
    let format = OutputFormat::detect(json);

    let result = skill::install(target, force);

    match result {
        Ok(installed) => match format {
            OutputFormat::Json => {
                let platform_hint = match target {
                    InstallTarget::ClaudeCode => Some("In Claude Code, type: /aris"),
                    InstallTarget::Gemini => Some("In Gemini CLI, type: /aris"),
                    InstallTarget::Codex => Some("In Codex CLI, type: /aris"),
                    InstallTarget::Cursor => {
                        Some("In Cursor, the skill loads automatically on relevant prompts")
                    }
                    InstallTarget::Windsurf => {
                        Some("In Windsurf, the skill loads automatically on relevant prompts")
                    }
                    InstallTarget::Copilot => Some("In GitHub Copilot, type: /aris"),
                    InstallTarget::Opencode | InstallTarget::Agents | InstallTarget::All => None,
                };
                let out = serde_json::json!({
                    "status": "success",
                    "installed": installed,
                    "files_count": installed.len(),
                    "next_steps": [
                        "Navigate to your project directory",
                        "Run: aris init --target-file <FILE> --eval-command '<COMMAND>'",
                        "Run: aris doctor",
                        "Tell your agent: /aris",
                    ],
                    "quick_start_prompt": "Optimize train.py to minimize loss. Use the aris workflow.",
                    "platform_hint": platform_hint,
                });
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            }
            OutputFormat::Table => {
                println!("Installed ARIS skill ({} files):", installed.len());
                for path in &installed {
                    println!("  -> {path}");
                }
                println!();
                println!("Next steps:");
                println!(
                    "  1. Navigate to your project directory"
                );
                println!(
                    "  2. Run: aris init --target-file <FILE> --eval-command '<COMMAND>'"
                );
                println!("  3. Run: aris doctor");
                println!("  4. Tell your agent: /aris");

                let platform_hint = match target {
                    InstallTarget::ClaudeCode => Some("In Claude Code, type: /aris"),
                    InstallTarget::Gemini => Some("In Gemini CLI, type: /aris"),
                    InstallTarget::Codex => Some("In Codex CLI, type: /aris"),
                    InstallTarget::Cursor => {
                        Some("In Cursor, the skill loads automatically on relevant prompts")
                    }
                    InstallTarget::Windsurf => {
                        Some("In Windsurf, the skill loads automatically on relevant prompts")
                    }
                    InstallTarget::Copilot => Some("In GitHub Copilot, type: /aris"),
                    InstallTarget::Opencode | InstallTarget::Agents | InstallTarget::All => None,
                };
                if let Some(hint) = platform_hint {
                    println!();
                    println!("Quick start: {hint}");
                }
                println!();
                println!("Copy-paste starter prompt:");
                println!(
                    "  \"Optimize train.py to minimize loss. Use the aris workflow.\""
                );
            }
        },
        Err(CliError::AlreadyInstalled(path)) => match format {
            OutputFormat::Json => {
                let out = serde_json::json!({
                    "status": "already_installed",
                    "message": format!("Skill already installed at {path}"),
                    "path": path,
                    "suggestion": "Use `aris install <target> --force` to reinstall with the latest skill version."
                });
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            }
            OutputFormat::Table => {
                println!("Skill already installed at {path}");
                println!(
                    "Use --force to reinstall, or update CLI version first for latest protocol."
                );
            }
        },
        Err(e) => return Err(e),
    }

    Ok(())
}

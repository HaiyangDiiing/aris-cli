pub mod templates;

use crate::cli::InstallTarget;
use crate::errors::CliError;
use crate::protocol;
use std::fs;
use std::path::PathBuf;

/// Install the ARIS skill for a specific target platform.
///
/// Deploys all protocol files: SKILL.md + references/*.md + commands/*.md
pub fn install(target: &InstallTarget, force: bool) -> Result<Vec<String>, CliError> {
    match target {
        InstallTarget::All => {
            let mut installed = Vec::new();
            let targets = [
                InstallTarget::ClaudeCode,
                InstallTarget::Gemini,
                InstallTarget::Codex,
                InstallTarget::Opencode,
                InstallTarget::Copilot,
                InstallTarget::Cursor,
                InstallTarget::Windsurf,
                InstallTarget::Agents,
            ];
            for t in &targets {
                match install_platform(t, force) {
                    Ok(paths) => installed.extend(paths),
                    Err(CliError::AlreadyInstalled(p)) => {
                        installed.push(format!("{p} (already installed)"));
                    }
                    Err(e) => {
                        eprintln!("  warning: failed to install for {t:?}: {e}");
                    }
                }
            }
            Ok(installed)
        }
        other => install_platform(other, force),
    }
}

/// Install all protocol files for a single platform.
fn install_platform(target: &InstallTarget, force: bool) -> Result<Vec<String>, CliError> {
    let (skill_dir, cmd_dir) = platform_dirs(target)?;

    // Check if already installed (same version) unless --force
    if !force {
        let skill_md_path = skill_dir.join("SKILL.md");
        if skill_md_path.exists() {
            if let Ok(existing) = fs::read_to_string(&skill_md_path) {
                if existing.contains(&format!("version: \"{}\"", env!("CARGO_PKG_VERSION"))) {
                    return Err(CliError::AlreadyInstalled(
                        skill_md_path.to_string_lossy().to_string(),
                    ));
                }
            }
        }
    }

    let mut installed = Vec::new();

    // Write skill files (SKILL.md + references/*.md)
    for file in protocol::skill_files() {
        let path = skill_dir.join(file.rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // For SKILL.md, apply platform-specific frontmatter
        let content = if file.rel_path == "SKILL.md" {
            apply_platform_frontmatter(file.content, target)
        } else {
            file.content.to_string()
        };

        fs::write(&path, content)?;
        installed.push(path.to_string_lossy().to_string());
    }

    // Write command dispatcher files
    for file in protocol::command_files() {
        let path = cmd_dir.join(file.rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, file.content)?;
        installed.push(path.to_string_lossy().to_string());
    }

    Ok(installed)
}

/// Get the skill and command directories for a platform.
fn platform_dirs(target: &InstallTarget) -> Result<(PathBuf, PathBuf), CliError> {
    let (skill_dir, cmd_dir) = match target {
        InstallTarget::ClaudeCode => {
            let home = home_dir()?;
            (
                home.join(".claude/skills/aris"),
                home.join(".claude/commands"),
            )
        }
        InstallTarget::Gemini => {
            let home = home_dir()?;
            (
                home.join(".gemini/skills/aris"),
                home.join(".gemini/commands"),
            )
        }
        InstallTarget::Codex => {
            let home = home_dir()?;
            (
                home.join(".codex/skills/aris"),
                home.join(".codex/commands"),
            )
        }
        InstallTarget::Opencode => {
            let home = home_dir()?;
            (
                home.join(".config/opencode/skills/aris"),
                home.join(".config/opencode/commands"),
            )
        }
        InstallTarget::Copilot => (
            PathBuf::from(".github/skills/aris"),
            PathBuf::from(".github/commands"),
        ),
        InstallTarget::Cursor => (
            PathBuf::from(".cursor/skills/aris"),
            PathBuf::from(".cursor/commands"),
        ),
        InstallTarget::Windsurf => (
            PathBuf::from(".windsurf/skills/aris"),
            PathBuf::from(".windsurf/commands"),
        ),
        InstallTarget::Agents => (
            PathBuf::from(".agents/skills/aris"),
            PathBuf::from(".agents/commands"),
        ),
        InstallTarget::All => unreachable!(),
    };
    Ok((skill_dir, cmd_dir))
}

/// Apply platform-specific YAML frontmatter to SKILL.md.
///
/// Some platforms support different frontmatter fields. We adjust the embedded
/// SKILL.md content to match each platform's requirements.
fn apply_platform_frontmatter(content: &str, target: &InstallTarget) -> String {
    match target {
        // Claude Code: supports all fields (argument-hint, user-invocable, metadata)
        InstallTarget::ClaudeCode | InstallTarget::Copilot => content.to_string(),

        // Gemini CLI: ONLY name + description allowed
        InstallTarget::Gemini => strip_extra_frontmatter(content),

        // Codex CLI: name + description only
        InstallTarget::Codex => strip_extra_frontmatter(content),

        // Cursor: supports metadata but not argument-hint
        InstallTarget::Cursor => {
            let mut result = String::new();
            let mut in_frontmatter = false;
            let mut frontmatter_end = false;
            for line in content.lines() {
                if line.trim() == "---" && !in_frontmatter {
                    in_frontmatter = true;
                    result.push_str(line);
                    result.push('\n');
                } else if line.trim() == "---" && in_frontmatter {
                    frontmatter_end = true;
                    in_frontmatter = false;
                    result.push_str(line);
                    result.push('\n');
                } else if in_frontmatter {
                    // Skip argument-hint and user-invocable for Cursor
                    if line.trim().starts_with("argument-hint:")
                        || line.trim().starts_with("user-invocable:")
                    {
                        continue;
                    }
                    result.push_str(line);
                    result.push('\n');
                } else {
                    if frontmatter_end || !in_frontmatter {
                        result.push_str(line);
                        result.push('\n');
                    }
                }
            }
            result
        }

        // Windsurf, OpenCode, Agents: name + description only
        InstallTarget::Windsurf | InstallTarget::Opencode | InstallTarget::Agents => {
            strip_extra_frontmatter(content)
        }

        InstallTarget::All => unreachable!(),
    }
}

/// Strip all frontmatter fields except name and description.
fn strip_extra_frontmatter(content: &str) -> String {
    let mut result = String::new();
    let mut in_frontmatter = false;
    let mut in_description = false;

    for line in content.lines() {
        if line.trim() == "---" && !in_frontmatter {
            in_frontmatter = true;
            result.push_str(line);
            result.push('\n');
        } else if line.trim() == "---" && in_frontmatter {
            in_frontmatter = false;
            in_description = false;
            result.push_str(line);
            result.push('\n');
        } else if in_frontmatter {
            if line.starts_with("name:") || line.starts_with("description:") {
                in_description = line.starts_with("description:");
                result.push_str(line);
                result.push('\n');
            } else if in_description && (line.starts_with("  ") || line.starts_with('\t')) {
                // Continuation of multi-line description
                result.push_str(line);
                result.push('\n');
            } else {
                in_description = false;
                // Skip other frontmatter fields
            }
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}

fn home_dir() -> Result<PathBuf, CliError> {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .ok_or_else(|| CliError::Config("Could not determine home directory".to_string()))
}

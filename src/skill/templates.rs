use crate::protocol;

/// Platform-specific SKILL.md generator.
///
/// Returns the embedded SKILL.md content with platform-specific frontmatter adjustments.
/// The actual content now lives in `skills/aris/SKILL.md` and is embedded via
/// `include_str!()` in the protocol module.
pub fn skill_md(platform: &str) -> String {
    let content = protocol::SKILL_MD;
    let desc = "Autonomous experiment loop — iteratively improve any measurable metric by modifying code, evaluating results, and keeping improvements. Use when the user says \"aris\", \"autoresearch\", \"start experiments\", \"optimize this\", \"run the loop\", or wants autonomous iteration on any measurable goal. Reads autoresearch.toml for config. Run `aris init` first.";

    match platform {
        // Claude Code: supports argument-hint, user-invocable, allowed-tools
        "claude-code" | "copilot" => content.to_string(),

        // Gemini CLI: ONLY name + description allowed
        "gemini" | "codex" | "windsurf" | "opencode" | "agents" => {
            format!(
                r#"---
name: aris
description: >
  {desc}
---
{body}"#,
                desc = desc,
                body = strip_frontmatter(content),
            )
        }

        // Cursor Skills: supports metadata but not argument-hint
        "cursor" => {
            format!(
                r#"---
name: aris
description: >
  {desc}
metadata:
  version: "{version}"
---
{body}"#,
                desc = desc,
                version = env!("CARGO_PKG_VERSION"),
                body = strip_frontmatter(content),
            )
        }

        // Default: base spec only
        _ => {
            format!(
                r#"---
name: aris
description: >
  {desc}
---
{body}"#,
                desc = desc,
                body = strip_frontmatter(content),
            )
        }
    }
}

/// Returns the full loop protocol as plain text — for `aris guide` command.
///
/// This is the autonomous-loop-protocol.md content, the core methodology.
/// Agents without skills installed can get the full playbook via this command.
pub fn guide_text() -> String {
    protocol::AUTONOMOUS_LOOP_PROTOCOL.to_string()
}

/// Strip YAML frontmatter from Markdown content.
fn strip_frontmatter(content: &str) -> &str {
    let mut lines = content.lines();
    if let Some(first) = lines.next() {
        if first.trim() == "---" {
            // Find the closing ---
            let mut idx = first.len() + 1; // +1 for newline
            for line in lines {
                idx += line.len() + 1;
                if line.trim() == "---" {
                    return &content[idx..];
                }
            }
        }
    }
    content
}

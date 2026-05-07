//! Protocol module — compile-time embedded Markdown protocol files.
//!
//! All ARIS protocol files (SKILL.md, references/*.md, commands/*.md) are embedded
//! in the binary via `include_str!()`. This ensures version coherence: the protocol
//! and CLI are always from the same release.
//!
//! The Markdown files in `skills/aris/` and `commands/aris/` are the source of truth.
//! Edit those files, not this module, to change protocol behavior.

// -- SKILL.md (master index) --
pub const SKILL_MD: &str = include_str!("../../skills/aris/SKILL.md");

// -- Reference files --
pub const AUTONOMOUS_LOOP_PROTOCOL: &str =
    include_str!("../../skills/aris/references/autonomous-loop-protocol.md");
pub const CORE_PRINCIPLES: &str =
    include_str!("../../skills/aris/references/core-principles.md");
pub const RESULTS_LOGGING: &str =
    include_str!("../../skills/aris/references/results-logging.md");
pub const PLAN_WORKFLOW: &str =
    include_str!("../../skills/aris/references/plan-workflow.md");
pub const DEBUG_WORKFLOW: &str =
    include_str!("../../skills/aris/references/debug-workflow.md");
pub const FIX_WORKFLOW: &str =
    include_str!("../../skills/aris/references/fix-workflow.md");

// -- Command dispatcher files --
pub const CMD_ARIS: &str = include_str!("../../commands/aris.md");
pub const CMD_ARIS_PLAN: &str = include_str!("../../commands/aris/plan.md");
pub const CMD_ARIS_DEBUG: &str = include_str!("../../commands/aris/debug.md");
pub const CMD_ARIS_FIX: &str = include_str!("../../commands/aris/fix.md");

/// All protocol files with their relative install paths.
///
/// Used by the install command to write all files to the target platform.
pub struct ProtocolFile {
    /// Relative path under the skill/command directory (e.g., "SKILL.md", "references/core-principles.md")
    pub rel_path: &'static str,
    /// File content (embedded at compile time)
    pub content: &'static str,
}

/// Returns all skill files (SKILL.md + references/) for installation.
pub fn skill_files() -> Vec<ProtocolFile> {
    vec![
        ProtocolFile {
            rel_path: "SKILL.md",
            content: SKILL_MD,
        },
        ProtocolFile {
            rel_path: "references/autonomous-loop-protocol.md",
            content: AUTONOMOUS_LOOP_PROTOCOL,
        },
        ProtocolFile {
            rel_path: "references/core-principles.md",
            content: CORE_PRINCIPLES,
        },
        ProtocolFile {
            rel_path: "references/results-logging.md",
            content: RESULTS_LOGGING,
        },
        ProtocolFile {
            rel_path: "references/plan-workflow.md",
            content: PLAN_WORKFLOW,
        },
        ProtocolFile {
            rel_path: "references/debug-workflow.md",
            content: DEBUG_WORKFLOW,
        },
        ProtocolFile {
            rel_path: "references/fix-workflow.md",
            content: FIX_WORKFLOW,
        },
    ]
}

/// Returns all command dispatcher files for installation.
pub fn command_files() -> Vec<ProtocolFile> {
    vec![
        ProtocolFile {
            rel_path: "aris.md",
            content: CMD_ARIS,
        },
        ProtocolFile {
            rel_path: "aris/plan.md",
            content: CMD_ARIS_PLAN,
        },
        ProtocolFile {
            rel_path: "aris/debug.md",
            content: CMD_ARIS_DEBUG,
        },
        ProtocolFile {
            rel_path: "aris/fix.md",
            content: CMD_ARIS_FIX,
        },
    ]
}

/// Extract the version from SKILL.md frontmatter.
pub fn protocol_version() -> Option<&'static str> {
    // Parse version from YAML frontmatter: `  version: "0.3.3"`
    for line in SKILL_MD.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("version:") {
            return Some(
                trimmed
                    .trim_start_matches("version:")
                    .trim()
                    .trim_matches('"'),
            );
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_md_not_empty() {
        assert!(!SKILL_MD.is_empty());
        assert!(SKILL_MD.contains("name: aris"));
    }

    #[test]
    fn test_all_references_not_empty() {
        assert!(!AUTONOMOUS_LOOP_PROTOCOL.is_empty());
        assert!(!CORE_PRINCIPLES.is_empty());
        assert!(!RESULTS_LOGGING.is_empty());
        assert!(!PLAN_WORKFLOW.is_empty());
        assert!(!DEBUG_WORKFLOW.is_empty());
        assert!(!FIX_WORKFLOW.is_empty());
    }

    #[test]
    fn test_all_commands_not_empty() {
        assert!(!CMD_ARIS.is_empty());
        assert!(!CMD_ARIS_PLAN.is_empty());
        assert!(!CMD_ARIS_DEBUG.is_empty());
        assert!(!CMD_ARIS_FIX.is_empty());
    }

    #[test]
    fn test_skill_files_count() {
        assert_eq!(skill_files().len(), 7);
    }

    #[test]
    fn test_command_files_count() {
        assert_eq!(command_files().len(), 4);
    }

    #[test]
    fn test_protocol_version() {
        let version = protocol_version();
        assert!(version.is_some());
        // Version should match Cargo.toml
        assert_eq!(version.unwrap(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_autonomous_loop_has_phases() {
        assert!(AUTONOMOUS_LOOP_PROTOCOL.contains("Phase 0"));
        assert!(AUTONOMOUS_LOOP_PROTOCOL.contains("Phase 1"));
        assert!(AUTONOMOUS_LOOP_PROTOCOL.contains("Phase 8"));
    }

    #[test]
    fn test_skill_md_has_required_sections() {
        assert!(SKILL_MD.contains("Interactive Setup Gate"));
        assert!(SKILL_MD.contains("Subcommands"));
        assert!(SKILL_MD.contains("Core Principles"));
        assert!(SKILL_MD.contains("CLI Command Reference"));
    }
}

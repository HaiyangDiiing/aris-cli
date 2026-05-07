use crate::errors::CliError;
use crate::output::format::OutputFormat;

pub fn run(json: bool) -> Result<(), CliError> {
    let format = OutputFormat::detect(json);

    let info = serde_json::json!({
        "name": "aris",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "ARIS CLI — install skills, track experiments, optimize any measurable metric",
        "commands": {
            "install <target>": "Install skill into an AI agent (claude-code, codex, opencode, cursor, windsurf, gemini, all)",
            "init": "Scaffold project (--target-file, --eval-command, --metric-name, --metric-direction, --time-budget, --branch, --template)",
            "doctor": "Pre-flight check (14 checks). Run FIRST before any experiment loop.",
            "record": "Record experiment (--metric <V> --status kept|discarded|baseline --summary '<M>' [--guard-status pass|fail|skipped]). Handles JSONL, run numbering, deltas, guard checks, reward-hacking detection.",
            "log [-n N]": "Show experiment history with metrics and status",
            "best [--pareto]": "Show best experiment + diff from baseline + improvement %. Use --pareto for multi-metric Pareto front.",
            "diff <a> <b>": "Compare two experiments by run number",
            "status": "Project state, best metric, loop running status",
            "export": "Export history (--format csv|json|jsonl, -o file)",
            "fork <names...> [--parallel]": "Create parallel exploration branches. --parallel creates git worktrees for true parallel execution.",
            "merge-best [--cleanup]": "Compare all fork branches, rank by metric, identify winner. --cleanup removes worktrees.",
            "review": "Generate cross-model review prompt with stuck detection and failure pattern analysis",
            "watch": "Live terminal dashboard (-i seconds for refresh interval)",
            "report [-o file]": "Generate markdown research report",
            "learn": "Analyze experiment history and generate lessons.md with effective strategies, dead ends, and recommendations",
            "verify --run N": "Re-run eval at experiment N's commit and compare metric (5% drift threshold)",
            "retrospective": "Generate research retrospective with timeline, metrics analysis, key decisions (--format markdown|latex-data)",
            "pr [--base X] [--draft]": "Create GitHub PR via gh CLI with structured experiment summary",
            "pool <start|status|rebalance|stop>": "Multi-agent pool orchestration: manage parallel experiment agents across worktrees",
            "agent-info": "This command — capabilities + best practices for agents",
        },
        "workflow": {
            "step_1": "aris doctor (validate environment)",
            "step_2": "Read autoresearch.toml + program.md",
            "step_3": "git checkout -b <branch>",
            "step_4": "Record baseline: aris record --metric <V> --status baseline --summary 'Initial baseline'",
            "step_5": "Loop: hypothesize → implement → commit → eval → record (kept/discarded) → repeat",
            "step_6": "When done: aris report",
        },
        "best_practices": {
            "experiment_order": [
                "1. Hyperparameters first (learning rate, batch size, weight decay) — lowest risk, highest signal",
                "2. Regularization second (dropout, decay schedules, gradient clipping)",
                "3. Architecture changes third (high variance — most fail, but winners are big)",
                "4. Exotic/novel ideas last (papers, unconventional techniques)",
            ],
            "when_stuck": [
                "After 5+ consecutive discards, you are in a local minimum.",
                "Run: aris review (generates cross-model analysis prompt)",
                "Try the OPPOSITE of what you've been doing",
                "Remove something — the best optimization is often removal",
                "Fork with: aris fork approach-a approach-b",
            ],
            "anti_patterns": [
                "DO NOT combine multiple changes in one experiment",
                "DO NOT skip the commit before eval",
                "DO NOT write to experiments.jsonl directly — use aris record",
                "DO NOT repeat a failed approach without a new angle",
                "DO NOT ignore the reward-hacking warning from record",
            ],
            "reward_hacking": "The metric can lie. Watch for overfitting the eval, secondary costs (time/memory), and changes that game the metric without real improvement.",
        },
        "supported_targets": ["claude-code", "codex", "opencode", "cursor", "windsurf"],
        "config_file": "autoresearch.toml",
        "experiment_log": ".autoresearch/experiments.jsonl",
        "global_flags": ["--json", "--help", "--version"],
    });

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&info).unwrap());
        }
        OutputFormat::Table => {
            println!("aris v{}", env!("CARGO_PKG_VERSION"));
            println!();
            println!("Commands:");
            if let Some(cmds) = info.get("commands").and_then(|c| c.as_object()) {
                for (name, desc) in cmds {
                    println!("  {name:20} {}", desc.as_str().unwrap_or(""));
                }
            }
            println!();
            println!("Workflow: doctor → init → baseline → loop (hypothesize → implement → commit → eval → record) → report");
            println!();
            println!("Best practices:");
            println!("  1. Hyperparameters first, architecture last");
            println!("  2. One atomic change per experiment");
            println!("  3. After 5+ discards → run `aris review` or fork");
            println!("  4. The metric can lie — watch for reward hacking");
            println!("  5. The best optimization is often removal");
            println!();
            println!("Use --json for full machine-readable output with all best practices.");
        }
    }

    Ok(())
}

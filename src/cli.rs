use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "aris",
    version,
    about = "ARIS CLI — Autonomous Research Iteration System. Optimize any metric with autonomous experiments.",
    long_about = "ARIS CLI brings Karpathy's autoresearch pattern to any project.\n\n\
        Workflow: doctor → init → baseline → loop (hypothesize → implement → commit → eval → record) → report\n\n\
        Quick start:\n  \
        1. aris install claude-code  (or codex, opencode, cursor, windsurf, all)\n  \
        2. aris init --target-file <F> --eval-command <C>\n  \
        3. aris doctor  (validate before starting)\n  \
        4. Tell your agent: /aris\n\n\
        Best practices: hyperparameters first, one change per experiment, fork when stuck.\n\
        Run `aris agent-info --json` for full machine-readable capabilities + strategy guide."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Output as JSON (auto-enabled when piped)
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Install the ARIS skill into an AI coding agent
    Install {
        /// Target agent platform
        #[arg(value_enum)]
        target: InstallTarget,

        /// Force reinstall even if same version is already installed
        #[arg(long)]
        force: bool,
    },

    /// Initialize aris in the current project
    Init {
        /// Target file the agent may modify
        #[arg(long)]
        target_file: Option<String>,

        /// Eval command that produces the metric
        #[arg(long)]
        eval_command: Option<String>,

        /// Metric name (e.g., val_bpb, accuracy, p99_latency)
        #[arg(long, default_value = "metric")]
        metric_name: String,

        /// Metric direction: lower or higher
        #[arg(long, default_value = "lower")]
        metric_direction: String,

        /// Time budget per experiment (e.g., 5m, 30s)
        #[arg(long, default_value = "5m")]
        time_budget: String,

        /// Git branch for experiments
        #[arg(long, default_value = "autoresearch")]
        branch: String,

        /// Domain template (ml-training, web-perf, test-coverage, build-speed, code-quality, quantum-vqe)
        #[arg(long)]
        template: Option<String>,
    },

    /// Record an experiment result (for agent use)
    Record {
        /// Metric value(s) from this experiment. Optional for error statuses (crash, no-op, hook-blocked, metric-error).
        /// Multiple values compute median automatically. Use = for negatives: --metric=-0.5
        #[arg(long, allow_negative_numbers = true, num_args = 0..)]
        metric: Vec<f64>,

        /// Status: baseline, kept, discarded, crash, no-op, hook-blocked, metric-error
        #[arg(long)]
        status: String,

        /// Summary of what was tried
        #[arg(long)]
        summary: String,

        /// Guard check result: pass, fail, skipped
        #[arg(long)]
        guard_status: Option<String>,

        /// Preview what would be recorded without writing
        #[arg(long)]
        dry_run: bool,
    },

    /// Show experiment history from git log
    Log {
        /// Maximum number of entries to show
        #[arg(short = 'n', long, default_value = "20")]
        limit: usize,
    },

    /// Show the best experiment and its diff from baseline
    Best {
        /// Show Pareto front (non-dominated set) for multi-metric
        #[arg(long)]
        pareto: bool,
    },

    /// Compare two experiments by run number
    Diff {
        /// First run number
        run_a: usize,
        /// Second run number
        run_b: usize,
    },

    /// Check if an aris loop is currently running
    Status,

    /// Export experiment history
    Export {
        /// Export format
        #[arg(long, value_enum, default_value = "csv")]
        format: ExportFormat,

        /// Output file path (stdout if not specified)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Pre-flight check before starting an experiment loop
    Doctor,

    /// Fork experiments into parallel branches for multi-direction exploration
    Fork {
        /// Names for each fork (creates autoresearch/<name> branches)
        #[arg(required = true)]
        names: Vec<String>,

        /// Create git worktrees for true parallel execution
        #[arg(long)]
        parallel: bool,

        /// Preview what branches would be created without creating them
        #[arg(long)]
        dry_run: bool,
    },

    /// Generate a cross-model review prompt from experiment history
    Review,

    /// Live terminal dashboard — watch experiments as they happen
    Watch {
        /// Refresh interval in seconds
        #[arg(short, long, default_value = "2")]
        interval: u64,
    },

    /// Compare fork branches and merge the best one back
    MergeBest {
        /// Remove worktrees after merge
        #[arg(long)]
        cleanup: bool,
    },

    /// Generate a markdown report of the research session
    Report {
        /// Output file path (stdout if not specified)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Analyze experiment history and generate lessons learned
    Learn,

    /// Verify an experiment's metric by re-running eval at that commit
    Verify {
        /// Run number to verify
        #[arg(long)]
        run: usize,
    },

    /// Generate a research retrospective from experiment history
    Retrospective {
        /// Output format: markdown (default) or latex-data
        #[arg(long, default_value = "markdown")]
        format: String,

        /// Maximum number of experiments to include (default 100)
        #[arg(long)]
        limit: Option<usize>,

        /// Include all experiments (override --limit)
        #[arg(long)]
        full: bool,

        /// Only include experiments since this date (RFC3339)
        #[arg(long)]
        since: Option<String>,
    },

    /// Create a GitHub PR from the experiment branch with structured summary
    Pr {
        /// Target base branch (defaults to repo default)
        #[arg(long)]
        base: Option<String>,

        /// Create PR as draft
        #[arg(long)]
        draft: bool,
    },

    /// Manage a pool of parallel experiment agents
    Pool {
        /// Subcommand: start, status, rebalance, stop
        #[arg(value_enum)]
        action: PoolAction,

        /// Number of concurrent slots (for start)
        #[arg(long)]
        slots: Option<usize>,

        /// Fork names (comma-separated, for start)
        #[arg(long, value_delimiter = ',')]
        forks: Vec<String>,

        /// Remove worktrees on stop
        #[arg(long)]
        cleanup: bool,
    },

    /// Print the full ARIS methodology guide (works without skill installed)
    Guide,

    /// Show CLI capabilities for agent discovery
    AgentInfo,

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for (bash, zsh, fish, powershell, elvish)
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(ValueEnum, Clone, Debug)]
pub enum InstallTarget {
    /// Claude Code (~/.claude/skills/)
    ClaudeCode,
    /// Gemini CLI (~/.gemini/skills/)
    Gemini,
    /// Codex CLI (~/.codex/skills/)
    Codex,
    /// OpenCode (~/.config/opencode/skills/)
    Opencode,
    /// GitHub Copilot (.github/skills/)
    Copilot,
    /// Cursor (.cursor/skills/)
    Cursor,
    /// Windsurf (.windsurf/skills/)
    Windsurf,
    /// Universal .agents/skills/ (Augment, Goose, Roo, etc.)
    Agents,
    /// Install into all supported agents
    All,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum PoolAction {
    /// Start a pool with slots and forks
    Start,
    /// Show pool status and fork health
    Status,
    /// Rebalance: deactivate stuck forks, activate queued ones
    Rebalance,
    /// Stop pool: compare results, find winner
    Stop,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum ExportFormat {
    Csv,
    Json,
    Jsonl,
}

pub fn parse() -> Cli {
    Cli::parse()
}

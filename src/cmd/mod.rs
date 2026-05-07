mod agent_info;
mod best;
mod diff;
mod doctor;
mod export;
mod fork;
mod guide;
mod init;
mod install;
mod learn;
mod log;
mod merge_best;
pub(crate) mod pareto;
mod pool;
mod pr;
mod record;
mod report;
mod retrospective;
mod review;
mod status;
mod strategy;
pub(crate) mod templates;
mod verify;
mod watch;

use clap::CommandFactory;

use crate::cli::{Cli, Commands};
use crate::errors::CliError;

pub fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Commands::Install { target, force } => install::run(&target, force, cli.json),
        Commands::Init {
            target_file,
            eval_command,
            metric_name,
            metric_direction,
            time_budget,
            branch,
            template,
        } => init::run(
            target_file,
            eval_command,
            &metric_name,
            &metric_direction,
            &time_budget,
            &branch,
            template.as_deref(),
            cli.json,
        ),
        Commands::Record {
            metric,
            status,
            summary,
            guard_status,
            dry_run,
        } => record::run(metric, &status, &summary, guard_status.as_deref(), cli.json, dry_run),
        Commands::Log { limit } => log::run(limit, cli.json),
        Commands::Best { pareto } => best::run(cli.json, pareto),
        Commands::Diff { run_a, run_b } => diff::run(run_a, run_b, cli.json),
        Commands::Status => status::run(cli.json),
        Commands::Export { format, output } => export::run(&format, output.as_deref(), cli.json),
        Commands::Doctor => doctor::run(cli.json),
        Commands::Fork { names, parallel, dry_run } => fork::run(&names, parallel, cli.json, dry_run),
        Commands::Review => review::run(cli.json),
        Commands::Watch { interval } => watch::run(interval),
        Commands::MergeBest { cleanup } => merge_best::run(cli.json, cleanup),
        Commands::Report { output } => report::run(output.as_deref(), cli.json),
        Commands::Learn => learn::run(cli.json),
        Commands::Retrospective { format, limit, full, since } => {
            let limit = if full { None } else { limit.or(Some(100)) };
            retrospective::run(&format, limit, since.as_deref(), cli.json)
        }
        Commands::Pool { action, slots, forks, cleanup } => {
            let action_str = match action {
                crate::cli::PoolAction::Start => "start",
                crate::cli::PoolAction::Status => "status",
                crate::cli::PoolAction::Rebalance => "rebalance",
                crate::cli::PoolAction::Stop => "stop",
            };
            pool::run(action_str, slots, &forks, cleanup, cli.json)
        }
        Commands::Verify { run } => verify::run(run, cli.json),
        Commands::Pr { base, draft } => pr::run(base.as_deref(), draft, cli.json),
        Commands::Guide => guide::run(cli.json),
        Commands::AgentInfo => agent_info::run(cli.json),
        Commands::Completions { shell } => {
            let mut cmd = crate::cli::Cli::command();
            clap_complete::generate(shell, &mut cmd, "aris", &mut std::io::stdout());
            Ok(())
        }
    }
}

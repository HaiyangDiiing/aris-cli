use crate::errors::CliError;
use crate::git;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Sparkline, Table},
    Terminal,
};
use std::io;
use std::time::Duration;

struct DashboardState {
    experiments: Vec<git::Experiment>,
    total: usize,
    kept_count: usize,
    discarded_count: usize,
    best_metric: Option<f64>,
    baseline_metric: Option<f64>,
    kept_metrics: Vec<f64>,
    fork_branches: Vec<(String, usize)>,
    loop_running: bool,
    loop_iteration: Option<u64>,
    loop_started: Option<String>,
    new_experiments: usize,
}

impl DashboardState {
    fn load(branch: &str, lower_is_better: bool, last_count: &mut usize) -> Self {
        let experiments = if git::experiment_branch_exists(branch) {
            git::parse_experiments(branch, 10000).unwrap_or_default()
        } else {
            vec![]
        };

        let total = experiments.len();
        let kept_count = experiments
            .iter()
            .filter(|e| e.status == git::ExperimentStatus::Kept)
            .count();
        let discarded_count = experiments
            .iter()
            .filter(|e| e.status == git::ExperimentStatus::Discarded)
            .count();

        let best_metric = experiments
            .iter()
            .filter(|e| {
                e.metric.is_some()
                    && (e.status == git::ExperimentStatus::Kept
                        || e.status == git::ExperimentStatus::Baseline)
            })
            .filter_map(|e| e.metric)
            .reduce(if lower_is_better {
                f64::min
            } else {
                f64::max
            });

        let baseline_metric = experiments
            .iter()
            .find(|e| e.status == git::ExperimentStatus::Baseline)
            .and_then(|e| e.metric);

        let kept_metrics: Vec<f64> = experiments
            .iter()
            .rev()
            .filter(|e| {
                e.metric.is_some()
                    && (e.status == git::ExperimentStatus::Kept
                        || e.status == git::ExperimentStatus::Baseline)
            })
            .filter_map(|e| e.metric)
            .collect();

        let loop_running = git::is_loop_running();
        let loop_state = git::loop_state();
        let loop_iteration = loop_state
            .as_ref()
            .and_then(|s| s.get("iteration").and_then(|i| i.as_u64()));
        let loop_started = loop_state
            .as_ref()
            .and_then(|s| s.get("started_at").and_then(|t| t.as_str()).map(|s| s.to_string()));

        let fork_branches = list_fork_branches()
            .into_iter()
            .map(|fb| {
                let count = git::parse_experiments(&fb, 10000)
                    .map(|e| e.len())
                    .unwrap_or(0);
                (fb, count)
            })
            .collect();

        let new_experiments = if total > *last_count && *last_count > 0 {
            total - *last_count
        } else {
            0
        };
        *last_count = total;

        DashboardState {
            experiments,
            total,
            kept_count,
            discarded_count,
            best_metric,
            baseline_metric,
            kept_metrics,
            fork_branches,
            loop_running,
            loop_iteration,
            loop_started,
            new_experiments,
        }
    }
}

pub fn run(interval: u64) -> Result<(), CliError> {
    let config = load_config()?;
    let branch = config
        .get("branch")
        .and_then(|v| v.as_str())
        .unwrap_or("autoresearch");
    // Support multi-metric display
    let (metric_defs, _is_multi) = super::pareto::parse_metrics_config(&config);
    let metric_name = if metric_defs.len() > 1 {
        // Show all metric names
        metric_defs.iter().map(|d| d.name.as_str()).collect::<Vec<_>>().join(", ")
    } else {
        config
            .get("metric_name")
            .and_then(|v| v.as_str())
            .unwrap_or("metric")
            .to_string()
    };
    let metric_name = metric_name.as_str();
    let metric_direction = config
        .get("metric_direction")
        .and_then(|v| v.as_str())
        .unwrap_or("lower");
    let target_file = config
        .get("target_file")
        .and_then(|v| v.as_str())
        .unwrap_or("?");

    let lower_is_better = metric_direction != "higher";
    let mut last_count = 0usize;

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(
        &mut terminal,
        interval,
        branch,
        metric_name,
        metric_direction,
        target_file,
        lower_is_better,
        &mut last_count,
    );

    // Cleanup — always runs even on error
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    interval: u64,
    branch: &str,
    metric_name: &str,
    metric_direction: &str,
    target_file: &str,
    lower_is_better: bool,
    last_count: &mut usize,
) -> Result<(), CliError> {
    loop {
        let state = DashboardState::load(branch, lower_is_better, last_count);

        terminal.draw(|frame| {
            render_dashboard(
                frame,
                &state,
                interval,
                metric_name,
                metric_direction,
                target_file,
                lower_is_better,
            );
        })?;

        // Non-blocking poll: check for 'q' key or wait for interval
        if event::poll(Duration::from_secs(interval))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press
                    && (key.code == KeyCode::Char('q') || key.code == KeyCode::Esc)
                {
                    break;
                }
            }
        }
    }
    Ok(())
}

fn render_dashboard(
    frame: &mut ratatui::Frame,
    state: &DashboardState,
    interval: u64,
    metric_name: &str,
    metric_direction: &str,
    target_file: &str,
    lower_is_better: bool,
) {
    let area = frame.area();

    // Determine layout chunks based on whether we have sparkline data and forks
    let has_sparkline = state.kept_metrics.len() >= 2;
    let has_forks = !state.fork_branches.is_empty();

    let mut constraints = vec![
        Constraint::Length(3),  // Header
        Constraint::Length(5),  // Stats
    ];
    if has_sparkline {
        constraints.push(Constraint::Length(4)); // Sparkline
    }
    constraints.push(Constraint::Min(8)); // Table (takes remaining space)
    if has_forks {
        constraints.push(Constraint::Length(2 + state.fork_branches.len() as u16));
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut chunk_idx = 0;

    // -- Header --
    let loop_status = if state.loop_running {
        let iter_str = state
            .loop_iteration
            .map(|i| format!(" #{i}"))
            .unwrap_or_default();
        format!("  LOOP RUNNING{iter_str}")
    } else {
        "  Loop not running".to_string()
    };

    let header_text = vec![
        Line::from(vec![
            Span::styled(
                format!("  target: {target_file}  metric: {metric_name} ({metric_direction} is better)"),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("  "),
            if state.loop_running {
                Span::styled(loop_status.clone(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            } else {
                Span::styled(loop_status.clone(), Style::default().fg(Color::DarkGray))
            },
        ]),
    ];

    let header = Paragraph::new(header_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(
                " AUTORESEARCH WATCH  ({}s refresh, q to quit) ",
                interval
            ))
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    );
    frame.render_widget(header, chunks[chunk_idx]);
    chunk_idx += 1;

    // -- Stats --
    let mut stats_lines = vec![
        Line::from(vec![
            Span::raw("  Experiments: "),
            Span::styled(
                state.total.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("Kept: {}", state.kept_count),
                Style::default().fg(Color::Green),
            ),
            Span::raw("  "),
            Span::styled(
                format!("Discarded: {}", state.discarded_count),
                Style::default().fg(Color::Red),
            ),
        ]),
    ];

    if let (Some(best), Some(bl)) = (state.best_metric, state.baseline_metric) {
        let improvement = if lower_is_better {
            ((bl - best) / bl) * 100.0
        } else {
            ((best - bl) / bl) * 100.0
        };
        stats_lines.push(Line::from(vec![
            Span::raw(format!("  Baseline: {:.6}  Best: ", bl)),
            Span::styled(
                format!("{:.6}", best),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Improvement: "),
            Span::styled(
                format!("{:.2}%", improvement),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ]));
    } else if let Some(bl) = state.baseline_metric {
        stats_lines.push(Line::from(format!("  Baseline: {:.6}  Best: -", bl)));
    }

    if state.new_experiments > 0 {
        stats_lines.push(Line::from(Span::styled(
            format!("  +{} new experiment(s)", state.new_experiments),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));
    }

    let stats = Paragraph::new(stats_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Summary ")
            .title_style(Style::default().fg(Color::White)),
    );
    frame.render_widget(stats, chunks[chunk_idx]);
    chunk_idx += 1;

    // -- Sparkline --
    if has_sparkline {
        let min = state
            .kept_metrics
            .iter()
            .cloned()
            .reduce(f64::min)
            .unwrap_or(0.0);
        let max = state
            .kept_metrics
            .iter()
            .cloned()
            .reduce(f64::max)
            .unwrap_or(1.0);
        let range = max - min;

        let spark_data: Vec<u64> = state
            .kept_metrics
            .iter()
            .map(|v| {
                if range == 0.0 {
                    50
                } else {
                    let normalized = ((v - min) / range * 100.0) as u64;
                    if lower_is_better {
                        100 - normalized.min(100)
                    } else {
                        normalized.min(100)
                    }
                }
            })
            .collect();

        let sparkline = Sparkline::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Progress ")
                    .title_style(Style::default().fg(Color::White)),
            )
            .data(&spark_data)
            .style(Style::default().fg(Color::Cyan));
        frame.render_widget(sparkline, chunks[chunk_idx]);
        chunk_idx += 1;
    }

    // -- Experiments Table --
    let header_cells = ["Run", "Metric", "Delta", "Status", "Summary"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD)));
    let table_header = Row::new(header_cells).height(1);

    let recent: Vec<_> = state.experiments.iter().take(20).collect();
    let rows: Vec<Row> = recent
        .iter()
        .map(|exp| {
            let metric_str = exp
                .metric
                .map(|m| format!("{:.6}", m))
                .unwrap_or_else(|| "-".into());

            let delta_str = exp
                .metric
                .and_then(|m| {
                    state.baseline_metric.map(|bl| {
                        let d = m - bl;
                        if d > 0.0 {
                            format!("+{:.4}", d)
                        } else {
                            format!("{:.4}", d)
                        }
                    })
                })
                .unwrap_or_else(|| "-".into());

            let (status_str, color) = match exp.status {
                git::ExperimentStatus::Kept => ("kept", Color::Green),
                git::ExperimentStatus::Discarded => ("discard", Color::Red),
                git::ExperimentStatus::Baseline => ("baseline", Color::Cyan),
                git::ExperimentStatus::Crash => ("crash", Color::Red),
                git::ExperimentStatus::NoOp => ("no-op", Color::DarkGray),
                git::ExperimentStatus::HookBlocked => ("hook-blk", Color::Yellow),
                git::ExperimentStatus::MetricError => ("met-err", Color::Magenta),
                git::ExperimentStatus::Unknown => ("?", Color::DarkGray),
            };

            let summary = crate::output::truncate(&exp.summary, 40);

            let style = Style::default().fg(color);
            Row::new(vec![
                Cell::from(format!("{:>4}", exp.run)).style(style),
                Cell::from(format!("{:>10}", metric_str)).style(style),
                Cell::from(format!("{:>8}", delta_str)).style(style),
                Cell::from(format!("{:>10}", status_str)).style(style),
                Cell::from(summary).style(style),
            ])
        })
        .collect();

    let more_msg = if state.total > 20 {
        format!(" Recent Experiments ({} more) ", state.total - 20)
    } else {
        " Recent Experiments ".to_string()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Min(20),
        ],
    )
    .header(table_header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(more_msg)
            .title_style(Style::default().fg(Color::White)),
    );
    frame.render_widget(table, chunks[chunk_idx]);

    // -- Fork branches --
    if has_forks {
        let fork_chunk_idx = chunk_idx + 1;
        let fork_lines: Vec<Line> = state
            .fork_branches
            .iter()
            .map(|(name, count)| {
                Line::from(format!("    {name} ({count} experiments)"))
            })
            .collect();

        let forks = Paragraph::new(fork_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Fork Branches ")
                .title_style(Style::default().fg(Color::White)),
        );
        frame.render_widget(forks, chunks[fork_chunk_idx]);
    }
}

fn list_fork_branches() -> Vec<String> {
    let output = std::process::Command::new("git")
        .args(["branch", "--list", "autoresearch-fork-*"])
        .output()
        .ok();

    match output {
        Some(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.trim().trim_start_matches("* ").to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        _ => vec![],
    }
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

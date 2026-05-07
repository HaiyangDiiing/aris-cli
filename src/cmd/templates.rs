/// Domain templates for aris projects.
pub struct DomainTemplate {
    pub name: &'static str,
    pub description: &'static str,
    pub metric_name: &'static str,
    pub metric_direction: &'static str,
    pub eval_command: &'static str,
    pub guard_command: Option<&'static str>,
    pub time_budget: &'static str,
    pub program_md: &'static str,
}

pub const TEMPLATES: &[DomainTemplate] = &[
    DomainTemplate {
        name: "ml-training",
        description: "Machine learning model training (loss minimization)",
        metric_name: "val_loss",
        metric_direction: "lower",
        eval_command: "python train.py 2>&1 | grep 'val_loss' | tail -1 | awk '{print $NF}'",
        guard_command: None,
        time_budget: "30m",
        program_md: r#"# ML Training Optimization

## Goal
Minimize validation loss by tuning hyperparameters and model architecture.

## Ideas to Explore
1. Learning rate schedule (cosine annealing, warm restarts)
2. Batch size tuning (larger batch → more stable gradients)
3. Weight decay / regularization
4. Data augmentation strategies
5. Architecture modifications (layer count, width, activation)

## Constraints
- One change per experiment (atomic)
- Hyperparameters first, architecture second
- Always validate on held-out set
"#,
    },
    DomainTemplate {
        name: "web-perf",
        description: "Web application performance (latency reduction)",
        metric_name: "p99_latency_ms",
        metric_direction: "lower",
        eval_command: "npm run benchmark 2>&1 | grep 'p99' | awk '{print $NF}'",
        guard_command: Some("npm test"),
        time_budget: "10m",
        program_md: r#"# Web Performance Optimization

## Goal
Reduce p99 latency by optimizing critical path, caching, and resource loading.

## Ideas to Explore
1. Bundle size reduction (tree shaking, code splitting)
2. Caching strategy (HTTP cache, service worker, in-memory)
3. Database query optimization (indexes, query plans)
4. Connection pooling and keep-alive
5. Async/lazy loading of non-critical resources

## Constraints
- All tests must pass (guard: npm test)
- Measure under realistic load
- No functionality regressions
"#,
    },
    DomainTemplate {
        name: "test-coverage",
        description: "Test coverage improvement",
        metric_name: "coverage_pct",
        metric_direction: "higher",
        eval_command: "python -m pytest --cov --cov-report=term-missing -q 2>&1 | grep TOTAL | awk '{print $NF}' | tr -d '%'",
        guard_command: Some("python -m pytest -x -q"),
        time_budget: "10m",
        program_md: r#"# Test Coverage Improvement

## Goal
Increase test coverage by adding meaningful tests for uncovered code paths.

## Ideas to Explore
1. Cover untested edge cases in existing functions
2. Add integration tests for API endpoints
3. Test error handling paths
4. Add property-based tests for core logic
5. Cover boundary conditions

## Constraints
- All existing tests must pass (guard)
- Tests must be meaningful, not just coverage-farming
- Prefer testing behavior over implementation details
"#,
    },
    DomainTemplate {
        name: "build-speed",
        description: "Build/compilation time reduction",
        metric_name: "build_time_s",
        metric_direction: "lower",
        eval_command: "time cargo build --release 2>&1 | grep real | awk '{print $2}' | sed 's/m/*60+/;s/s//' | bc",
        guard_command: Some("cargo test"),
        time_budget: "15m",
        program_md: r#"# Build Speed Optimization

## Goal
Reduce build time by optimizing dependency graph, compilation units, and build configuration.

## Ideas to Explore
1. Reduce unnecessary dependencies
2. Feature flag unused optional deps
3. Split large crates into smaller units
4. Optimize proc macro usage
5. Parallel compilation settings

## Constraints
- All tests must pass after changes
- No functionality removed
- Measure cold build time (clean first)
"#,
    },
    DomainTemplate {
        name: "code-quality",
        description: "Code quality score improvement (linter/complexity)",
        metric_name: "quality_score",
        metric_direction: "higher",
        eval_command: "python -m pylint src/ --output-format=text 2>&1 | grep 'rated at' | awk '{print $7}' | tr -d '/10'",
        guard_command: Some("python -m pytest -x -q"),
        time_budget: "10m",
        program_md: r#"# Code Quality Improvement

## Goal
Improve code quality score by reducing complexity, fixing lint warnings, and improving structure.

## Ideas to Explore
1. Extract complex functions into smaller units
2. Fix type annotation gaps
3. Remove dead code and unused imports
4. Simplify deeply nested conditionals
5. Improve naming consistency

## Constraints
- All tests must pass
- No behavior changes (refactoring only)
- One category of improvement per experiment
"#,
    },
    DomainTemplate {
        name: "quantum-vqe",
        description: "Variational Quantum Eigensolver optimization",
        metric_name: "energy",
        metric_direction: "lower",
        eval_command: "python vqe.py 2>&1 | grep 'energy' | tail -1 | awk '{print $NF}'",
        guard_command: None,
        time_budget: "60m",
        program_md: r#"# VQE Ground State Energy Optimization

## Goal
Find the ground state energy of the target Hamiltonian using variational quantum circuits.

## Ideas to Explore
1. Ansatz design (hardware-efficient, UCCSD, adaptive)
2. Optimizer choice (COBYLA, L-BFGS-B, SPSA, gradient-free)
3. Initial parameter strategies
4. Circuit depth vs noise tradeoff
5. Error mitigation techniques

## Constraints
- Use multi-sample recording (--metric repeated) for noise robustness
- Energy must be physically meaningful (above exact ground state)
- Track circuit depth as secondary metric
"#,
    },
];

/// Find a template by name (case-insensitive).
pub fn find_template(name: &str) -> Option<&'static DomainTemplate> {
    TEMPLATES.iter().find(|t| t.name.eq_ignore_ascii_case(name))
}

/// List all available template names.
pub fn available_templates() -> Vec<&'static str> {
    TEMPLATES.iter().map(|t| t.name).collect()
}

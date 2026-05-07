//! Multi-metric Pareto optimization support.
//!
//! Handles [[metrics]] config, weighted composite scoring, and Pareto front computation.

use std::collections::HashMap;

/// A metric definition from config.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricDef {
    pub name: String,
    pub direction: String, // "higher" or "lower"
    pub weight: f64,
}

/// Multi-metric values for a single experiment.
pub type MetricValues = HashMap<String, f64>;

/// Parse metrics config: supports both legacy single-metric and new [[metrics]] array.
/// Returns (metrics_defs, is_multi).
pub fn parse_metrics_config(config: &toml::Table) -> (Vec<MetricDef>, bool) {
    // Try new [[metrics]] array first
    if let Some(metrics_array) = config.get("metrics").and_then(|v| v.as_array()) {
        let defs: Vec<MetricDef> = metrics_array
            .iter()
            .filter_map(|v| {
                let name = v.get("name").and_then(|n| n.as_str())?;
                let direction = v
                    .get("direction")
                    .and_then(|d| d.as_str())
                    .unwrap_or("lower");
                let weight = v
                    .get("weight")
                    .and_then(|w| w.as_float())
                    .unwrap_or(1.0);
                Some(MetricDef {
                    name: name.to_string(),
                    direction: direction.to_string(),
                    weight,
                })
            })
            .collect();

        if !defs.is_empty() {
            return (defs, true);
        }
    }

    // Legacy single-metric fallback
    let name = config
        .get("metric_name")
        .and_then(|v| v.as_str())
        .unwrap_or("metric");
    let direction = config
        .get("metric_direction")
        .and_then(|v| v.as_str())
        .unwrap_or("lower");

    (
        vec![MetricDef {
            name: name.to_string(),
            direction: direction.to_string(),
            weight: 1.0,
        }],
        false,
    )
}

/// Parse --metric CLI args into named metrics.
/// Supports both legacy `f64` and named `name=f64` syntax.
/// For legacy mode, uses the first metric def's name.
pub fn parse_metric_args(
    args: &[String],
    defs: &[MetricDef],
) -> Result<MetricValues, String> {
    let mut values = MetricValues::new();

    for arg in args {
        if let Some((name, val_str)) = arg.split_once('=') {
            // Named: accuracy=0.766
            let val: f64 = val_str
                .parse()
                .map_err(|_| format!("Invalid metric value for '{name}': {val_str}"))?;
            values.insert(name.to_string(), val);
        } else {
            // Legacy: just a number
            let val: f64 = arg
                .parse()
                .map_err(|_| format!("Invalid metric value: {arg}"))?;
            if let Some(def) = defs.first() {
                values.insert(def.name.clone(), val);
            }
        }
    }

    Ok(values)
}

/// Normalize a metric value to [0, 1] given min/max bounds and direction.
fn normalize(value: f64, min: f64, max: f64, direction: &str) -> f64 {
    if (max - min).abs() < f64::EPSILON {
        return 0.5; // No range, assume middle
    }
    let normalized = (value - min) / (max - min);
    if direction == "higher" {
        normalized
    } else {
        1.0 - normalized // Lower is better → flip
    }
}

/// Compute weighted composite score for multi-metric values.
/// Each metric is normalized to [0, 1], multiplied by weight, summed.
/// Returns None if no metrics match the definitions.
pub fn composite_score(
    values: &MetricValues,
    defs: &[MetricDef],
    all_values: &[MetricValues],
) -> Option<f64> {
    if values.is_empty() || defs.is_empty() {
        return None;
    }

    // Compute min/max for each metric across all experiments
    let mut score = 0.0;
    let mut total_weight = 0.0;

    for def in defs {
        if let Some(&val) = values.get(&def.name) {
            let all_vals: Vec<f64> = all_values
                .iter()
                .filter_map(|v| v.get(&def.name).copied())
                .collect();

            if all_vals.is_empty() {
                continue;
            }

            let min = all_vals.iter().copied().fold(f64::INFINITY, f64::min);
            let max = all_vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);

            score += normalize(val, min, max, &def.direction) * def.weight;
            total_weight += def.weight;
        }
    }

    if total_weight > 0.0 {
        Some(score / total_weight)
    } else {
        None
    }
}

/// Compute the Pareto front (non-dominated set) from experiments.
/// Returns indices of non-dominated experiments.
pub fn pareto_front(
    all_values: &[MetricValues],
    defs: &[MetricDef],
) -> Vec<usize> {
    let n = all_values.len();
    let mut dominated = vec![false; n];

    for i in 0..n {
        if dominated[i] {
            continue;
        }
        for j in 0..n {
            if i == j || dominated[j] {
                continue;
            }
            if dominates(&all_values[j], &all_values[i], defs) {
                dominated[i] = true;
                break;
            }
        }
    }

    (0..n).filter(|&i| !dominated[i]).collect()
}

/// Check if experiment A dominates experiment B.
/// A dominates B if A is at least as good in all metrics and strictly better in at least one.
fn dominates(a: &MetricValues, b: &MetricValues, defs: &[MetricDef]) -> bool {
    let mut at_least_one_better = false;

    for def in defs {
        let va = a.get(&def.name);
        let vb = b.get(&def.name);

        match (va, vb) {
            (Some(&va), Some(&vb)) => {
                let a_better = if def.direction == "higher" {
                    va > vb
                } else {
                    va < vb
                };
                let a_worse = if def.direction == "higher" {
                    va < vb
                } else {
                    va > vb
                };
                if a_worse {
                    return false; // A is worse in at least one metric
                }
                if a_better {
                    at_least_one_better = true;
                }
            }
            _ => continue, // Skip missing metrics
        }
    }

    at_least_one_better
}

/// Validate that weights sum to approximately 1.0. Returns normalized weights if they don't.
pub fn validate_weights(defs: &mut [MetricDef]) -> Option<String> {
    let total: f64 = defs.iter().map(|d| d.weight).sum();
    if (total - 1.0).abs() > 0.01 {
        // Auto-normalize
        for def in defs.iter_mut() {
            def.weight /= total;
        }
        Some(format!(
            "Metric weights summed to {total:.2}, auto-normalized to 1.0"
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_composite_score_single() {
        let defs = vec![MetricDef {
            name: "accuracy".into(),
            direction: "higher".into(),
            weight: 1.0,
        }];
        let values: MetricValues = [("accuracy".into(), 0.8)].into();
        let all: Vec<MetricValues> = vec![
            [("accuracy".into(), 0.6)].into(),
            [("accuracy".into(), 0.8)].into(),
            [("accuracy".into(), 1.0)].into(),
        ];
        let score = composite_score(&values, &defs, &all).unwrap();
        assert!((score - 0.5).abs() < 0.01); // 0.8 is in the middle
    }

    #[test]
    fn test_composite_score_multi() {
        let defs = vec![
            MetricDef { name: "accuracy".into(), direction: "higher".into(), weight: 0.7 },
            MetricDef { name: "latency".into(), direction: "lower".into(), weight: 0.3 },
        ];
        let values: MetricValues = [
            ("accuracy".into(), 0.9),
            ("latency".into(), 50.0),
        ].into();
        let all: Vec<MetricValues> = vec![
            [("accuracy".into(), 0.7), ("latency".into(), 100.0)].into(),
            [("accuracy".into(), 0.9), ("latency".into(), 50.0)].into(),
        ];
        let score = composite_score(&values, &defs, &all).unwrap();
        assert!(score > 0.8); // Good accuracy + good latency
    }

    #[test]
    fn test_pareto_front() {
        let defs = vec![
            MetricDef { name: "a".into(), direction: "higher".into(), weight: 0.5 },
            MetricDef { name: "b".into(), direction: "higher".into(), weight: 0.5 },
        ];
        let all: Vec<MetricValues> = vec![
            [("a".into(), 1.0), ("b".into(), 3.0)].into(), // Pareto (best b)
            [("a".into(), 2.0), ("b".into(), 2.0)].into(), // Pareto (balanced)
            [("a".into(), 3.0), ("b".into(), 1.0)].into(), // Pareto (best a)
            [("a".into(), 1.0), ("b".into(), 1.0)].into(), // Dominated by all above
        ];
        let front = pareto_front(&all, &defs);
        assert_eq!(front, vec![0, 1, 2]); // Index 3 is dominated
    }

    #[test]
    fn test_dominates() {
        let defs = vec![
            MetricDef { name: "x".into(), direction: "higher".into(), weight: 1.0 },
            MetricDef { name: "y".into(), direction: "higher".into(), weight: 1.0 },
        ];
        let a: MetricValues = [("x".into(), 2.0), ("y".into(), 2.0)].into();
        let b: MetricValues = [("x".into(), 1.0), ("y".into(), 1.0)].into();
        assert!(dominates(&a, &b, &defs));
        assert!(!dominates(&b, &a, &defs));
    }

    #[test]
    fn test_validate_weights() {
        let mut defs = vec![
            MetricDef { name: "a".into(), direction: "higher".into(), weight: 2.0 },
            MetricDef { name: "b".into(), direction: "lower".into(), weight: 3.0 },
        ];
        let warning = validate_weights(&mut defs);
        assert!(warning.is_some());
        let total: f64 = defs.iter().map(|d| d.weight).sum();
        assert!((total - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_metric_args_named() {
        let defs = vec![
            MetricDef { name: "accuracy".into(), direction: "higher".into(), weight: 0.5 },
            MetricDef { name: "latency".into(), direction: "lower".into(), weight: 0.5 },
        ];
        let args = vec!["accuracy=0.95".to_string(), "latency=42.0".to_string()];
        let values = parse_metric_args(&args, &defs).unwrap();
        assert_eq!(values["accuracy"], 0.95);
        assert_eq!(values["latency"], 42.0);
    }

    #[test]
    fn test_legacy_config_compat() {
        let mut config = toml::map::Map::new();
        config.insert("metric_name".into(), toml::Value::String("loss".into()));
        config.insert("metric_direction".into(), toml::Value::String("lower".into()));
        let (defs, is_multi) = parse_metrics_config(&config);
        assert!(!is_multi);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "loss");
        assert_eq!(defs[0].direction, "lower");
    }
}

use std::fs;

const STRATEGY_PATH: &str = ".autoresearch/strategy.json";

/// Persistent strategy state for graduated stuck recovery.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[derive(Default)]
pub struct StrategyState {
    pub level: u8,
    pub consecutive_discards: usize,
    pub pivot_count: usize,
    pub last_kept_run: Option<usize>,
    #[serde(default)]
    pub pivot_events: Vec<PivotEvent>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PivotEvent {
    pub timestamp: String,
    pub at_run: usize,
    pub level: u8,
}


impl StrategyState {
    /// Load strategy from disk. Returns default if file doesn't exist.
    pub fn load() -> Self {
        fs::read_to_string(STRATEGY_PATH)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Save strategy to disk.
    pub fn save(&self) -> Result<(), std::io::Error> {
        fs::create_dir_all(".autoresearch")?;
        fs::write(STRATEGY_PATH, serde_json::to_string_pretty(self).unwrap())
    }

    /// Update state after recording an experiment. Returns the computed escalation level.
    pub fn update(&mut self, status: &str, run_number: usize) -> u8 {
        let non_kept = [
            "discarded", "crash", "no-op", "hook-blocked", "metric-error",
        ];

        if non_kept.contains(&status) {
            self.consecutive_discards += 1;
        } else if status == "kept" || status == "baseline" {
            self.consecutive_discards = 0;
            self.last_kept_run = Some(run_number);
        }

        // Compute level from consecutive_discards + pivot history
        self.level = self.compute_level();

        // Record pivot event when crossing level 2+
        if self.level >= 2 {
            let should_record_pivot = self.pivot_events.last()
                .map(|p| p.level < self.level)
                .unwrap_or(true);

            if should_record_pivot {
                self.pivot_count += 1;
                self.pivot_events.push(PivotEvent {
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    at_run: run_number,
                    level: self.level,
                });
            }
        }

        let _ = self.save();
        self.level
    }

    /// Compute escalation level based on current state.
    fn compute_level(&self) -> u8 {
        // Count pivots without improvement (pivots since last kept)
        let pivots_without_improvement = self.pivot_events.iter().rev()
            .take_while(|p| {
                self.last_kept_run.map(|k| p.at_run > k).unwrap_or(true)
            })
            .count();

        // Level 5: 3+ pivots without any kept
        if pivots_without_improvement >= 3 {
            return 5;
        }

        // Level 4: 10+ consecutive discards
        if self.consecutive_discards >= 10 {
            return 4;
        }

        // Level 3: 7+ consecutive discards AND 2+ previous pivots
        if self.consecutive_discards >= 7 && self.pivot_count >= 2 {
            return 3;
        }

        // Level 2: 5+ consecutive discards
        if self.consecutive_discards >= 5 {
            return 2;
        }

        // Level 1: 3+ consecutive discards
        if self.consecutive_discards >= 3 {
            return 1;
        }

        0
    }

    /// Generate escalation hint based on current level.
    pub fn escalation_hint(&self) -> Option<String> {
        match self.level {
            1 => Some("Level 1 — Refine: Adjust parameters within the current approach direction.".into()),
            2 => Some("Level 2 — Pivot: Switch to a fundamentally different approach. Run `aris review` for cross-model analysis.".into()),
            3 => Some("Level 3 — Search: 7+ discards with 2 failed pivots. Search external resources for alternative approaches.".into()),
            4 => Some("Level 4 — Fork: 10+ consecutive discards. Run `aris fork` to explore multiple directions in parallel.".into()),
            5 => Some("Level 5 — STOP: 3 pivots without improvement. Human intervention recommended. Review the approach fundamentally or change the objective.".into()),
            _ => None,
        }
    }

    /// Return JSON value for escalation object (included in record output when level >= 2).
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "level": self.level,
            "consecutive_discards": self.consecutive_discards,
            "pivot_count": self.pivot_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state() {
        let s = StrategyState::default();
        assert_eq!(s.level, 0);
        assert_eq!(s.consecutive_discards, 0);
        assert_eq!(s.pivot_count, 0);
    }

    #[test]
    fn test_level_1_at_3_discards() {
        let mut s = StrategyState::default();
        s.consecutive_discards = 3;
        s.level = s.compute_level();
        assert_eq!(s.level, 1);
    }

    #[test]
    fn test_level_2_at_5_discards() {
        let mut s = StrategyState::default();
        s.consecutive_discards = 5;
        s.level = s.compute_level();
        assert_eq!(s.level, 2);
    }

    #[test]
    fn test_level_3_at_7_discards_with_pivots() {
        let mut s = StrategyState::default();
        s.consecutive_discards = 7;
        s.pivot_count = 2;
        s.level = s.compute_level();
        assert_eq!(s.level, 3);
    }

    #[test]
    fn test_level_3_needs_pivots() {
        let mut s = StrategyState::default();
        s.consecutive_discards = 7;
        s.pivot_count = 1; // not enough pivots
        s.level = s.compute_level();
        assert_eq!(s.level, 2); // falls back to level 2
    }

    #[test]
    fn test_level_4_at_10_discards() {
        let mut s = StrategyState::default();
        s.consecutive_discards = 10;
        s.level = s.compute_level();
        assert_eq!(s.level, 4);
    }

    #[test]
    fn test_level_5_three_pivots_without_improvement() {
        let mut s = StrategyState::default();
        s.last_kept_run = Some(5);
        s.pivot_events = vec![
            PivotEvent { timestamp: String::new(), at_run: 10, level: 2 },
            PivotEvent { timestamp: String::new(), at_run: 15, level: 3 },
            PivotEvent { timestamp: String::new(), at_run: 20, level: 4 },
        ];
        s.pivot_count = 3;
        s.consecutive_discards = 15;
        s.level = s.compute_level();
        assert_eq!(s.level, 5);
    }

    #[test]
    fn test_reset_on_kept() {
        let mut s = StrategyState::default();
        s.consecutive_discards = 8;
        s.pivot_count = 2;
        s.update("kept", 20);
        assert_eq!(s.consecutive_discards, 0);
        assert_eq!(s.pivot_count, 2); // preserved
        assert_eq!(s.last_kept_run, Some(20));
        assert_eq!(s.level, 0);
    }

    #[test]
    fn test_escalation_hints() {
        let mut s = StrategyState::default();
        assert!(s.escalation_hint().is_none());

        s.level = 1;
        assert!(s.escalation_hint().unwrap().contains("Refine"));

        s.level = 2;
        assert!(s.escalation_hint().unwrap().contains("Pivot"));

        s.level = 5;
        assert!(s.escalation_hint().unwrap().contains("STOP"));
    }
}

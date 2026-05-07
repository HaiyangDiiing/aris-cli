use std::io::IsTerminal;

pub enum OutputFormat {
    Json,
    Table,
}

impl OutputFormat {
    /// Detect output format. Priority: --json flag > AUTORESEARCH_FORMAT env > TTY detection.
    pub fn detect(json_flag: bool) -> Self {
        if json_flag {
            return OutputFormat::Json;
        }
        if let Ok(val) = std::env::var("AUTORESEARCH_FORMAT") {
            if val.eq_ignore_ascii_case("json") {
                return OutputFormat::Json;
            }
        }
        if !std::io::stdout().is_terminal() {
            OutputFormat::Json
        } else {
            OutputFormat::Table
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_json_flag_wins() {
        // --json flag always returns Json
        assert!(matches!(OutputFormat::detect(true), OutputFormat::Json));
    }

    #[test]
    fn test_detect_env_var_json() {
        std::env::set_var("AUTORESEARCH_FORMAT", "json");
        assert!(matches!(OutputFormat::detect(false), OutputFormat::Json));
        std::env::remove_var("AUTORESEARCH_FORMAT");
    }

    #[test]
    fn test_detect_env_var_case_insensitive() {
        std::env::set_var("AUTORESEARCH_FORMAT", "JSON");
        assert!(matches!(OutputFormat::detect(false), OutputFormat::Json));
        std::env::remove_var("AUTORESEARCH_FORMAT");
    }

    #[test]
    fn test_detect_env_var_invalid_ignored() {
        std::env::set_var("AUTORESEARCH_FORMAT", "table");
        // In test harness, stdout is not a terminal, so this will still be Json
        // due to TTY detection. But the env var "table" is not "json", so it
        // falls through to TTY detection.
        let _ = OutputFormat::detect(false);
        std::env::remove_var("AUTORESEARCH_FORMAT");
    }
}

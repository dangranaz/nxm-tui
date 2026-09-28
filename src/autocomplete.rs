//! Fuzzy autocomplete for nexum-terminal TUI.
//! Provides command and history fuzzy matching with scoring.

/// Available commands for autocomplete, one entry per root command, kept in
/// alphabetical order (the command popup relies on this ordering).
static COMMANDS: &[(&str, &str)] = &[
    ("/clear", "Clear chat history"),
    ("/config", "Show or generate config"),
    ("/context", "Set context limit"),
    ("/help", "Show this help"),
    ("/load", "Load session"),
    ("/metrics", "Show metrics"),
    ("/mode", "Set agent mode: chat|architect|developer|researcher"),
    ("/new", "New session"),
    ("/provider", "Manage LLM providers: list|add|use|remove"),
    ("/quit", "Quit"),
    ("/save", "Save session"),
    ("/server", "Manage local server: start|stop|status"),
    ("/sidebar", "Toggle sidebar"),
];

/// Score match quality (higher = better).
fn score_match(pattern: &str, text: &str) -> u32 {
    let pattern_lower = pattern.to_lowercase();
    let text_lower = text.to_lowercase();
    
    if text_lower.starts_with(&pattern_lower) {
        return 1000 + (200 - pattern.len() as u32);
    }
    if text_lower.contains(&pattern_lower) {
        return 500 + (100 - pattern.len() as u32);
    }
    
    // Levenshtein distance approximation
    let mut score = 0u32;
    let mut p_idx = 0;
    for (t_idx, c) in text_lower.chars().enumerate() {
        if p_idx < pattern_lower.len() && c == pattern_lower.chars().nth(p_idx).unwrap_or('\0') {
            score += 10;
            p_idx += 1;
        }
        if t_idx < pattern.len() {
            score += 1;
        }
    }
    score
}

/// Find matching commands for autocomplete.
pub fn find_matches(input: &str, limit: usize) -> Vec<(String, String)> {
    let mut matches: Vec<(String, u32, String, String)> = Vec::new();
    
    for &(cmd, desc) in COMMANDS {
        if input.is_empty() || cmd.starts_with('/') && !input.starts_with('/') {
            continue; // Only complete slash commands
        }
        let score = score_match(input, cmd);
        if score > 0 {
            matches.push((cmd.to_string(), score, desc.to_string(), cmd.to_string()));
        }
    }
    
    matches.sort_by_key(|m| std::cmp::Reverse(m.1));
    matches.into_iter().take(limit).map(|(c, _, d, _)| (c, d)).collect()
}

/// Commands shown in the slash-command popup.
///
/// Filters `COMMANDS` by the first whitespace-delimited token of `input`
/// (the root command being typed, e.g. `/pro`), matching case-insensitively
/// on the command prefix. `COMMANDS` is already alphabetical, so the returned
/// list preserves that order. When `input` is just `/` (or empty after it),
/// every command is returned.
pub fn menu_matches(input: &str) -> Vec<(&'static str, &'static str)> {
    if !input.starts_with('/') {
        return Vec::new();
    }
    // Only the root token drives the popup; args (after a space) don't filter.
    let root = input.split_whitespace().next().unwrap_or("/");
    let needle = root.to_lowercase();
    COMMANDS
        .iter()
        .filter(|(cmd, _)| cmd.to_lowercase().starts_with(&needle))
        .copied()
        .collect()
}
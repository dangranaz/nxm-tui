//! Fuzzy autocomplete for nexum-terminal TUI.
//! Provides command and history fuzzy matching with scoring.

/// Available commands for autocomplete.
static COMMANDS: &[(&str, &str)] = &[
    ("/help", "Show this help"),
    ("/clear", "Clear chat history"),
    ("/quit", "Quit"),
    ("/server start", "Start local server"),
    ("/server stop", "Stop local server"),
    ("/server status", "Server status"),
    ("/config status", "Show config"),
    ("/config generate", "Generate engine/model config"),
    ("/mode chat", "Chat mode"),
    ("/mode architect", "Architect mode"),
    ("/mode developer", "Developer mode"),
    ("/mode researcher", "Researcher mode"),
    ("/save", "Save session"),
    ("/load", "Load session"),
    ("/new", "New session"),
    ("/metrics", "Show metrics"),
    ("/context", "Set context limit"),
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
    
    matches.sort_by(|a, b| b.1.cmp(&a.1));
    matches.into_iter().take(limit).map(|(c, _, d, _)| (c, d)).collect()
}
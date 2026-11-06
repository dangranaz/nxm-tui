//! Response/session metrics: char and token throughput for the `/metrics`
//! overlay.
//!
//! Extracted verbatim from `app.rs` (T6 split, no behavior change) and
//! re-exported from `app` so existing call sites keep working.

use std::collections::VecDeque;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct Metrics {
    /// When current response started
    pub response_start: Option<Instant>,
    /// Char count of current response so far
    pub response_char_count: usize,
    /// Last N response times in milliseconds
    pub response_times_ms: VecDeque<u64>,
    /// Last N response estimated token counts
    pub response_tokens: VecDeque<u64>,
    /// Total chars generated ever (across sessions)
    pub total_chars: u64,
    /// Total tokens estimated ever
    pub total_tokens: u64,
    /// Total responses ever
    pub total_responses: u64,
    /// User messages in current session
    pub session_user_msgs: usize,
    /// When current session started
    pub session_start: Instant,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            response_start: None,
            response_char_count: 0,
            response_times_ms: VecDeque::with_capacity(50),
            response_tokens: VecDeque::with_capacity(50),
            total_chars: 0,
            total_tokens: 0,
            total_responses: 0,
            session_user_msgs: 0,
            session_start: Instant::now(),
        }
    }

    pub fn start_response(&mut self) {
        self.response_start = Some(Instant::now());
        self.response_char_count = 0;
    }

    pub fn record_chars(&mut self, n: usize) {
        self.response_char_count += n;
        self.total_chars += n as u64;
    }

    pub fn finish_response(&mut self) {
        self.total_responses += 1;
        let elapsed = self
            .response_start
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0);
        let tokens = (self.response_char_count / 4).max(1) as u64;
        self.total_tokens += tokens;
        self.response_times_ms.push_back(elapsed);
        self.response_tokens.push_back(tokens);
        if self.response_times_ms.len() > 50 {
            self.response_times_ms.pop_front();
        }
        if self.response_tokens.len() > 50 {
            self.response_tokens.pop_front();
        }
    }

    pub fn reset_session(&mut self) {
        self.response_times_ms.clear();
        self.response_tokens.clear();
        self.session_user_msgs = 0;
        self.session_start = Instant::now();
    }

    pub fn avg_response_time_ms(&self) -> u64 {
        let n = self.response_times_ms.len();
        if n == 0 {
            return 0;
        }
        self.response_times_ms.iter().sum::<u64>() / n as u64
    }

    pub fn avg_tokens_per_response(&self) -> u64 {
        let n = self.response_tokens.len();
        if n == 0 {
            return 0;
        }
        self.response_tokens.iter().sum::<u64>() / n as u64
    }

    pub fn tokens_per_second(&self) -> f64 {
        let total_ms: u64 = self.response_times_ms.iter().sum();
        if total_ms == 0 {
            return 0.0;
        }
        let total_tok: u64 = self.response_tokens.iter().sum();
        (total_tok as f64) / (total_ms as f64 / 1000.0)
    }
}

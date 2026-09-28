//! Prompt input component for nexum-terminal TUI.
//! Handles multiline input, history navigation, and state management.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::{Color, Style};
use ratatui::widgets::Paragraph;

/// Maximum number of prompt history entries to store.
const MAX_HISTORY: usize = 100;

/// Prompt state containing input lines and cursor/cursor position.
#[derive(Debug, Clone)]
pub struct PromptState {
    /// Current input lines (split by \n for multiline)
    pub lines: Vec<String>,
    /// Current cursor line index
    pub cursor_line: usize,
    /// Current cursor column within the line
    pub cursor_col: usize,
    /// History entries (most recent at the end)
    pub history: Vec<String>,
    /// Current history position (-1 means not browsing history)
    pub history_pos: isize,
    /// Ghost placeholder text shown when input is empty
    pub placeholder: &'static str,
}

impl Default for PromptState {
    fn default() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            history: Vec::new(),
            history_pos: -1,
            placeholder: "...  (/help for commands)",
        }
    }
}

impl PromptState {
    /// Creates a new empty prompt state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current text (all lines joined by \n).
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Inserts a character at the current cursor position.
    pub fn insert_char(&mut self, c: char) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        let line = self.cursor_line.min(self.lines.len() - 1);
        let col = self.cursor_col.min(self.lines[line].len());
        self.lines[line].insert(col, c);
        self.cursor_col += 1;
        self.history_pos = -1; // Reset history navigation
    }

    /// Deletes character before cursor (backspace).
    pub fn delete_char(&mut self) {
        if self.cursor_line >= self.lines.len() {
            return;
        }
        let col = self.cursor_col;
        let line = self.cursor_line;
        if col > 0 {
            self.lines[line].remove(col - 1);
            self.cursor_col -= 1;
        } else if line > 0 {
            // Merge with previous line
            let merged = self.lines[line].clone();
            self.lines.remove(line);
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].len();
            self.lines[self.cursor_line].push_str(&merged);
        }
    }

    /// Delete word backward (Ctrl+Backspace).
    pub fn delete_word_backward(&mut self) {
        if self.cursor_line >= self.lines.len() {
            return;
        }
        let line = self.cursor_line;
        let col = self.cursor_col;

        if col > 0 {
            // Delete word before cursor
            let before_cursor = &self.lines[line][..col];
            let word_end = before_cursor.rfind(|c: char| c.is_whitespace()).map(|i| i + 1).unwrap_or(0);
            self.lines[line].replace_range(word_end..col, "");
            self.cursor_col = word_end;
        } else if line > 0 {
            // At start of line: merge with previous and delete
            let merged = self.lines[line].clone();
            self.lines.remove(line);
            self.cursor_line -= 1;
            self.lines[self.cursor_line].push_str(&merged);
            self.cursor_col = self.lines[self.cursor_line].len();
        }
    }

    /// Insert newline (Shift+Enter).
    pub fn insert_newline(&mut self) {
        let line = self.cursor_line.min(self.lines.len() - 1);
        let col = self.cursor_col.min(self.lines[line].len());
        let current_line = &self.lines[line];
        
        let new_line = current_line[col..].to_string();
        self.lines[line].truncate(col);
        self.lines.insert(line + 1, new_line);
        self.cursor_line += 1;
        self.cursor_col = 0;
    }

    /// Navigate history (up arrow = older, down arrow = newer).
    pub fn navigate_history(&mut self, direction: HistoryDirection) {
        if self.history.is_empty() {
            return;
        }

        match direction {
            HistoryDirection::Up => {
                if self.history_pos == -1 {
                    // First time: save current text and go to most recent history
                    self.history_pos = self.history.len() as isize - 1;
                } else if self.history_pos > 0 {
                    self.history_pos -= 1;
                }
            }
            HistoryDirection::Down => {
                if self.history_pos >= 0 {
                    self.history_pos += 1;
                    if self.history_pos >= self.history.len() as isize {
                        // Reset to empty input
                        self.history_pos = -1;
                    }
                }
            }
        }

        // Apply history entry
        if self.history_pos >= 0 {
            let entry = self.history[self.history_pos as usize].clone();
            self.set_text(entry);
        } else {
            self.set_text(String::new());
        }
    }

    /// Set the text content (clears cursor, resets history nav).
    pub fn set_text(&mut self, text: String) {
        self.lines = if text.is_empty() {
            vec![String::new()]
        } else {
            text.split('\n').map(|s| s.to_string()).collect()
        };
        self.cursor_line = self.lines.len() - 1;
        self.cursor_col = self.lines.last().map(|l| l.len()).unwrap_or(0);
    }

    /// Submit the current prompt text.
    /// Returns the text and adds it to history.
    pub fn submit(&mut self) -> Option<String> {
        let text = self.text();
        if !text.trim().is_empty() {
            self.add_to_history(text.clone());
        }
        self.lines = vec![String::new()];
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.history_pos = -1;
        Some(text).filter(|t| !t.is_empty())
    }

    /// Add a prompt to history (after submission).
    fn add_to_history(&mut self, prompt: String) {
        // Don't add if same as last entry
        if self.history.last().map(|p| p.as_str()) != Some(&prompt) {
            self.history.push(prompt);
            if self.history.len() > MAX_HISTORY {
                self.history.remove(0);
            }
        }
    }
}

/// History navigation direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryDirection {
    Up,
    Down,
}

/// Render the prompt input into the provided buffer.
pub fn render_prompt(state: &PromptState, area: Rect, buf: &mut Buffer) {
    let lines = crate::prompt_lines::generate(state);
    let paragraph = Paragraph::new(lines).style(Style::default().bg(Color::Rgb(20, 20, 20)));
    paragraph.render(area, buf);
}
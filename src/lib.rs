//! Nexum Terminal — library surface for the TUI binary and headless tests.
//!
//! The crate is bin + lib: `main.rs` (bin) and `tests/*.rs` (integration)
//! both consume these modules via `nxm_tui::`, so cross-module `crate::`
//! references resolve once, in one module graph — no per-test `#[path]`
//! mirrors to keep in sync.

pub mod agent;
pub mod app;
pub mod autocomplete;
pub mod bottom;
pub mod config;
pub mod connection;
pub mod event;
pub mod handler;
pub mod history;
pub mod markdown;
pub mod mode_bar;
pub mod overlays;
pub mod prompt;
pub mod prompt_lines;
pub mod keys;
pub mod provider;
pub mod server_proc;
pub mod session;
pub mod sidebar;
pub mod tool_overlay;
pub mod tool_types;
pub mod ui;
pub mod utils;

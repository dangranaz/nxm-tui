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
pub mod bounded;
pub mod circuit_breaker;
pub mod commands;
pub mod config;
pub mod connection;
pub mod enter_plan_mode;
pub mod event;
pub mod exit_plan_mode;
pub mod goal;
pub mod handler;
pub mod highlight;
pub mod history;
pub mod keys;
pub mod keystore;
pub mod markdown;
mod markdown_layout;
mod markdown_legacy;
pub mod mcp;
pub mod md;
pub mod message;
pub mod metrics;
pub mod mode_bar;
pub mod models;
pub mod overlays;
pub mod plan_mode;
pub mod prompt;
pub mod prompt_lines;
pub mod provider;
pub mod search;
pub mod search_ui;
pub mod server_proc;
pub mod session;
pub mod sidebar;
pub mod todo;
pub mod todo_overlay;
pub mod todo_tool;
pub mod tool_overlay;
pub mod tool_types;
pub mod tools_overlay;
pub mod ui;
pub mod update_goal;
pub mod utils;

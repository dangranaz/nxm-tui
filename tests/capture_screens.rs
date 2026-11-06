//! Capture REAL rendered screens of nxm-tui into docs/screens/*.txt.
//!
//! Uses the production `ui::render` path on ratatui's `TestBackend` (no TTY),
//! exactly like the overlay regression tests. Each state is built from the
//! real `App` + domain types, rendered, and the terminal buffer dumped to a
//! text file. Run with:
//!   cargo test --test capture_screens -- --nocapture
//!
//! Output goes to docs/screens/ (one file per state). These are the real
//! interface, not a mockup.

#![allow(unused_imports)]

use std::fs;
use std::path::PathBuf;

use nxm_tui::app::{AgentMode, App, Message, PendingApproval, Role, RunState};
use nxm_tui::tool_types::{ToolInvocation, ToolPart};
use nxm_tui::ui::render;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

const W: u16 = 130;
const H: u16 = 38;

/// Render the full production UI for `app` and return the screen as text.
fn render_app(app: &mut App) -> String {
    let backend = TestBackend::new(W, H);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|f| render(f, app))
        .expect("draw must not panic");
    let buf = terminal.backend().buffer().clone();
    let (w, h) = (buf.area.width as usize, buf.area.height as usize);
    let mut out = String::new();
    for y in 0..h {
        for x in 0..w {
            if let Some(cell) = buf.cell((x as u16, y as u16)) {
                out.push_str(cell.symbol());
            }
        }
        // trim trailing spaces on each row for cleaner files
        while out.ends_with(' ') {
            out.pop();
        }
        out.push('\n');
    }
    out
}

fn screens_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/screens");
    fs::create_dir_all(&dir).expect("create docs/screens");
    dir
}

fn save(name: &str, content: &str) {
    let path = screens_dir().join(name);
    fs::write(&path, content).expect("write screen");
    println!("\n================ {name} ================\n{content}");
}

/// Base app: connected to Ollama, a model selected, developer mode.
fn connected_app() -> App {
    let mut app = App::new();
    app.state = RunState::Running;
    app.endpoint = "http://127.0.0.1:11434".to_string();
    app.server_name = "Ollama".to_string();
    app.model_name = Some("qwen2.5:1.5b".to_string());
    app.mode = AgentMode::Developer;
    app
}

#[test]
fn capture_01_chat_conversation() {
    let mut app = connected_app();
    app.messages.push(Message::new(
        Role::User,
        "Explain what this project does in one sentence.".to_string(),
    ));
    app.messages.push(Message::new(
        Role::Assistant,
        "**nxm-tui** is a terminal client for local LLMs: it connects to any \
         OpenAI-compatible server, streams responses, and can call tools \
         (read files, search, edit) under a human approval gate."
            .to_string(),
    ));
    app.messages.push(Message::new(
        Role::User,
        "Nice. Now list the agent modes.".to_string(),
    ));
    app.messages.push(Message::new(
        Role::Assistant,
        "The four agent modes are:\n\n\
         1. `Chat` — plain conversation\n\
         2. `Architect` — planning and design\n\
         3. `Developer` — reads/edits code with tools\n\
         4. `Researcher` — web search and synthesis"
            .to_string(),
    ));
    save("01-chat-conversation.txt", &render_app(&mut app));
}

#[test]
fn capture_02_streaming() {
    let mut app = connected_app();
    app.state = RunState::Thinking;
    app.messages.push(Message::new(
        Role::User,
        "Write a Rust function that reverses a string.".to_string(),
    ));
    // a partial assistant message mid-stream
    app.messages.push(Message::new(
        Role::Assistant,
        "Here you go:\n\n```rust\nfn reverse(s: &str) -> String {\n    s.chars().rev".to_string(),
    ));
    save("02-streaming.txt", &render_app(&mut app));
}

#[test]
fn capture_03_tool_approval() {
    let mut app = connected_app();
    app.messages.push(Message::new(
        Role::User,
        "Read the project README and summarize it.".to_string(),
    ));
    app.messages.push(Message::with_tool(
        Role::Assistant,
        vec![ToolPart::ToolInvocation(ToolInvocation {
            id: "call_1".to_string(),
            name: "read_file".to_string(),
            args: "{\"path\": \"/home/u/.env\"}".to_string(),
        })],
    ));
    let (reply_tx, _rx) = tokio::sync::oneshot::channel();
    app.pending_approval = Some(PendingApproval {
        invocation: ToolInvocation {
            id: "call_1".to_string(),
            name: "read_file".to_string(),
            args: "{\"path\": \"/home/u/.env\"}".to_string(),
        },
        reply: reply_tx,
    });
    app.show_approval = true;
    save("03-tool-approval.txt", &render_app(&mut app));
}

#[test]
fn capture_04_reasoning_overlay() {
    let mut app = connected_app();
    app.messages.push(Message::new(
        Role::User,
        "Is 561 a Carmichael number?".to_string(),
    ));
    app.append_thinking_delta("561 = 3 x 11 x 17, squarefree. ");
    app.append_thinking_delta("Check Korselt: for each prime p|n, (p-1)|(n-1). ");
    app.append_thinking_delta("n-1 = 560. 2|560, 10|560, 16|560 -> yes. ");
    app.append_thinking_delta("So 561 is a Carmichael number.");
    app.show_thinking = true;
    save("04-reasoning-overlay.txt", &render_app(&mut app));
}

#[test]
fn capture_05_tools_overlay() {
    let mut app = connected_app();
    app.messages.push(Message::new(
        Role::User,
        "Refactor the parser module.".to_string(),
    ));
    app.show_tools = true;
    save("05-tools-overlay.txt", &render_app(&mut app));
}

#[test]
fn capture_06_tool_detail() {
    let mut app = connected_app();
    app.messages.push(Message::with_tool(
        Role::Assistant,
        vec![ToolPart::ToolInvocation(ToolInvocation {
            id: "call_7".to_string(),
            name: "grep".to_string(),
            args: "{\"pattern\": \"fn render\", \"path\": \"src\"}".to_string(),
        })],
    ));
    app.messages.push(Message::tool(
        "call_7",
        "src/ui.rs:13:pub fn render(f: &mut Frame, app: &mut App) {\n\
         src/overlays.rs:20:pub fn render_help(f: &mut Frame, app: &App) {\n\
         src/tool_overlay.rs:9:pub fn render_tool(f: &mut Frame, app: &App) {"
            .to_string(),
    ));
    app.show_tool = true;
    save("06-tool-detail.txt", &render_app(&mut app));
}

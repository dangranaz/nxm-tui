//! Tests for the session passphrase prompt (I2 — wire KeyStore in production).
//!
//! Contract, modeled on the `/provider set-key` masked entry:
//! - typed ONCE per session, cached in `app.passphrase` (a `Zeroizing`
//!   buffer) and never logged;
//! - the buffer is zeroed (slot cleared) on Enter and on Esc;
//! - Esc also cancels every action that was waiting for the passphrase
//!   (models fetch, MCP connect, a queued send) so nothing hangs or
//!   re-prompts in a loop;
//! - opening the prompt twice must not wipe what is being typed;
//! - the overlay masks the characters (the literal passphrase never hits
//!   the screen).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use nxm_tui::app::{App, PassphraseEntry, RunState};
use nxm_tui::handler::handle_key;
use nxm_tui::keys::KeySourceMode;
use nxm_tui::ui;
use zeroize::Zeroizing;

fn entry(buffer: &str) -> PassphraseEntry {
    PassphraseEntry {
        buffer: Zeroizing::new(buffer.to_string()),
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn screen_text(app: &mut App) -> String {
    let backend = ratatui::backend::TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|f| ui::render(f, app))
        .expect("draw must not panic");
    let buf = terminal.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            if let Some(cell) = buf.cell((x, y)) {
                out.push_str(cell.symbol());
            }
        }
        out.push('\n');
    }
    out
}

#[test]
fn passphrase_enter_caches_once_and_clears_the_buffer() {
    let mut app = App::new();
    app.passphrase_pending = Some(entry("s3cret-pass"));

    handle_key(&mut app, key(KeyCode::Enter));

    assert!(app.passphrase_pending.is_none(), "buffer slot cleared");
    assert_eq!(
        app.passphrase.as_deref().map(|p| p.as_str()),
        Some("s3cret-pass"),
        "cached for the rest of the session"
    );
}

#[test]
fn passphrase_typing_and_backspace_edit_the_masked_buffer() {
    let mut app = App::new();
    app.request_passphrase();

    for c in "abc".chars() {
        handle_key(&mut app, key(KeyCode::Char(c)));
    }
    handle_key(&mut app, key(KeyCode::Backspace));
    assert_eq!(
        app.passphrase_pending.as_ref().map(|e| e.buffer.as_str()),
        Some("ab")
    );
}

#[test]
fn passphrase_esc_cancels_every_gated_action() {
    let mut app = App::new();
    app.passphrase_pending = Some(entry("half-typed"));
    app.models_fetch_pending = Some("http://localhost".into());
    app.mcp_connect_pending = true;
    app.pending_message = Some("hello".into());
    app.state = RunState::Thinking;

    handle_key(&mut app, key(KeyCode::Esc));

    assert!(app.passphrase_pending.is_none(), "buffer slot cleared");
    assert!(app.passphrase.is_none(), "no passphrase cached on cancel");
    assert!(app.models_fetch_pending.is_none(), "models fetch cancelled");
    assert!(!app.mcp_connect_pending, "MCP connect cancelled");
    assert_eq!(app.state, RunState::Running, "queued send cancelled");
    assert!(app.pending_message.is_none(), "queued message dropped");
}

#[test]
fn request_passphrase_is_idempotent_keeps_typed_buffer() {
    let mut app = App::new();
    app.request_passphrase();
    if let Some(entry) = app.passphrase_pending.as_mut() {
        entry.buffer.push_str("typed-so-far");
    }

    app.request_passphrase(); // a second gate must not wipe the input

    assert_eq!(
        app.passphrase_pending.as_ref().map(|e| e.buffer.as_str()),
        Some("typed-so-far")
    );
}

#[test]
fn passphrase_overlay_masks_the_literal_value() {
    let mut app = App::new();
    app.passphrase_pending = Some(entry("hunter2"));

    let screen = screen_text(&mut app);
    assert!(screen.contains("Passphrase"), "prompt must be visible");
    assert!(screen.contains("•••••••"), "characters must be masked");
    assert!(
        !screen.contains("hunter2"),
        "literal value must never render"
    );
}

/// The shared gate: prompt only when the mode is passphrase-locked AND no
/// session passphrase is cached. Once cached, no gated action re-prompts.
#[test]
fn passphrase_missing_gates_only_locked_mode_without_cache() {
    let mut app = App::new();
    assert!(app.passphrase_missing(KeySourceMode::EncryptedFile));
    assert!(!app.passphrase_missing(KeySourceMode::EnvFirst));
    assert!(!app.passphrase_missing(KeySourceMode::KeychainFirst));
    assert!(!app.passphrase_missing(KeySourceMode::OsKeychain));

    app.passphrase = Some(Zeroizing::new("cached".to_string()));
    assert!(
        !app.passphrase_missing(KeySourceMode::EncryptedFile),
        "cached passphrase → gate never fires again this session"
    );
}

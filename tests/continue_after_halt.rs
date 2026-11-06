//! Continue-after-halt (loopeng-01-03): `/continue` riprende il loop dopo un
//! hard-stop (cap iterazioni o breaker di stagnazione).
//!
//! R1 — Parsing: `parse_command("/continue")` (e alias `/cont`) → `Command::Continue`.
//! R2 — Flag di halt in `App`: `halted` default `false`; `mark_halted()` lo imposta.
//! R3 — `continue_loop()` riprende solo dopo un halt: con `halted=true` passa a
//! `RunState::Thinking` + `pending_message` valorizzato e azzera `halted`;
//! con `halted=false` è no-op con messaggio di stato.

use nxm_tui::app::{parse_command, App, Command, RunState};

// ---- R1: parsing ----

#[test]
fn parse_continue_command() {
    assert_eq!(parse_command("/continue"), Command::Continue);
    assert_eq!(parse_command("/cont"), Command::Continue);
}

// ---- R2: flag di halt ----

#[test]
fn app_halted_defaults_false() {
    let app = App::new();
    assert!(!app.halted, "halted deve essere false di default");
}

#[test]
fn mark_halted_sets_flag() {
    let mut app = App::new();
    app.mark_halted();
    assert!(app.halted, "mark_halted deve impostare halted");
}

// ---- R3: continue riprende solo dopo un halt ----

#[test]
fn continue_loop_resumes_after_halt() {
    let mut app = App::new();
    app.mark_halted();
    assert!(app.halted);

    assert!(
        app.continue_loop(),
        "continue_loop deve tornare true dopo un halt"
    );
    assert!(!app.halted, "halted deve essere azzerato alla ripresa");
    assert_eq!(
        app.state,
        RunState::Thinking,
        "dopo continue lo stato deve essere Thinking"
    );
    assert!(
        app.pending_message.is_some(),
        "pending_message deve essere valorizzato per far ripartire lo spawn"
    );
}

#[test]
fn continue_loop_is_noop_without_halt() {
    let mut app = App::new();
    assert!(!app.halted);

    assert!(
        !app.continue_loop(),
        "continue_loop deve tornare false senza halt"
    );
    assert!(!app.halted, "halted deve restare false");
    assert_eq!(
        app.state,
        RunState::Running,
        "senza halt lo stato non deve cambiare"
    );
    assert!(
        app.pending_message.is_none(),
        "nessun pending_message senza halt"
    );
    assert!(
        app.status_message.is_some(),
        "deve esserci un messaggio di stato no-op"
    );
}

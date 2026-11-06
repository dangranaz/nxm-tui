use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::task::block_in_place;
use zeroize::Zeroizing;

use crate::app::{self, App, Command, Message, Role, RunState, ServerCommand};
use crate::keystore::KeyStoreError;

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('q') {
        app.state = RunState::Quit;
        return;
    }

    // Modal tool approval: force an explicit Y/N decision, swallow the rest
    // so a gated tool can never run (or be skipped) by an unrelated key.
    if app.pending_approval.is_some() {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => app.resolve_approval(true),
            KeyCode::Char('n') | KeyCode::Char('N') => app.resolve_approval(false),
            KeyCode::Esc => app.resolve_approval(false),
            _ => {}
        }
        return;
    }

    // Masked session-passphrase entry (I2): asked at most once per session,
    // buffer ZEROED after Enter/Esc — the passphrase is never logged (mai-log).
    // Wins over set-key input so the save flow can prompt on top of a
    // retained key buffer and resume afterwards.
    if let Some(entry) = app.passphrase_pending.as_mut() {
        match key.code {
            KeyCode::Enter => {
                let buffer = std::mem::take(&mut entry.buffer);
                app.passphrase_pending = None;
                // The buffer is `Zeroizing<String>`: take() moves the only
                // copy straight into the session cache — bytes stay scrubbed.
                app.passphrase = Some(buffer);
                tracing::info!(target: "nexum::keystore", "passphrase captured for session");
                let status = if app.set_key_pending.is_some() {
                    "Passphrase pronta — premi di nuovo Enter per salvare la key".to_string()
                } else {
                    "Passphrase memorizzata per la sessione".to_string()
                };
                app.set_status(status);
                return;
            }
            KeyCode::Esc => {
                app.passphrase_pending = None;
                // Declining cancels everything waiting on the passphrase, so
                // no gated action hangs or re-prompts in a loop.
                app.models_fetch_pending = None;
                if app.mcp_connect_pending {
                    app.mcp_connect_pending = false;
                    tracing::info!(target: "nexum::mcp", "connect cancelled: passphrase declined");
                }
                if app.state == RunState::Thinking && app.pending_message.is_some() {
                    app.stop_operation();
                    app.state = RunState::Running;
                    app.pending_message = None;
                }
                app.set_status("Passphrase annullata — azione in attesa annullata".into());
                return;
            }
            KeyCode::Backspace => {
                entry.buffer.pop();
                return;
            }
            KeyCode::Char(c) => {
                entry.buffer.push(c);
                return;
            }
            _ => {
                return;
            }
        }
    }

    // I2 write-path gate for `/provider set-key`: the save goes through the
    // KeyStore seam, and an encrypted-file store cannot be opened without the
    // session passphrase. Prompt FIRST (the key buffer is retained above) so
    // a second Enter performs the save — never a panic, never a blind write.
    if key.code == KeyCode::Enter
        && app.set_key_pending.is_some()
        && app.passphrase_missing(crate::keys::current_mode())
    {
        app.request_passphrase();
        app.set_status(
            "Keystore cifrato: inserisci la passphrase, poi premi di nuovo Enter".into(),
        );
        return;
    }

    // Masked key entry for `/provider set-key`: dedicated state, not prompt_state.
    // Buffer is ZEROED after save or Esc — key never lingers in memory.
    if let Some(entry) = app.set_key_pending.as_mut() {
        match key.code {
            KeyCode::Enter => {
                let provider_name = entry.provider.clone();
                let key_value = Zeroizing::new(std::mem::take(&mut entry.buffer));
                app.set_key_pending = None; // zeroed immediately

                // I2 write-path: save through the KeyStore seam (factory by
                // mode) via block_in_place — the store may hit the OS keychain
                // or the encrypted file. mai-log: the key value is never logged.
                let mode = crate::keys::current_mode();
                let pass = app.passphrase.clone();
                let store_name = provider_name.clone();
                let result = block_in_place(move || {
                    let store = crate::keystore::store_for(mode, pass.as_ref())?;
                    store.set(&store_name, &key_value)
                });
                match result {
                    Ok(()) => {
                        tracing::info!(target: "nexum::keystore", provider = %provider_name, mode = ?mode, "key stored");
                        app.set_status(format!("Key saved for {provider_name}"));
                    }
                    Err(KeyStoreError::Unavailable(msg)) => {
                        tracing::warn!(target: "nexum::keystore", provider = %provider_name, error = %msg, "keystore unavailable on save");
                        app.set_status(format!(
                            "Keystore non disponibile per {provider_name}: {msg}"
                        ));
                    }
                    Err(e) => {
                        tracing::error!(target: "nexum::keystore", provider = %provider_name, error = %e, "keystore save error");
                        app.set_status(format!("Key save error for {provider_name}: {e}"));
                    }
                }
                return;
            }
            KeyCode::Esc => {
                // Discard and zero
                app.set_key_pending = None;
                app.set_status("Key entry cancelled".into());
                return;
            }
            KeyCode::Backspace => {
                entry.buffer.pop();
                return;
            }
            KeyCode::Char(c) => {
                entry.buffer.push(c);
                return;
            }
            _ => {
                return;
            }
        }
    }

    // Interactive search mode: when active, all input goes to the query.
    // Esc closes, Enter confirms (keeps search active, closes query bar),
    // n/N navigate matches, other chars update the query live.
    if app.search.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.search = None;
                app.set_status("Search closed".into());
                return;
            }
            KeyCode::Enter => {
                // Confirm: keep search active but close query bar
                app.set_status("Search confirmed — use n/N to navigate, Esc to close".into());
                return;
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                let status_msg = if let Some(ref mut search) = app.search {
                    if search.matches.is_empty() {
                        return;
                    }
                    let len = search.matches.len();
                    let forward = key.code == KeyCode::Char('n');
                    // search.current is the POSITION in the matches list (0-based),
                    // not a line index. Wrap with modulo.
                    search.current = if forward {
                        (search.current + 1) % len
                    } else {
                        (search.current + len - 1) % len
                    };
                    // The actual scrollback jump is computed in render_history
                    // where lines.len() and area.height are known.
                    format!("Match {}/{}", search.current + 1, len)
                } else {
                    return;
                };
                app.set_status(status_msg);
                return;
            }
            KeyCode::Char(c) => {
                let status_msg = if let Some(ref mut search) = app.search {
                    search.query.push(c);
                    // Recompile matcher
                    search.matcher = Some(crate::search::TextMatcher::new(
                        search.query.clone(),
                        crate::search::QueryKind::Substring,
                    ));
                    // Recalculate matches (will be updated on next render)
                    search.matches.clear();
                    search.current = 0;
                    format!("Searching for: {}", search.query)
                } else {
                    return;
                };
                app.set_status(status_msg);
                return;
            }
            KeyCode::Backspace => {
                let status_msg = if let Some(ref mut search) = app.search {
                    search.query.pop();
                    if search.query.is_empty() {
                        app.search = None;
                        "Search closed".to_string()
                    } else {
                        search.matcher = Some(crate::search::TextMatcher::new(
                            search.query.clone(),
                            crate::search::QueryKind::Substring,
                        ));
                        search.matches.clear();
                        search.current = 0;
                        format!("Searching for: {}", search.query)
                    }
                } else {
                    return;
                };
                app.set_status(status_msg);
                return;
            }
            _ => {
                return;
            }
        }
    }

    // Slash-command popup: while open, arrows move the selection, Tab/Enter
    // complete the highlighted command, Esc dismisses it. Handled before the
    // generic key logic so it takes over navigation only when visible.
    if app.command_menu_open() {
        match key.code {
            KeyCode::Up => {
                app.command_menu_up();
                return;
            }
            KeyCode::Down => {
                app.command_menu_down();
                return;
            }
            KeyCode::Tab => {
                app.complete_command();
                return;
            }
            KeyCode::Enter if key.modifiers != KeyModifiers::ALT => {
                app.complete_command();
                return;
            }
            KeyCode::Esc => {
                app.command_menu_dismissed = true;
                app.command_menu_selected = 0;
                return;
            }
            _ => {}
        }
    }

    if key.code == KeyCode::Esc {
        if app.show_help {
            app.show_help = false;
            return;
        }
        if app.show_sessions {
            app.show_sessions = false;
            return;
        }
        if app.show_metrics {
            app.show_metrics = false;
            return;
        }
        if app.show_context {
            app.show_context = false;
            return;
        }
        if app.show_thinking {
            app.show_thinking = false;
            return;
        }
        if app.show_tool {
            app.show_tool = false;
            return;
        }
        if app.show_tools {
            app.show_tools = false;
            return;
        }
        if app.show_todo {
            app.show_todo = false;
            return;
        }
        if app.show_approval {
            app.show_approval = false;
            return;
        }
        if app.state == RunState::Thinking {
            app.stop_operation();
            app.state = RunState::Running;
            app.pending_message = None;
            return;
        }
        if !app.input.is_empty() {
            app.input.clear();
            return;
        }
    }

    if app.state == RunState::NoServer {
        if let KeyCode::Char(c) = key.code {
            if let Some(idx) = c.to_digit(10) {
                if idx >= 1 {
                    let cfg = crate::config::TuiConfig::load();
                    let providers = crate::provider::all_providers(&cfg.providers);
                    if let Some(p) = providers.get((idx - 1) as usize) {
                        app.endpoint = p.base_url.clone();
                        app.server_name = p.name.clone();
                        app.state = RunState::Connecting;
                    }
                }
            }
        }
        return;
    }

    // Tools overlay (T1): modal — every key routes to the overlay while it
    // is open (Esc already returned in the Esc block above; Ctrl chords for
    // other overlays are deliberately swallowed, same as search mode).
    if app.show_tools {
        crate::tools_overlay::handle_tools_key(app, key);
        return;
    }

    if key.modifiers == KeyModifiers::CONTROL {
        match key.code {
            KeyCode::Char('s') => {
                let name = app
                    .session_name
                    .clone()
                    .or_else(|| Some(chrono::Local::now().format("%Y%m%d_%H%M%S").to_string()));
                if let Some(ref n) = name.clone() {
                    match app::save_session(n, &app.messages, app.mode) {
                        Ok(()) => {
                            app.session_name = name;
                            app.set_status(format!("Session saved: {n}"));
                        }
                        Err(e) => app.set_status(format!("Save error: {e}")),
                    }
                }
                return;
            }
            KeyCode::Char('h') | KeyCode::Char('/') => {
                app.show_help = !app.show_help;
                app.show_sessions = false;
                return;
            }
            KeyCode::Char('t') => {
                app.show_sessions = !app.show_sessions;
                app.show_help = false;
                return;
            }
            KeyCode::Char('m') => {
                app.show_metrics = !app.show_metrics;
                app.show_help = false;
                app.show_sessions = false;
                return;
            }
            KeyCode::Char('u') => {
                app.show_context = !app.show_context;
                app.show_help = false;
                app.show_sessions = false;
                return;
            }
            KeyCode::Char('r') => {
                app.show_thinking = !app.show_thinking;
                app.show_help = false;
                app.show_sessions = false;
                app.show_metrics = false;
                app.show_context = false;
                app.show_tool = false;
                return;
            }
            KeyCode::Char('o') => {
                app.show_tool = !app.show_tool;
                app.show_help = false;
                app.show_sessions = false;
                app.show_metrics = false;
                app.show_context = false;
                app.show_thinking = false;
                return;
            }
            KeyCode::Char('e') => {
                // Tools selection overlay (T1). Ctrl+T was taken (sessions);
                // Ctrl+E is free.
                app.show_tools = !app.show_tools;
                if app.show_tools {
                    let len = app.tool_registry.tool_names().len();
                    app.tools_cursor = app.tools_cursor.min(len.saturating_sub(1));
                    app.show_help = false;
                    app.show_sessions = false;
                    app.show_metrics = false;
                    app.show_context = false;
                    app.show_thinking = false;
                    app.show_tool = false;
                }
                return;
            }
            KeyCode::Char('d') => {
                // Todo overlay (MT-3, read-only). Ctrl+D is free (not used
                // by any other overlay/action).
                app.show_todo = !app.show_todo;
                if app.show_todo {
                    app.show_help = false;
                    app.show_sessions = false;
                    app.show_metrics = false;
                    app.show_context = false;
                    app.show_thinking = false;
                    app.show_tool = false;
                    app.show_tools = false;
                }
                return;
            }
            KeyCode::Char('b') => {
                // Toggle sidebar visibility
                app.sidebar_open = !app.sidebar_open;
                app.set_status(
                    if app.sidebar_open {
                        "Sidebar enabled"
                    } else {
                        "Sidebar disabled"
                    }
                    .to_string(),
                );
                return;
            }
            KeyCode::Char('k') => {
                // Compact chat - handled in command parsing
                return;
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Enter if !app.input.is_empty() => {
            let input = std::mem::take(&mut app.input);
            let cmd = app::parse_command(&input);

            match cmd {
                Command::Quit => {
                    app.state = RunState::Quit;
                }
                Command::Clear => {
                    app.messages.clear();
                    app.stick_to_bottom();
                    app.set_status("Chat cleared".into());
                }
                Command::Help => {
                    app.show_help = !app.show_help;
                }
                Command::Metrics => {
                    app.show_metrics = !app.show_metrics;
                    app.show_help = false;
                    app.show_sessions = false;
                }
                Command::Context(None) => {
                    app.show_context = !app.show_context;
                    app.show_help = false;
                    app.show_sessions = false;
                }
                Command::Context(Some(n)) => {
                    app.max_context = n;
                    app.show_context = false;
                    app.set_status(format!("Max context set to {n} tokens"));
                }
                Command::Server(ServerCommand::Status) => {
                    // /server status — show server status
                    if let Some(ref proc) = app.server_process {
                        app.set_status(format!(
                            "Server running on port {} (PID {})",
                            proc.port,
                            proc.child.id()
                        ));
                        return;
                    }
                    match crate::server_proc::server_running_by_pid() {
                        Some(pid) => app.set_status(format!("Server running (PID {pid})")),
                        None => app.set_status("No server running".into()),
                    }
                }
                Command::Config(cfg_cmd) => {
                    match cfg_cmd {
                        crate::app::ConfigCommand::Status => {
                            let cfg = crate::config::TuiConfig::load();
                            let mut status = String::from("Config: ");
                            if let Some(ref ep) = cfg.endpoint {
                                status.push_str(&format!("endpoint={ep} "));
                            }
                            if let Some(ctx) = cfg.max_context {
                                status.push_str(&format!("context={ctx}"));
                            }
                            app.set_status(status);
                        }
                        crate::app::ConfigCommand::Generate { engine: _, model } => {
                            if model.is_empty() {
                                app.set_status(
                                    "Usage: /config generate <model_path> [engine]".into(),
                                );
                                return;
                            }
                            // Resolve model path (supports relative or ~/nexum/models/... )
                            let model_path = shellexpand::tilde(&model).to_string();
                            let model_dir = std::path::Path::new(&model_path);
                            if !model_dir.exists() {
                                // Try $HOME/nexum/models/<model>
                                let home = dirs::home_dir().unwrap_or_default();
                                let alt = home.join("nexum").join("models").join(&model);
                                if !alt.exists() {
                                    app.set_status(format!("Model not found: {model_path}"));
                                    return;
                                }
                                // TODO: introspect not yet migrated
                                app.set_status(
                                    "Model introspect not yet available (pending migration)"
                                        .to_string(),
                                );
                                return;
                            }
                            // TODO: introspect not yet migrated
                            app.set_status(
                                "Model introspect not yet available (pending migration)"
                                    .to_string(),
                            );
                        }
                    }
                }
                Command::Mode(mode) => {
                    app.mode = mode;
                    app.set_status(format!("Mode: {}", mode.label()));
                }
                Command::NewSession => {
                    app.new_session();
                }
                Command::SaveSession(name) => {
                    let n = name.unwrap_or_else(|| {
                        app.session_name.clone().unwrap_or_else(|| {
                            chrono::Local::now().format("%Y%m%d_%H%M%S").to_string()
                        })
                    });
                    match app::save_session(&n, &app.messages, app.mode) {
                        Ok(()) => {
                            app.session_name = Some(n.clone());
                            app.set_status(format!("Session saved: {n}"));
                        }
                        Err(e) => app.set_status(format!("Save error: {e}")),
                    }
                }
                Command::LoadSession(name) => match app::load_session(&name) {
                    Ok((msgs, mode)) => {
                        app.messages = msgs;
                        app.mode = mode;
                        app.session_name = Some(name.clone());
                        app.stick_to_bottom();
                        app.set_status(format!("Loaded: {name}"));
                    }
                    Err(e) => app.set_status(format!("Load error: {e}")),
                },
                Command::ListSessions => {
                    app.show_sessions = !app.show_sessions;
                }
                Command::Compact => {
                    // Placeholder: compression feature will be added later
                    app.set_status("Compression feature coming soon".to_string());
                }
                Command::Continue => {
                    // /continue riprende il loop solo dopo un halt.
                    // continue_loop gestisce sia il sì (ripresa) che il no (no-op),
                    // impostando in entrambi i casi un messaggio di stato chiaro.
                    app.continue_loop();
                }
                Command::Sidebar => {
                    app.sidebar_open = !app.sidebar_open;
                    app.set_status(
                        if app.sidebar_open {
                            "Sidebar enabled"
                        } else {
                            "Sidebar disabled"
                        }
                        .to_string(),
                    );
                }
                Command::Server(cmd) => match cmd {
                    crate::app::ServerCommand::Start => {
                        if let Some(pid) = crate::server_proc::server_running_by_pid() {
                            tracing::warn!(target: "nexum::server", pid, "Server already running via PID file");
                            app.set_status(format!("Server already running (PID {pid})"));
                        }
                        if app.server_is_running() {
                            app.set_status(format!(
                                "Server already running on port {}",
                                app.server_process.as_ref().unwrap().port
                            ));
                            return;
                        }
                        let port = 11434u16;
                        tracing::info!(target: "nexum::server", port, "Starting server from TUI");
                        match app.start_server(port) {
                            Ok(()) => {
                                tracing::info!(target: "nexum::server", port, "Server process spawned");
                                app.set_status(format!("Server starting on port {port}..."));
                            }
                            Err(e) => {
                                tracing::error!(target: "nexum::server", error = %e, "Failed to start server");
                                app.set_status(format!("Error: {e}"));
                            }
                        }
                    }
                    crate::app::ServerCommand::Stop => {
                        if app.server_is_running() {
                            tracing::info!(target: "nexum::server", "Stopping server (child process)");
                            match app.stop_server() {
                                Ok(()) => {
                                    tracing::info!(target: "nexum::server", "Server stopped");
                                    app.set_status("Server stopped".into());
                                    return;
                                }
                                Err(e) => {
                                    tracing::warn!(target: "nexum::server", error = %e, "Child stop failed, trying PID");
                                }
                            }
                        }
                        match crate::server_proc::stop_server_by_pid() {
                            Ok(()) => {
                                tracing::info!(target: "nexum::server", "Server stopped via PID");
                                app.set_status("Server stopped".into());
                            }
                            Err(e) => {
                                tracing::warn!(target: "nexum::server", error = %e, "No server to stop");
                                app.set_status(e.to_string());
                            }
                        }
                    }
                    crate::app::ServerCommand::Status => {
                        if let Some(ref proc) = app.server_process {
                            app.set_status(format!(
                                "Server running on port {} (PID {})",
                                proc.port,
                                proc.child.id()
                            ));
                            return;
                        }
                        match crate::server_proc::server_running_by_pid() {
                            Some(pid) => {
                                app.set_status(format!("Server running (PID {pid})"));
                            }
                            None => {
                                app.set_status("No server running".into());
                            }
                        }
                    }
                },
                Command::Provider(pcmd) => {
                    handle_provider_command(app, pcmd);
                }
                Command::Models => {
                    // Set pending state; the async main loop will fetch models.
                    app.models_fetch_pending = Some(app.endpoint.clone());
                    app.set_status("Fetching models...".into());
                }
                Command::ModelUse(x) => {
                    handle_model_use(app, &x);
                }
                Command::Normal(text) => {
                    if let Some(sys) = app.system_prompt_for_mode() {
                        if !app
                            .messages
                            .iter()
                            .any(|m| matches!(m.role, Role::System) && m.content == sys)
                        {
                            app.messages
                                .push(Message::new(Role::System, sys.to_string()));
                        }
                    }
                    app.messages.push(Message::new(Role::User, text.clone()));
                    app.stick_to_bottom();
                    app.pending_message = Some(text);
                    app.state = RunState::Thinking;
                }
            }
        }
        KeyCode::Enter
            if app.show_help || app.show_sessions || app.show_metrics || app.show_context =>
        {
            app.show_help = false;
            app.show_sessions = false;
            app.show_metrics = false;
            app.show_context = false;
        }
        KeyCode::Enter if app.state == RunState::Thinking => {
            // Cancel thinking on Enter
            if let Some(text) = app.prompt_state.submit() {
                app.input = text.clone();
            }
            app.state = RunState::Running;
            app.pending_message = None;
        }
        KeyCode::Enter => {
            // Handle Alt+Enter for multiline
            if key.modifiers == KeyModifiers::ALT {
                app.prompt_state.insert_newline();
                return;
            }
            // Submit prompt
            if let Some(text) = app.prompt_state.submit() {
                app.input = text.clone();
            }
        }
        KeyCode::Backspace => {
            // Handle Ctrl+Backspace for word delete, otherwise regular
            if key.modifiers == KeyModifiers::CONTROL {
                app.prompt_state.delete_word_backward();
            } else {
                app.prompt_state.delete_char();
            }
            // Editing the prompt re-opens the command popup if it applies.
            app.command_menu_dismissed = false;
            app.clamp_command_menu();
        }
        KeyCode::Char('/') if app.prompt_state.text().is_empty() && app.search.is_none() => {
            // Open interactive search mode
            app.search = Some(crate::app::SearchState {
                query: String::new(),
                matcher: None,
                matches: Vec::new(),
                current: 0,
            });
            app.set_status("Search: type query, n/N navigate, Enter confirm, Esc close".into());
        }
        KeyCode::Char(c) => {
            app.prompt_state.insert_char(c);
            // Editing the prompt re-opens the command popup if it applies.
            app.command_menu_dismissed = false;
            app.clamp_command_menu();
        }
        KeyCode::Up => {
            // History navigation with Alt/Ctrl modifiers
            if key.modifiers == KeyModifiers::ALT || key.modifiers == KeyModifiers::CONTROL {
                app.prompt_state
                    .navigate_history(crate::prompt::HistoryDirection::Up);
            } else {
                app.scroll_up();
            }
        }
        KeyCode::Down => {
            if key.modifiers == KeyModifiers::ALT || key.modifiers == KeyModifiers::CONTROL {
                app.prompt_state
                    .navigate_history(crate::prompt::HistoryDirection::Down);
            } else {
                app.scroll_down();
            }
        }
        KeyCode::PageUp => {
            app.page_up(10);
        }
        KeyCode::PageDown => {
            app.page_down(10);
        }
        KeyCode::Tab => {
            if app.input.starts_with('/') {
                // Command autocomplete
                let text = app.input.trim();
                if let Some((cmd, _)) = crate::autocomplete::find_matches(text, 1).first() {
                    app.input = cmd.clone();
                    app.prompt_state.set_text(cmd.clone());
                }
            } else {
                app.show_help = !app.show_help;
                app.show_sessions = false;
            }
        }
        _ => {}
    }
}

/// Handle `/model use <id|numero>`: select a model from the current list.
///
/// If `x` is a number, it maps to the index in `app.models_list`.
/// If `x` is a string, it matches the model id directly.
/// Sets `cfg.model_name` and `Provider.default_model` for the active provider.
fn handle_model_use(app: &mut App, x: &str) {
    let model_id = if let Ok(idx) = x.parse::<usize>() {
        // Numeric index into the current list
        if idx == 0 || idx > app.models_list.len() {
            app.set_status(format!(
                "Invalid model index: {idx} (1-{})",
                app.models_list.len()
            ));
            return;
        }
        app.models_list[idx - 1].id.clone()
    } else {
        // Direct id match
        if !app.models_list.iter().any(|m| m.id == x) {
            app.set_status(format!(
                "Unknown model: {x} — use /models to list available models"
            ));
            return;
        }
        x.to_string()
    };

    // Update config and provider default_model
    let mut cfg = crate::config::TuiConfig::load();
    cfg.model_name = Some(model_id.clone());

    // Update the active provider's default_model
    let providers = crate::provider::all_providers(&cfg.providers);
    if let Some(active) = providers.iter().find(|p| p.base_url == app.endpoint) {
        let mut updated = active.clone();
        updated.default_model = Some(model_id.clone());
        cfg.upsert_provider(updated);
    }

    cfg.save();
    app.model_name = Some(model_id.clone());
    app.set_status(format!("Model set to: {model_id}"));
    tracing::info!(target: "nexum::models", model = %model_id, "model selected");
}

/// Handle `/provider` subcommands: list, add, use, remove, set-key, remove-key.
/// Uses `TuiConfig` on demand (same pattern as `/config`) so custom providers persist to disk.
fn handle_provider_command(app: &mut App, pcmd: crate::app::ProviderCommand) {
    use crate::app::ProviderCommand;
    use crate::provider::{all_providers, find_provider, Provider};

    let mut cfg = crate::config::TuiConfig::load();
    match pcmd {
        ProviderCommand::List => {
            let names: Vec<String> = all_providers(&cfg.providers)
                .into_iter()
                .map(|p| {
                    let active = if p.base_url == app.endpoint { "*" } else { "" };
                    let key = if p.requires_api_key { " (key)" } else { "" };
                    format!("{}{}{}", active, p.name, key)
                })
                .collect();
            tracing::info!(target: "nexum::provider::list", count = names.len(), "listed providers");
            app.set_status(format!("Providers: {}", names.join(", ")));
        }
        ProviderCommand::Add { name, base_url } => match Provider::custom(&name, &base_url) {
            Ok(p) => {
                tracing::info!(target: "nexum::provider::add", name = %p.name, url = %p.base_url, "added provider");
                cfg.upsert_provider(p);
                app.set_status(format!("Provider added: {name} ({base_url})"));
            }
            Err(e) => {
                tracing::warn!(target: "nexum::provider::add", error = %e, "invalid provider");
                app.set_status(format!("Provider error: {e}"));
            }
        },
        ProviderCommand::Use(name) => match find_provider(&name, &cfg.providers) {
            Some(p) => {
                app.endpoint = p.base_url.clone();
                app.server_name = p.name.clone();
                app.state = RunState::Connecting;
                cfg.endpoint = Some(p.base_url.clone());
                cfg.save();
                // Check if key is missing for this provider (D7-4c guided error)
                if p.requires_api_key {
                    let mode = crate::keys::current_mode();
                    if app.passphrase_missing(mode) {
                        // I2: prompt instead of blocking on a store we cannot
                        // open yet — no spawn, no inline keychain/file hit.
                        app.request_passphrase();
                        app.set_status(format!(
                            "Passphrase richiesta — poi riprova `/provider use {name}`"
                        ));
                    } else {
                        let pass = app.passphrase.clone();
                        let provider = p.clone();
                        let resolution = block_in_place(move || {
                            crate::keys::resolve_key_via(&provider, mode, pass)
                        });
                        if matches!(resolution, crate::keys::KeyResolution::Missing) {
                            app.set_status(format!(
                                "Using {name} — key mancante: usa `/provider set-key {name}`"
                            ));
                        } else {
                            app.set_status(format!("Using provider: {}", p.name));
                        }
                    }
                } else {
                    app.set_status(format!("Using provider: {}", p.name));
                }
                tracing::info!(target: "nexum::provider::use", name = %p.name, url = %p.base_url, "selected provider");
            }
            None => app.set_status(format!("Unknown provider: {name}")),
        },
        ProviderCommand::Remove(name) => {
            if cfg.remove_provider(&name) {
                tracing::info!(target: "nexum::provider::remove", name = %name, "removed provider");
                app.set_status(format!("Provider removed: {name}"));
            } else {
                app.set_status(format!("Not a custom provider: {name}"));
            }
        }
        ProviderCommand::SetKey(name) => {
            // Verify provider exists
            let providers = all_providers(&cfg.providers);
            if providers.iter().any(|p| p.name.eq_ignore_ascii_case(&name)) {
                // Enter masked input mode for this provider
                app.set_key_pending = Some(crate::app::SetKeyEntry {
                    provider: name.clone(),
                    buffer: String::new(),
                });
                app.set_status(format!(
                    "Enter API key for {name} (masked, Enter=save, Esc=cancel)"
                ));
            } else {
                app.set_status(format!("Unknown provider: {name}"));
            }
        }
        ProviderCommand::RemoveKey(name) => {
            // I2 gate: an encrypted-file store cannot be opened without the
            // session passphrase — prompt instead of failing the delete.
            let mode = crate::keys::current_mode();
            if app.passphrase_missing(mode) {
                app.request_passphrase();
                app.set_status(format!(
                    "Passphrase richiesta — poi riprova `/provider remove-key {name}`"
                ));
                return;
            }
            // I2 write-path: delete through the KeyStore seam (factory by
            // mode) via block_in_place instead of a direct `keyring::Entry`.
            let pass = app.passphrase.clone();
            let name_clone = name.clone();
            let result = block_in_place(move || {
                let store = crate::keystore::store_for(mode, pass.as_ref())?;
                store.delete(&name_clone)
            });
            match result {
                Ok(()) => {
                    tracing::info!(target: "nexum::keystore", provider = %name, mode = ?mode, "key deleted");
                    app.set_status(format!("Key removed for {name}"));
                }
                Err(KeyStoreError::Unavailable(msg)) => {
                    tracing::warn!(target: "nexum::keystore", provider = %name, error = %msg, "keystore unavailable on delete");
                    app.set_status(format!("Keystore non disponibile per {name}: {msg}"));
                }
                Err(e) => {
                    tracing::error!(target: "nexum::keystore", provider = %name, error = %e, "keystore delete error");
                    app.set_status(format!("Key delete error for {name}: {e}"));
                }
            }
        }
        ProviderCommand::MigrateKeys => {
            // Import every provider key from the OS keychain into the portable
            // encrypted file. The destination is passphrase-locked, so gate on
            // the session passphrase first (prompt, never block/spawn blind).
            let passphrase = match app.passphrase.clone() {
                Some(p) if !p.is_empty() => p,
                _ => {
                    app.request_passphrase();
                    app.set_status(
                        "Passphrase richiesta — poi riprova `/provider migrate-keys`".to_string(),
                    );
                    return;
                }
            };
            let names: Vec<String> = all_providers(&cfg.providers)
                .into_iter()
                .filter(|p| p.requires_api_key)
                .map(|p| p.name)
                .collect();
            let result = block_in_place(move || {
                let source = crate::keystore::OsKeychainStore::new();
                let dest = crate::keystore::EncryptedFileStore::new(passphrase.as_str())?;
                Ok::<_, KeyStoreError>(crate::keystore::migrate_keys(&source, &dest, &names))
            });
            match result {
                Ok(report) => {
                    // mai-log: only counts, never a value.
                    tracing::info!(
                        target: "nexum::keystore::migrate",
                        migrated = report.migrated,
                        absent = report.absent,
                        errors = report.errors,
                        "migrate-keys complete"
                    );
                    app.set_status(report.status_line());
                }
                Err(e) => {
                    tracing::warn!(target: "nexum::keystore::migrate", error = %e, "migrate-keys: destination keystore unavailable");
                    app.set_status(format!("Migrate-keys: keystore non disponibile: {e}"));
                }
            }
        }
    }
}

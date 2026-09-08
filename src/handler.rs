use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::app::{self, App, Command, Message, Role, RunState, ServerCommand};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('q') {
        app.state = RunState::Quit;
        return;
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

    if key.modifiers == KeyModifiers::CONTROL {
        match key.code {
            KeyCode::Char('s') => {
                let name = app.session_name.clone().or_else(|| {
                    Some(chrono::Local::now().format("%Y%m%d_%H%M%S").to_string())
                });
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
            KeyCode::Char('b') => {
                // Toggle sidebar visibility
                app.sidebar_open = !app.sidebar_open;
                app.set_status(if app.sidebar_open { "Sidebar enabled" } else { "Sidebar disabled" }.to_string());
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
                    app.scroll_offset = 0;
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
                        crate::app::ConfigCommand::Generate { engine, model } => {
                            if model.is_empty() {
                                app.set_status("Usage: /config generate <model_path> [engine]".into());
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
                                app.set_status("Model introspect not yet available (pending migration)".to_string());
                                return;
                            }
                            // TODO: introspect not yet migrated
                            app.set_status("Model introspect not yet available (pending migration)".to_string());
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
                Command::LoadSession(name) => {
                    match app::load_session(&name) {
                        Ok((msgs, mode)) => {
                            app.messages = msgs;
                            app.mode = mode;
                            app.session_name = Some(name.clone());
                            app.scroll_offset = 0;
                            app.set_status(format!("Loaded: {name}"));
                        }
                        Err(e) => app.set_status(format!("Load error: {e}")),
                    }
                }
                Command::ListSessions => {
                    app.show_sessions = !app.show_sessions;
                }
                Command::Compact => {
                    // Placeholder: compression feature will be added later
                    app.set_status("Compression feature coming soon".to_string());
                }
                Command::Sidebar => {
                    app.sidebar_open = !app.sidebar_open;
                    app.set_status(if app.sidebar_open { "Sidebar enabled" } else { "Sidebar disabled" }.to_string());
                }
                Command::Server(cmd) => {
                    match cmd {
                        crate::app::ServerCommand::Start => {
                            match crate::server_proc::server_running_by_pid() {
                                Some(pid) => {
                                    tracing::warn!(target: "nexum::server", pid, "Server already running via PID file");
                                    app.set_status(format!("Server already running (PID {pid})"));
                                }
                                None => {}
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
                                    app.set_status(format!("{e}"));
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
                    }
                }
                Command::Provider(pcmd) => {
                    handle_provider_command(app, pcmd);
                }
                Command::Normal(text) => {
                    if let Some(sys) = app.system_prompt_for_mode() {
                        if !app.messages.iter().any(|m| {
                            matches!(m.role, Role::System) && m.content == sys
                        }) {
                            app.messages.push(Message::new(Role::System, sys.to_string()));
                        }
                    }
                    app.messages.push(Message::new(Role::User, text.clone()));
                    app.scroll_offset = 0;
                    app.pending_message = Some(text);
                    app.state = RunState::Thinking;
                }
            }
        }
        KeyCode::Enter if app.show_help || app.show_sessions || app.show_metrics || app.show_context => {
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
        }
        KeyCode::Char(c) => {
            app.prompt_state.insert_char(c);
        }
        KeyCode::Up => {
            // History navigation with Alt/Ctrl modifiers
            if key.modifiers == KeyModifiers::ALT || key.modifiers == KeyModifiers::CONTROL {
                app.prompt_state.navigate_history(crate::prompt::HistoryDirection::Up);
            } else {
                app.scroll_offset = app.scroll_offset.saturating_add(1);
            }
        }
        KeyCode::Down => {
            if key.modifiers == KeyModifiers::ALT || key.modifiers == KeyModifiers::CONTROL {
                app.prompt_state.navigate_history(crate::prompt::HistoryDirection::Down);
            } else {
                app.scroll_offset = app.scroll_offset.saturating_sub(1);
            }
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

/// Handle `/provider` subcommands: list, add, use, remove. Uses `TuiConfig`
/// on demand (same pattern as `/config`) so custom providers persist to disk.
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
                if p.requires_api_key && cfg.api_key.is_none() {
                    let hint = p
                        .api_key_env
                        .as_deref()
                        .unwrap_or("NEXUM_API_KEY");
                    app.set_status(format!("Using {} — set ${hint} for auth", p.name));
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
    }
}

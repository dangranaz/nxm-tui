//! Nexum Terminal — pure client TUI for LLM chat.
//!
//! Connects to any OpenAI-compatible server (Nexum Inferentia, Ollama, LM Studio).
//! No server management, no HW detection — just chat.

use std::io;

use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tracing::info;

use nxm_tui::app::{App, Message, Role, RunState};
use nxm_tui::config::TuiConfig;
use nxm_tui::event::AppEvent;
use nxm_tui::handler::handle_key;
use nxm_tui::keys::{resolve_key, KeyResolution};
use nxm_tui::tool_types::{ApprovalRequest, ToolPart};
use nxm_tui::ui::render;
use nxm_tui::{app, agent, event, provider};

fn main() -> io::Result<()> {
    // Init logging (file-based, daily rotation)
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("NEXUM_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = rt.block_on(run(&mut terminal));

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

async fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut app = App::new();

    // Load or create config
    let mut cfg = TuiConfig::load();

    // Auto-detect endpoint
    let endpoint = detect_endpoint(&cfg).await;

    match endpoint {
        Some((url, name)) => {
            app.endpoint = url;
            app.server_name = name;
            app.state = RunState::Running;

            // Query /v1/status to get actual role assignments
            let client = reqwest::Client::new();
            let status_url = format!("{}/v1/status", app.endpoint);
            if let Ok(resp) = client.get(&status_url).send().await {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    if let Some(roles) = data["roles"].as_array() {
                        for role in roles {
                            let role_name = role["role"].as_str().unwrap_or("");
                            let model_id = role["model_id"].as_str().unwrap_or("");
                            let status = role["status"].as_str().unwrap_or("unknown").to_string();
                            match role_name {
                                "router" => {
                                    app.model_router = app::ModelRoleInfo {
                                        role: "Router",
                                        name: model_id.to_string(),
                                        status: status.clone(),
                                    };
                                }
                                "worker" => {
                                    app.model_worker = app::ModelRoleInfo {
                                        role: "Worker",
                                        name: model_id.to_string(),
                                        status: status.clone(),
                                    };
                                }
                                "indexer" => {
                                    app.model_indexer = app::ModelRoleInfo {
                                        role: "Indexer",
                                        name: model_id.to_string(),
                                        status,
                                    };
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }

            info!(endpoint = %app.endpoint, "connected to {}", app.server_name);
        }
        None => {
            app.state = RunState::NoServer;
        }
    }

    // Main loop
    let (part_tx, mut part_rx) = mpsc::unbounded_channel::<ToolPart>();
    let (approval_tx, mut approval_rx) = mpsc::unbounded_channel::<ApprovalRequest>();
    let mut inference_task: Option<tokio::task::JoinHandle<()>> = None;

    // Draw-on-change (T3): the frame is rebuilt only when something visible
    // changed (key, resize, token drain, status/health change, spinner tick).
    // Idle ticks do no draw work at all.
    let mut dirty = true;

    loop {
        if app.state == RunState::Quit {
            // D4: cancel any in-flight agent stream on quit (Ctrl-C / Ctrl-Q)
            // so no orphaned tokio task keeps the SSE channel open. The partial
            // assistant message already lives in app.messages (not lost).
            if let Some(handle) = inference_task.take() {
                handle.abort();
            }
            break;
        }

        if dirty {
            terminal.draw(|f| render(f, &mut app))?;
            dirty = false;
        }

        match event::poll_event()? {
            AppEvent::Key(key) => {
                handle_key(&mut app, key);

                // Handle server selection from NoServer menu
                if app.state == RunState::Connecting {
                    let url = app.endpoint.clone();
                    app.state = RunState::Running;
                    if app.server_name.is_empty() {
                        app.server_name = server_name_from_url(&url, &cfg);
                    }
                    cfg.endpoint = Some(url.clone());
                    cfg.save();

                    // Query /v1/status to get actual role assignments
                    let client = reqwest::Client::new();
                    let status_url = format!("{}/v1/status", app.endpoint);
                    if let Ok(resp) = client.get(&status_url).send().await {
                        if let Ok(data) = resp.json::<serde_json::Value>().await {
                            if let Some(roles) = data["roles"].as_array() {
                                for role in roles {
                                    let role_name = role["role"].as_str().unwrap_or("");
                                    let model_id = role["model_id"].as_str().unwrap_or("");
                                    let status = role["status"].as_str().unwrap_or("unknown").to_string();
                                    match role_name {
                                        "router" => {
                                            app.model_router = app::ModelRoleInfo {
                                                role: "Router",
                                                name: model_id.to_string(),
                                                status: status.clone(),
                                            };
                                        }
                                        "worker" => {
                                            app.model_worker = app::ModelRoleInfo {
                                                role: "Worker",
                                                name: model_id.to_string(),
                                                status: status.clone(),
                                            };
                                        }
                                        "indexer" => {
                                            app.model_indexer = app::ModelRoleInfo {
                                                role: "Indexer",
                                                name: model_id.to_string(),
                                                status,
                                            };
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }

                    info!(endpoint = %url, "connected");
                }

                // Start inference
                if app.state == RunState::Thinking && app.pending_message.is_some() && inference_task.is_none() {
                    app.start_operation("Thinking");
                    app.metrics.start_response();
                    let _msg = app.pending_message.take().unwrap();
                    app.metrics.session_user_msgs += 1;
                    let base = app.endpoint.clone();
                    let client = reqwest::Client::new();
                    let model = cfg.model_name.clone().unwrap_or_else(|| "default".to_string());
                    let initial_msgs = app.messages.clone();
                    let tx = part_tx.clone();
                    let approval = approval_tx.clone();

                    // Resolve API key for the active provider (once per agent spawn)
                    let providers = provider::all_providers(&cfg.providers);
                    let active_provider = providers.iter().find(|p| {
                        p.base_url == base || p.base_url.trim_end_matches("/v1") == base.trim_end_matches("/v1")
                    }).cloned();

                    let api_key = if let Some(ref p) = active_provider {
                        if p.requires_api_key {
                            let provider = p.clone();
                            match tokio::task::spawn_blocking(move || resolve_key(&provider)).await {
                                Ok(resolution) => {
                                    // One-time warning if keychain unavailable and key from env
                                    if let KeyResolution::Env { keychain_available: false, .. } = &resolution {
                                        app.set_status("keychain non disponibile — key da env (non persistente)".to_string());
                                    }
                                    resolution.into_key()
                                }
                                Err(_) => None,
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    inference_task = Some(tokio::spawn(async move {
                        let mut agent = agent::Agent::new(
                            &client, &base, &model, api_key, initial_msgs,
                            Some(approval),
                        );
                        agent.run(&tx).await.unwrap_or_else(|e| {
                            tracing::error!("agent error: {e}");
                        });
                    }));
                }
                dirty = true;
            }
            AppEvent::Resize(_, _) => {
                dirty = true;
            }
            AppEvent::Tick => {
                app.tick = app.tick.wrapping_add(1);
                let mut changed = app.tick_status();
                changed |= app.check_server_health();

                if matches!(app.state, RunState::Thinking) {
                    while let Ok(req) = approval_rx.try_recv() {
                        app.pending_approval = Some(app::PendingApproval {
                            invocation: req.invocation,
                            reply: req.reply,
                        });
                        app.show_approval = true;
                        changed = true;
                    }
                    while let Ok(part) = part_rx.try_recv() {
                        match part {
                            ToolPart::Text(s) => {
                                app.metrics.record_chars(s.chars().count());
                                app.push_token(&s);
                            }
                            ToolPart::Reasoning(s) => app.append_thinking_delta(&s),
                            ToolPart::ToolInvocation(inv) => {
                                app.messages.push(Message::with_tool(
                                    Role::Assistant,
                                    vec![ToolPart::ToolInvocation(inv)],
                                ));
                            }
                            ToolPart::ToolResult(res) => {
                                app.messages.push(Message::tool(&res.call_id, res.output.clone()));
                            }
                            ToolPart::Error(e) => app.set_status(e),
                        }
                        changed = true;
                    }
                    if let Some(ref handle) = inference_task {
                        if handle.is_finished() {
                            inference_task = None;
                            app.metrics.finish_response();
                            app.state = RunState::Running;
                            changed = true;
                        }
                    }
                }
                // The Thinking spinner animates per tick: keep redrawing while
                // working even when the stream stalls between tokens.
                if changed || app.is_working() {
                    dirty = true;
                }
            }
        }
    }

    // Cleanup: kill server if we started it
    if let Some(mut proc) = app.server_process {
        let _ = proc.child.kill();
        let _ = proc.child.wait();
    }

    Ok(())
}

async fn detect_endpoint(cfg: &TuiConfig) -> Option<(String, String)> {
    // Check saved config first
    if let Some(ref url) = cfg.endpoint {
        let client = reqwest::Client::new();
        let resp = client.get(format!("{}/v1/models", url)).send().await;
        if resp.is_ok() {
            return Some((url.clone(), server_name_from_url(url, cfg)));
        }
    }

    // Try the known providers (presets + custom), skipping cloud ones which
    // cannot be auto-detected without a key.
    let providers = provider::all_providers(&cfg.providers);
    let client = reqwest::Client::new();
    for p in providers {
        if p.is_cloud {
            continue;
        }
        // Probe the base as-is and with a /v1 suffix (bare Nexum host vs OpenAI dialect).
        let candidates = [p.base_url.clone(), format!("{}/v1", p.base_url.trim_end_matches("/v1"))];
        for base in candidates {
            let resp = client.get(format!("{}/v1/models", base.trim_end_matches("/v1"))).send().await;
            if resp.is_ok() {
                return Some((p.base_url.clone(), p.name.clone()));
            }
        }
    }

    None
}

fn server_name_from_url(url: &str, cfg: &TuiConfig) -> String {
    provider::all_providers(&cfg.providers)
        .into_iter()
        .find(|p| p.base_url == url || p.base_url.trim_end_matches("/v1") == url.trim_end_matches("/v1"))
        .map(|p| p.name)
        .unwrap_or_else(|| "Custom Server".to_string())
}

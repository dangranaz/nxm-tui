//! Nexum Terminal — pure client TUI for LLM chat.
//!
//! Connects to any OpenAI-compatible server (Nexum Inferentia, Ollama, LM Studio).
//! No server management, no HW detection — just chat.

use std::io;

use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tracing::info;

use nxm_tui::app::{App, Message, Role, RunState};
use nxm_tui::config::TuiConfig;
use nxm_tui::event::AppEvent;
use nxm_tui::handler::handle_key;
use nxm_tui::keys::{current_mode, resolve_key_via, KeyResolution};
use nxm_tui::mcp::client::McpHandle;
use nxm_tui::models::fetch_models;
use nxm_tui::tool_types::{ApprovalRequest, ToolPart};
use nxm_tui::ui::render;
use nxm_tui::{agent, app, event, provider};
use zeroize::Zeroizing;

fn main() -> io::Result<()> {
    // Load a local .env if present (dev-friendly key loading). Keys defined
    // there enter the process environment and are picked up by `keys.rs`
    // env-first mode, so no OS keychain prompt appears during development.
    // Absent in production → no-op.
    let _ = dotenvy::dotenv();

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
            // Sync model_name from config so the mode bar shows the configured
            // model immediately (single source of truth: cfg.model_name).
            app.model_name = cfg.model_name.clone();

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

    // MCP host (sampling): connect as soon as the keystore passphrase
    // situation is resolved (I2). In EncryptedFile mode the session
    // passphrase is not cached yet at startup → the connect is DEFERRED to
    // the loop, where the masked prompt runs first (Esc cancels it).
    // Kept alive for the whole loop via `mcp_handle` (dropping it stops the
    // run loop). A clean no-op when disabled/absent — never blocks chat.
    let mut mcp_handle: Option<McpHandle> = None;
    app.mcp_connect_pending = app.state == RunState::Running;
    if app.mcp_connect_pending && !app.passphrase_missing(current_mode()) {
        app.mcp_connect_pending = false;
        mcp_handle = open_mcp(&cfg, &app.endpoint, app.passphrase.clone()).await;
    }

    // Main loop
    let (part_tx, mut part_rx) = mpsc::unbounded_channel::<ToolPart>();
    let (approval_tx, mut approval_rx) = mpsc::unbounded_channel::<ApprovalRequest>();
    let mut inference_task: Option<tokio::task::JoinHandle<()>> = None;
    let mut models_fetch_task: Option<
        tokio::task::JoinHandle<anyhow::Result<Vec<nxm_tui::models::ModelEntry>>>,
    > = None;

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

        // I2: deferred MCP connect — fires once the session passphrase is
        // cached; while it is missing the masked prompt runs instead (Esc
        // clears mcp_connect_pending and the connect dies quietly).
        if app.mcp_connect_pending {
            if app.passphrase_missing(current_mode()) {
                app.request_passphrase();
                dirty = true;
            } else {
                app.mcp_connect_pending = false;
                mcp_handle = open_mcp(&cfg, &app.endpoint, app.passphrase.clone()).await;
            }
        }

        // Consume pending /models fetch: spawn async task, never block the UI.
        if app.models_fetch_pending.is_some() && models_fetch_task.is_none() {
            let mode = current_mode();
            let base_url_hint = app.models_fetch_pending.clone().unwrap_or_default();
            let providers = provider::all_providers(&cfg.providers);
            let active_provider = providers
                .iter()
                .find(|p| {
                    p.base_url == base_url_hint
                        || p.base_url.trim_end_matches("/v1")
                            == base_url_hint.trim_end_matches("/v1")
                })
                .cloned();

            // I2 gate (same rule as the agent spawn): prompt instead of
            // spawning only when the key is actually needed — the fetch
            // resumes on Enter (pending kept) and dies on Esc (pending
            // cleared in handler.rs).
            let passphrase_gate = active_provider.as_ref().is_some_and(|p| p.requires_api_key)
                && app.passphrase_missing(mode);

            if passphrase_gate {
                app.request_passphrase();
                dirty = true;
            } else {
                let base_url = app.models_fetch_pending.take().unwrap_or_default();

                // Resolve API key for the active provider (same pattern as Agent spawn)
                let pass = app.passphrase.clone();
                let api_key = if let Some(ref p) = active_provider {
                    if p.requires_api_key {
                        let provider = p.clone();
                        match tokio::task::spawn_blocking(move || {
                            resolve_key_via(&provider, mode, pass)
                        })
                        .await
                        {
                            Ok(resolution) => resolution.into_key(),
                            Err(_) => None,
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                let client = reqwest::Client::new();
                models_fetch_task = Some(tokio::spawn(async move {
                    fetch_models(&client, &base_url, api_key.as_deref()).await
                }));
            }
        }

        // Check if the models fetch task is finished
        if let Some(ref handle) = models_fetch_task {
            if handle.is_finished() {
                if let Some(handle) = models_fetch_task.take() {
                    let result = handle.await;
                    match result {
                        Ok(Ok(models)) => {
                            let count = models.len();
                            app.models_list = models;
                            app.set_status(format!("Loaded {count} models"));
                            dirty = true;
                        }
                        Ok(Err(e)) => {
                            app.set_status(format!("Error: {e}"));
                            dirty = true;
                        }
                        Err(e) => {
                            app.set_status(format!("Fetch error: {e}"));
                            dirty = true;
                        }
                    }
                }
            }
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
                    // Sync model_name from config (single source of truth: cfg.model_name)
                    app.model_name = cfg.model_name.clone();
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
                                    let status =
                                        role["status"].as_str().unwrap_or("unknown").to_string();
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
                if app.state == RunState::Thinking
                    && app.pending_message.is_some()
                    && inference_task.is_none()
                {
                    let base = app.endpoint.clone();
                    // Hard cap on ReAct iterations (loopeng-01): from config,
                    // falling back to the single source of the default.
                    let max_steps = cfg.max_steps.unwrap_or(nxm_tui::bounded::DEFAULT_MAX_STEPS);
                    // Stagnation breaker (loopeng-01-02): from config,
                    // falling back to the single source of the default.
                    let stall_threshold = cfg
                        .stall_threshold
                        .unwrap_or(nxm_tui::circuit_breaker::DEFAULT_STALL_THRESHOLD);

                    // Resolve the active provider once (I2): used by the
                    // passphrase gate below and by the key resolution.
                    let providers = provider::all_providers(&cfg.providers);
                    let active_provider = providers
                        .iter()
                        .find(|p| {
                            p.base_url == base
                                || p.base_url.trim_end_matches("/v1")
                                    == base.trim_end_matches("/v1")
                        })
                        .cloned();
                    let mode = current_mode();
                    let passphrase_gate =
                        active_provider.as_ref().is_some_and(|p| p.requires_api_key)
                            && app.passphrase_missing(mode);

                    if passphrase_gate {
                        // I2: prompt instead of spawning — pending_message is
                        // kept intact, so Enter on the prompt resumes this
                        // send and Esc cancels it (handler.rs).
                        app.request_passphrase();
                    } else if let Some(_msg) = app.pending_message.take() {
                        // Nuova inferenza (invio utente o /continue): il flag
                        // di halt si azzera — il budget fresco parte da zero.
                        app.halted = false;
                        app.start_operation("Thinking");
                        app.metrics.start_response();
                        app.metrics.session_user_msgs += 1;
                        let client = reqwest::Client::new();
                        let model = cfg
                            .model_name
                            .clone()
                            .unwrap_or_else(|| "default".to_string());
                        let initial_msgs = app.messages.clone();
                        let tx = part_tx.clone();
                        let approval = approval_tx.clone();
                        let pass = app.passphrase.clone();
                        // T1: hand the agent a snapshot of the tool selection
                        // (toggles in the Tools overlay apply from the next spawn).
                        let tool_registry = app.tool_registry.clone();
                        // T2: plan mode handle — shared with Agent for write-abstaining filter
                        let plan_mode_handle = app.plan_mode_handle.clone();

                        // Resolve API key for the active provider (once per agent spawn)
                        let api_key = if let Some(ref p) = active_provider {
                            if p.requires_api_key {
                                let provider = p.clone();
                                match tokio::task::spawn_blocking(move || {
                                    resolve_key_via(&provider, mode, pass)
                                })
                                .await
                                {
                                    Ok(resolution) => {
                                        // One-time warning if keychain unavailable and key from env
                                        if let KeyResolution::Env {
                                            keychain_available: false,
                                            ..
                                        } = &resolution
                                        {
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
                                &client,
                                &base,
                                &model,
                                api_key,
                                initial_msgs,
                                Some(approval),
                                tool_registry,
                                plan_mode_handle,
                                max_steps,
                                stall_threshold,
                            );
                            agent.run(&tx).await.unwrap_or_else(|e| {
                                tracing::error!("agent error: {e}");
                            });
                        }));
                    }
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
                                app.messages
                                    .push(Message::tool(&res.call_id, res.output.clone()));
                            }
                            ToolPart::Error(e) => app.set_status(e),
                            ToolPart::LoopHalted { reason, steps } => {
                                let status =
                                    format!("loop halted after {steps} steps \u{2014} {reason}");
                                app.messages.push(Message::with_tool(
                                    Role::Assistant,
                                    vec![ToolPart::LoopHalted { reason, steps }],
                                ));
                                app.set_status(status);
                                app.mark_halted(); // abilita /continue
                            }
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

    if mcp_handle.is_some() {
        info!(target: "nexum::mcp", "MCP session closed");
    }

    Ok(())
}

/// Open the MCP session from config, resolving the provider key through the
/// KeyStore seam (I2). Returns `None` on skip/failure so chat is unaffected.
async fn open_mcp(
    cfg: &TuiConfig,
    endpoint: &str,
    passphrase: Option<Zeroizing<String>>,
) -> Option<McpHandle> {
    match nxm_tui::mcp::connect_from_config(cfg, endpoint, passphrase).await {
        Ok(Some(handle)) => {
            info!(target: "nexum::mcp", endpoint, "MCP session open");
            Some(handle)
        }
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(target: "nexum::mcp", error = %e, "MCP connect failed; continuing without it");
            None
        }
    }
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
        let candidates = [
            p.base_url.clone(),
            format!("{}/v1", p.base_url.trim_end_matches("/v1")),
        ];
        for base in candidates {
            let resp = client
                .get(format!("{}/v1/models", base.trim_end_matches("/v1")))
                .send()
                .await;
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
        .find(|p| {
            p.base_url == url || p.base_url.trim_end_matches("/v1") == url.trim_end_matches("/v1")
        })
        .map(|p| p.name)
        .unwrap_or_else(|| "Custom Server".to_string())
}

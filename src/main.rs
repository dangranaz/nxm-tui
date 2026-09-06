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

mod app;
mod agent;
mod autocomplete;
mod bottom;
mod config;
mod connection;
mod event;
mod handler;
mod history;
mod markdown;
mod mode_bar;
mod overlays;
mod prompt;
mod prompt_lines;
mod server_proc;
mod session;
mod sidebar;
mod tool_types;
mod ui;

// Utils module
mod utils;

use app::{App, Message, Role, RunState};
use config::TuiConfig;
use event::AppEvent;
use handler::handle_key;
use tool_types::ToolPart;
use ui::render;

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
    let mut inference_task: Option<tokio::task::JoinHandle<()>> = None;

    loop {
        if app.state == RunState::Quit {
            break;
        }

        terminal.draw(|f| render(f, &app))?;

        match event::poll_event()? {
            AppEvent::Key(key) => {
                handle_key(&mut app, key);

                // Handle server selection from NoServer menu
                if app.state == RunState::Connecting {
                    let url = app.endpoint.clone();
                    app.state = RunState::Running;
                    app.server_name = server_name_from_url(&url);
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

                    inference_task = Some(tokio::spawn(async move {
                        let mut agent = agent::Agent::new(&client, &base, &model, initial_msgs);
                        agent.run(&tx).await.unwrap_or_else(|e| {
                            tracing::error!("agent error: {e}");
                        });
                    }));
                }
            }
            AppEvent::Resize(_, _) => {}
            AppEvent::Tick => {
                app.tick = app.tick.wrapping_add(1);
                app.tick_status();
                app.check_server_health();

                if matches!(app.state, RunState::Thinking) {
                    while let Ok(part) = part_rx.try_recv() {
                        match part {
                            ToolPart::Text(s) => app.push_token(&s),
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
                    }
                    if let Some(ref handle) = inference_task {
                        if handle.is_finished() {
                            inference_task = None;
                            app.metrics.finish_response();
                            app.state = RunState::Running;
                        }
                    }
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
            return Some((url.clone(), server_name_from_url(url)));
        }
    }

    // Try common endpoints
    let endpoints = vec![
        ("http://127.0.0.1:11434", "Nexum Local"),
        ("http://127.0.0.1:11435", "Ollama"),
        ("http://127.0.0.1:1234", "LM Studio"),
    ];

    let client = reqwest::Client::new();
    for (url, name) in endpoints {
        let resp = client.get(format!("{}/v1/models", url)).send().await;
        if resp.is_ok() {
            return Some((url.to_string(), name.to_string()));
        }
    }

    None
}

fn server_name_from_url(url: &str) -> String {
    if url.contains("11434") {
        "Nexum Local".to_string()
    } else if url.contains("11435") {
        "Ollama".to_string()
    } else if url.contains("1234") {
        "LM Studio".to_string()
    } else {
        "Custom Server".to_string()
    }
}

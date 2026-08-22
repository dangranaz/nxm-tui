use std::collections::VecDeque;
use std::time::Instant;

// Re-export ServerProcess for App struct
use crate::server_proc::ServerProcess;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    Running,
    Thinking,
    NoServer,
    Connecting,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentMode {
    Chat,
    Architect,
    Developer,
    Researcher,
}

impl AgentMode {
    pub fn label(&self) -> &'static str {
        match self {
            AgentMode::Chat => "Chat",
            AgentMode::Architect => "Architect",
            AgentMode::Developer => "Developer",
            AgentMode::Researcher => "Researcher",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
    /// Optional tool parts for tool call output.
    pub tool_parts: Vec<crate::tool_types::ToolPart>,
}

impl Message {
    pub fn new(role: Role, content: String) -> Self {
        Self { role, content, tool_parts: Vec::new() }
    }

    pub fn with_tool(role: Role, parts: Vec<crate::tool_types::ToolPart>) -> Self {
        Self { role, content: String::new(), tool_parts: parts }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Metrics,
    Clear,
    Context(Option<u32>),
    Mode(AgentMode),
    Config(ConfigCommand),
    NewSession,
    SaveSession(Option<String>),
    LoadSession(String),
    ListSessions,
    Compact,
    Sidebar,
    Server(ServerCommand),
    Quit,
    Normal(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerCommand {
    Start,
    Stop,
    Status,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigCommand {
    Status,
    Generate { engine: String, model: String },
}

/// Model role status for the bottom bar.
#[derive(Debug, Clone)]
pub struct ModelRoleInfo {
    pub role: &'static str,
    pub name: String,
    pub status: String,
}

impl ModelRoleInfo {
    pub fn none(role: &'static str) -> Self {
        Self { role, name: "—".into(), status: "not loaded".into() }
    }
}

pub fn parse_command(input: &str) -> Command {
    if !input.starts_with('/') {
        return Command::Normal(input.to_string());
    }
    let trimmed = input.trim();
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    match tokens[0] {
        "/help" | "/h" | "/?" => Command::Help,
        "/clear" => Command::Clear,
        "/metrics" | "/m" => Command::Metrics,
        "/context" | "/ctx" => {
            let n = tokens.get(1).and_then(|s| s.parse::<u32>().ok());
            Command::Context(n)
        }
        "/quit" | "/q" => Command::Quit,
        "/mode" if tokens.len() > 1 => match tokens[1] {
            "chat" | "c" => Command::Mode(AgentMode::Chat),
            "architect" | "arch" | "a" => Command::Mode(AgentMode::Architect),
            "developer" | "dev" | "d" => Command::Mode(AgentMode::Developer),
            "researcher" | "research" | "r" => Command::Mode(AgentMode::Researcher),
            _ => Command::Normal(input.to_string()),
        },
        "/new" | "/session" if tokens.get(1).copied() == Some("new") => Command::NewSession,
        "/session" if tokens.len() == 1 => Command::ListSessions,
        "/save" => {
            let name = tokens.get(1).map(|s| s.to_string());
            Command::SaveSession(name)
        }
        "/load" if tokens.len() > 1 => Command::LoadSession(tokens[1].to_string()),
        "/session" if tokens.len() > 2 && tokens[1] == "load" => {
            Command::LoadSession(tokens[2].to_string())
        }
        "/session" if tokens.len() > 2 && tokens[1] == "save" => {
            let name = tokens.get(2).map(|s| s.to_string());
            Command::SaveSession(name)
        }
        "/compact" | "/k" | "/comp" => Command::Compact,
        "/sidebar" => Command::Sidebar,
        "/server" | "/srv" => match tokens.get(1) {
            Some(&"start") | Some(&"on") => Command::Server(ServerCommand::Start),
            Some(&"stop") | Some(&"off") => Command::Server(ServerCommand::Stop),
            Some(&"status") | Some(&"st") => Command::Server(ServerCommand::Status),
            _ => Command::Server(ServerCommand::Status),
        },
        "/config" | "/cnf" => match tokens.get(1) {
            Some(&"status") | Some(&"st") | None => Command::Config(ConfigCommand::Status),
            Some(&"generate") | Some(&"gen") => {
                // /config generate mlx model OR /config generate ssd mlx model
                let args: Vec<&str> = tokens.iter().skip(2).cloned().collect();
                if args.is_empty() {
                    Command::Normal(input.to_string())
                } else if args[0] == "ssd" {
                    // /config generate ssd <engine> <model>
                    Command::Config(ConfigCommand::Generate {
                        engine: format!("ssd-{}", args.get(1).unwrap_or(&"mlx")),
                        model: args.get(2).unwrap_or(&"").to_string(),
                    })
                } else {
                    // /config generate <engine> <model>
                    Command::Config(ConfigCommand::Generate {
                        engine: args[0].to_string(),
                        model: args.get(1).unwrap_or(&"").to_string(),
                    })
                }
            }
            _ => Command::Normal(input.to_string()),
        },
        _ => Command::Normal(input.to_string()),
    }
}

// Re-export session functions for convenience
pub use crate::session::{list_sessions, load_session, save_session, session_dir, sanitize_session_name};

/// Get current directory name (for sidebar display).
pub fn current_dir_name() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_default()
}

/// Get current git branch name (if in a git repository).
pub fn git_branch() -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

pub struct App {
    pub state: RunState,
    /// Legacy single-line input field (for compatibility).
    /// New code should use prompt_state.
    pub input: String,
    /// Prompt state for multiline input + history.
    pub prompt_state: crate::prompt::PromptState,
    pub messages: Vec<Message>,
    pub scroll_offset: u16,
    pub tick: u32,
    pub pending_message: Option<String>,
    pub endpoint: String,
    pub server_name: String,
    pub operation_label: String,
    pub operation_start: Option<Instant>,
    pub menu_selection: usize,
    pub mode: AgentMode,
    pub session_name: Option<String>,
    pub show_help: bool,
    pub show_sessions: bool,
    pub show_metrics: bool,
    pub show_context: bool,
    pub max_context: u32,
    pub metrics: Metrics,
    pub status_message: Option<String>,
    pub status_timer: u32,
    pub server_process: Option<ServerProcess>,
    pub in_thinking: bool,
    pub thinking_content: String,
    pub model_indexer: ModelRoleInfo,
    pub model_router: ModelRoleInfo,
    pub model_worker: ModelRoleInfo,
    /// Sidebar visibility (auto/hide based on terminal width)
    pub sidebar_open: bool,
}

#[derive(Debug, Clone)]
pub struct Metrics {
    /// When current response started
    pub response_start: Option<Instant>,
    /// Char count of current response so far
    pub response_char_count: usize,
    /// Last N response times in milliseconds
    pub response_times_ms: VecDeque<u64>,
    /// Last N response estimated token counts
    pub response_tokens: VecDeque<u64>,
    /// Total chars generated ever (across sessions)
    pub total_chars: u64,
    /// Total tokens estimated ever
    pub total_tokens: u64,
    /// Total responses ever
    pub total_responses: u64,
    /// User messages in current session
    pub session_user_msgs: usize,
    /// When current session started
    pub session_start: Instant,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            response_start: None,
            response_char_count: 0,
            response_times_ms: VecDeque::with_capacity(50),
            response_tokens: VecDeque::with_capacity(50),
            total_chars: 0,
            total_tokens: 0,
            total_responses: 0,
            session_user_msgs: 0,
            session_start: Instant::now(),
        }
    }

    pub fn start_response(&mut self) {
        self.response_start = Some(Instant::now());
        self.response_char_count = 0;
    }

    pub fn record_chars(&mut self, n: usize) {
        self.response_char_count += n;
        self.total_chars += n as u64;
    }

    pub fn finish_response(&mut self) {
        self.total_responses += 1;
        let elapsed = self
            .response_start
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0);
        let tokens = (self.response_char_count / 4).max(1) as u64;
        self.total_tokens += tokens;
        self.response_times_ms.push_back(elapsed);
        self.response_tokens.push_back(tokens);
        if self.response_times_ms.len() > 50 {
            self.response_times_ms.pop_front();
        }
        if self.response_tokens.len() > 50 {
            self.response_tokens.pop_front();
        }
    }

    pub fn reset_session(&mut self) {
        self.response_times_ms.clear();
        self.response_tokens.clear();
        self.session_user_msgs = 0;
        self.session_start = Instant::now();
    }

    pub fn avg_response_time_ms(&self) -> u64 {
        let n = self.response_times_ms.len();
        if n == 0 { return 0; }
        self.response_times_ms.iter().sum::<u64>() / n as u64
    }

    pub fn avg_tokens_per_response(&self) -> u64 {
        let n = self.response_tokens.len();
        if n == 0 { return 0; }
        self.response_tokens.iter().sum::<u64>() / n as u64
    }

    pub fn tokens_per_second(&self) -> f64 {
        let total_ms: u64 = self.response_times_ms.iter().sum();
        if total_ms == 0 { return 0.0; }
        let total_tok: u64 = self.response_tokens.iter().sum();
        (total_tok as f64) / (total_ms as f64 / 1000.0)
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            state: RunState::Running,
            input: String::new(),
            prompt_state: crate::prompt::PromptState::new(),
            messages: Vec::new(),
            scroll_offset: 0,
            tick: 0,
            pending_message: None,
            endpoint: String::new(),
            server_name: String::new(),
            operation_label: String::new(),
            operation_start: None,
            menu_selection: 0,
            mode: AgentMode::Chat,
            session_name: None,
            show_help: false,
            show_sessions: false,
            show_metrics: false,
            show_context: false,
            max_context: 8192,
            metrics: Metrics::new(),
            status_message: None,
            status_timer: 0,
            server_process: None,
            in_thinking: false,
            thinking_content: String::new(),
            model_indexer: ModelRoleInfo::none("Indexer"),
            model_router: ModelRoleInfo::none("Router"),
            model_worker: ModelRoleInfo::none("Worker"),
            sidebar_open: true,
        }
    }

    pub fn start_operation(&mut self, label: &str) {
        self.operation_label = label.to_string();
        self.operation_start = Some(Instant::now());
    }

    pub fn stop_operation(&mut self) {
        self.operation_label.clear();
        self.operation_start = None;
    }

    pub fn operation_elapsed(&self) -> u64 {
        self.operation_start.map(|s| s.elapsed().as_secs()).unwrap_or(0)
    }

    pub fn is_working(&self) -> bool {
        matches!(self.state, RunState::Thinking)
    }

    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some(msg);
        self.status_timer = 120;
    }

    pub fn tick_status(&mut self) {
        if self.status_timer > 0 {
            self.status_timer -= 1;
            if self.status_timer == 0 {
                self.status_message = None;
            }
        }
    }

    pub fn clear_session(&mut self) {
        self.messages.clear();
        self.session_name = None;
        self.scroll_offset = 0;
        self.metrics.reset_session();
        self.set_status("Session cleared".into());
    }

    pub fn new_session(&mut self) {
        self.messages.clear();
        self.session_name = None;
        self.scroll_offset = 0;
        self.mode = AgentMode::Chat;
        self.metrics.reset_session();
        self.set_status("New session started".into());
    }

    pub fn session_display_name(&self) -> String {
        self.session_name
            .clone()
            .unwrap_or_else(|| "unnamed".to_string())
    }

    pub fn system_prompt_for_mode(&self) -> Option<&'static str> {
        match self.mode {
            AgentMode::Chat => None,
            AgentMode::Architect => Some(
                "You are an expert software architect. Focus on high-level design, architecture decisions, \
                 and planning. Do not write code unless specifically asked. Provide clear technical \
                 documents and design proposals."
            ),
            AgentMode::Developer => Some(
                "You are a skilled software developer. Write clean, idiomatic, well-tested code. \
                 Follow existing project conventions. Prefer simple solutions over complex ones."
            ),
            AgentMode::Researcher => Some(
                "You are a technical researcher. Analyze codebases, find patterns, explain how things \
                 work. You have read-only access — do not modify files. Provide thorough analysis."
            ),
        }
    }

    /// Append a delta to the thinking stream.
    pub fn append_thinking_delta(&mut self, delta: &str) {
        self.thinking_content.push_str(delta);
    }

    /// Get and clear the thinking content.
    pub fn take_thinking(&mut self) -> Option<String> {
        if self.thinking_content.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.thinking_content))
        }
    }

    pub fn estimated_tokens(&self) -> usize {
        self.messages.iter().map(|m| m.content.len() / 4 + 4).sum::<usize>().max(8)
    }

    pub fn server_is_running(&self) -> bool {
        self.server_process.is_some()
    }

    pub fn push_token(&mut self, token: &str) {
        // Filter reasoning tokens (Ollama puts thinking in <thinking>...</thinking> tags)
        if token == "<" || token == "thinking" || token == ">" || token == "/thinking" {
            return;
        }

        if self.in_thinking {
            if token.contains('>') {
                self.in_thinking = false;
                return;
            }
            self.thinking_content.push_str(token);
            return;
        }

        if token.contains("<thinking>") {
            self.in_thinking = true;
            let parts: Vec<&str> = token.split("<thinking>").collect();
            if parts.len() > 1 && !parts[0].is_empty() {
                if let Some(last_msg) = self.messages.last_mut() {
                    if last_msg.role == Role::Assistant {
                        last_msg.content.push_str(parts[0]);
                        return;
                    }
                }
                self.messages.push(Message::new(Role::Assistant, parts[0].to_string()));
            }
            return;
        }

        // Find the last assistant message or create one
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == Role::Assistant {
                last_msg.content.push_str(token);
                return;
            }
        }
        self.messages.push(Message::new(Role::Assistant, token.to_string()));
    }

    pub fn context_pct(&self) -> f64 {
        let used = self.estimated_tokens() as f64;
        let max = self.max_context as f64;
        if max == 0.0 { return 0.0; }
        (used / max * 100.0).min(100.0)
    }
}

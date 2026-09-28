use std::cell::Cell;
use std::collections::VecDeque;
use std::time::Instant;

// Re-export ServerProcess for App struct
use crate::server_proc::ServerProcess;

/// A tool call waiting for the user's Y/N decision in the approval overlay.
/// Holds the oneshot `reply` back to the agent task: `resolve_approval`
/// answers it and clears the slot. Dropped without answering only on quit,
/// which aborts the agent task first (fail-safe deny on its end).
pub struct PendingApproval {
    pub invocation: crate::tool_types::ToolInvocation,
    pub reply: tokio::sync::oneshot::Sender<bool>,
}

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
    /// `tool_call_id` for `role: "tool"` messages (OpenAI round-trip).
    pub tool_call_id: Option<String>,
}

impl Message {
    pub fn new(role: Role, content: String) -> Self {
        Self { role, content, tool_parts: Vec::new(), tool_call_id: None }
    }

    pub fn with_tool(role: Role, parts: Vec<crate::tool_types::ToolPart>) -> Self {
        Self { role, content: String::new(), tool_parts: parts, tool_call_id: None }
    }

    /// A `role: "tool"` result message carrying the assistant `tool_call_id`.
    pub fn tool(call_id: &str, content: String) -> Self {
        Self {
            role: Role::Tool,
            content,
            tool_parts: Vec::new(),
            tool_call_id: Some(call_id.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

/// Append streaming text to the thinking buffer, stripping a trailing partial
/// tag fragment ("hidden</thinking" arriving before its ">") so the overlay
/// never shows tag noise.
fn push_thinking(thinking: &mut String, token: &str) {
    if token == "<" || token == "thinking" || token == "/thinking" || token == "</thinking" {
        return;
    }
    let mut clean = token;
    for frag in ["</thinking", "</think", "</", "<"] {
        if let Some(stripped) = clean.strip_suffix(frag) {
            clean = stripped;
            break;
        }
    }
    thinking.push_str(clean);
}

/// Absorb one token delta into `messages`, filtering `<thinking>` tags into
/// `thinking` (stateful via `in_thinking`) and otherwise appending to the last
/// assistant message. Shared by `App::push_token` (main loop) and
/// `agent::Agent` (its transcript) so `<thinking>` handling stays consistent
/// between the live UI and the transcript the agent re-serializes.
pub(crate) fn apply_token(
    messages: &mut Vec<Message>,
    in_thinking: &mut bool,
    thinking: &mut String,
    token: &str,
) {
    // Inside a thinking span the closing tag must exit thinking mode even when
    // the tokenizer splits it ("hidden", "</thinking", ">", "visible"). Text
    // before the tag belongs to thinking, text after belongs to the answer.
    if *in_thinking {
        if let Some((before, after)) = token.split_once("</thinking>") {
            push_thinking(thinking, before);
            *in_thinking = false;
            if !after.is_empty() {
                apply_token(messages, in_thinking, thinking, after);
            }
            return;
        }
        if let Some(idx) = token.find('>') {
            // Bare closing bracket (standalone ">" or ">visible"): anything
            // before it is tag noise, anything after is the answer.
            *in_thinking = false;
            let after = token[idx + 1..].to_string();
            if !after.is_empty() {
                apply_token(messages, in_thinking, thinking, &after);
            }
            return;
        }
        push_thinking(thinking, token);
        return;
    }
    if token == "<" || token == "thinking" || token == ">" || token == "/thinking" {
        return;
    }
    if token.contains("<thinking>") {
        *in_thinking = true;
        let parts: Vec<&str> = token.split("<thinking>").collect();
        if parts.len() > 1 && !parts[0].is_empty() {
            if let Some(last_msg) = messages.last_mut() {
                if last_msg.role == Role::Assistant {
                    last_msg.content.push_str(parts[0]);
                } else {
                    messages.push(Message::new(Role::Assistant, parts[0].to_string()));
                }
            } else {
                messages.push(Message::new(Role::Assistant, parts[0].to_string()));
            }
        }
        // Content after the opening tag belongs to thinking (and may itself
        // contain the closing tag), so feed it back through.
        if parts.len() > 1 && !parts[1].is_empty() {
            apply_token(messages, in_thinking, thinking, parts[1]);
        }
        return;
    }
    // A closing tag arriving outside thinking mode (single token carrying both
    // tags, e.g. "foo</thinking>bar"): drop the tag, keep both sides.
    if token.contains("</thinking>") {
        let parts: Vec<&str> = token.split("</thinking>").collect();
        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }
            if i == 0 {
                if let Some(last_msg) = messages.last_mut() {
                    if last_msg.role == Role::Assistant {
                        last_msg.content.push_str(part);
                        continue;
                    }
                }
                messages.push(Message::new(Role::Assistant, part.to_string()));
            } else {
                apply_token(messages, in_thinking, thinking, part);
            }
        }
        return;
    }
    if let Some(last_msg) = messages.last_mut() {
        if last_msg.role == Role::Assistant {
            last_msg.content.push_str(token);
            return;
        }
    }
    messages.push(Message::new(Role::Assistant, token.to_string()));
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
    Provider(ProviderCommand),
    Quit,
    Normal(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderCommand {
    /// `/provider` — list available providers.
    List,
    /// `/provider add <name> <base_url>` — add/replace a custom provider.
    Add { name: String, base_url: String },
    /// `/provider use <name>` — select a provider as the active endpoint.
    Use(String),
    /// `/provider remove <name>` — remove a custom provider.
    Remove(String),
    /// `/provider set-key <name>` — store API key in keychain for provider.
    SetKey(String),
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
        "/provider" | "/prov" | "/p" => match tokens.get(1).copied() {
            None | Some("list") | Some("ls") => Command::Provider(ProviderCommand::List),
            Some("add") => {
                // /provider add <name> <base_url> — name may be multi-word if
                // quoted is not supported, so take token[2] as name, token[3] as url.
                match (tokens.get(2), tokens.get(3)) {
                    (Some(name), Some(url)) => Command::Provider(ProviderCommand::Add {
                        name: name.to_string(),
                        base_url: url.to_string(),
                    }),
                    _ => Command::Normal(input.to_string()),
                }
            }
            Some("use") | Some("select") => match tokens.get(2) {
                Some(name) => Command::Provider(ProviderCommand::Use(name.to_string())),
                None => Command::Normal(input.to_string()),
            },
            Some("remove") | Some("rm") | Some("del") => match tokens.get(2) {
                Some(name) => Command::Provider(ProviderCommand::Remove(name.to_string())),
                None => Command::Normal(input.to_string()),
            },
            Some("set-key") => match tokens.get(2) {
                Some(name) => Command::Provider(ProviderCommand::SetKey(name.to_string())),
                None => Command::Normal(input.to_string()),
            },
            _ => Command::Normal(input.to_string()),
        },
        _ => Command::Normal(input.to_string()),
    }
}

// Re-export session functions for convenience
pub use crate::session::{list_sessions, load_session, save_session, session_dir};

/// Get current directory name (for sidebar display).
///
/// Cached once: the render loop calls this every frame and
/// `std::env::current_dir()` is a syscall per frame, while the process never
/// chdirs mid-session. ponytail: computed once per process; switch to a TTL
/// cache (like `git_branch`) if a future feature ever chdirs.
pub fn current_dir_name() -> String {
    use std::sync::OnceLock;

    static CACHE: OnceLock<String> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            std::env::current_dir()
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .unwrap_or_default()
        })
        .clone()
}

/// Get current git branch name (if in a git repository).
///
/// Cached with a short TTL: the render loop calls this every frame, and
/// spawning `git` per frame hammered macOS Gatekeeper/XProtect (syspolicyd
/// pegged a core doing repeated malware scans of the un-notarized binary's
/// child processes). The branch changes rarely, so we refresh at most once
/// every few seconds and serve the cached value in between.
pub fn git_branch() -> Option<String> {
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    const TTL: Duration = Duration::from_secs(5);
    // (last_refresh, cached_branch)
    static CACHE: OnceLock<Mutex<(Option<Instant>, Option<String>)>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new((None, None)));

    let mut guard = match cache.lock() {
        Ok(g) => g,
        Err(_) => return None,
    };

    let fresh = guard
        .0
        .map(|t| t.elapsed() < TTL)
        .unwrap_or(false);
    if fresh {
        return guard.1.clone();
    }

    // Stale (or first call): refresh by spawning git once.
    let branch = git_branch_uncached();
    guard.0 = Some(Instant::now());
    guard.1 = branch.clone();
    branch
}

/// Actually shell out to `git` to read the current branch. Not called per
/// frame — only through `git_branch()`'s TTL cache.
fn git_branch_uncached() -> Option<String> {
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
    /// Per-message render cache for the chat history (see `history::HistoryCache`).
    pub history_cache: crate::history::HistoryCache,
    /// Lines hidden below the viewport (0 = pinned to the live tail).
    /// Up scrolls into older lines, Down returns toward the tail; any new
    /// token keeps a pinned view glued to the bottom automatically.
    pub scrollback: u16,
    pub tick: u32,
    pub pending_message: Option<String>,
    pub endpoint: String,
    pub server_name: String,
    pub operation_label: String,
    pub operation_start: Option<Instant>,
    #[allow(dead_code)] // reserved for NoServer menu keyboard selection
    pub menu_selection: usize,
    pub mode: AgentMode,
    pub session_name: Option<String>,
    pub show_help: bool,
    pub show_sessions: bool,
    pub show_metrics: bool,
    pub show_context: bool,
    pub show_thinking: bool,
    pub show_tool: bool,
    pub show_approval: bool,
    pub pending_approval: Option<PendingApproval>,
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
    /// Memoized `estimated_tokens` — O(N) over messages, called several
    /// times per frame by the render path. Fingerprinted by (len, last
    /// message) so it recomputes only when messages actually change.
    token_estimate_cache: Cell<Option<((usize, Option<(usize, u8)>), usize)>>,
    /// Sidebar visibility (auto/hide based on terminal width)
    pub sidebar_open: bool,
    /// Selected index in the slash-command popup. The popup is visible
    /// whenever the prompt text starts with '/' and has matches; this only
    /// tracks which entry is highlighted.
    pub command_menu_selected: usize,
    /// Set when the user dismisses the popup (Esc, or after completing a
    /// command) so it stays closed until they edit the prompt again. Reset
    /// on the next keystroke that changes the prompt text.
    pub command_menu_dismissed: bool,
    /// Pending `/provider set-key` entry: provider name + secret buffer.
    /// The buffer is ZEROED after save or Esc — key never lingers in memory.
    pub set_key_pending: Option<SetKeyEntry>,
}

/// Entry for `/provider set-key` prompt: provider name + masked buffer.
#[derive(Debug, Clone)]
pub struct SetKeyEntry {
    pub provider: String,
    pub buffer: String,
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
            history_cache: crate::history::HistoryCache::new(),
            scrollback: 0,
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
            show_thinking: false,
            show_tool: false,
            show_approval: false,
            pending_approval: None,
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
            token_estimate_cache: Cell::new(None),
            sidebar_open: true,
            command_menu_selected: 0,
            command_menu_dismissed: false,
            set_key_pending: None,
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

    /// Age the status timer one tick. Returns true only when something
    /// visible changed (the status message expired and was cleared) — the
    /// main loop redraws only on change.
    pub fn tick_status(&mut self) -> bool {
        if self.status_timer > 0 {
            self.status_timer -= 1;
            if self.status_timer == 0 {
                self.status_message = None;
                return true;
            }
        }
        false
    }

    #[allow(dead_code)] // exposed for /clear-session; not yet bound to a key
    pub fn clear_session(&mut self) {
        self.messages.clear();
        self.session_name = None;
        self.scrollback = 0;
        self.metrics.reset_session();
        self.set_status("Session cleared".into());
    }

    pub fn new_session(&mut self) {
        self.messages.clear();
        self.session_name = None;
        self.scrollback = 0;
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
    #[allow(dead_code)] // consumed by the reasoning-overlay path (not yet wired)
    pub fn take_thinking(&mut self) -> Option<String> {
        if self.thinking_content.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.thinking_content))
        }
    }

    /// Fingerprint of `messages` for the token-estimate cache: cheap O(1)
    /// (message count + last-message identity). Sound because only the last
    /// assistant message is ever mutated after push (`apply_token`); the
    /// count changes on any new/cleared message.
    fn messages_fingerprint(&self) -> (usize, Option<(usize, u8)>) {
        (
            self.messages.len(),
            self.messages.last().map(|m| (m.content.len(), m.role as u8)),
        )
    }

    /// Estimated token count for the transcript (len/4+4 per message,
    /// floored at 8). Memoized — the render path calls this several times
    /// per frame; recomputed only when messages change.
    pub fn estimated_tokens(&self) -> usize {
        let fp = self.messages_fingerprint();
        if let Some((cached_fp, value)) = self.token_estimate_cache.get() {
            if cached_fp == fp {
                return value;
            }
        }
        let value = self
            .messages
            .iter()
            .map(|m| m.content.len() / 4 + 4)
            .sum::<usize>()
            .max(8);
        self.token_estimate_cache.set(Some((fp, value)));
        value
    }

    pub fn server_is_running(&self) -> bool {
        self.server_process.is_some()
    }

    pub fn push_token(&mut self, token: &str) {
        apply_token(
            &mut self.messages,
            &mut self.in_thinking,
            &mut self.thinking_content,
            token,
        );
    }

    /// Scroll one line toward older history.
    pub fn scroll_up(&mut self) {
        self.scrollback = self.scrollback.saturating_add(1);
    }

    /// Scroll one page toward older history.
    pub fn page_up(&mut self, page: u16) {
        self.scrollback = self.scrollback.saturating_add(page.max(1));
    }

    /// Scroll one line toward the live tail.
    pub fn scroll_down(&mut self) {
        self.scrollback = self.scrollback.saturating_sub(1);
    }

    /// Scroll one page toward the live tail.
    pub fn page_down(&mut self, page: u16) {
        self.scrollback = self.scrollback.saturating_sub(page.max(1));
    }

    /// Pin the view to the live tail (new tokens stay visible).
    pub fn stick_to_bottom(&mut self) {
        self.scrollback = 0;
    }

    /// True while the view is pinned to the live tail.
    #[allow(dead_code)] // public scroll-model API; the history overlay reads `scrollback` directly
    pub fn following(&self) -> bool {
        self.scrollback == 0
    }

    /// Ratatui scroll offset for a `total`-line buffer in a `height`-line
    /// viewport with `scrollback` lines hidden below. Clamps so short buffers
    /// pin to the top and deep scrollbacks pin to the oldest line.
    pub fn viewport_offset(total: usize, height: usize, scrollback: u16) -> u16 {
        let max = u16::try_from(total.saturating_sub(height)).unwrap_or(u16::MAX);
        max.saturating_sub(scrollback.min(max))
    }

    /// Answer the pending tool approval (true = run, false = skip), hide the
    /// overlay and clear the slot. A failed send means the agent task is
    /// already gone — still clear so the UI never sticks on a dead prompt.
    pub fn resolve_approval(&mut self, allow: bool) {
        if let Some(pending) = self.pending_approval.take() {
            let _ = pending.reply.send(allow);
            self.set_status(if allow {
                format!("Approved: {}", pending.invocation.name)
            } else {
                format!("Denied: {} (skipped)", pending.invocation.name)
            });
        }
        self.show_approval = false;
    }

    pub fn context_pct(&self) -> f64 {
        let used = self.estimated_tokens() as f64;
        let max = self.max_context as f64;
        if max == 0.0 { return 0.0; }
        (used / max * 100.0).min(100.0)
    }

    /// Current slash-command popup entries for the prompt text, in the fixed
    /// alphabetical order defined in `autocomplete::COMMANDS`. Empty when the
    /// prompt does not start with '/' or nothing matches.
    pub fn command_menu(&self) -> Vec<(&'static str, &'static str)> {
        crate::autocomplete::menu_matches(&self.prompt_state.text())
    }

    /// True when the slash-command popup should be shown.
    pub fn command_menu_open(&self) -> bool {
        !self.command_menu_dismissed && !self.command_menu().is_empty()
    }

    /// Move the popup selection one entry up (wraps to the bottom).
    pub fn command_menu_up(&mut self) {
        let len = self.command_menu().len();
        if len == 0 {
            return;
        }
        self.command_menu_selected = (self.command_menu_selected + len - 1) % len;
    }

    /// Move the popup selection one entry down (wraps to the top).
    pub fn command_menu_down(&mut self) {
        let len = self.command_menu().len();
        if len == 0 {
            return;
        }
        self.command_menu_selected = (self.command_menu_selected + 1) % len;
    }

    /// The command string currently highlighted in the popup, if any.
    pub fn command_menu_selection(&self) -> Option<&'static str> {
        let menu = self.command_menu();
        if menu.is_empty() {
            return None;
        }
        let idx = self.command_menu_selected.min(menu.len() - 1);
        Some(menu[idx].0)
    }

    /// Clamp the selection into range after the filtered list shrinks
    /// (e.g. the user typed another character). Called when the prompt changes.
    pub fn clamp_command_menu(&mut self) {
        let len = self.command_menu().len();
        if len == 0 {
            self.command_menu_selected = 0;
        } else if self.command_menu_selected >= len {
            self.command_menu_selected = len - 1;
        }
    }

    /// Complete the prompt with the currently highlighted command and dismiss
    /// the popup. Commands that take arguments get a trailing space so the
    /// user can keep typing; argument-less commands are left ready to submit.
    pub fn complete_command(&mut self) {
        if let Some(cmd) = self.command_menu_selection() {
            let takes_args = matches!(
                cmd,
                "/config" | "/mode" | "/provider" | "/server" | "/load" | "/save" | "/context"
            );
            let text = if takes_args {
                format!("{cmd} ")
            } else {
                cmd.to_string()
            };
            self.prompt_state.set_text(text);
            self.command_menu_dismissed = true;
            self.command_menu_selected = 0;
        }
    }
}

#[cfg(test)]
mod git_branch_cache_tests {
    use super::git_branch;
    use std::time::Instant;

    /// The render loop calls `git_branch()` every frame. The TTL cache must
    /// make repeated calls cheap (no per-call `git` spawn) and consistent.
    /// A per-frame spawn is what pegged macOS syspolicyd; this guards it.
    #[test]
    fn repeated_calls_are_cached_and_cheap() {
        // Prime the cache once (may spawn git a single time).
        let first = git_branch();

        // Simulate a burst of frames. If each call spawned `git`, 2000
        // iterations would take hundreds of ms to seconds; with the cache
        // they are in-memory clones and finish near-instantly.
        let start = Instant::now();
        for _ in 0..2000 {
            let b = git_branch();
            // Value stays consistent within the TTL window.
            assert_eq!(b, first);
        }
        let elapsed = start.elapsed();

        // Generous bound: 2000 cached reads must be well under the time a
        // single process spawn per call would cost. Even one git spawn is
        // typically >1ms, so 2000 real spawns would be >>200ms.
        assert!(
            elapsed.as_millis() < 200,
            "2000 cached git_branch() calls took {:?} — cache likely not working \
             (would spawn git per frame)",
            elapsed
        );
    }
}

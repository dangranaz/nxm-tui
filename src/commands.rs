//! Slash-command vocabulary: the `Command` enum and its sub-enums, plus the
//! parser that turns prompt input into a `Command`.
//!
//! Extracted verbatim from `app.rs` (T6 split, no behavior change) and
//! re-exported from `app` so existing call sites keep working.

use crate::app::AgentMode;

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
    /// `/continue` — riprende il loop dopo un hard-stop (cap o breaker).
    /// Ri-spawn un nuovo `Agent` con il transcript corrente e budget fresco.
    Continue,
    Sidebar,
    Server(ServerCommand),
    Provider(ProviderCommand),
    Models,
    ModelUse(String),
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
    /// `/provider remove-key <name>` — delete API key from keychain.
    RemoveKey(String),
    /// `/provider migrate-keys` — import keys from the OS keychain into the
    /// portable encrypted file store (idempotent, one-shot).
    MigrateKeys,
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
        "/continue" | "/cont" => Command::Continue,
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
            Some("remove-key") | Some("rm-key") => match tokens.get(2) {
                Some(name) => Command::Provider(ProviderCommand::RemoveKey(name.to_string())),
                None => Command::Normal(input.to_string()),
            },
            Some("migrate-keys") | Some("migrate") => {
                Command::Provider(ProviderCommand::MigrateKeys)
            }
            _ => Command::Normal(input.to_string()),
        },
        "/models" => Command::Models,
        "/model" if tokens.len() > 1 && tokens[1] == "use" => match tokens.get(2) {
            Some(x) => Command::ModelUse(x.to_string()),
            None => Command::Normal(input.to_string()),
        },
        _ => Command::Normal(input.to_string()),
    }
}

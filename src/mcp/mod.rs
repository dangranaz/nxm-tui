//! MCP host subsystem for nxm-tui.
//!
//! nxm-tui acts as an MCP **client/host**: it spawns an MCP server (e.g.
//! `nxm-session-mcp --transport stdio`) as its own child, completes the
//! `initialize` handshake declaring the `sampling` capability, and serves
//! `sampling/createMessage` requests by forwarding them to the TUI's LLM
//! backend. This is what lets a server-side tool such as `generate_handoff`
//! obtain completions and populate every section of its output.
//!
//! ## Why spawn instead of "attach"?
//! A `--transport stdio` server talks over the stdin/stdout of *its own*
//! process. A server started by an external script is owned by that script's
//! pipes; nothing else can attach to it. So "is the server present?" is a
//! *launchability* check ([`transport::binary_present`]), and when present the
//! host spawns its own short-lived child and owns the pipes.
//!
//! ## Layout
//! - [`protocol`]  — JSON-RPC 2.0 + sampling wire types (serde).
//! - [`transport`] — child spawn + newline-delimited stdio framing.
//! - [`sampling`]  — the bridge to the LLM backend (auto-approved).
//! - [`client`]    — session lifecycle + inbound/outbound dispatch.
//!
//! See `docs/mcp/ARCHITECTURE.md` for the refinement roadmap (tools/call,
//! approval overlay, socket transport, persistent sessions).

pub mod client;
pub mod protocol;
pub mod sampling;
pub mod transport;

use anyhow::Result;
use reqwest::Client;
use tracing::{debug, info, warn};
use zeroize::Zeroizing;

use crate::mcp::client::{spawn_session, McpClient, McpHandle};
use crate::mcp::sampling::LlmSamplingHandler;
use crate::mcp::transport::{binary_present, StdioTransport};

/// Tracing target per `RULES.md`.
const TARGET: &str = "nexum::mcp";

/// Connect to an MCP server if its binary is present, wiring the sampling
/// bridge to the given LLM backend. Returns `Ok(None)` when the server binary
/// is absent (a clean skip, not an error) so callers can no-op gracefully.
pub async fn connect_if_present(
    command: &str,
    args: &[String],
    http: Client,
    base_url: impl Into<String>,
    model: impl Into<String>,
    api_key: Option<String>,
) -> Result<Option<McpHandle>> {
    if !binary_present(command) {
        info!(target: TARGET, %command, "MCP server binary not present; skipping");
        return Ok(None);
    }
    info!(target: TARGET, %command, "MCP server present; connecting");

    let transport = match StdioTransport::spawn(command, args) {
        Ok(t) => t,
        Err(e) => {
            warn!(target: TARGET, error = %e, "spawn failed; skipping MCP");
            return Ok(None);
        }
    };

    let handler = Box::new(LlmSamplingHandler::new(http, base_url, model, api_key));
    let mut client = McpClient::new(transport, handler);
    client.initialize().await?;
    let handle = spawn_session(client);
    Ok(Some(handle))
}

/// Open an MCP session from the TUI config and active endpoint, resolving the
/// provider API key through the `KeyStore` seam the same way the agent does
/// (I2: `store_for(mode, passphrase)` + `resolve_key_with`, off the async
/// runtime via `spawn_blocking`).
///
/// The session passphrase (encrypted-file keystore) is passed in by the
/// caller, which gates on it first — this function never prompts. It is a
/// clean no-op (returns `Ok(None)`) when `cfg.mcp.enabled` is false or the
/// server binary is absent, so the normal chat path is never affected.
///
/// The resolved key value is NEVER logged (only its source), per `RULES.md`.
///
/// # Example
///
/// ```ignore
/// let handle = nxm_tui::mcp::connect_from_config(&cfg, &endpoint, pass).await?;
/// // keep `handle` alive for the session; dropping it stops the run loop.
/// ```
pub async fn connect_from_config(
    cfg: &crate::config::TuiConfig,
    endpoint: &str,
    passphrase: Option<Zeroizing<String>>,
) -> Result<Option<McpHandle>> {
    if !cfg.mcp.enabled {
        info!(target: TARGET, "MCP disabled in config; skipping");
        return Ok(None);
    }

    let model = cfg
        .model_name
        .clone()
        .unwrap_or_else(|| "default".to_string());

    // Resolve the active provider's key through the KeyStore seam (mode →
    // backend → env), off the async runtime for the blocking backend call.
    // Key source is logged, never the value.
    let providers = crate::provider::all_providers(&cfg.providers);
    let active = providers.iter().find(|p| {
        p.base_url == endpoint
            || p.base_url.trim_end_matches("/v1") == endpoint.trim_end_matches("/v1")
    });

    let api_key = match active {
        Some(p) if p.requires_api_key => {
            let provider = p.clone();
            let mode = crate::keys::current_mode();
            match tokio::task::spawn_blocking(move || {
                crate::keys::resolve_key_via(&provider, mode, passphrase)
            })
            .await
            {
                Ok(resolution) => {
                    debug!(target: TARGET, source = resolution.source(), "resolved MCP backend key");
                    resolution.into_key()
                }
                Err(e) => {
                    warn!(target: TARGET, error = %e, "key resolution task failed; proceeding without key");
                    None
                }
            }
        }
        _ => None,
    };

    let http = Client::new();
    connect_if_present(
        &cfg.mcp.command,
        &cfg.mcp.args,
        http,
        endpoint.to_string(),
        model,
        api_key,
    )
    .await
}

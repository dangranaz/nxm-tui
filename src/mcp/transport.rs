//! stdio transport for an MCP server child process.
//!
//! An MCP server launched with `--transport stdio` speaks JSON-RPC 2.0 over
//! its own stdin/stdout, one JSON object per line (newline-delimited). Because
//! stdio is owned by whoever holds the pipes, the TUI cannot attach to a server
//! started by an *external* script; it must spawn its **own** child and own the
//! pipes. "Is the server present?" therefore means: is its binary launchable?
//!
//! [`StdioTransport`] owns the child, exposes a `send` half (writes framed
//! JSON to the child's stdin) and an incoming line stream (the child's stdout).
//! Framing is deliberately minimal: `serialize + '\n'` out, `read_line` in.

use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Tracing target per `RULES.md`.
const TARGET: &str = "nexum::mcp::transport";

/// Check whether an MCP server binary is launchable on `PATH` (or as an
/// absolute path). This is the "is it present?" probe: no process is started.
pub fn binary_present(command: &str) -> bool {
    // Absolute/relative path that exists and is a file.
    let p = std::path::Path::new(command);
    if p.is_file() {
        return true;
    }
    // Otherwise search PATH for the bare command name.
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            if dir.join(command).is_file() {
                return true;
            }
        }
    }
    false
}

/// A spawned MCP server child plus its framed stdio channels.
pub struct StdioTransport {
    child: Child,
    stdin: ChildStdin,
    /// Receives one raw JSON line per server→client message.
    incoming: mpsc::UnboundedReceiver<String>,
}

impl StdioTransport {
    /// Spawn `command args...` with piped stdio and start the read loop.
    ///
    /// Returns an error if the binary is absent or the pipes cannot be taken.
    /// stderr is inherited so server diagnostics remain visible in the terminal.
    pub fn spawn(command: &str, args: &[String]) -> Result<Self> {
        info!(target: TARGET, %command, ?args, "ENTER spawn mcp server");
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("failed to spawn MCP server `{command}`"))?;

        let stdin = child.stdin.take().context("child stdin was not piped")?;
        let stdout = child.stdout.take().context("child stdout was not piped")?;

        let (tx, incoming) = mpsc::unbounded_channel();
        tokio::spawn(read_loop(stdout, tx));

        info!(target: TARGET, "EXIT spawn mcp server (ok)");
        Ok(Self {
            child,
            stdin,
            incoming,
        })
    }

    /// Write one JSON value framed as a single newline-terminated line.
    pub async fn send_line(&mut self, json: &str) -> Result<()> {
        debug!(target: TARGET, bytes = json.len(), "send line");
        self.stdin
            .write_all(json.as_bytes())
            .await
            .context("write to MCP server stdin failed")?;
        self.stdin
            .write_all(b"\n")
            .await
            .context("write newline to MCP server stdin failed")?;
        self.stdin
            .flush()
            .await
            .context("flush MCP server stdin failed")?;
        Ok(())
    }

    /// Await the next raw JSON line from the server, or `None` when the child's
    /// stdout has closed (server exited).
    pub async fn next_line(&mut self) -> Option<String> {
        self.incoming.recv().await
    }

    /// Terminate the child. Best-effort; errors are logged, not propagated.
    pub async fn shutdown(mut self) {
        debug!(target: TARGET, "shutdown mcp child");
        if let Err(e) = self.child.start_kill() {
            warn!(target: TARGET, error = %e, "start_kill failed");
        }
        let _ = self.child.wait().await;
    }
}

/// Read the child's stdout line-by-line, forwarding each non-empty line to the
/// channel. Exits when stdout closes or the receiver is dropped.
async fn read_loop(stdout: ChildStdout, tx: mpsc::UnboundedSender<String>) {
    let mut reader = BufReader::new(stdout).lines();
    loop {
        match reader.next_line().await {
            Ok(Some(line)) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if tx.send(trimmed.to_string()).is_err() {
                    debug!(target: TARGET, "incoming receiver dropped; stopping read loop");
                    return;
                }
            }
            Ok(None) => {
                debug!(target: TARGET, "server stdout closed");
                return;
            }
            Err(e) => {
                warn!(target: TARGET, error = %e, "read from server stdout failed");
                return;
            }
        }
    }
}

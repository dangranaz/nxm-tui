//! Server process management for nexum-terminal TUI.
//! Handles spawn/stop/health check for nexum-local-srv.

use std::path::PathBuf;
use std::process::Child;

/// Get path for today's log file.
fn nxm_log_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let log_dir = home.join(".nxm").join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let date = chrono::Local::now().format("%Y-%m-%d");
    log_dir.join(format!("{date}.log"))
}
use crate::app::App;

/// Find the server binary path.
pub fn find_server_binary() -> Result<String, String> {
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let debug = PathBuf::from(&manifest_dir)
            .join("../../target/debug/nexum-local-srv");
        if debug.exists() {
            return Ok(debug.to_string_lossy().to_string());
        }
        let release = PathBuf::from(&manifest_dir)
            .join("../../target/release/nexum-local-srv");
        if release.exists() {
            return Ok(release.to_string_lossy().to_string());
        }
    }

    let debug = PathBuf::from("target/debug/nexum-local-srv");
    if debug.exists() {
        return Ok(debug.to_string_lossy().to_string());
    }
    let release = PathBuf::from("target/release/nexum-local-srv");
    if release.exists() {
        return Ok(release.to_string_lossy().to_string());
    }

    Err("Server binary not found. Run `cargo build --release -p nexum-local-srv` first".into())
}

/// Stop server using PID file (works for servers started from CLI too).
pub fn stop_server_by_pid() -> Result<(), String> {
    let pid_file = dirs::home_dir()
        .unwrap_or_default()
        .join(".nexum")
        .join("server.pid");

    let pid: u32 = std::fs::read_to_string(&pid_file)
        .map_err(|_| "No server running (no PID file)".to_string())?
        .trim()
        .parse()
        .map_err(|_| "Invalid PID file".to_string())?;

    let alive = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !alive {
        let _ = std::fs::remove_file(&pid_file);
        return Err("No server running (stale PID)".into());
    }

    let _ = std::process::Command::new("kill").args(["-15", &pid.to_string()]).status();

    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let still_alive = std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !still_alive {
            let _ = std::fs::remove_file(&pid_file);
            return Ok(());
        }
    }

    let _ = std::process::Command::new("kill").args(["-9", &pid.to_string()]).status();
    std::thread::sleep(std::time::Duration::from_millis(200));
    let _ = std::fs::remove_file(&pid_file);
    Ok(())
}

/// Check if a server is running via PID file.
pub fn server_running_by_pid() -> Option<u32> {
    let pid_file = dirs::home_dir()
        .unwrap_or_default()
        .join(".nexum")
        .join("server.pid");

    let pid: u32 = std::fs::read_to_string(&pid_file).ok()?.trim().parse().ok()?;

    let alive = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if alive {
        Some(pid)
    } else {
        let _ = std::fs::remove_file(&pid_file);
        None
    }
}

/// ServerProcess info struct.
pub struct ServerProcess {
    pub child: Child,
    pub port: u16,
}

impl App {
    /// Start the nexum-local-srv binary as a child process.
    pub fn start_server(&mut self, port: u16) -> Result<(), String> {
        use std::process::Command as StdCommand;
        if self.server_process.is_some() {
            return Err("Server already running".into());
        }

        let binary = find_server_binary()?;

        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(nxm_log_path())
            .map_err(|e| format!("Failed to open log file: {e}"))?;

        let output = StdCommand::new(&binary)
            .arg("--port")
            .arg(port.to_string())
            .stdout(log_file.try_clone().map_err(|e| e.to_string())?)
            .stderr(log_file)
            .spawn()
            .map_err(|e| format!("Failed to start server: {e}"))?;

        self.server_process = Some(ServerProcess { child: output, port });
        self.endpoint = format!("http://127.0.0.1:{port}");
        self.server_name = "Nexum Local".into();
        self.state = crate::app::RunState::Connecting;

        Ok(())
    }

    /// Stop the running server.
    pub fn stop_server(&mut self) -> Result<(), String> {
        match self.server_process.as_mut() {
            Some(proc) => {
                proc.child.kill().map_err(|e| format!("Failed to kill server: {e}"))?;
                proc.child.wait().ok();
                self.server_process = None;
                Ok(())
            }
            None => Err("No server running".into()),
        }
    }

    /// Check if the server child is still alive, and clean up if it exited.
    /// Returns true only when something visible changed (the process slot
    /// was cleared / a status was set) — the main loop redraws only on change.
    pub fn check_server_health(&mut self) -> bool {
        if let Some(ref mut proc) = self.server_process {
            match proc.child.try_wait() {
                Ok(Some(_status)) => {
                    self.server_process = None;
                    self.set_status("Server exited".into());
                    true
                }
                Ok(None) => false,
                Err(_) => {
                    self.server_process = None;
                    true
                }
            }
        } else {
            false
        }
    }
}
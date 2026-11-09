//! Terminates and relaunches the Cursor desktop process around a takeover.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::process::Command;

use crate::{Error, Result};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
fn hide_console(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

pub async fn terminate_cursor() -> Result<()> {
    terminate_platform_cursor().await
}

/// Records whether Cursor was running and where it lives, so a configuration
/// change can put the editor back exactly the way the user left it.
#[derive(Debug, Default)]
pub struct RestartPlan {
    was_running: bool,
    executable: Option<PathBuf>,
}

impl RestartPlan {
    pub async fn capture() -> Self {
        let was_running = cursor_running().await;
        if !was_running {
            return Self::default();
        }
        Self {
            was_running,
            executable: cursor_executable().await,
        }
    }

    /// Relaunches Cursor only when it was running before and is gone now, so a
    /// takeover that never stopped the editor does not spawn a second window.
    /// A missing executable is not an error: the editor may be started by hand.
    pub async fn relaunch(&self) {
        if !self.was_running || cursor_running().await {
            return;
        }
        wait_until_stopped(Duration::from_secs(5)).await;
        if let Err(error) = launch_cursor(self.executable.as_deref()).await {
            tracing::warn!(%error, "could not relaunch Cursor after the configuration change");
        }
    }
}

async fn wait_until_stopped(timeout: Duration) {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if !cursor_running().await {
            // Give the desktop a moment to release its single-instance lock.
            tokio::time::sleep(Duration::from_millis(600)).await;
            return;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[cfg(windows)]
pub async fn cursor_running() -> bool {
    let mut list = Command::new("tasklist");
    hide_console(&mut list);
    match list
        .args(["/FI", "IMAGENAME eq Cursor.exe", "/NH", "/FO", "CSV"])
        .output()
        .await
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout)
            .to_ascii_lowercase()
            .contains("cursor.exe"),
        Err(_) => false,
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub async fn cursor_running() -> bool {
    let name = if cfg!(target_os = "macos") { "Cursor" } else { "cursor" };
    matches!(
        Command::new("pgrep").args(["-x", name]).status().await,
        Ok(status) if status.success()
    )
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub async fn cursor_running() -> bool {
    false
}

#[cfg(windows)]
pub async fn cursor_executable() -> Option<PathBuf> {
    let mut query = Command::new("powershell");
    hide_console(&mut query);
    if let Ok(output) = query
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-Process -Name Cursor -ErrorAction SilentlyContinue \
             | Where-Object { $_.Path } | Select-Object -First 1 -ExpandProperty Path)",
        ])
        .output()
        .await
    {
        let reported = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !reported.is_empty() {
            return Some(PathBuf::from(reported));
        }
    }
    windows_install_candidates()
        .into_iter()
        .find(|candidate| candidate.is_file())
}

#[cfg(windows)]
fn windows_install_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(Path::new(&local).join(r"Programs\cursor\Cursor.exe"));
    }
    if let Ok(programs) = std::env::var("PROGRAMFILES") {
        candidates.push(Path::new(&programs).join(r"Cursor\Cursor.exe"));
    }
    candidates
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub async fn cursor_executable() -> Option<PathBuf> {
    None
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub async fn cursor_executable() -> Option<PathBuf> {
    None
}

#[cfg(windows)]
async fn launch_cursor(executable: Option<&Path>) -> Result<()> {
    let path = executable.ok_or_else(|| {
        Error::Config("could not find the Cursor executable to restart".into())
    })?;
    Command::new(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| Error::Config(format!("failed to restart Cursor: {error}")))
}

#[cfg(target_os = "macos")]
async fn launch_cursor(_executable: Option<&Path>) -> Result<()> {
    Command::new("open")
        .args(["-a", "Cursor"])
        .spawn()
        .map(|_| ())
        .map_err(|error| Error::Config(format!("failed to restart Cursor: {error}")))
}

#[cfg(target_os = "linux")]
async fn launch_cursor(_executable: Option<&Path>) -> Result<()> {
    Command::new("cursor")
        .spawn()
        .map(|_| ())
        .map_err(|error| Error::Config(format!("failed to restart Cursor: {error}")))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
async fn launch_cursor(_executable: Option<&Path>) -> Result<()> {
    Err(Error::Config(format!(
        "restarting Cursor is unsupported on {}",
        std::env::consts::OS
    )))
}

#[cfg(target_os = "macos")]
async fn terminate_platform_cursor() -> Result<()> {
    terminate_unix_process("Cursor").await
}

#[cfg(target_os = "linux")]
async fn terminate_platform_cursor() -> Result<()> {
    terminate_unix_process("cursor").await?;
    terminate_unix_process("Cursor").await
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
async fn terminate_unix_process(name: &str) -> Result<()> {
    let running = Command::new("pgrep").args(["-x", name]).status().await?;
    if !running.success() {
        return match running.code() {
            Some(1) => Ok(()),
            _ => Err(Error::Config(format!(
                "failed to inspect the {name} process"
            ))),
        };
    }
    let terminated = Command::new("pkill").args(["-x", name]).status().await?;
    if terminated.success() || terminated.code() == Some(1) {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "failed to terminate the {name} process"
        )))
    }
}

#[cfg(target_os = "windows")]
async fn terminate_platform_cursor() -> Result<()> {
    let mut list = Command::new("tasklist");
    hide_console(&mut list);
    let processes = list
        .args(["/FI", "IMAGENAME eq Cursor.exe", "/NH", "/FO", "CSV"])
        .output()
        .await?;
    if !processes.status.success() {
        return Err(Error::Config(
            "failed to inspect the Cursor.exe process".into(),
        ));
    }
    if !String::from_utf8_lossy(&processes.stdout)
        .to_ascii_lowercase()
        .contains("cursor.exe")
    {
        return Ok(());
    }
    let mut kill = Command::new("taskkill");
    hide_console(&mut kill);
    let terminated = kill
        .args(["/F", "/T", "/IM", "Cursor.exe"])
        .status()
        .await?;
    if terminated.success() {
        Ok(())
    } else {
        Err(Error::Config(
            "failed to terminate the Cursor.exe process".into(),
        ))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
async fn terminate_platform_cursor() -> Result<()> {
    Err(Error::Config(format!(
        "terminating Cursor is unsupported on {}",
        std::env::consts::OS
    )))
}

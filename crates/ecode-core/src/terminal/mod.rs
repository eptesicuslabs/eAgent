//! Terminal manager — embedded PTY terminal sessions.

use anyhow::{Context, Result};
use ecode_contracts::ids::TerminalId;
use ecode_contracts::terminal::{TerminalConfig, TerminalEvent};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{error, info};

/// A running terminal session.
struct TerminalSession {
    /// Writer to the PTY.
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    /// Whether the session is still alive.
    alive: Arc<std::sync::atomic::AtomicBool>,
}

/// Manager for terminal sessions.
pub struct TerminalManager {
    sessions: Arc<Mutex<HashMap<TerminalId, TerminalSession>>>,
    event_tx: mpsc::UnboundedSender<TerminalEvent>,
}

impl TerminalManager {
    /// Create a new terminal manager. Returns the manager and a receiver for terminal events.
    pub fn new() -> (Self, mpsc::UnboundedReceiver<TerminalEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Self {
                sessions: Arc::new(Mutex::new(HashMap::new())),
                event_tx: tx,
            },
            rx,
        )
    }

    /// Open a new terminal session.
    pub fn open(&self, config: TerminalConfig) -> Result<TerminalId> {
        let terminal_id = TerminalId::new();
        let pty_system = native_pty_system();

        let pty_pair = pty_system.openpty(PtySize {
            rows: config.rows,
            cols: config.cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        // Determine the shell to use
        let shell = config
            .shell
            .clone()
            .unwrap_or_else(|| crate::platform::default_shell().to_string());

        let mut cmd = CommandBuilder::new(&shell);
        if !config.cwd.is_empty() {
            cmd.cwd(&config.cwd);
        }

        let _child = pty_pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn terminal process")?;

        let writer = pty_pair
            .master
            .take_writer()
            .context("Failed to take PTY writer")?;

        let alive = Arc::new(std::sync::atomic::AtomicBool::new(true));

        // Spawn a reader thread to capture output
        let mut reader = pty_pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;
        let event_tx = self.event_tx.clone();
        let tid = terminal_id;
        let alive_clone = alive.clone();

        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        alive_clone.store(false, std::sync::atomic::Ordering::SeqCst);
                        let _ = event_tx.send(TerminalEvent::Exited {
                            terminal_id: tid,
                            exit_code: None,
                        });
                        break;
                    }
                    Ok(n) => {
                        let data = String::from_utf8_lossy(&buf[..n]).to_string();
                        let _ = event_tx.send(TerminalEvent::Output {
                            terminal_id: tid,
                            data,
                        });
                    }
                    Err(e) => {
                        error!(%e, "Terminal reader error");
                        alive_clone.store(false, std::sync::atomic::Ordering::SeqCst);
                        let _ = event_tx.send(TerminalEvent::Exited {
                            terminal_id: tid,
                            exit_code: None,
                        });
                        break;
                    }
                }
            }
        });

        let session = TerminalSession {
            writer: Arc::new(Mutex::new(writer)),
            alive,
        };

        self.sessions.lock().unwrap().insert(terminal_id, session);

        info!(%terminal_id, shell = %shell, "Opened terminal session");
        Ok(terminal_id)
    }

    /// Write data to a terminal.
    pub fn write(&self, terminal_id: &TerminalId, data: &[u8]) -> Result<()> {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get(terminal_id)
            .context("Terminal session not found")?;

        let mut writer = session.writer.lock().unwrap();
        writer
            .write_all(data)
            .context("Failed to write to terminal")?;
        writer.flush().context("Failed to flush terminal writer")?;
        Ok(())
    }

    /// Resize a terminal.
    pub fn resize(&self, terminal_id: &TerminalId, cols: u16, rows: u16) -> Result<()> {
        // portable-pty resize is done on the master, which we don't store directly.
        // For now, send a resize event so the UI knows.
        let _ = self.event_tx.send(TerminalEvent::Resized {
            terminal_id: *terminal_id,
            cols,
            rows,
        });
        Ok(())
    }

    /// Close a terminal session.
    pub fn close(&self, terminal_id: &TerminalId) -> Result<()> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.remove(terminal_id) {
            session
                .alive
                .store(false, std::sync::atomic::Ordering::SeqCst);
            info!(%terminal_id, "Closed terminal session");
        }
        Ok(())
    }

    /// Check if a terminal session is alive.
    pub fn is_alive(&self, terminal_id: &TerminalId) -> bool {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .get(terminal_id)
            .map(|s| s.alive.load(std::sync::atomic::Ordering::SeqCst))
            .unwrap_or(false)
    }

    /// Get the list of active terminal IDs.
    pub fn active_terminals(&self) -> Vec<TerminalId> {
        let sessions = self.sessions.lock().unwrap();
        sessions
            .iter()
            .filter(|(_, s)| s.alive.load(std::sync::atomic::Ordering::SeqCst))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Close all terminal sessions.
    pub fn close_all(&self) {
        let mut sessions = self.sessions.lock().unwrap();
        for (id, session) in sessions.drain() {
            session
                .alive
                .store(false, std::sync::atomic::Ordering::SeqCst);
            info!(%id, "Closed terminal session (shutdown)");
        }
    }
}

impl Drop for TerminalManager {
    fn drop(&mut self) {
        self.close_all();
    }
}

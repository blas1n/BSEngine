//! Owns spawned `bsengine-runtime --test` child processes, one per test
//! session, and speaks the private newline-delimited JSON protocol
//! documented in `docs/superpowers/specs/2026-07-22-ai-gameplay-e2e-testing-design.md`.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde_json::Value;

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

struct Session {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

/// Manages live `bsengine-runtime --test` child processes keyed by session id.
pub struct SessionRegistry {
    runtime_bin: PathBuf,
    games_root: PathBuf,
    sessions: Mutex<HashMap<String, Session>>,
}

impl SessionRegistry {
    pub fn new(runtime_bin: PathBuf, games_root: PathBuf) -> Self {
        Self {
            runtime_bin,
            games_root,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Spawns `bsengine-runtime --test <games_root>/<game>` and returns a
    /// new session id for it.
    pub fn start_session(&self, game: &str) -> Result<String, String> {
        let game_dir = self.games_root.join(game);
        let mut child = Command::new(&self.runtime_bin)
            .arg("--test")
            .arg(&game_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to spawn bsengine-runtime --test: {e}"))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "failed to capture child stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "failed to capture child stdout".to_string())?;

        let session_id = format!("session-{}", NEXT_SESSION_ID.fetch_add(1, Ordering::SeqCst));
        let session = Session {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        };
        self.sessions
            .lock()
            .unwrap()
            .insert(session_id.clone(), session);
        Ok(session_id)
    }

    /// Sends one command to the given session's child process and returns
    /// its one-line JSON response.
    pub fn send(&self, session_id: &str, command: Value) -> Result<Value, String> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("no such session: {session_id}"))?;
        write_and_read(session, &command)
    }

    /// Sends `shutdown` to the session's child (best-effort), waits for it
    /// to exit, and removes it from the registry.
    pub fn stop_session(&self, session_id: &str) -> Result<(), String> {
        let mut session = {
            let mut sessions = self.sessions.lock().unwrap();
            sessions
                .remove(session_id)
                .ok_or_else(|| format!("no such session: {session_id}"))?
        };
        let _ = write_and_read(&mut session, &serde_json::json!({"cmd": "shutdown"}));
        let _ = session.child.wait();
        Ok(())
    }
}

fn write_and_read(session: &mut Session, command: &Value) -> Result<Value, String> {
    let line =
        serde_json::to_string(command).map_err(|e| format!("failed to encode command: {e}"))?;
    writeln!(session.stdin, "{line}").map_err(|e| format!("failed to write to session: {e}"))?;
    session
        .stdin
        .flush()
        .map_err(|e| format!("failed to flush session: {e}"))?;

    let mut response_line = String::new();
    let bytes_read = session
        .stdout
        .read_line(&mut response_line)
        .map_err(|e| format!("failed to read from session: {e}"))?;
    if bytes_read == 0 {
        return Err("session closed the connection unexpectedly".to_string());
    }
    serde_json::from_str(&response_line).map_err(|e| format!("invalid response from session: {e}"))
}

impl Drop for SessionRegistry {
    fn drop(&mut self) {
        let mut sessions = self.sessions.lock().unwrap();
        for (_, mut session) in sessions.drain() {
            let _ = session.child.kill();
            let _ = session.child.wait();
        }
    }
}

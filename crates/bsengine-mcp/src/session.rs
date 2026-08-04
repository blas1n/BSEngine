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

/// Result of running a saved recording via `bsengine-runtime --test --replay`.
pub struct ReplayResult {
    /// Whether the replayed run's assertions all passed (child exit success).
    pub passed: bool,
    /// The child process's captured stderr output.
    pub stderr: String,
}

/// Result of running `bsengine-runtime --fixup --json` over one game project.
pub struct FixupRun {
    /// Whether the child left nothing undone — false when the report carries a
    /// problem, which is what the child's non-zero exit means.
    pub clean: bool,
    /// The report the child printed, parsed. Its shape is
    /// `bsengine_asset::identity::FixupReport`.
    pub report: Value,
    /// The child's captured stderr: the scan's own diagnostics, which are
    /// deliberately on a different stream from the report.
    pub stderr: String,
}

struct Session {
    game: String,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    actions: Vec<Value>,
}

/// Manages live `bsengine-runtime --test` child processes keyed by session id.
pub struct SessionRegistry {
    runtime_bin: PathBuf,
    games_root: PathBuf,
    sessions: Mutex<HashMap<String, Session>>,
}

impl SessionRegistry {
    /// Creates a registry with no active sessions, using `runtime_bin` to
    /// spawn `bsengine-runtime` and resolving game paths under `games_root`.
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
            game: game.to_string(),
            child,
            stdin,
            stdout: BufReader::new(stdout),
            actions: Vec::new(),
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
        let response = write_and_read(session, &command)?;
        if command.get("cmd").and_then(|v| v.as_str()) != Some("query") {
            session.actions.push(command);
        }
        Ok(response)
    }

    /// Serializes the session's accumulated action log to
    /// `<games_root>/<game>/tests/<name>.testlog.json` and returns that path.
    pub fn save_recording(&self, session_id: &str, name: &str) -> Result<PathBuf, String> {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get(session_id)
            .ok_or_else(|| format!("no such session: {session_id}"))?;

        let log = serde_json::json!({ "game": session.game, "actions": session.actions });
        let dir = self.games_root.join(&session.game).join("tests");
        std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create tests dir: {e}"))?;
        let path = dir.join(format!("{name}.testlog.json"));
        let content = serde_json::to_string_pretty(&log)
            .map_err(|e| format!("failed to encode recording: {e}"))?;
        std::fs::write(&path, content).map_err(|e| format!("failed to write recording: {e}"))?;
        Ok(path)
    }

    /// Spawns `bsengine-runtime --test <game> --replay <name>.testlog.json`,
    /// waits for it to finish, and reports pass/fail. Independent of any
    /// live interactive session.
    pub fn run_replay(&self, game: &str, name: &str) -> Result<ReplayResult, String> {
        let game_dir = self.games_root.join(game);
        let log_path = self
            .games_root
            .join(game)
            .join("tests")
            .join(format!("{name}.testlog.json"));
        if !log_path.exists() {
            return Err(format!("no recorded log at {}", log_path.display()));
        }

        let output = Command::new(&self.runtime_bin)
            .arg("--test")
            .arg(&game_dir)
            .arg("--replay")
            .arg(&log_path)
            .output()
            .map_err(|e| format!("failed to run replay: {e}"))?;

        Ok(ReplayResult {
            passed: output.status.success(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    /// Spawns `bsengine-runtime --fixup <game> --json`, waits for it to
    /// finish, and returns the report it printed.
    ///
    /// # Why a child process rather than a call
    ///
    /// The work itself is `bsengine_asset::identity::fixup`, and this crate
    /// could in principle call it directly — it needs no engine, no `App` and
    /// no running game. It does not, for the reason this crate does not depend
    /// on the engine anywhere: `bsengine-mcp` is a JSON-RPC binary that drives
    /// `bsengine-runtime` as a child process, and taking on `bsengine-asset`
    /// would pull `bevy_asset`, `image`, `blake3`, `notify` and `uuid` into it
    /// to run one directory walk. `game_validate` already refuses the same
    /// trade against `bsengine-scene`. The process boundary is the one this
    /// crate already has, so using it is not a second mechanism.
    ///
    /// # Why it is not session-scoped
    ///
    /// Unlike everything else here, `fixup` operates on a project *directory*
    /// rather than on a running game — it is repairing the files a session
    /// would load, and a session holding them open is if anything a reason not
    /// to. So there is no `session_id`, nothing is started or stopped, and this
    /// is independent of every live session, exactly as [`Self::run_replay`]
    /// is.
    pub fn fixup(&self, game: &str) -> Result<FixupRun, String> {
        let game_dir = self.games_root.join(game);
        if !game_dir.join("project.toml").exists() {
            return Err(format!(
                "no game at {} — it needs a project.toml",
                game_dir.display()
            ));
        }

        let output = Command::new(&self.runtime_bin)
            .arg("--fixup")
            .arg(&game_dir)
            .arg("--json")
            .output()
            .map_err(|e| format!("failed to run bsengine-runtime --fixup: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        // A missing `assets/` directory, and anything else that stops the run
        // before it can report, arrives here as an empty stdout. Reporting the
        // child's stderr with it is what turns "fixup failed" into something
        // actionable.
        let report: Value = serde_json::from_str(stdout.trim()).map_err(|e| {
            format!("bsengine-runtime --fixup printed no readable report ({e}); it said: {stderr}")
        })?;

        Ok(FixupRun {
            clean: output.status.success(),
            report,
            stderr,
        })
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

//! JSON-line command/response protocol between `bsengine-mcp-server` and a
//! `bsengine-runtime --test` child process. Private to the two processes —
//! never exposed to MCP clients directly.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    Step { frames: u32 },
    PressKey { key: String },
    ReleaseKey { key: String },
    PressMouse { button: u8 },
    ReleaseMouse { button: u8 },
    Query { tool: String, args: Value },
    Assert {
        query: QuerySpec,
        path: String,
        op: String,
        value: Value,
        label: String,
    },
    Shutdown,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QuerySpec {
    pub tool: String,
    pub args: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl CommandResponse {
    pub fn ok(data: Value) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_step_command() {
        let cmd: Command = serde_json::from_str(r#"{"cmd":"step","frames":30}"#).unwrap();
        match cmd {
            Command::Step { frames } => assert_eq!(frames, 30),
            other => panic!("expected Step, got {other:?}"),
        }
    }

    #[test]
    fn parses_press_key_command() {
        let cmd: Command = serde_json::from_str(r#"{"cmd":"press_key","key":"W"}"#).unwrap();
        match cmd {
            Command::PressKey { key } => assert_eq!(key, "W"),
            other => panic!("expected PressKey, got {other:?}"),
        }
    }

    #[test]
    fn parses_assert_command() {
        let cmd: Command = serde_json::from_str(
            r#"{"cmd":"assert","query":{"tool":"get_transform","args":{"name":"Player"}},"path":"z","op":">","value":3,"label":"moved"}"#,
        )
        .unwrap();
        match cmd {
            Command::Assert {
                query,
                path,
                op,
                value,
                label,
            } => {
                assert_eq!(query.tool, "get_transform");
                assert_eq!(path, "z");
                assert_eq!(op, ">");
                assert_eq!(value, json!(3));
                assert_eq!(label, "moved");
            }
            other => panic!("expected Assert, got {other:?}"),
        }
    }

    #[test]
    fn parses_shutdown_command() {
        let cmd: Command = serde_json::from_str(r#"{"cmd":"shutdown"}"#).unwrap();
        assert!(matches!(cmd, Command::Shutdown));
    }

    #[test]
    fn ok_response_serializes_without_error_field() {
        let resp = CommandResponse::ok(json!({"frame": 30}));
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"ok\":true"));
        assert!(!s.contains("error"));
    }

    #[test]
    fn err_response_serializes_without_data_field() {
        let resp = CommandResponse::err("boom");
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"ok\":false"));
        assert!(s.contains("boom"));
        assert!(!s.contains("\"data\""));
    }
}

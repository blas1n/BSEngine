use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bsengine_mcp::{game_tools, test_tools, McpServer, McpToolRegistry, SessionRegistry};

fn main() {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().expect("cannot determine working directory"));

    let mut registry = McpToolRegistry::new();
    for tool in game_tools(root.clone()) {
        registry.register(tool);
    }

    let runtime_bin = std::env::var("BSENGINE_RUNTIME_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("bsengine-runtime"));
    let session_registry = Arc::new(SessionRegistry::new(runtime_bin, root.join("games")));
    for tool in test_tools(session_registry) {
        registry.register(tool);
    }

    let server = McpServer::new(Arc::new(Mutex::new(registry)));
    server.run_stdio();
}

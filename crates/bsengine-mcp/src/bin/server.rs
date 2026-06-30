use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bsengine_mcp::{game_tools, McpServer, McpToolRegistry};

fn main() {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().expect("cannot determine working directory"));

    let mut registry = McpToolRegistry::new();
    for tool in game_tools(root) {
        registry.register(tool);
    }

    let server = McpServer::new(Arc::new(Mutex::new(registry)));
    server.run_stdio();
}

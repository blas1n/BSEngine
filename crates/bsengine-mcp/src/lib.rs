//! Generic Model Context Protocol (MCP) server runtime for BSEngine.
//!
//! `McpServer` exposes `McpTool`s (registered via `McpToolRegistry`) over
//! MCP so an external client — typically an AI agent — can invoke them.
//! `McpPlugin`/`McpRegistryResource` wire the registry into the app;
//! `bsengine-editor` is the primary consumer, registering its ~700 tools
//! here rather than reimplementing a protocol server itself.
#![warn(missing_docs)]

/// MCP tools for scaffolding, writing, and validating BSEngine game projects.
pub mod game_tools;
/// Wires `McpToolRegistry` into a Bevy `App` as an `McpPlugin`.
pub mod plugin;
/// Stores and dispatches the set of `McpTool`s available to a server.
pub mod registry;
/// JSON-RPC server loop that exposes registered tools over stdio.
pub mod server;
pub mod session;
pub mod test_tools;
/// Core `McpTool`/`McpToolOutput` types shared by every tool implementation.
pub mod tool;
pub use game_tools::game_tools;
pub use plugin::{McpPlugin, McpRegistryResource};
pub use registry::McpToolRegistry;
pub use server::McpServer;
pub use session::SessionRegistry;
pub use test_tools::test_tools;
pub use tool::{McpTool, McpToolOutput};

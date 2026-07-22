//! Generic Model Context Protocol (MCP) server runtime for BSEngine.
//!
//! `McpServer` exposes `McpTool`s (registered via `McpToolRegistry`) over
//! MCP so an external client — typically an AI agent — can invoke them.
//! `McpPlugin`/`McpRegistryResource` wire the registry into the app;
//! `bsengine-editor` is the primary consumer, registering its ~700 tools
//! here rather than reimplementing a protocol server itself.
#![warn(missing_docs)]

pub mod game_tools;
pub mod plugin;
pub mod registry;
pub mod server;
pub mod session;
pub mod test_tools;
pub mod tool;
pub use game_tools::game_tools;
pub use plugin::{McpPlugin, McpRegistryResource};
pub use registry::McpToolRegistry;
pub use server::McpServer;
pub use session::SessionRegistry;
pub use test_tools::test_tools;
pub use tool::{McpTool, McpToolOutput};

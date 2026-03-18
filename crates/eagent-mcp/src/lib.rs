//! eAgent MCP — eMCP client that bridges MCP-compatible servers into the ToolRegistry.
//!
//! eMCPs are MCP-compatible connectors that extend the tool registry with
//! external service integrations (email, calendar, project management, etc.).
//!
//! Each connector lives in a directory containing:
//!
//! - `manifest.json` — identity, transport, tool definitions, auth requirements
//! - `server.js|py|rs` — MCP server implementation (not managed by this crate)
//!
//! This crate handles manifest parsing, tool registration bridging, and connector
//! lifecycle management. Actual MCP protocol communication (JSON-RPC over
//! stdio/SSE) will be implemented in a future phase.

pub mod manifest;
pub mod loader;

pub use manifest::{McpManifest, McpTransport, McpToolDef, McpAuth};
pub use loader::{LoadedMcp, McpLoader, McpStatus, McpError};

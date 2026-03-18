//! eMCP loader — loads MCP connector manifests and manages their lifecycle.
//!
//! This module handles loading eMCP manifest files from disk. The actual MCP
//! protocol communication (stdio/SSE transport, JSON-RPC) is stubbed for now
//! and will be implemented in a future phase.

use std::collections::HashMap;
use std::path::Path;

use tracing::{info, warn};

use crate::manifest::McpManifest;

/// Status of an eMCP connector.
#[derive(Debug, Clone)]
pub enum McpStatus {
    /// Manifest has been parsed successfully.
    Loaded,
    /// MCP server is running and connected (future).
    Connected,
    /// An error occurred.
    Error(String),
}

/// An eMCP connector that has been loaded.
#[derive(Debug, Clone)]
pub struct LoadedMcp {
    /// Parsed manifest.
    pub manifest: McpManifest,
    /// Current connection status.
    pub status: McpStatus,
}

/// Errors that can occur when loading eMCPs.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("manifest.json not found in eMCP directory: {0}")]
    ManifestNotFound(String),

    #[error("failed to read file {path}: {source}")]
    IoError {
        path: String,
        source: std::io::Error,
    },

    #[error("failed to parse manifest.json in {path}: {source}")]
    ManifestParseError {
        path: String,
        source: serde_json::Error,
    },
}

/// Loads and manages eMCP connectors from the filesystem.
pub struct McpLoader {
    connectors: HashMap<String, LoadedMcp>,
}

impl McpLoader {
    /// Create a new empty MCP loader.
    pub fn new() -> Self {
        Self {
            connectors: HashMap::new(),
        }
    }

    /// Load an eMCP connector from a directory.
    ///
    /// The directory must contain a `manifest.json` file.
    /// Returns the connector name on success.
    pub fn load_from_dir(&mut self, path: &Path) -> Result<String, McpError> {
        let dir_str = path.display().to_string();

        // Read and parse manifest.json
        let manifest_path = path.join("manifest.json");
        if !manifest_path.exists() {
            return Err(McpError::ManifestNotFound(dir_str));
        }

        let manifest_content =
            std::fs::read_to_string(&manifest_path).map_err(|e| McpError::IoError {
                path: manifest_path.display().to_string(),
                source: e,
            })?;

        let manifest: McpManifest =
            serde_json::from_str(&manifest_content).map_err(|e| McpError::ManifestParseError {
                path: manifest_path.display().to_string(),
                source: e,
            })?;

        let name = manifest.name.clone();
        let tool_count = manifest.tools.len();

        info!(
            connector = %name,
            version = %manifest.version,
            tools = tool_count,
            "loaded eMCP connector"
        );

        self.connectors.insert(
            name.clone(),
            LoadedMcp {
                manifest,
                status: McpStatus::Loaded,
            },
        );

        Ok(name)
    }

    /// Load all eMCP connectors from a parent directory.
    ///
    /// Each immediate subdirectory of `mcp_dir` is treated as a potential
    /// connector directory. Subdirectories that fail to load are warned about
    /// and skipped.
    ///
    /// Returns the list of successfully loaded connector names.
    pub fn load_all(&mut self, mcp_dir: &Path) -> Result<Vec<String>, McpError> {
        let mut loaded = Vec::new();

        let entries = std::fs::read_dir(mcp_dir).map_err(|e| McpError::IoError {
            path: mcp_dir.display().to_string(),
            source: e,
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| McpError::IoError {
                path: mcp_dir.display().to_string(),
                source: e,
            })?;

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            match self.load_from_dir(&path) {
                Ok(name) => loaded.push(name),
                Err(e) => {
                    warn!(
                        dir = %path.display(),
                        error = %e,
                        "skipping eMCP directory that failed to load"
                    );
                }
            }
        }

        info!(count = loaded.len(), "loaded eMCP connectors from directory");
        Ok(loaded)
    }

    /// Get a loaded connector by name.
    pub fn get(&self, name: &str) -> Option<&LoadedMcp> {
        self.connectors.get(name)
    }

    /// List manifests of all loaded connectors.
    pub fn list(&self) -> Vec<&McpManifest> {
        self.connectors.values().map(|c| &c.manifest).collect()
    }
}

impl Default for McpLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{McpAuth, McpToolDef, McpTransport};
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create a valid eMCP directory with a manifest.json.
    fn create_mcp_dir(
        parent: &Path,
        name: &str,
        transport: &str,
        tools: &[(&str, &str)],
    ) -> std::path::PathBuf {
        let dir = parent.join(name);
        fs::create_dir_all(&dir).unwrap();

        let tools_json: Vec<serde_json::Value> = tools
            .iter()
            .map(|(n, risk)| {
                serde_json::json!({
                    "name": n,
                    "description": format!("Tool: {n}"),
                    "risk_level": risk
                })
            })
            .collect();

        let transport_value = if transport == "stdio" {
            serde_json::json!("stdio")
        } else {
            serde_json::json!({ "sse": { "url": transport } })
        };

        let manifest = serde_json::json!({
            "name": name,
            "version": "1.0.0",
            "transport": transport_value,
            "tools": tools_json,
            "auth": null
        });

        fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
        dir
    }

    #[test]
    fn load_mcp_from_dir() {
        let tmp = TempDir::new().unwrap();
        let mcp_dir = create_mcp_dir(
            tmp.path(),
            "emcp-test",
            "stdio",
            &[("tool_a", "low"), ("tool_b", "medium")],
        );

        let mut loader = McpLoader::new();
        let name = loader.load_from_dir(&mcp_dir).unwrap();

        assert_eq!(name, "emcp-test");

        let mcp = loader.get("emcp-test").unwrap();
        assert_eq!(mcp.manifest.version, "1.0.0");
        assert_eq!(mcp.manifest.tools.len(), 2);
        assert!(matches!(mcp.status, McpStatus::Loaded));
    }

    #[test]
    fn load_mcp_with_sse_transport() {
        let tmp = TempDir::new().unwrap();
        let mcp_dir = create_mcp_dir(
            tmp.path(),
            "emcp-sse",
            "http://localhost:3000/sse",
            &[("query", "low")],
        );

        let mut loader = McpLoader::new();
        loader.load_from_dir(&mcp_dir).unwrap();

        let mcp = loader.get("emcp-sse").unwrap();
        match &mcp.manifest.transport {
            McpTransport::Sse { url } => assert_eq!(url, "http://localhost:3000/sse"),
            _ => panic!("expected SSE transport"),
        }
    }

    #[test]
    fn load_mcp_with_auth() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("emcp-authed");
        fs::create_dir_all(&dir).unwrap();

        let manifest = serde_json::json!({
            "name": "emcp-authed",
            "version": "2.0.0",
            "transport": "stdio",
            "tools": [
                { "name": "send_email", "description": "Send email", "risk_level": "high" }
            ],
            "auth": {
                "type": "oauth2",
                "provider": "google"
            }
        });
        fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();

        let mut loader = McpLoader::new();
        loader.load_from_dir(&dir).unwrap();

        let mcp = loader.get("emcp-authed").unwrap();
        let auth = mcp.manifest.auth.as_ref().unwrap();
        assert_eq!(auth.auth_type, "oauth2");
        assert_eq!(auth.provider.as_deref(), Some("google"));
    }

    #[test]
    fn load_all_from_parent_dir() {
        let tmp = TempDir::new().unwrap();
        create_mcp_dir(tmp.path(), "emcp-a", "stdio", &[("a", "low")]);
        create_mcp_dir(tmp.path(), "emcp-b", "stdio", &[("b", "medium")]);

        // Also create a file (not a directory) — should be skipped
        fs::write(tmp.path().join("not-a-mcp.txt"), "hi").unwrap();

        let mut loader = McpLoader::new();
        let loaded = loader.load_all(tmp.path()).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loader.list().len(), 2);
    }

    #[test]
    fn missing_manifest_error() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("bad-mcp");
        fs::create_dir_all(&dir).unwrap();
        // No manifest.json

        let mut loader = McpLoader::new();
        let err = loader.load_from_dir(&dir).unwrap_err();
        assert!(matches!(err, McpError::ManifestNotFound(_)));
    }

    #[test]
    fn manifest_serde_roundtrip() {
        let manifest = McpManifest {
            name: "emcp-roundtrip".into(),
            version: "3.0.0".into(),
            transport: McpTransport::Sse {
                url: "http://example.com/events".into(),
            },
            tools: vec![McpToolDef {
                name: "fetch_data".into(),
                description: "Fetch data from API".into(),
                risk_level: "low".into(),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "endpoint": { "type": "string" }
                    }
                })),
            }],
            auth: Some(McpAuth {
                auth_type: "api_key".into(),
                provider: None,
            }),
        };

        let json = serde_json::to_string(&manifest).unwrap();
        let back: McpManifest = serde_json::from_str(&json).unwrap();

        assert_eq!(back.name, "emcp-roundtrip");
        assert_eq!(back.version, "3.0.0");
        assert!(matches!(back.transport, McpTransport::Sse { .. }));
        assert_eq!(back.tools.len(), 1);
        assert!(back.tools[0].parameters.is_some());
        assert_eq!(back.auth.unwrap().auth_type, "api_key");
    }

    #[test]
    fn transport_variants() {
        // Stdio
        let stdio = McpTransport::Stdio;
        let json = serde_json::to_string(&stdio).unwrap();
        let back: McpTransport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, McpTransport::Stdio);

        // SSE
        let sse = McpTransport::Sse {
            url: "http://localhost:9090".into(),
        };
        let json = serde_json::to_string(&sse).unwrap();
        let back: McpTransport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, sse);
    }

    #[test]
    fn list_returns_all_manifests() {
        let tmp = TempDir::new().unwrap();
        create_mcp_dir(tmp.path(), "emcp-one", "stdio", &[("x", "low")]);
        create_mcp_dir(tmp.path(), "emcp-two", "stdio", &[("y", "high")]);

        let mut loader = McpLoader::new();
        loader.load_all(tmp.path()).unwrap();

        let manifests = loader.list();
        assert_eq!(manifests.len(), 2);

        let names: Vec<&str> = manifests.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"emcp-one"));
        assert!(names.contains(&"emcp-two"));
    }
}

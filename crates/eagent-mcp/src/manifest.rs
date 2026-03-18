//! eMCP manifest definition and parsing.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Describes an eMCP connector — its identity, transport, tools, and auth requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpManifest {
    /// Unique connector name (e.g. "emcp-gmail").
    pub name: String,
    /// Semver version string.
    pub version: String,
    /// Transport mechanism for the MCP server.
    pub transport: McpTransport,
    /// Tools provided by this eMCP connector.
    pub tools: Vec<McpToolDef>,
    /// Optional authentication configuration.
    pub auth: Option<McpAuth>,
}

/// Transport mechanism for communicating with the MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTransport {
    /// Communicate via stdin/stdout.
    Stdio,
    /// Communicate via Server-Sent Events at the given URL.
    Sse {
        url: String,
    },
}

/// A tool provided by an eMCP connector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDef {
    /// Tool name as it will appear in the tool registry.
    pub name: String,
    /// Human-readable description of the tool.
    pub description: String,
    /// Risk level: "low", "medium", or "high".
    pub risk_level: String,
    /// Optional JSON Schema for the tool's parameters.
    pub parameters: Option<Value>,
}

/// Authentication configuration for an eMCP connector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpAuth {
    /// Auth type (e.g. "oauth2", "api_key", "bearer").
    #[serde(rename = "type")]
    pub auth_type: String,
    /// Optional provider identifier (e.g. "google", "github").
    pub provider: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_serde_roundtrip() {
        let manifest = McpManifest {
            name: "emcp-gmail".into(),
            version: "1.0.0".into(),
            transport: McpTransport::Stdio,
            tools: vec![
                McpToolDef {
                    name: "search_emails".into(),
                    description: "Search emails by query".into(),
                    risk_level: "low".into(),
                    parameters: Some(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "query": { "type": "string" }
                        }
                    })),
                },
                McpToolDef {
                    name: "send_draft".into(),
                    description: "Create an email draft".into(),
                    risk_level: "medium".into(),
                    parameters: None,
                },
            ],
            auth: Some(McpAuth {
                auth_type: "oauth2".into(),
                provider: Some("google".into()),
            }),
        };

        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let back: McpManifest = serde_json::from_str(&json).unwrap();

        assert_eq!(back.name, "emcp-gmail");
        assert_eq!(back.version, "1.0.0");
        assert_eq!(back.transport, McpTransport::Stdio);
        assert_eq!(back.tools.len(), 2);
        assert_eq!(back.tools[0].name, "search_emails");
        assert_eq!(back.tools[1].risk_level, "medium");

        let auth = back.auth.unwrap();
        assert_eq!(auth.auth_type, "oauth2");
        assert_eq!(auth.provider.unwrap(), "google");
    }

    #[test]
    fn transport_stdio_serde() {
        let json = serde_json::to_string(&McpTransport::Stdio).unwrap();
        assert_eq!(json, "\"stdio\"");

        let back: McpTransport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, McpTransport::Stdio);
    }

    #[test]
    fn transport_sse_serde() {
        let transport = McpTransport::Sse {
            url: "http://localhost:8080/events".into(),
        };
        let json = serde_json::to_string(&transport).unwrap();
        let back: McpTransport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, transport);
    }

    #[test]
    fn manifest_without_auth() {
        let manifest = McpManifest {
            name: "emcp-noauth".into(),
            version: "0.1.0".into(),
            transport: McpTransport::Stdio,
            tools: vec![],
            auth: None,
        };

        let json = serde_json::to_string(&manifest).unwrap();
        let back: McpManifest = serde_json::from_str(&json).unwrap();
        assert!(back.auth.is_none());
    }
}

//! eAgent Tools — trait definition and built-in tool implementations.

pub mod registry;

use eagent_protocol::messages::RiskLevel;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use thiserror::Error;

/// Error from tool execution.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("invalid parameters: {0}")]
    InvalidParams(String),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("timeout after {0} seconds")]
    Timeout(u64),
}

/// Context provided to a tool during execution.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Workspace root directory (tools must not escape this).
    pub workspace_root: String,
    /// The agent ID that requested this tool call.
    pub agent_id: eagent_protocol::ids::AgentId,
    /// The task ID this tool call belongs to.
    pub task_id: eagent_protocol::ids::TaskId,
}

/// Result of a tool execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolResult {
    /// The output data.
    pub output: Value,
    /// Whether this result represents an error.
    pub is_error: bool,
}

/// Definition of a tool for LLM function-calling schemas.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub risk_level: RiskLevel,
}

/// The Tool trait that all built-in and eMCP tools implement.
///
/// Uses a boxed future return type so the trait is dyn-compatible
/// and tools can be stored in the registry as `Arc<dyn Tool>`.
pub trait Tool: Send + Sync {
    /// The tool's unique name.
    fn name(&self) -> &str;

    /// Human-readable description.
    fn description(&self) -> &str;

    /// Risk level for the oversight system.
    fn risk_level(&self) -> RiskLevel;

    /// JSON Schema for the tool's parameters.
    fn parameter_schema(&self) -> Value;

    /// Execute the tool with the given parameters.
    fn execute(
        &self,
        params: Value,
        ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>>;

    /// Build a ToolDef for LLM function-calling.
    fn to_def(&self) -> ToolDef {
        ToolDef {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameter_schema(),
            risk_level: self.risk_level(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::registry::ToolRegistry;
    use eagent_protocol::messages::RiskLevel;
    use serde_json::json;
    use std::sync::Arc;

    struct MockTool;

    impl Tool for MockTool {
        fn name(&self) -> &str { "mock_tool" }
        fn description(&self) -> &str { "A mock tool for testing" }
        fn risk_level(&self) -> RiskLevel { RiskLevel::Low }
        fn parameter_schema(&self) -> serde_json::Value {
            json!({"type": "object", "properties": {"input": {"type": "string"}}})
        }
        fn execute(
            &self,
            params: serde_json::Value,
            _ctx: &ToolContext,
        ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
            Box::pin(async move {
                let input = params.get("input").and_then(|v| v.as_str()).unwrap_or("");
                Ok(ToolResult { output: json!({"echo": input}), is_error: false })
            })
        }
    }

    #[test]
    fn registry_register_and_get() {
        let mut reg = ToolRegistry::new();
        reg.register(Arc::new(MockTool));
        assert_eq!(reg.len(), 1);
        assert!(reg.get("mock_tool").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn registry_list_defs() {
        let mut reg = ToolRegistry::new();
        reg.register(Arc::new(MockTool));
        let defs = reg.list_defs();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "mock_tool");
        assert_eq!(defs[0].risk_level, RiskLevel::Low);
    }

    #[test]
    fn registry_list_filtered() {
        let mut reg = ToolRegistry::new();
        reg.register(Arc::new(MockTool));
        let allowed = vec!["mock_tool".to_string()];
        assert_eq!(reg.list_defs_filtered(&allowed).len(), 1);
        let empty = vec!["nonexistent".to_string()];
        assert_eq!(reg.list_defs_filtered(&empty).len(), 0);
    }

    #[test]
    fn tool_def_from_trait() {
        let tool = MockTool;
        let def = tool.to_def();
        assert_eq!(def.name, "mock_tool");
        assert_eq!(def.description, "A mock tool for testing");
    }

    #[tokio::test]
    async fn mock_tool_executes() {
        let tool = MockTool;
        let ctx = ToolContext {
            workspace_root: "/tmp".into(),
            agent_id: eagent_protocol::ids::AgentId::new(),
            task_id: eagent_protocol::ids::TaskId::new(),
        };
        let result = tool.execute(json!({"input": "hello"}), &ctx).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(result.output["echo"], "hello");
    }
}

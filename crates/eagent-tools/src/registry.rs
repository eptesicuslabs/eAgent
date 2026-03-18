use crate::{Tool, ToolDef};
use std::collections::HashMap;
use std::sync::Arc;

/// Registry of available tools.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    /// Register a tool. Overwrites any existing tool with the same name.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    /// List all registered tool definitions.
    pub fn list_defs(&self) -> Vec<ToolDef> {
        self.tools.values().map(|t| t.to_def()).collect()
    }

    /// List tool definitions filtered by allowed names.
    pub fn list_defs_filtered(&self, allowed: &[String]) -> Vec<ToolDef> {
        allowed.iter()
            .filter_map(|name| self.tools.get(name))
            .map(|t| t.to_def())
            .collect()
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

use eagent_contracts::provider::ProviderEvent;
use eagent_protocol::messages::TaskConstraints;
use eagent_protocol::task_graph::{TaskGraph, TaskNode, TaskStatus};
use eagent_protocol::{TaskGraphId, TaskId};
use eagent_providers::registry::ProviderRegistry;
use eagent_providers::{ProviderMessage, ProviderMessageRole, SessionConfig};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlannerError {
    #[error("provider error: {0}")]
    Provider(String),
    #[error("failed to parse plan: {0}")]
    Parse(String),
    #[error("no provider configured for planning")]
    NoProvider,
}

/// LLM-powered planner that decomposes user prompts into multi-task DAGs.
///
/// Calls a configured provider with a planner system prompt, receives a
/// JSON task decomposition, and constructs a TaskGraph.
pub struct LlmPlanner {
    providers: Arc<ProviderRegistry>,
    provider_name: String,
}

impl LlmPlanner {
    pub fn new(providers: Arc<ProviderRegistry>, provider_name: String) -> Self {
        Self {
            providers,
            provider_name,
        }
    }

    /// Decompose a user prompt into a TaskGraph by calling an LLM.
    ///
    /// Falls back to a single-task graph if the LLM response can't be parsed.
    pub async fn plan(
        &self,
        prompt: &str,
        workspace_root: &str,
        project_summary: Option<&str>,
    ) -> Result<TaskGraph, PlannerError> {
        let provider = self
            .providers
            .get(&self.provider_name)
            .ok_or(PlannerError::NoProvider)?
            .clone();

        let system_prompt = build_planner_system_prompt(workspace_root, project_summary);

        let session = provider
            .create_session(SessionConfig {
                model: String::new(),
                system_prompt: Some(system_prompt),
                workspace_root: Some(workspace_root.to_string()),
            })
            .await
            .map_err(|e| PlannerError::Provider(e.to_string()))?;

        let messages = vec![ProviderMessage {
            role: ProviderMessageRole::User,
            tool_call_id: None,
            tool_calls: None,
            content: format!(
                "Decompose this request into a task plan:\n\n{prompt}\n\n\
                 Respond with a JSON object following the schema described in your system prompt."
            ),
        }];

        let mut event_rx = provider
            .send(&session, messages, vec![])
            .await
            .map_err(|e| PlannerError::Provider(e.to_string()))?;

        // Collect the full response
        let mut response_text = String::new();
        while let Some(event) = event_rx.recv().await {
            match event {
                ProviderEvent::TokenDelta { text } => response_text.push_str(&text),
                ProviderEvent::Completion { .. } => break,
                ProviderEvent::Error { message } => {
                    return Err(PlannerError::Provider(message));
                }
                _ => {}
            }
        }

        tracing::debug!(response_len = response_text.len(), "planner response received");

        // Try to parse the JSON response into a task plan
        match parse_plan_response(&response_text, prompt) {
            Ok(graph) => {
                tracing::info!(
                    tasks = graph.nodes.len(),
                    edges = graph.edges.len(),
                    "LLM planner produced task graph"
                );
                Ok(graph)
            }
            Err(e) => {
                tracing::warn!(
                    err = %e,
                    "failed to parse LLM plan, falling back to single task"
                );
                Ok(single_task_fallback(prompt))
            }
        }
    }
}

/// Build the system prompt that instructs the LLM to produce a task plan.
fn build_planner_system_prompt(workspace_root: &str, project_summary: Option<&str>) -> String {
    let mut prompt = format!(
        "You are a task planner for an AI coding agent system. Your job is to decompose \
         a user's request into a structured plan of sub-tasks that can be executed by \
         independent worker agents.\n\n\
         Workspace: {workspace_root}\n"
    );

    if let Some(summary) = project_summary {
        prompt.push_str(&format!("\nProject context:\n{summary}\n"));
    }

    prompt.push_str(
        r#"
## Instructions

Analyze the user's request and break it into independent sub-tasks. Each sub-task should be:
- Small enough for a single agent to complete
- Have clear, specific deliverables
- Specify which tools the agent will need

If tasks depend on each other, specify dependencies. Independent tasks can run in parallel.

## Available Tools

Agents have access to these tools:
- list_directory, read_file, read_multiple_files, search_files (read codebase)
- write_file, edit_file, apply_patch (modify files)
- git_status, git_diff, git_commit, git_branch (git operations)
- web_search, web_fetch (internet access)

## Response Format

Respond with ONLY a JSON object (no markdown, no explanation):

```json
{
  "tasks": [
    {
      "id": "task_1",
      "description": "Clear description of what this agent should do",
      "tools": ["read_file", "edit_file", "run_command"],
      "depends_on": []
    },
    {
      "id": "task_2",
      "description": "Another task that depends on task_1",
      "tools": ["read_file", "write_file"],
      "depends_on": ["task_1"]
    }
  ]
}
```

Rules:
- Use 1-6 tasks. Simple requests need 1 task. Complex ones need more.
- Task IDs must be unique strings like "task_1", "task_2", etc.
- depends_on references other task IDs (must not create cycles)
- Only include tools the task actually needs
- If the request is simple (single file edit, quick question), use exactly 1 task
"#,
    );

    prompt
}

/// JSON schema for the planner response.
#[derive(Debug, Deserialize)]
struct PlanResponse {
    tasks: Vec<PlanTask>,
}

#[derive(Debug, Deserialize)]
struct PlanTask {
    id: String,
    description: String,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    depends_on: Vec<String>,
}

/// Parse the LLM response text into a TaskGraph.
fn parse_plan_response(response: &str, user_prompt: &str) -> Result<TaskGraph, PlannerError> {
    // Extract JSON from response (handle markdown code blocks)
    let json_str = extract_json(response);

    let plan: PlanResponse =
        serde_json::from_str(json_str).map_err(|e| PlannerError::Parse(e.to_string()))?;

    if plan.tasks.is_empty() {
        return Err(PlannerError::Parse("plan has no tasks".into()));
    }

    let graph_id = TaskGraphId::new();

    // Map plan task IDs to real TaskIds
    let mut id_map: HashMap<String, TaskId> = HashMap::new();
    for task in &plan.tasks {
        id_map.insert(task.id.clone(), TaskId::new());
    }

    // Build nodes
    let mut nodes: HashMap<TaskId, TaskNode> = HashMap::new();
    let mut root_task_id = None;

    for task in &plan.tasks {
        let task_id = id_map[&task.id];
        if root_task_id.is_none() {
            root_task_id = Some(task_id);
        }

        let tools = if task.tools.is_empty() {
            default_tools()
        } else {
            task.tools.clone()
        };

        nodes.insert(
            task_id,
            TaskNode {
                id: task_id,
                description: task.description.clone(),
                status: TaskStatus::Pending,
                assigned_agent: None,
                assigned_provider: None,
                tools_allowed: tools,
                constraints: TaskConstraints::default(),
                result: None,
                trace: vec![],
                parent_task_id: None,
                depth: 0,
            },
        );
    }

    // Build edges (dependency → dependent)
    let mut edges = Vec::new();
    for task in &plan.tasks {
        let dependent_id = id_map[&task.id];
        for dep_str in &task.depends_on {
            if let Some(&dependency_id) = id_map.get(dep_str) {
                edges.push((dependency_id, dependent_id));
            } else {
                tracing::warn!(
                    task = %task.id,
                    dependency = %dep_str,
                    "ignoring unknown dependency"
                );
            }
        }
    }

    Ok(TaskGraph {
        id: graph_id,
        root_task_id: root_task_id.unwrap(),
        user_prompt: user_prompt.to_string(),
        nodes,
        edges,
    })
}

/// Extract JSON from a response that may contain markdown code blocks.
fn extract_json(text: &str) -> &str {
    let trimmed = text.trim();

    // Try to find ```json ... ``` block
    if let Some(start) = trimmed.find("```json") {
        let json_start = start + 7;
        if let Some(end) = trimmed[json_start..].find("```") {
            return trimmed[json_start..json_start + end].trim();
        }
    }

    // Try to find ``` ... ``` block
    if let Some(start) = trimmed.find("```") {
        let json_start = start + 3;
        // Skip optional language tag on first line
        let content = &trimmed[json_start..];
        let actual_start = content.find('\n').map(|i| i + 1).unwrap_or(0);
        if let Some(end) = content[actual_start..].find("```") {
            return content[actual_start..actual_start + end].trim();
        }
    }

    // Try to find raw JSON object
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            return &trimmed[start..=end];
        }
    }

    trimmed
}

/// Default tool set for tasks that don't specify tools.
fn default_tools() -> Vec<String> {
    vec![
        "list_directory".into(),
        "read_file".into(),
        "read_multiple_files".into(),
        "search_files".into(),
        "write_file".into(),
        "edit_file".into(),
        "apply_patch".into(),
        "git_status".into(),
        "git_diff".into(),
        "git_commit".into(),
        "web_search".into(),
        "web_fetch".into(),
    ]
}

/// Fallback: create a single-task graph (same as SimplePlanner).
fn single_task_fallback(prompt: &str) -> TaskGraph {
    use crate::simple::SimplePlanner;
    SimplePlanner::new().plan(prompt, ".")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_plan() {
        let json = r#"{"tasks": [{"id": "task_1", "description": "Fix the bug", "tools": ["read_file", "edit_file"], "depends_on": []}]}"#;
        let graph = parse_plan_response(json, "Fix the bug").unwrap();
        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.edges.is_empty());
        let root = &graph.nodes[&graph.root_task_id];
        assert_eq!(root.description, "Fix the bug");
        assert_eq!(root.tools_allowed, vec!["read_file", "edit_file"]);
    }

    #[test]
    fn parse_multi_task_plan() {
        let json = r#"{
            "tasks": [
                {"id": "t1", "description": "Read the codebase", "tools": ["read_file"], "depends_on": []},
                {"id": "t2", "description": "Update middleware", "tools": ["edit_file"], "depends_on": ["t1"]},
                {"id": "t3", "description": "Write tests", "tools": ["write_file", "run_command"], "depends_on": ["t1"]}
            ]
        }"#;
        let graph = parse_plan_response(json, "Refactor auth").unwrap();
        assert_eq!(graph.nodes.len(), 3);
        // t2 and t3 both depend on t1
        assert_eq!(graph.edges.len(), 2);
    }

    #[test]
    fn parse_with_code_block() {
        let response = "Here's the plan:\n\n```json\n{\"tasks\": [{\"id\": \"task_1\", \"description\": \"Do it\", \"tools\": [], \"depends_on\": []}]}\n```\n\nLet me know!";
        let graph = parse_plan_response(response, "Do it").unwrap();
        assert_eq!(graph.nodes.len(), 1);
    }

    #[test]
    fn parse_raw_json() {
        let response = "{\"tasks\": [{\"id\": \"a\", \"description\": \"Hello\", \"tools\": [\"read_file\"], \"depends_on\": []}]}";
        let graph = parse_plan_response(response, "Hello").unwrap();
        assert_eq!(graph.nodes.len(), 1);
    }

    #[test]
    fn empty_tasks_returns_error() {
        let json = r#"{"tasks": []}"#;
        assert!(parse_plan_response(json, "test").is_err());
    }

    #[test]
    fn unknown_dependency_is_ignored() {
        let json = r#"{"tasks": [{"id": "t1", "description": "task", "tools": [], "depends_on": ["nonexistent"]}]}"#;
        let graph = parse_plan_response(json, "test").unwrap();
        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn extract_json_from_markdown() {
        let text = "```json\n{\"key\": \"value\"}\n```";
        assert_eq!(extract_json(text), "{\"key\": \"value\"}");
    }

    #[test]
    fn extract_json_raw() {
        let text = "Some text before {\"key\": \"value\"} and after";
        assert_eq!(extract_json(text), "{\"key\": \"value\"}");
    }

    #[test]
    fn default_tools_populated_when_empty() {
        let json = r#"{"tasks": [{"id": "t1", "description": "task", "tools": [], "depends_on": []}]}"#;
        let graph = parse_plan_response(json, "test").unwrap();
        let root = &graph.nodes[&graph.root_task_id];
        assert!(!root.tools_allowed.is_empty());
        assert!(root.tools_allowed.contains(&"read_file".to_string()));
    }

    #[test]
    fn dependencies_create_correct_edges() {
        let json = r#"{
            "tasks": [
                {"id": "a", "description": "first", "tools": [], "depends_on": []},
                {"id": "b", "description": "second", "tools": [], "depends_on": ["a"]},
                {"id": "c", "description": "third", "tools": [], "depends_on": ["a", "b"]}
            ]
        }"#;
        let graph = parse_plan_response(json, "test").unwrap();
        // a→b, a→c, b→c = 3 edges
        assert_eq!(graph.edges.len(), 3);
    }
}

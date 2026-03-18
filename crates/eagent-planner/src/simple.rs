use eagent_protocol::messages::TaskConstraints;
use eagent_protocol::task_graph::{TaskGraph, TaskNode, TaskStatus};
use eagent_protocol::{TaskGraphId, TaskId};
use std::collections::HashMap;

/// Simple planner that creates a single-task graph from a user prompt.
/// A real planner would use an LLM to decompose the task.
pub struct SimplePlanner;

impl SimplePlanner {
    pub fn new() -> Self {
        Self
    }

    /// Create a single-task graph from a user prompt.
    ///
    /// The resulting graph contains exactly one task in `Ready` status
    /// (no dependencies, so it can be scheduled immediately).
    pub fn plan(&self, prompt: &str, _workspace_root: &str) -> TaskGraph {
        let graph_id = TaskGraphId::new();
        let task_id = TaskId::new();

        let task = TaskNode {
            id: task_id,
            description: prompt.to_string(),
            status: TaskStatus::Ready, // single task, no deps, immediately ready
            assigned_agent: None,
            assigned_provider: None,
            tools_allowed: vec![
                "list_directory".into(),
                "read_file".into(),
                "read_multiple_files".into(),
                "search_files".into(),
                "write_file".into(),
                "edit_file".into(),
                "apply_patch".into(),
                "run_command".into(),
                "web_search".into(),
                "web_fetch".into(),
                "git_status".into(),
                "git_diff".into(),
                "git_commit".into(),
                "git_branch".into(),
            ],
            constraints: TaskConstraints::default(),
            result: None,
            trace: vec![],
        };

        let mut nodes = HashMap::new();
        nodes.insert(task_id, task);

        TaskGraph {
            id: graph_id,
            root_task_id: task_id,
            user_prompt: prompt.to_string(),
            nodes,
            edges: vec![],
        }
    }
}

impl Default for SimplePlanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_creates_valid_graph() {
        let planner = SimplePlanner::new();
        let graph = planner.plan("Fix the login bug", "/tmp/project");

        assert_eq!(graph.user_prompt, "Fix the login bug");
        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.edges.is_empty());
        assert!(graph.nodes.contains_key(&graph.root_task_id));
    }

    #[test]
    fn single_task_is_ready() {
        let planner = SimplePlanner::new();
        let graph = planner.plan("Refactor auth module", "/tmp/project");

        let root = &graph.nodes[&graph.root_task_id];
        assert_eq!(root.status, TaskStatus::Ready);
        assert_eq!(root.description, "Refactor auth module");
        assert!(root.assigned_agent.is_none());
        assert!(root.assigned_provider.is_none());
    }

    #[test]
    fn tools_are_populated() {
        let planner = SimplePlanner::new();
        let graph = planner.plan("Add tests", "/tmp/project");

        let root = &graph.nodes[&graph.root_task_id];
        assert!(!root.tools_allowed.is_empty());
        assert!(root.tools_allowed.contains(&"read_file".to_string()));
        assert!(root.tools_allowed.contains(&"write_file".to_string()));
        assert!(root.tools_allowed.contains(&"run_command".to_string()));
        assert!(root.tools_allowed.contains(&"git_commit".to_string()));
    }

    #[test]
    fn constraints_are_default() {
        let planner = SimplePlanner::new();
        let graph = planner.plan("Hello", "/tmp/project");

        let root = &graph.nodes[&graph.root_task_id];
        assert!(root.constraints.max_tokens.is_none());
        assert!(root.constraints.max_tool_calls.is_none());
        assert!(root.constraints.max_time_secs.is_none());
        assert!(root.constraints.allowed_paths.is_none());
    }

    #[test]
    fn each_plan_gets_unique_ids() {
        let planner = SimplePlanner::new();
        let g1 = planner.plan("task one", "/tmp/a");
        let g2 = planner.plan("task two", "/tmp/b");

        assert_ne!(g1.id, g2.id);
        assert_ne!(g1.root_task_id, g2.root_task_id);
    }
}

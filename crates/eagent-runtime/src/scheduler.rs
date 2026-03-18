use eagent_protocol::ids::TaskId;
use eagent_protocol::task_graph::{TaskGraph, TaskStatus};
use std::collections::{HashMap, VecDeque};

use crate::error::SchedulerError;

/// TaskGraph scheduler — identifies ready tasks, validates the DAG, and manages
/// state transitions from Pending to Ready as dependencies are satisfied.
pub struct Scheduler {
    max_concurrency: u32,
}

impl Scheduler {
    pub fn new(max_concurrency: u32) -> Self {
        Self { max_concurrency }
    }

    /// Find all tasks in Ready state, respecting the concurrency limit.
    /// Returns at most `max_concurrency - currently_running` task IDs.
    pub fn next_tasks(&self, graph: &TaskGraph) -> Vec<TaskId> {
        let running_count = graph
            .nodes
            .values()
            .filter(|n| matches!(n.status, TaskStatus::Running | TaskStatus::Scheduled))
            .count() as u32;

        let available_slots = self.max_concurrency.saturating_sub(running_count);
        if available_slots == 0 {
            return Vec::new();
        }

        graph
            .nodes
            .values()
            .filter(|n| n.status == TaskStatus::Ready)
            .map(|n| n.id)
            .take(available_slots as usize)
            .collect()
    }

    /// Check if all dependencies of a task are in the Complete state.
    pub fn dependencies_met(&self, graph: &TaskGraph, task_id: TaskId) -> bool {
        let deps = Self::get_dependencies(graph, task_id);
        deps.iter().all(|dep_id| {
            graph
                .nodes
                .get(dep_id)
                .map(|n| n.status == TaskStatus::Complete)
                .unwrap_or(false)
        })
    }

    /// Validate that the task graph is a well-formed DAG:
    /// - No cycles
    /// - All referenced task IDs in edges exist in nodes
    /// - Root task exists
    /// - Graph is non-empty
    pub fn validate_dag(graph: &TaskGraph) -> Result<(), SchedulerError> {
        if graph.nodes.is_empty() {
            return Err(SchedulerError::EmptyGraph);
        }

        if !graph.nodes.contains_key(&graph.root_task_id) {
            return Err(SchedulerError::RootTaskMissing(graph.root_task_id));
        }

        // Check that all edge references exist
        for (dep, dependent) in &graph.edges {
            if !graph.nodes.contains_key(dep) {
                return Err(SchedulerError::DanglingDependency {
                    dependency: *dep,
                    dependent: *dependent,
                });
            }
            if !graph.nodes.contains_key(dependent) {
                return Err(SchedulerError::DanglingDependency {
                    dependency: *dep,
                    dependent: *dependent,
                });
            }
        }

        // Cycle detection via Kahn's algorithm (topological sort)
        Self::detect_cycle(graph)?;

        Ok(())
    }

    /// Transition tasks from Pending to Ready when all their dependencies are met.
    pub fn update_ready_states(graph: &mut TaskGraph) {
        // Collect IDs of tasks that should become Ready (cannot mutate while iterating)
        let to_ready: Vec<TaskId> = graph
            .nodes
            .values()
            .filter(|n| n.status == TaskStatus::Pending)
            .filter(|n| {
                let deps = Self::get_dependencies(graph, n.id);
                deps.iter().all(|dep_id| {
                    graph
                        .nodes
                        .get(dep_id)
                        .map(|dn| dn.status == TaskStatus::Complete)
                        .unwrap_or(false)
                })
            })
            .map(|n| n.id)
            .collect();

        for id in to_ready {
            if let Some(node) = graph.nodes.get_mut(&id) {
                node.status = TaskStatus::Ready;
            }
        }
    }

    /// Get the direct dependencies (predecessors) of a task.
    fn get_dependencies(graph: &TaskGraph, task_id: TaskId) -> Vec<TaskId> {
        graph
            .edges
            .iter()
            .filter(|(_, dependent)| *dependent == task_id)
            .map(|(dep, _)| *dep)
            .collect()
    }

    /// Detect cycles using Kahn's algorithm. Returns Ok(()) if the graph is a DAG.
    fn detect_cycle(graph: &TaskGraph) -> Result<(), SchedulerError> {
        // Build adjacency list and in-degree map
        let mut in_degree: HashMap<TaskId, usize> = HashMap::new();
        let mut adjacency: HashMap<TaskId, Vec<TaskId>> = HashMap::new();

        for id in graph.nodes.keys() {
            in_degree.entry(*id).or_insert(0);
            adjacency.entry(*id).or_default();
        }

        for (dep, dependent) in &graph.edges {
            adjacency.entry(*dep).or_default().push(*dependent);
            *in_degree.entry(*dependent).or_insert(0) += 1;
        }

        // Start with nodes that have no incoming edges
        let mut queue: VecDeque<TaskId> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(id, _)| *id)
            .collect();

        let mut visited_count = 0usize;

        while let Some(node) = queue.pop_front() {
            visited_count += 1;

            if let Some(neighbors) = adjacency.get(&node) {
                for neighbor in neighbors {
                    if let Some(deg) = in_degree.get_mut(neighbor) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(*neighbor);
                        }
                    }
                }
            }
        }

        if visited_count != graph.nodes.len() {
            return Err(SchedulerError::CycleDetected);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eagent_protocol::ids::{TaskGraphId, TaskId};
    use eagent_protocol::messages::TaskConstraints;
    use eagent_protocol::task_graph::{TaskGraph, TaskNode, TaskStatus};
    use std::collections::HashMap;

    /// Helper: create a minimal TaskNode with the given status.
    fn make_node(id: TaskId, status: TaskStatus) -> TaskNode {
        TaskNode {
            id,
            description: format!("Task {id}"),
            status,
            assigned_agent: None,
            assigned_provider: None,
            tools_allowed: vec![],
            constraints: TaskConstraints::default(),
            result: None,
            trace: vec![],
        }
    }

    /// Helper: build a TaskGraph from nodes and edges.
    fn make_graph(
        nodes: Vec<TaskNode>,
        edges: Vec<(TaskId, TaskId)>,
    ) -> TaskGraph {
        let root_task_id = nodes[0].id;
        let mut node_map = HashMap::new();
        for n in nodes {
            node_map.insert(n.id, n);
        }
        TaskGraph {
            id: TaskGraphId::new(),
            root_task_id,
            user_prompt: "test".into(),
            nodes: node_map,
            edges,
        }
    }

    // --- validate_dag ---

    #[test]
    fn validate_dag_accepts_linear_chain() {
        let a = TaskId::new();
        let b = TaskId::new();
        let c = TaskId::new();
        let graph = make_graph(
            vec![
                make_node(a, TaskStatus::Pending),
                make_node(b, TaskStatus::Pending),
                make_node(c, TaskStatus::Pending),
            ],
            vec![(a, b), (b, c)],
        );
        assert!(Scheduler::validate_dag(&graph).is_ok());
    }

    #[test]
    fn validate_dag_accepts_diamond() {
        //   A
        //  / \
        // B   C
        //  \ /
        //   D
        let a = TaskId::new();
        let b = TaskId::new();
        let c = TaskId::new();
        let d = TaskId::new();
        let graph = make_graph(
            vec![
                make_node(a, TaskStatus::Pending),
                make_node(b, TaskStatus::Pending),
                make_node(c, TaskStatus::Pending),
                make_node(d, TaskStatus::Pending),
            ],
            vec![(a, b), (a, c), (b, d), (c, d)],
        );
        assert!(Scheduler::validate_dag(&graph).is_ok());
    }

    #[test]
    fn validate_dag_accepts_single_node() {
        let a = TaskId::new();
        let graph = make_graph(vec![make_node(a, TaskStatus::Pending)], vec![]);
        assert!(Scheduler::validate_dag(&graph).is_ok());
    }

    #[test]
    fn validate_dag_rejects_cycle() {
        let a = TaskId::new();
        let b = TaskId::new();
        let c = TaskId::new();
        let graph = make_graph(
            vec![
                make_node(a, TaskStatus::Pending),
                make_node(b, TaskStatus::Pending),
                make_node(c, TaskStatus::Pending),
            ],
            vec![(a, b), (b, c), (c, a)],
        );
        let err = Scheduler::validate_dag(&graph).unwrap_err();
        assert!(matches!(err, SchedulerError::CycleDetected));
    }

    #[test]
    fn validate_dag_rejects_self_loop() {
        let a = TaskId::new();
        let graph = make_graph(
            vec![make_node(a, TaskStatus::Pending)],
            vec![(a, a)],
        );
        let err = Scheduler::validate_dag(&graph).unwrap_err();
        assert!(matches!(err, SchedulerError::CycleDetected));
    }

    #[test]
    fn validate_dag_rejects_dangling_dependency() {
        let a = TaskId::new();
        let phantom = TaskId::new(); // not in nodes
        let graph = make_graph(
            vec![make_node(a, TaskStatus::Pending)],
            vec![(phantom, a)],
        );
        let err = Scheduler::validate_dag(&graph).unwrap_err();
        assert!(matches!(err, SchedulerError::DanglingDependency { .. }));
    }

    // --- dependencies_met ---

    #[test]
    fn dependencies_met_when_all_complete() {
        let a = TaskId::new();
        let b = TaskId::new();
        let graph = make_graph(
            vec![
                make_node(a, TaskStatus::Complete),
                make_node(b, TaskStatus::Pending),
            ],
            vec![(a, b)],
        );
        let scheduler = Scheduler::new(4);
        assert!(scheduler.dependencies_met(&graph, b));
    }

    #[test]
    fn dependencies_not_met_when_dep_running() {
        let a = TaskId::new();
        let b = TaskId::new();
        let graph = make_graph(
            vec![
                make_node(a, TaskStatus::Running),
                make_node(b, TaskStatus::Pending),
            ],
            vec![(a, b)],
        );
        let scheduler = Scheduler::new(4);
        assert!(!scheduler.dependencies_met(&graph, b));
    }

    #[test]
    fn dependencies_met_when_no_deps() {
        let a = TaskId::new();
        let graph = make_graph(vec![make_node(a, TaskStatus::Pending)], vec![]);
        let scheduler = Scheduler::new(4);
        assert!(scheduler.dependencies_met(&graph, a));
    }

    // --- next_tasks ---

    #[test]
    fn next_tasks_returns_ready_tasks() {
        let a = TaskId::new();
        let b = TaskId::new();
        let graph = make_graph(
            vec![
                make_node(a, TaskStatus::Ready),
                make_node(b, TaskStatus::Pending),
            ],
            vec![(a, b)],
        );
        let scheduler = Scheduler::new(4);
        let next = scheduler.next_tasks(&graph);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0], a);
    }

    #[test]
    fn next_tasks_respects_concurrency_limit() {
        let a = TaskId::new();
        let b = TaskId::new();
        let c = TaskId::new();
        let graph = make_graph(
            vec![
                make_node(a, TaskStatus::Running),
                make_node(b, TaskStatus::Ready),
                make_node(c, TaskStatus::Ready),
            ],
            vec![],
        );
        // Only 1 slot available (max 2, 1 running)
        let scheduler = Scheduler::new(2);
        let next = scheduler.next_tasks(&graph);
        assert_eq!(next.len(), 1);
    }

    #[test]
    fn next_tasks_returns_empty_when_at_capacity() {
        let a = TaskId::new();
        let b = TaskId::new();
        let graph = make_graph(
            vec![
                make_node(a, TaskStatus::Running),
                make_node(b, TaskStatus::Ready),
            ],
            vec![],
        );
        let scheduler = Scheduler::new(1);
        let next = scheduler.next_tasks(&graph);
        assert!(next.is_empty());
    }

    // --- update_ready_states ---

    #[test]
    fn update_ready_states_promotes_pending_when_deps_complete() {
        let a = TaskId::new();
        let b = TaskId::new();
        let mut graph = make_graph(
            vec![
                make_node(a, TaskStatus::Complete),
                make_node(b, TaskStatus::Pending),
            ],
            vec![(a, b)],
        );
        Scheduler::update_ready_states(&mut graph);
        assert_eq!(graph.nodes[&b].status, TaskStatus::Ready);
    }

    #[test]
    fn update_ready_states_does_not_promote_when_deps_incomplete() {
        let a = TaskId::new();
        let b = TaskId::new();
        let mut graph = make_graph(
            vec![
                make_node(a, TaskStatus::Running),
                make_node(b, TaskStatus::Pending),
            ],
            vec![(a, b)],
        );
        Scheduler::update_ready_states(&mut graph);
        assert_eq!(graph.nodes[&b].status, TaskStatus::Pending);
    }

    #[test]
    fn update_ready_states_promotes_root_with_no_deps() {
        let a = TaskId::new();
        let mut graph = make_graph(
            vec![make_node(a, TaskStatus::Pending)],
            vec![],
        );
        Scheduler::update_ready_states(&mut graph);
        assert_eq!(graph.nodes[&a].status, TaskStatus::Ready);
    }
}

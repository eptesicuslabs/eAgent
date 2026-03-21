use eagent_persistence::EventStore;
use eagent_protocol::events::TaskGraphEvent;
use eagent_protocol::ids::{TaskGraphId, TaskId};
use eagent_protocol::messages::AgentMessage;
use eagent_protocol::task_graph::{TaskGraph, TaskStatus};
use eagent_protocol::traits::AgentContext;
use eagent_providers::registry::ProviderRegistry;
use eagent_tools::registry::ToolRegistry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing;

use crate::agent_pool::AgentPool;
use crate::conflict::ConflictResolver;
use crate::error::RuntimeError;
use crate::scheduler::Scheduler;

/// Events emitted by the runtime engine for consumption by the UI layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RuntimeEvent {
    GraphCreated {
        graph_id: TaskGraphId,
    },
    TaskScheduled {
        graph_id: TaskGraphId,
        task_id: TaskId,
    },
    TaskStarted {
        graph_id: TaskGraphId,
        task_id: TaskId,
    },
    AgentMessage {
        graph_id: TaskGraphId,
        task_id: TaskId,
        message: AgentMessage,
    },
    TaskCompleted {
        graph_id: TaskGraphId,
        task_id: TaskId,
    },
    TaskFailed {
        graph_id: TaskGraphId,
        task_id: TaskId,
        error: String,
    },
    ToolResult {
        graph_id: TaskGraphId,
        task_id: TaskId,
        tool_name: String,
        result: eagent_tools::ToolResult,
    },
    GraphCompleted {
        graph_id: TaskGraphId,
    },
}

/// Configuration for the runtime engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Maximum number of concurrent agents across all graphs.
    pub max_concurrency: u32,
    /// Maximum retry attempts for a failed task.
    pub max_retries: u32,
    /// Default provider name to use when a task does not specify one.
    pub default_provider: String,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 4,
            max_retries: 2,
            default_provider: "codex".into(),
        }
    }
}

/// The runtime engine — ties together the scheduler, agent pool, conflict
/// resolver, and event store to orchestrate multi-agent task execution.
pub struct RuntimeEngine {
    scheduler: Scheduler,
    agent_pool: AgentPool,
    #[allow(dead_code)]
    conflict_resolver: ConflictResolver,
    event_store: Arc<EventStore>,
    graphs: Arc<RwLock<HashMap<TaskGraphId, TaskGraph>>>,
    event_tx: mpsc::UnboundedSender<RuntimeEvent>,
    config: RuntimeConfig,
    /// Agent context shared with all spawned agents.
    agent_ctx: AgentContext,
    /// Tool registry for executing tool calls from agents.
    tool_registry: Arc<ToolRegistry>,
    /// Receivers for in-flight agent message streams, keyed by (graph_id, task_id).
    agent_receivers: Arc<RwLock<HashMap<(TaskGraphId, TaskId), mpsc::UnboundedReceiver<AgentMessage>>>>,
}

impl RuntimeEngine {
    /// Create a new RuntimeEngine. Returns the engine and a receiver for
    /// RuntimeEvents that the UI can subscribe to.
    pub fn new(
        providers: Arc<ProviderRegistry>,
        tools: Arc<ToolRegistry>,
        event_store: Arc<EventStore>,
        config: RuntimeConfig,
        agent_ctx: AgentContext,
    ) -> (Self, mpsc::UnboundedReceiver<RuntimeEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let scheduler = Scheduler::new(config.max_concurrency);
        let agent_pool = AgentPool::new(providers, tools.clone());

        let engine = Self {
            scheduler,
            agent_pool,
            conflict_resolver: ConflictResolver,
            event_store,
            graphs: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            config,
            agent_ctx,
            tool_registry: tools,
            agent_receivers: Arc::new(RwLock::new(HashMap::new())),
        };

        (engine, event_rx)
    }

    /// Submit a new task graph for execution. Validates the DAG, persists the
    /// creation event, and makes the graph available for scheduling.
    pub async fn submit(&self, mut graph: TaskGraph) -> Result<TaskGraphId, RuntimeError> {
        // Validate the DAG structure
        Scheduler::validate_dag(&graph)?;

        let graph_id = graph.id;

        // Persist the GraphCreated event
        self.persist_event(&TaskGraphEvent::GraphCreated {
            graph_id,
            root_task: graph.nodes[&graph.root_task_id].clone(),
            user_prompt: graph.user_prompt.clone(),
            timestamp: chrono::Utc::now(),
        })?;

        // Initialize ready states — tasks with no dependencies become Ready
        Scheduler::update_ready_states(&mut graph);

        // Store the graph
        {
            let mut graphs = self.graphs.write().await;
            graphs.insert(graph_id, graph);
        }

        // Emit GraphCreated event
        let _ = self.event_tx.send(RuntimeEvent::GraphCreated { graph_id });

        tracing::info!(?graph_id, "task graph submitted");

        Ok(graph_id)
    }

    /// Run the main scheduling loop. This should be called inside a
    /// `tokio::spawn`. It continuously checks for ready tasks, spawns agents,
    /// collects results, and updates the graph until all graphs are complete.
    pub async fn run(&self) {
        tracing::info!("runtime engine scheduling loop started");

        loop {
            let mut any_active = false;

            // Collect graph IDs to process
            let graph_ids: Vec<TaskGraphId> = {
                let graphs = self.graphs.read().await;
                graphs.keys().copied().collect()
            };

            let mut completed_graph_ids = Vec::new();

            for graph_id in graph_ids {
                // First, poll existing agent receivers for this graph
                self.poll_agent_messages(graph_id).await;

                // Then check for new tasks to schedule
                let tasks_to_schedule = {
                    let graphs = self.graphs.read().await;
                    if let Some(graph) = graphs.get(&graph_id) {
                        // Check if graph is still active
                        let has_active_tasks = graph.nodes.values().any(|n| {
                            matches!(
                                n.status,
                                TaskStatus::Pending
                                    | TaskStatus::Ready
                                    | TaskStatus::Scheduled
                                    | TaskStatus::Running
                                    | TaskStatus::AwaitingReview
                            )
                        });

                        if has_active_tasks {
                            any_active = true;
                            self.scheduler.next_tasks(graph)
                        } else {
                            // Check if graph just completed (all tasks terminal)
                            let all_terminal = graph.nodes.values().all(|n| {
                                matches!(
                                    n.status,
                                    TaskStatus::Complete
                                        | TaskStatus::Failed { .. }
                                        | TaskStatus::Cancelled { .. }
                                )
                            });
                            if all_terminal {
                                completed_graph_ids.push(graph_id);
                            }
                            vec![]
                        }
                    } else {
                        vec![]
                    }
                };

                // Spawn agents for scheduled tasks
                for task_id in tasks_to_schedule {
                    self.schedule_task(graph_id, task_id).await;
                }
            }

            // Finalize completed graphs (outside read lock)
            for graph_id in completed_graph_ids {
                self.complete_graph(graph_id).await;
            }

            if !any_active {
                // No active graphs — wait a bit before checking again.
                // In a real system this would be event-driven, but for v1
                // a simple polling loop is sufficient.
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            } else {
                // Yield to allow other tasks to run, but don't wait long
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        }
    }

    /// Cancel all tasks in a graph.
    pub async fn cancel_graph(&self, graph_id: TaskGraphId) -> Result<(), RuntimeError> {
        let task_ids: Vec<TaskId> = {
            let graphs = self.graphs.read().await;
            let graph = graphs
                .get(&graph_id)
                .ok_or(RuntimeError::GraphNotFound(graph_id))?;
            graph
                .nodes
                .values()
                .filter(|n| {
                    matches!(
                        n.status,
                        TaskStatus::Running | TaskStatus::Scheduled | TaskStatus::Ready
                    )
                })
                .map(|n| n.id)
                .collect()
        };

        for task_id in &task_ids {
            // Cancel running agents (ignore errors for tasks not yet spawned)
            let _ = self.agent_pool.cancel_agent(*task_id).await;
        }

        // Update graph state
        {
            let mut graphs = self.graphs.write().await;
            if let Some(graph) = graphs.get_mut(&graph_id) {
                for task_id in &task_ids {
                    if let Some(node) = graph.nodes.get_mut(task_id) {
                        node.status = TaskStatus::Cancelled {
                            reason: "graph cancelled by user".into(),
                        };
                    }
                }
            }
        }

        tracing::info!(?graph_id, cancelled_count = task_ids.len(), "graph cancelled");

        Ok(())
    }

    /// Get a clone of the current state of a graph.
    pub async fn get_graph(&self, graph_id: TaskGraphId) -> Option<TaskGraph> {
        let graphs = self.graphs.read().await;
        graphs.get(&graph_id).cloned()
    }

    // --- internal helpers ---

    /// Schedule a single task: mark it as Scheduled, spawn an agent, mark it as Running.
    async fn schedule_task(&self, graph_id: TaskGraphId, task_id: TaskId) {
        let task_node = {
            let mut graphs = self.graphs.write().await;
            let graph = match graphs.get_mut(&graph_id) {
                Some(g) => g,
                None => return,
            };
            let node = match graph.nodes.get_mut(&task_id) {
                Some(n) => n,
                None => return,
            };
            node.status = TaskStatus::Scheduled;
            node.clone()
        };

        let _ = self.event_tx.send(RuntimeEvent::TaskScheduled {
            graph_id,
            task_id,
        });

        // Determine which provider to use
        let provider_name = task_node
            .assigned_provider
            .map(|id| id.to_string())
            .unwrap_or_else(|| self.config.default_provider.clone());

        // Spawn the agent
        match self
            .agent_pool
            .spawn_agent(&task_node, &provider_name, self.agent_ctx.clone())
            .await
        {
            Ok(rx) => {
                // Mark as Running
                {
                    let mut graphs = self.graphs.write().await;
                    if let Some(graph) = graphs.get_mut(&graph_id) {
                        if let Some(node) = graph.nodes.get_mut(&task_id) {
                            node.status = TaskStatus::Running;
                        }
                    }
                }

                // Store the receiver for polling
                {
                    let mut receivers = self.agent_receivers.write().await;
                    receivers.insert((graph_id, task_id), rx);
                }

                let _ = self.event_tx.send(RuntimeEvent::TaskStarted {
                    graph_id,
                    task_id,
                });

                self.persist_event(&TaskGraphEvent::TaskStarted {
                    graph_id,
                    task_id,
                    timestamp: chrono::Utc::now(),
                })
                .ok();

                tracing::info!(?graph_id, ?task_id, "task started");
            }
            Err(e) => {
                // Mark as Failed
                let error = e.to_string();
                {
                    let mut graphs = self.graphs.write().await;
                    if let Some(graph) = graphs.get_mut(&graph_id) {
                        if let Some(node) = graph.nodes.get_mut(&task_id) {
                            node.status = TaskStatus::Failed {
                                error: error.clone(),
                                retries: 0,
                            };
                        }
                    }
                }

                let _ = self.event_tx.send(RuntimeEvent::TaskFailed {
                    graph_id,
                    task_id,
                    error: error.clone(),
                });

                self.persist_event(&TaskGraphEvent::TaskFailed {
                    graph_id,
                    task_id,
                    error,
                    timestamp: chrono::Utc::now(),
                })
                .ok();

                tracing::error!(?graph_id, ?task_id, err = %e, "failed to spawn agent");
            }
        }
    }

    /// Poll all active agent message receivers for a graph and handle messages.
    async fn poll_agent_messages(&self, graph_id: TaskGraphId) {
        // Collect keys for this graph
        let keys: Vec<(TaskGraphId, TaskId)> = {
            let receivers = self.agent_receivers.read().await;
            receivers
                .keys()
                .filter(|(gid, _)| *gid == graph_id)
                .copied()
                .collect()
        };

        let mut completed_keys = Vec::new();

        for key in keys {
            let (_, task_id) = key;

            // Try to receive messages without blocking
            let messages: Vec<AgentMessage> = {
                let mut receivers = self.agent_receivers.write().await;
                if let Some(rx) = receivers.get_mut(&key) {
                    let mut msgs = Vec::new();
                    while let Ok(msg) = rx.try_recv() {
                        msgs.push(msg);
                    }
                    msgs
                } else {
                    continue;
                }
            };

            for msg in messages {
                match &msg {
                    AgentMessage::TaskComplete { task_id, result, .. } => {
                        let task_id = *task_id;
                        {
                            let mut graphs = self.graphs.write().await;
                            if let Some(graph) = graphs.get_mut(&graph_id) {
                                if let Some(node) = graph.nodes.get_mut(&task_id) {
                                    node.status = TaskStatus::Complete;
                                    node.result = Some(result.clone());
                                }
                                // Update ready states for dependent tasks
                                Scheduler::update_ready_states(graph);
                            }
                        }

                        let _ = self.event_tx.send(RuntimeEvent::TaskCompleted {
                            graph_id,
                            task_id,
                        });

                        self.persist_event(&TaskGraphEvent::TaskCompleted {
                            graph_id,
                            task_id,
                            result: result.clone(),
                            timestamp: chrono::Utc::now(),
                        })
                        .ok();

                        completed_keys.push(key);
                        tracing::info!(?graph_id, ?task_id, "task completed");
                    }
                    AgentMessage::TaskFailed {
                        task_id, error, ..
                    } => {
                        let task_id = *task_id;
                        let error = error.clone();
                        {
                            let mut graphs = self.graphs.write().await;
                            if let Some(graph) = graphs.get_mut(&graph_id) {
                                if let Some(node) = graph.nodes.get_mut(&task_id) {
                                    node.status = TaskStatus::Failed {
                                        error: error.clone(),
                                        retries: 0,
                                    };
                                }
                            }
                        }

                        let _ = self.event_tx.send(RuntimeEvent::TaskFailed {
                            graph_id,
                            task_id,
                            error: error.clone(),
                        });

                        self.persist_event(&TaskGraphEvent::TaskFailed {
                            graph_id,
                            task_id,
                            error,
                            timestamp: chrono::Utc::now(),
                        })
                        .ok();

                        completed_keys.push(key);
                        tracing::info!(?graph_id, ?task_id, "task failed");
                    }
                    AgentMessage::SubTaskProposal {
                        task_id: parent_tid,
                        sub_tasks,
                        edges: new_edges,
                    } => {
                        let parent_tid = *parent_tid;
                        // Get parent depth for limit check
                        let parent_depth = {
                            let graphs = self.graphs.read().await;
                            graphs
                                .get(&graph_id)
                                .and_then(|g| g.nodes.get(&parent_tid))
                                .map(|n| n.depth)
                                .unwrap_or(0)
                        };

                        const MAX_DEPTH: u32 = 5;
                        if parent_depth >= MAX_DEPTH {
                            tracing::warn!(
                                ?graph_id, ?parent_tid, parent_depth,
                                "SubTaskProposal rejected: max depth reached"
                            );
                        } else {
                            // Add child nodes and edges to the graph
                            let child_count = sub_tasks.len();
                            {
                                let mut graphs = self.graphs.write().await;
                                if let Some(graph) = graphs.get_mut(&graph_id) {
                                    for mut child in sub_tasks.clone() {
                                        child.parent_task_id = Some(parent_tid);
                                        child.depth = parent_depth + 1;
                                        child.status = TaskStatus::Pending;
                                        graph.nodes.insert(child.id, child);
                                    }
                                    for edge in new_edges {
                                        graph.edges.push(*edge);
                                    }
                                    // Update ready states so new tasks get scheduled
                                    Scheduler::update_ready_states(graph);
                                }
                            }

                            tracing::info!(
                                ?graph_id, ?parent_tid,
                                children = child_count,
                                depth = parent_depth + 1,
                                "agent spawned child tasks"
                            );

                            // Notify UI about new children — the scheduling loop will
                            // emit TaskScheduled/TaskStarted as they get picked up
                            for child in sub_tasks {
                                let _ = self.event_tx.send(RuntimeEvent::TaskScheduled {
                                    graph_id,
                                    task_id: child.id,
                                });
                            }
                        }
                    }
                    _ => {
                        // Forward all other messages (StatusUpdate, ToolRequest, etc.)
                        // Tool execution is handled inside the agent loop.
                        let _ = self.event_tx.send(RuntimeEvent::AgentMessage {
                            graph_id,
                            task_id,
                            message: msg,
                        });
                    }
                }
            }
        }

        // Remove completed receivers
        if !completed_keys.is_empty() {
            let mut receivers = self.agent_receivers.write().await;
            for key in completed_keys {
                receivers.remove(&key);
            }
        }
    }

    /// Mark a graph as completed, persist the event, and evict from active set.
    /// Emits the final graph snapshot BEFORE evicting so the event bridge can
    /// send the "complete" status to the UI (avoids race condition).
    async fn complete_graph(&self, graph_id: TaskGraphId) {
        // Emit GraphCompleted first — event bridge will fetch graph while it still exists
        let _ = self.event_tx.send(RuntimeEvent::GraphCompleted { graph_id });

        self.persist_event(&TaskGraphEvent::GraphCompleted {
            graph_id,
            timestamp: chrono::Utc::now(),
        })
        .ok();

        // Small yield to let the event bridge process GraphCompleted before eviction
        tokio::task::yield_now().await;

        // Evict from active graphs so we don't re-process
        {
            let mut graphs = self.graphs.write().await;
            graphs.remove(&graph_id);
        }

        tracing::info!(?graph_id, "task graph completed and evicted");
    }

    /// Persist a TaskGraphEvent to the event store.
    fn persist_event(&self, event: &TaskGraphEvent) -> Result<(), RuntimeError> {
        let stream_id = event.stream_id();
        let event_type = format!("{:?}", std::mem::discriminant(event));
        let payload =
            serde_json::to_value(event).map_err(|e| RuntimeError::Persistence(e.to_string()))?;

        self.event_store
            .append_events(&stream_id, &[(event_type, payload)])
            .map_err(|e| RuntimeError::Persistence(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eagent_protocol::ids::{TaskGraphId, TaskId};
    use eagent_protocol::messages::TaskConstraints;
    use eagent_protocol::task_graph::{TaskGraph, TaskNode, TaskStatus};
    use eagent_protocol::traits::AgentContext;

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
            parent_task_id: None,
            depth: 0,
        }
    }

    fn make_test_engine() -> (RuntimeEngine, mpsc::UnboundedReceiver<RuntimeEvent>) {
        let providers = Arc::new(ProviderRegistry::new());
        let tools = Arc::new(ToolRegistry::new());
        let event_store = Arc::new(EventStore::in_memory().unwrap());
        let config = RuntimeConfig::default();
        let ctx = AgentContext {
            workspace_root: "/tmp/test".into(),
            project_name: Some("test-project".into()),
            project_summary: None,
        };

        RuntimeEngine::new(providers, tools, event_store, config, ctx)
    }

    #[tokio::test]
    async fn submit_valid_graph() {
        let (engine, mut event_rx) = make_test_engine();

        let task_id = TaskId::new();
        let graph = TaskGraph {
            id: TaskGraphId::new(),
            root_task_id: task_id,
            user_prompt: "test prompt".into(),
            nodes: {
                let mut m = HashMap::new();
                m.insert(task_id, make_node(task_id, TaskStatus::Pending));
                m
            },
            edges: vec![],
        };

        let result = engine.submit(graph).await;
        assert!(result.is_ok());

        // Should have received a GraphCreated event
        let event = event_rx.try_recv().unwrap();
        assert!(matches!(event, RuntimeEvent::GraphCreated { .. }));
    }

    #[tokio::test]
    async fn submit_invalid_graph_rejected() {
        let (engine, _event_rx) = make_test_engine();

        let a = TaskId::new();
        let b = TaskId::new();
        let c = TaskId::new();
        let graph = TaskGraph {
            id: TaskGraphId::new(),
            root_task_id: a,
            user_prompt: "cyclic".into(),
            nodes: {
                let mut m = HashMap::new();
                m.insert(a, make_node(a, TaskStatus::Pending));
                m.insert(b, make_node(b, TaskStatus::Pending));
                m.insert(c, make_node(c, TaskStatus::Pending));
                m
            },
            edges: vec![(a, b), (b, c), (c, a)],
        };

        let result = engine.submit(graph).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn submit_updates_ready_states() {
        let (engine, _event_rx) = make_test_engine();

        let task_id = TaskId::new();
        let graph_id = TaskGraphId::new();
        let graph = TaskGraph {
            id: graph_id,
            root_task_id: task_id,
            user_prompt: "single task".into(),
            nodes: {
                let mut m = HashMap::new();
                m.insert(task_id, make_node(task_id, TaskStatus::Pending));
                m
            },
            edges: vec![],
        };

        engine.submit(graph).await.unwrap();

        // The single task with no dependencies should now be Ready
        let retrieved = engine.get_graph(graph_id).await.unwrap();
        assert_eq!(retrieved.nodes[&task_id].status, TaskStatus::Ready);
    }

    #[tokio::test]
    async fn cancel_graph_marks_tasks_cancelled() {
        let (engine, _event_rx) = make_test_engine();

        let task_id = TaskId::new();
        let graph_id = TaskGraphId::new();
        let graph = TaskGraph {
            id: graph_id,
            root_task_id: task_id,
            user_prompt: "cancel me".into(),
            nodes: {
                let mut m = HashMap::new();
                m.insert(task_id, make_node(task_id, TaskStatus::Ready));
                m
            },
            edges: vec![],
        };

        // Insert directly (bypass submit to avoid event emission complications)
        {
            let mut graphs = engine.graphs.write().await;
            graphs.insert(graph_id, graph);
        }

        engine.cancel_graph(graph_id).await.unwrap();

        let retrieved = engine.get_graph(graph_id).await.unwrap();
        assert!(matches!(
            retrieved.nodes[&task_id].status,
            TaskStatus::Cancelled { .. }
        ));
    }

    #[tokio::test]
    async fn get_graph_returns_none_for_unknown() {
        let (engine, _event_rx) = make_test_engine();
        let result = engine.get_graph(TaskGraphId::new()).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn cancel_unknown_graph_returns_error() {
        let (engine, _event_rx) = make_test_engine();
        let result = engine.cancel_graph(TaskGraphId::new()).await;
        assert!(result.is_err());
    }
}

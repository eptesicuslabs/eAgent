import { create } from "zustand";
import type {
  AgentConfig,
  AgentMode,
  AgentTracePayload,
  AppBootstrap,
  FileMutation,
  FileMutationPayload,
  OrchestrationSnapshot,
  OversightRequest,
  OversightRequestPayload,
  ProjectState,
  ProviderStatus,
  ProviderStatusPayload,
  TaskGraphState,
  TaskGraphUpdatePayload,
  TerminalEventPayload,
  TerminalState,
  ThreadState,
} from "./types";
import { sortByUpdatedAtDescending } from "./lib/utils";

// =============================================================================
// eAgent Store — multi-agent-aware state management
// =============================================================================

interface EAgentStore {
  // --- Mode ---
  mode: AgentMode;
  setMode: (mode: AgentMode) => void;

  // --- Active TaskGraphs ---
  activeGraphs: Map<string, TaskGraphState>;
  selectedGraphId: string | null;
  selectedTaskId: string | null;
  selectGraph: (id: string | null) => void;
  selectTask: (id: string | null) => void;

  // --- Provider state ---
  providers: Map<string, ProviderStatus>;

  // --- Terminals ---
  terminals: Map<string, TerminalState>;

  // --- Settings ---
  config: AgentConfig | null;

  // --- Actions for Tauri events ---
  onTaskGraphUpdate: (payload: TaskGraphUpdatePayload) => void;
  onAgentTrace: (payload: AgentTracePayload) => void;
  onFileMutation: (payload: FileMutationPayload) => void;
  onOversightRequest: (payload: OversightRequestPayload) => void;
  onTerminalEvent: (payload: TerminalEventPayload) => void;
  onProviderStatus: (payload: ProviderStatusPayload) => void;

  // --- Legacy compatibility ---
  bootstrap: AppBootstrap | null;
  snapshot: OrchestrationSnapshot | null;
  selectedThreadId: string | null;
  expandedProjectPaths: Record<string, boolean>;
  syncBootstrap: (bootstrap: AppBootstrap) => void;
  syncSnapshot: (snapshot: OrchestrationSnapshot) => void;
  setSelectedThreadId: (threadId: string | null) => void;
  toggleProject: (path: string) => void;
}

export const useStore = create<EAgentStore>((set, get) => ({
  // --- Mode ---
  mode: "ecode",
  setMode: (mode) => set({ mode }),

  // --- Active TaskGraphs ---
  activeGraphs: new Map(),
  selectedGraphId: null,
  selectedTaskId: null,
  selectGraph: (id) => set({ selectedGraphId: id, selectedTaskId: null }),
  selectTask: (id) => set({ selectedTaskId: id }),

  // --- Provider state ---
  providers: new Map(),

  // --- Terminals ---
  terminals: new Map(),

  // --- Settings ---
  config: null,

  // --- Tauri event handlers (incremental updates) ---

  onTaskGraphUpdate: (payload) =>
    set((state) => {
      const next = new Map(state.activeGraphs);
      const existing = next.get(payload.graphId);

      if (existing) {
        // Incremental update: merge nodes and edges, preserve traces/diffs/oversight
        next.set(payload.graphId, {
          ...existing,
          nodes: { ...existing.nodes, ...payload.nodes },
          edges: payload.edges,
          status: payload.status,
          updatedAt: new Date().toISOString(),
        });
      } else {
        // New graph
        next.set(payload.graphId, {
          graphId: payload.graphId,
          rootTaskId: payload.rootTaskId,
          userPrompt: payload.userPrompt,
          nodes: payload.nodes,
          edges: payload.edges,
          traces: {},
          diffs: {},
          oversightRequests: {},
          status: payload.status,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        });
      }

      return {
        activeGraphs: next,
        // Auto-select the first graph if none selected
        selectedGraphId: state.selectedGraphId ?? payload.graphId,
      };
    }),

  onAgentTrace: (payload) =>
    set((state) => {
      const next = new Map(state.activeGraphs);
      const graph = next.get(payload.graphId);
      if (!graph) return state;

      const taskTraces = graph.traces[payload.taskId] ?? [];
      next.set(payload.graphId, {
        ...graph,
        traces: {
          ...graph.traces,
          [payload.taskId]: [...taskTraces, payload.entry],
        },
        updatedAt: new Date().toISOString(),
      });

      return { activeGraphs: next };
    }),

  onFileMutation: (payload) =>
    set((state) => {
      const next = new Map(state.activeGraphs);
      const graph = next.get(payload.graphId);
      if (!graph) return state;

      const mutation: FileMutation = {
        taskId: payload.taskId,
        agentId: payload.agentId,
        path: payload.path,
        diff: payload.diff,
        timestamp: new Date().toISOString(),
      };

      const taskDiffs = graph.diffs[payload.taskId] ?? [];
      next.set(payload.graphId, {
        ...graph,
        diffs: {
          ...graph.diffs,
          [payload.taskId]: [...taskDiffs, mutation],
        },
        updatedAt: new Date().toISOString(),
      });

      return { activeGraphs: next };
    }),

  onOversightRequest: (payload) =>
    set((state) => {
      const next = new Map(state.activeGraphs);
      const graph = next.get(payload.graphId);
      if (!graph) return state;

      const request: OversightRequest = {
        requestId: payload.requestId,
        graphId: payload.graphId,
        taskId: payload.taskId,
        action: payload.action,
        context: payload.context,
        riskLevel: payload.riskLevel,
        timestamp: new Date().toISOString(),
      };

      next.set(payload.graphId, {
        ...graph,
        oversightRequests: {
          ...graph.oversightRequests,
          [payload.requestId]: request,
        },
        updatedAt: new Date().toISOString(),
      });

      return { activeGraphs: next };
    }),

  onTerminalEvent: (payload) =>
    set((state) => {
      const next = new Map(state.terminals);
      const existing = next.get(payload.terminalId);

      next.set(payload.terminalId, {
        id: payload.terminalId,
        agentId: payload.agentId ?? existing?.agentId ?? null,
        graphId: payload.graphId ?? existing?.graphId ?? null,
        taskId: payload.taskId ?? existing?.taskId ?? null,
        title: payload.title ?? existing?.title ?? "Terminal",
        buffer: (existing?.buffer ?? "") + payload.data,
        isAlive: payload.isAlive,
      });

      return { terminals: next };
    }),

  onProviderStatus: (payload) =>
    set((state) => {
      const next = new Map(state.providers);
      next.set(payload.providerId, {
        providerId: payload.providerId,
        kind: payload.kind,
        displayName: payload.displayName,
        status: payload.status,
        models: payload.models,
        error: payload.error,
        maxConcurrentSessions: payload.maxConcurrentSessions,
        activeSessions: payload.activeSessions,
      });
      return { providers: next };
    }),

  // --- Legacy compatibility (preserved for existing components) ---
  bootstrap: null,
  snapshot: null,
  selectedThreadId: null,
  expandedProjectPaths: {},
  syncBootstrap: (bootstrap) =>
    set((state) => ({
      bootstrap,
      selectedThreadId: state.selectedThreadId ?? bootstrap.currentThreadId,
    })),
  syncSnapshot: (snapshot) =>
    set((state) => ({
      snapshot,
      selectedThreadId: state.selectedThreadId ?? snapshot.currentThreadId,
    })),
  setSelectedThreadId: (threadId) => set({ selectedThreadId: threadId }),
  toggleProject: (path) =>
    set((state) => ({
      expandedProjectPaths: {
        ...state.expandedProjectPaths,
        [path]: !state.expandedProjectPaths[path],
      },
    })),
}));

// =============================================================================
// Selectors for eAgent data
// =============================================================================

export function selectActiveGraph(): TaskGraphState | null {
  const { activeGraphs, selectedGraphId } = useStore.getState();
  if (!selectedGraphId) return null;
  return activeGraphs.get(selectedGraphId) ?? null;
}

export function selectGraphList(): TaskGraphState[] {
  const { activeGraphs } = useStore.getState();
  return Array.from(activeGraphs.values()).sort(
    (a, b) => b.updatedAt.localeCompare(a.updatedAt),
  );
}

export function selectTracesForTask(graphId: string, taskId: string) {
  const { activeGraphs } = useStore.getState();
  const graph = activeGraphs.get(graphId);
  if (!graph) return [];
  return graph.traces[taskId] ?? [];
}

export function selectDiffsForTask(graphId: string, taskId: string) {
  const { activeGraphs } = useStore.getState();
  const graph = activeGraphs.get(graphId);
  if (!graph) return [];
  return graph.diffs[taskId] ?? [];
}

export function selectOversightRequests(graphId: string) {
  const { activeGraphs } = useStore.getState();
  const graph = activeGraphs.get(graphId);
  if (!graph) return [];
  return Object.values(graph.oversightRequests);
}

export function selectProviderList(): ProviderStatus[] {
  const { providers } = useStore.getState();
  return Array.from(providers.values());
}

export function selectTerminalList(): TerminalState[] {
  const { terminals } = useStore.getState();
  return Array.from(terminals.values());
}

// =============================================================================
// Legacy selectors (preserved for existing components)
// =============================================================================

export function selectProjects(snapshot: OrchestrationSnapshot | null) {
  return snapshot ? Object.values(snapshot.readModel.projects) : [];
}

export function selectThreads(snapshot: OrchestrationSnapshot | null) {
  if (!snapshot) return [];
  return sortByUpdatedAtDescending(Object.values(snapshot.readModel.threads));
}

export function selectProjectByPath(snapshot: OrchestrationSnapshot | null, path: string | null) {
  if (!snapshot || !path) return null;
  return (
    Object.values(snapshot.readModel.projects).find((project) => project.path === path) ?? null
  );
}

export function selectThreadsForProject(
  snapshot: OrchestrationSnapshot | null,
  project: ProjectState,
) {
  if (!snapshot) return [];
  return project.thread_ids
    .map((threadId) => snapshot.readModel.threads[threadId])
    .filter((thread): thread is ThreadState => Boolean(thread))
    .sort((left, right) => right.updated_at.localeCompare(left.updated_at));
}

export function selectActiveThread() {
  const { snapshot, selectedThreadId } = useStore.getState();
  if (!snapshot) return null;
  const preferredThreadId = selectedThreadId ?? snapshot.currentThreadId;
  return preferredThreadId ? snapshot.readModel.threads[preferredThreadId] ?? null : null;
}

export function getExpandedProject(path: string) {
  return useStore.getState().expandedProjectPaths[path] ?? true;
}

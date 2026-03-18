import { create } from "zustand";
import type { AppBootstrap, OrchestrationSnapshot, ProjectState, ThreadState } from "./types";
import { sortByUpdatedAtDescending } from "./lib/utils";

interface ShellStore {
  bootstrap: AppBootstrap | null;
  snapshot: OrchestrationSnapshot | null;
  selectedThreadId: string | null;
  expandedProjectPaths: Record<string, boolean>;
  syncBootstrap: (bootstrap: AppBootstrap) => void;
  syncSnapshot: (snapshot: OrchestrationSnapshot) => void;
  setSelectedThreadId: (threadId: string | null) => void;
  toggleProject: (path: string) => void;
}

export const useStore = create<ShellStore>((set, get) => ({
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

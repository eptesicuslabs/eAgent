import type { AppBootstrap, OrchestrationSnapshot, ProjectState, ThreadState } from "~/types";

export interface SidebarProjectGroup {
  project: ProjectState;
  threads: ThreadState[];
  isCurrent: boolean;
}

function rankThread(left: ThreadState, right: ThreadState) {
  return right.updated_at.localeCompare(left.updated_at);
}

export function deriveSidebarProjectGroups(
  snapshot: OrchestrationSnapshot | null,
  bootstrap: AppBootstrap | null,
) {
  if (!snapshot) return [];

  const projects = Object.values(snapshot.readModel.projects).sort((left, right) =>
    left.name.localeCompare(right.name),
  );

  return projects.map<SidebarProjectGroup>((project) => ({
    project,
    threads: project.thread_ids
      .map((threadId) => snapshot.readModel.threads[threadId])
      .filter((thread): thread is ThreadState => Boolean(thread) && !thread.deleted)
      .sort(rankThread),
    isCurrent: project.path === bootstrap?.currentProject,
  }));
}

export function sessionBadgeTone(thread: ThreadState) {
  switch (thread.session?.status) {
    case "running":
      return "text-emerald-300";
    case "waiting":
      return "text-amber-300";
    case "error":
      return "text-rose-300";
    default:
      return "text-muted-foreground";
  }
}

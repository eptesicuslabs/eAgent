import { useQuery } from "@tanstack/react-query";
import { readNativeApi } from "~/nativeApi";
import type { ProjectState, ThreadState } from "~/types";
import { DiffPanel } from "./DiffPanel";

export function DiffPanelShell(props: {
  project: ProjectState | null;
  thread: ThreadState;
  changedFiles: string[];
}) {
  const gitQuery = useQuery({
    queryKey: ["git", "status", props.project?.path],
    queryFn: async () => {
      const api = readNativeApi();
      if (!api || !props.project) {
        return null;
      }
      return api.git.diffWorkdir(props.project.path);
    },
    enabled: Boolean(props.project?.path),
  });

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden border-t border-border/60 bg-[linear-gradient(180deg,color-mix(in_srgb,var(--card)_92%,transparent)_0%,color-mix(in_srgb,var(--background)_96%,black)_100%)] px-4 py-4">
      <div className="mb-3 flex items-center justify-between gap-3">
        <div>
          <p className="text-[11px] font-semibold tracking-[0.24em] text-muted-foreground uppercase">
            Diff
          </p>
          <h2 className="text-sm font-medium text-foreground">
            {props.project?.name ?? "Workspace"} changes
          </h2>
        </div>
        <span className="rounded-full border border-border/70 bg-background/70 px-2.5 py-1 text-[10px] font-semibold tracking-[0.18em] text-muted-foreground uppercase">
          {props.thread.settings.runtime_mode}
        </span>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        <DiffPanel changedFiles={props.changedFiles} gitStatus={gitQuery.data ?? null} />
      </div>
    </div>
  );
}

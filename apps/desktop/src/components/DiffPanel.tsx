import type { GitStatusPayload } from "~/types";

export function DiffPanel(props: { gitStatus: GitStatusPayload | null; changedFiles: string[] }) {
  if (!props.gitStatus || !props.gitStatus.isGitRepo) {
    return (
      <div className="rounded-[1.4rem] border border-border/70 bg-card/50 p-4 text-sm text-muted-foreground">
        This workspace is not a git repository.
      </div>
    );
  }

  return (
    <div className="grid gap-3">
      <div className="rounded-[1.4rem] border border-border/70 bg-card/55 p-4">
        <p className="text-[11px] font-semibold tracking-[0.24em] text-muted-foreground uppercase">
          Worktree status
        </p>
        <div className="mt-3 grid gap-2">
          {props.gitStatus.statuses.length === 0 ? (
            <p className="text-sm text-muted-foreground">No unstaged or staged changes.</p>
          ) : (
            props.gitStatus.statuses.slice(0, 12).map((status) => (
              <div
                key={`${status.status}:${status.path}`}
                className="flex items-center justify-between gap-3 rounded-2xl border border-border/60 bg-background/75 px-3 py-2 text-sm"
              >
                <span className="truncate text-foreground">{status.path}</span>
                <span className="rounded-full bg-muted/60 px-2 py-1 text-[10px] font-semibold tracking-[0.18em] text-muted-foreground uppercase">
                  {status.status}
                </span>
              </div>
            ))
          )}
        </div>
      </div>

      <div className="rounded-[1.4rem] border border-border/70 bg-card/55 p-4">
        <p className="text-[11px] font-semibold tracking-[0.24em] text-muted-foreground uppercase">
          Runtime file mentions
        </p>
        <div className="mt-3 grid gap-2">
          {props.changedFiles.length === 0 ? (
            <p className="text-sm text-muted-foreground">No file paths surfaced in runtime events.</p>
          ) : (
            props.changedFiles.slice(0, 10).map((path) => (
              <div
                key={path}
                className="rounded-2xl border border-border/60 bg-background/75 px-3 py-2 text-sm text-foreground"
              >
                {path}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

import { Link, useNavigate, useRouterState } from "@tanstack/react-router";
import {
  ChevronDown,
  ChevronRight,
  FolderPlus,
  MessageSquarePlus,
  Settings,
} from "lucide-react";
import { readNativeApi } from "~/nativeApi";
import { getExpandedProject, useStore } from "~/store";
import { formatRelativeTime, titleFromPath } from "~/lib/utils";
import { deriveSidebarProjectGroups, sessionBadgeTone } from "./Sidebar.logic";

export function Sidebar() {
  const navigate = useNavigate();
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const bootstrap = useStore((state) => state.bootstrap);
  const snapshot = useStore((state) => state.snapshot);
  const toggleProject = useStore((state) => state.toggleProject);

  const groups = deriveSidebarProjectGroups(snapshot, bootstrap);

  return (
    <aside className="grid min-h-0 grid-rows-[auto_auto_1fr_auto] border-r border-border/70 bg-[linear-gradient(180deg,color-mix(in_srgb,var(--card)_94%,black)_0%,color-mix(in_srgb,var(--background)_97%,black)_100%)] px-3 py-3">
      <div className="rounded-[1.4rem] border border-border/70 bg-card/70 px-4 py-4">
        <div className="flex items-center justify-between gap-3">
          <div>
            <p className="text-[11px] font-semibold tracking-[0.26em] text-muted-foreground uppercase">
              eCode
            </p>
            <h1 className="mt-1 text-lg font-semibold tracking-[-0.03em] text-foreground">
              Desktop shell
            </h1>
          </div>
          <span className="rounded-full border border-border/70 bg-background/70 px-2.5 py-1 text-[10px] font-semibold tracking-[0.18em] text-muted-foreground uppercase">
            T3
          </span>
        </div>
        <p className="mt-3 text-xs leading-5 text-muted-foreground">
          {bootstrap?.statusMessage ?? "Runtime idle"}
        </p>
        <button
          className="mt-4 inline-flex w-full items-center justify-center gap-2 rounded-2xl border border-border/70 bg-foreground px-4 py-3 text-sm font-medium text-background transition hover:opacity-90"
          onClick={async () => {
            const api = readNativeApi();
            if (!api) return;
            await api.orchestration.dispatch({
              type: "createThread",
              name: `Thread ${new Intl.DateTimeFormat(undefined, {
                hour: "2-digit",
                minute: "2-digit",
              }).format(new Date())}`,
            });
            const snapshot = await api.orchestration.getSnapshot();
            useStore.getState().syncSnapshot(snapshot);
            if (snapshot.currentThreadId) {
              await navigate({
                to: "/threads/$threadId",
                params: { threadId: snapshot.currentThreadId },
              });
            }
          }}
          type="button"
        >
          <MessageSquarePlus className="size-4" />
          New thread
        </button>
      </div>

      <div className="mt-3 rounded-[1.4rem] border border-border/70 bg-card/50 px-4 py-4">
        <div className="flex items-center justify-between gap-3">
          <div>
            <p className="text-[11px] font-semibold tracking-[0.24em] text-muted-foreground uppercase">
              Current project
            </p>
            <p className="mt-2 truncate text-sm font-medium text-foreground">
              {bootstrap?.currentProject ? titleFromPath(bootstrap.currentProject) : "No workspace"}
            </p>
          </div>
          <button
            className="inline-flex size-9 items-center justify-center rounded-2xl border border-border/70 bg-background/70 text-muted-foreground transition hover:text-foreground"
            onClick={async () => {
              const api = readNativeApi();
              if (!api) return;
              const path = await api.app.pickFolder();
              if (!path) return;
              await api.projects.open(path);
            }}
            type="button"
          >
            <FolderPlus className="size-4" />
          </button>
        </div>
      </div>

      <div className="mt-3 min-h-0 overflow-hidden rounded-[1.6rem] border border-border/70 bg-card/45">
        <div className="border-b border-border/60 px-4 py-3">
          <p className="text-[11px] font-semibold tracking-[0.24em] text-muted-foreground uppercase">
            Threads
          </p>
        </div>
        <div className="h-full overflow-auto px-2 py-2">
          <div className="grid gap-2">
            {groups.length === 0 ? (
              <div className="rounded-[1.2rem] border border-dashed border-border/70 px-4 py-6 text-sm text-muted-foreground">
                No projects opened yet.
              </div>
            ) : null}

            {groups.map((group) => {
              const expanded = getExpandedProject(group.project.path);
              return (
                <section key={group.project.id} className="rounded-[1.2rem] px-2 py-2">
                  <button
                    className="flex w-full items-center gap-2 rounded-2xl px-2 py-2 text-left transition hover:bg-background/60"
                    onClick={() => toggleProject(group.project.path)}
                    type="button"
                  >
                    {expanded ? (
                      <ChevronDown className="size-4 text-muted-foreground" />
                    ) : (
                      <ChevronRight className="size-4 text-muted-foreground" />
                    )}
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm font-medium text-foreground">
                        {group.project.name}
                      </p>
                      <p className="truncate text-[11px] text-muted-foreground">
                        {group.project.path}
                      </p>
                    </div>
                    {group.isCurrent ? (
                      <span className="rounded-full bg-emerald-500/12 px-2 py-1 text-[10px] font-semibold tracking-[0.18em] text-emerald-300 uppercase">
                        Live
                      </span>
                    ) : null}
                  </button>

                  {expanded ? (
                    <div className="mt-1 grid gap-1 pl-7">
                      {group.threads.length === 0 ? (
                        <div className="rounded-xl border border-dashed border-border/60 px-3 py-3 text-xs text-muted-foreground">
                          No threads in this project.
                        </div>
                      ) : null}

                      {group.threads.map((thread) => {
                        const isActive = pathname === `/threads/${thread.id}`;
                        return (
                          <Link
                            key={thread.id}
                            className={`rounded-[1rem] border px-3 py-3 transition ${
                              isActive
                                ? "border-border bg-background/85 shadow-[0_12px_40px_rgba(0,0,0,0.18)]"
                                : "border-transparent hover:border-border/60 hover:bg-background/60"
                            }`}
                            params={{ threadId: thread.id }}
                            to="/threads/$threadId"
                          >
                            <div className="flex items-start justify-between gap-3">
                              <div className="min-w-0">
                                <p className="truncate text-sm font-medium text-foreground">
                                  {thread.name}
                                </p>
                                <p className="mt-1 line-clamp-2 text-xs leading-5 text-muted-foreground">
                                  {thread.turns.at(-1)?.input || "No prompt yet"}
                                </p>
                              </div>
                              <span
                                className={`mt-0.5 text-[10px] font-semibold tracking-[0.18em] uppercase ${sessionBadgeTone(thread)}`}
                              >
                                {thread.session?.status ?? "idle"}
                              </span>
                            </div>
                            <div className="mt-3 flex items-center justify-between gap-3 text-[11px] text-muted-foreground">
                              <span>{thread.settings.model}</span>
                              <span>{formatRelativeTime(thread.updated_at)}</span>
                            </div>
                          </Link>
                        );
                      })}
                    </div>
                  ) : null}
                </section>
              );
            })}
          </div>
        </div>
      </div>

      <div className="mt-3">
        <Link
          className={`flex items-center justify-between rounded-[1.2rem] border px-4 py-3 transition ${
            pathname === "/settings"
              ? "border-border bg-card/70"
              : "border-border/70 bg-card/45 hover:bg-card/70"
          }`}
          to="/settings"
        >
          <div>
            <p className="text-sm font-medium text-foreground">Settings</p>
            <p className="text-xs text-muted-foreground">
              Runtime defaults and model paths
            </p>
          </div>
          <Settings className="size-4 text-muted-foreground" />
        </Link>
      </div>
    </aside>
  );
}

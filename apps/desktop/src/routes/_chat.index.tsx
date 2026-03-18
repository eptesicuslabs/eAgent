import { createRoute, useNavigate } from "@tanstack/react-router";
import { useEffect } from "react";
import { readNativeApi } from "~/nativeApi";
import { useStore } from "~/store";
import { Composer } from "~/components/Composer";
import { Route as ChatRoute } from "./_chat";

export const Route = createRoute({
  getParentRoute: () => ChatRoute,
  path: "/",
  component: ChatIndexRoute,
});

function ChatIndexRoute() {
  const navigate = useNavigate();
  const bootstrap = useStore((state) => state.bootstrap);
  const snapshot = useStore((state) => state.snapshot);
  const mode = useStore((state) => state.mode);

  useEffect(() => {
    const threadId = snapshot?.currentThreadId ?? bootstrap?.currentThreadId;
    if (!threadId) return;
    void navigate({
      to: "/threads/$threadId",
      params: { threadId },
      replace: true,
    });
  }, [bootstrap?.currentThreadId, navigate, snapshot?.currentThreadId]);

  return (
    <section className="grid h-full min-w-0 grid-rows-[1fr_auto] overflow-hidden">
      {/* Main canvas: welcome message */}
      <div className="flex items-center justify-center overflow-auto">
        <div className="w-full max-w-3xl px-10">
          <div className="rounded-[2rem] border border-border/70 bg-card/80 p-10 shadow-[0_24px_80px_rgba(0,0,0,0.24)] backdrop-blur">
            <p className="text-xs font-semibold tracking-[0.24em] text-muted-foreground uppercase">
              {mode === "ecode" ? "eCode" : "eWork"} workstation
            </p>
            <h1 className="mt-4 text-4xl font-semibold tracking-[-0.04em] text-foreground">
              What would you like to build?
            </h1>
            <p className="mt-4 max-w-2xl text-sm leading-7 text-muted-foreground">
              Describe a task below. eAgent will plan, decompose, and execute it
              using parallel AI agents with full transparency and oversight.
            </p>
            <div className="mt-8 flex flex-wrap gap-3">
              <button
                className="inline-flex items-center rounded-full border border-border/70 bg-foreground px-4 py-2 text-sm font-medium text-background transition hover:opacity-90"
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
                }}
                type="button"
              >
                New thread (legacy)
              </button>
              <button
                className="inline-flex items-center rounded-full border border-border/70 bg-muted/40 px-4 py-2 text-sm font-medium text-foreground transition hover:bg-muted/70"
                onClick={async () => {
                  const api = readNativeApi();
                  if (!api) return;
                  const path = await api.app.pickFolder();
                  if (!path) return;
                  await api.projects.open(path);
                }}
                type="button"
              >
                Open project
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Composer at the bottom */}
      <Composer />
    </section>
  );
}

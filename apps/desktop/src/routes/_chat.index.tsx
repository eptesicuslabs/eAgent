import { createRoute } from "@tanstack/react-router";
import { useStore } from "~/store";
import { Composer } from "~/components/Composer";
import { AgentTraceView } from "~/components/AgentTraceView";
import { ProviderSetup } from "~/components/ProviderSetup";
import { Route as ChatRoute } from "./_chat";

export const Route = createRoute({
  getParentRoute: () => ChatRoute,
  path: "/",
  component: ChatIndexRoute,
});

function ChatIndexRoute() {
  const selectedGraphId = useStore((s) => s.selectedGraphId);
  const mode = useStore((s) => s.mode);
  const providers = useStore((s) => s.providers);

  const hasProvider = providers.size > 0;
  const hasGraph = selectedGraphId !== null;

  return (
    <section className="grid h-full min-w-0 grid-rows-[1fr_auto] overflow-hidden">
      {hasGraph ? (
        <AgentTraceView />
      ) : (
        <div className="flex items-center justify-center overflow-auto">
          <div className="w-full max-w-lg px-8">
            <p className="text-[10px] font-bold tracking-[0.2em] text-muted-foreground/50 uppercase">
              {mode === "ecode" ? "eCode" : "eWork"}
            </p>
            <h1 className="mt-2 text-2xl font-semibold tracking-tight text-foreground">
              What would you like to build?
            </h1>
            <p className="mt-2 text-[13px] leading-6 text-muted-foreground/70">
              Describe a task and eAgent will plan, decompose, and execute it
              with parallel AI agents.
            </p>

            {!hasProvider ? (
              <div className="mt-8 rounded-xl border border-border bg-card/60 p-5">
                <ProviderSetup />
              </div>
            ) : null}
          </div>
        </div>
      )}
      <Composer />
    </section>
  );
}

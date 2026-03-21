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
  const providers = useStore((s) => s.providers);
  const hasProvider = providers.size > 0;

  return (
    <section className="grid h-full grid-rows-[1fr_auto] overflow-hidden">
      {selectedGraphId ? (
        <AgentTraceView />
      ) : (
        <div className="flex items-center justify-center overflow-auto">
          <div className="max-w-md px-6 text-xs">
            <div className="text-foreground text-sm font-bold mb-1">eAgent</div>
            <div className="text-muted-foreground mb-6">
              describe a task. agents will plan and execute it.
            </div>
            {!hasProvider ? <ProviderSetup /> : (
              <div className="text-muted-foreground">provider connected. type a task below.</div>
            )}
          </div>
        </div>
      )}
      <Composer />
    </section>
  );
}

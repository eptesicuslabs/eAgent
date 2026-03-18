import {
  QueryClient,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import {
  Outlet,
  createRootRouteWithContext,
} from "@tanstack/react-router";
import { startTransition, useEffect, useRef } from "react";
import { readNativeApi } from "~/nativeApi";
import { useStore } from "~/store";
import { setupEAgentEventBridge } from "~/eventBridge";

export const bootstrapQueryKey = ["app", "bootstrap"] as const;
export const snapshotQueryKey = ["orchestration", "snapshot"] as const;

export const Route = createRootRouteWithContext<{
  queryClient: QueryClient;
}>()({
  component: RootRouteView,
});

function RootRouteView() {
  const api = readNativeApi();
  const queryClient = useQueryClient();
  const syncBootstrap = useStore((state) => state.syncBootstrap);
  const syncSnapshot = useStore((state) => state.syncSnapshot);
  const eventBridgeCleanup = useRef<(() => void) | null>(null);

  const bootstrapQuery = useQuery({
    queryKey: bootstrapQueryKey,
    queryFn: () => {
      if (!api) {
        throw new Error("Native bridge unavailable.");
      }
      return api.app.getBootstrap();
    },
    enabled: Boolean(api),
  });

  const snapshotQuery = useQuery({
    queryKey: snapshotQueryKey,
    queryFn: () => {
      if (!api) {
        throw new Error("Native bridge unavailable.");
      }
      return api.orchestration.getSnapshot();
    },
    enabled: Boolean(api),
  });

  useEffect(() => {
    document.body.classList.add("dark");
    return () => document.body.classList.remove("dark");
  }, []);

  useEffect(() => {
    if (!bootstrapQuery.data) return;
    startTransition(() => {
      syncBootstrap(bootstrapQuery.data);
    });
  }, [bootstrapQuery.data, syncBootstrap]);

  useEffect(() => {
    if (!snapshotQuery.data) return;
    startTransition(() => {
      syncSnapshot(snapshotQuery.data);
    });
  }, [snapshotQuery.data, syncSnapshot]);

  // Legacy event listeners (domain, terminal, settings, status)
  useEffect(() => {
    if (!api) return;

    let disposed = false;
    let cleanups: Array<() => void> = [];

    void (async () => {
      cleanups = await Promise.all([
        api.app.onDomainEvent(() => {
          void queryClient.invalidateQueries({ queryKey: snapshotQueryKey });
        }),
        api.app.onTerminalEvent(() => {
          void queryClient.invalidateQueries({ queryKey: snapshotQueryKey });
        }),
        api.app.onSettingsUpdated(() => {
          void queryClient.invalidateQueries({ queryKey: bootstrapQueryKey });
          void queryClient.invalidateQueries({ queryKey: snapshotQueryKey });
        }),
        api.app.onStatusChanged(() => {
          void queryClient.invalidateQueries({ queryKey: bootstrapQueryKey });
        }),
      ]);

      if (disposed) {
        for (const cleanup of cleanups) cleanup();
      }
    })();

    return () => {
      disposed = true;
      for (const cleanup of cleanups) cleanup();
    };
  }, [api, queryClient]);

  // eAgent event bridge (task-graph-update, agent-trace, etc.)
  useEffect(() => {
    let disposed = false;

    void (async () => {
      try {
        const cleanup = await setupEAgentEventBridge();
        if (disposed) {
          cleanup();
        } else {
          eventBridgeCleanup.current = cleanup;
        }
      } catch (error) {
        console.warn("[eAgent] Event bridge setup failed (backend may not be ready yet):", error);
      }
    })();

    return () => {
      disposed = true;
      if (eventBridgeCleanup.current) {
        eventBridgeCleanup.current();
        eventBridgeCleanup.current = null;
      }
    };
  }, []);

  // Fetch initial provider status
  useEffect(() => {
    if (!api) return;
    void (async () => {
      try {
        const providers = await api.eagent.getProviders();
        for (const provider of providers) {
          useStore.getState().onProviderStatus(provider);
        }
      } catch {
        // Backend not ready yet, providers will arrive via events
      }
    })();
  }, [api]);

  if (!api || bootstrapQuery.isLoading || snapshotQuery.isLoading) {
    return (
      <div className="flex h-screen items-center justify-center bg-background text-sm text-muted-foreground">
        Connecting to eAgent runtime...
      </div>
    );
  }

  if (bootstrapQuery.isError || snapshotQuery.isError) {
    const message =
      bootstrapQuery.error instanceof Error
        ? bootstrapQuery.error.message
        : snapshotQuery.error instanceof Error
          ? snapshotQuery.error.message
          : "Unknown error";

    return (
      <div className="flex h-screen items-center justify-center bg-background px-6">
        <div className="max-w-xl rounded-3xl border border-border/70 bg-card/90 p-6 shadow-2xl">
          <p className="text-xs font-semibold tracking-[0.24em] text-rose-400 uppercase">
            Shell bootstrap failed
          </p>
          <h1 className="mt-3 text-2xl font-semibold text-foreground">
            The desktop shell could not load runtime state.
          </h1>
          <p className="mt-3 text-sm leading-6 text-muted-foreground">{message}</p>
        </div>
      </div>
    );
  }

  return <Outlet />;
}

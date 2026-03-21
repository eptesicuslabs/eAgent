import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AppBootstrap,
  AppConfig,
  NativeApi,
  OrchestrationDispatchCommand,
  OrchestrationSnapshot,
  ProjectSearchEntry,
  ProviderStatus,
  TaskGraphState,
  TerminalRecord,
} from "./types";

async function subscribe(eventName: string, listener: () => void) {
  const unlisten = await listen(eventName, () => listener());
  return () => {
    unlisten();
  };
}

/**
 * Safely invoke a Tauri command. If the command doesn't exist yet
 * (backend not implemented), catches the error and returns the fallback.
 */
async function safeInvoke<T>(command: string, args?: Record<string, unknown>, fallback?: T): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error: unknown) {
    // If the backend command doesn't exist yet, return fallback gracefully
    const message = error instanceof Error ? error.message : String(error);
    if (
      message.includes("not found") ||
      message.includes("unknown command") ||
      message.includes("not registered") ||
      message.includes("plugin not found")
    ) {
      console.warn(`[eAgent] Tauri command "${command}" not available yet:`, message);
      return fallback as T;
    }
    throw error;
  }
}

export function createTauriNativeApi(): NativeApi {
  return {
    app: {
      getBootstrap: () => invoke<AppBootstrap>("app_get_bootstrap"),
      pickFolder: () => invoke<string | null>("app_pick_folder"),
      openExternal: (url) => invoke("app_open_external", { url }),
      onDomainEvent: (listener) => subscribe("ecode://domain-event", listener),
      onTerminalEvent: (listener) => subscribe("ecode://terminal-event", listener),
      onSettingsUpdated: (listener) => subscribe("ecode://settings-updated", listener),
      onStatusChanged: async (listener) => {
        const unlisten = await listen<{ statusMessage: string }>("ecode://app-status", (event) => {
          listener(event.payload.statusMessage);
        });
        return () => {
          unlisten();
        };
      },
    },
    shell: {
      openInEditor: (path) => invoke("shell_open_in_editor", { path }),
    },
    orchestration: {
      getSnapshot: () => invoke<OrchestrationSnapshot>("orchestration_get_snapshot"),
      dispatch: (command: OrchestrationDispatchCommand) =>
        invoke("orchestration_dispatch", { command }),
    },
    terminal: {
      list: () => invoke<TerminalRecord[]>("terminal_list"),
      open: () => invoke("terminal_open"),
      write: (terminalId, input) => invoke("terminal_write", { terminalId, input }),
      resize: (terminalId, cols, rows) =>
        invoke("terminal_resize", { terminalId, cols, rows }),
      close: (terminalId) => invoke("terminal_close", { terminalId }),
      clear: (terminalId) => invoke("terminal_clear", { terminalId }),
    },
    git: {
      status: (cwd) => invoke("git_status", { cwd }),
      listBranches: (cwd) => invoke("git_list_branches", { cwd }),
      diffWorkdir: (cwd) => invoke("git_diff_workdir", { cwd }),
      createWorktree: (input) => invoke("git_create_worktree", { input }),
      removeWorktree: (input) => invoke("git_remove_worktree", { input }),
    },
    projects: {
      open: (path) => invoke("projects_open", { path }),
      searchEntries: (cwd, query, limit) =>
        invoke<ProjectSearchEntry[]>("projects_search_entries", {
          input: { cwd, query, limit },
        }),
      writeFile: (cwd, relativePath, contents) =>
        invoke("projects_write_file", {
          input: { cwd, relativePath, contents },
        }).then(() => undefined),
    },
    settings: {
      get: () => invoke<AppConfig>("settings_get"),
      save: (config) => invoke("settings_save", { config }),
    },
    // --- eAgent platform API ---
    eagent: {
      submitTask: (prompt) =>
        safeInvoke<{ graphId: string; rootTaskId: string; status: string }>(
          "eagent_submit_task", { prompt }, { graphId: "", rootTaskId: "", status: "failed" }
        ),
      cancelGraph: (graphId) =>
        safeInvoke<void>("eagent_cancel_graph", { graphId }),
      getGraph: (graphId) =>
        safeInvoke<TaskGraphState>("eagent_get_graph", { graphId }, {
          graphId,
          rootTaskId: "",
          userPrompt: "",
          nodes: {},
          edges: [],
          traces: {},
          diffs: {},
          oversightRequests: {},
          status: "planning",
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        }),
      approveOversight: (requestId) =>
        safeInvoke<void>("eagent_approve_oversight", { requestId }),
      denyOversight: (requestId) =>
        safeInvoke<void>("eagent_deny_oversight", { requestId }),
      getProviders: () =>
        safeInvoke<ProviderStatus[]>("eagent_get_providers", undefined, []),
      configureProvider: (input: { endpoint: string; apiKey: string; model: string; name?: string }) =>
        safeInvoke<ProviderStatus>("eagent_configure_provider", { input }),
    },
  };
}

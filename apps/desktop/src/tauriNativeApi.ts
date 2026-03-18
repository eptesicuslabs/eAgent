import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AppBootstrap,
  AppConfig,
  NativeApi,
  OrchestrationDispatchCommand,
  OrchestrationSnapshot,
  ProjectSearchEntry,
  TerminalRecord,
} from "./types";

async function subscribe(eventName: string, listener: () => void) {
  const unlisten = await listen(eventName, () => listener());
  return () => {
    unlisten();
  };
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
  };
}

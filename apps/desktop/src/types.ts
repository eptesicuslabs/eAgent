export type ProviderKind = "codex" | "llama-cpp";
export type RuntimeMode = "approval-required" | "full-access";
export type InteractionMode = "chat" | "plan";
export type CodexReasoningEffort = "low" | "medium" | "high";
export type ProviderSessionStatus =
  | "starting"
  | "ready"
  | "running"
  | "waiting"
  | "stopped"
  | "error";
export type TurnStatus =
  | "requested"
  | "running"
  | "waiting"
  | "completed"
  | "interrupted"
  | "failed";
export type MessageRole = "user" | "assistant" | "system";
export type ApprovalKind = "command_execution" | "file_change" | "file_read";
export type ProviderRuntimeEventKind =
  | "session_state_changed"
  | "turn_started"
  | "content_delta"
  | "turn_completed"
  | "request_opened"
  | "request_resolved"
  | "user_input_requested"
  | "tool_started"
  | "tool_updated"
  | "tool_completed"
  | "runtime_warning"
  | "runtime_error";

export interface ThreadSettings {
  provider: ProviderKind;
  model: string;
  runtime_mode: RuntimeMode;
  interaction_mode: InteractionMode;
  codex_reasoning_effort: CodexReasoningEffort;
  codex_fast_mode: boolean;
  local_agent_web_search_enabled: boolean;
}

export interface MessageItem {
  item_id: string;
  role: MessageRole;
  content: string;
  timestamp: string;
}

export interface TurnState {
  id: string;
  input: string;
  images: string[];
  settings_snapshot: ThreadSettings;
  status: TurnStatus;
  messages: MessageItem[];
  started_at: string;
  completed_at: string | null;
  provider_turn_id: string | null;
}

export interface SessionState {
  session_id: string;
  provider: ProviderKind;
  provider_thread_id: string;
  status: ProviderSessionStatus;
  established_at: string;
  last_error: string | null;
  last_message: string | null;
}

export interface PendingApproval {
  id: string;
  turn_id: string;
  rpc_id: number;
  kind: ApprovalKind;
  details: unknown;
  requested_at: string;
}

export interface PendingUserInput {
  id: string;
  turn_id: string;
  rpc_id: number;
  questions: unknown;
  requested_at: string;
}

export interface ProviderRuntimeEvent {
  provider: ProviderKind;
  event_type: ProviderRuntimeEventKind;
  turn_id: string | null;
  item_id: string | null;
  request_id: string | null;
  summary: string | null;
  data: unknown;
  timestamp: string;
}

export interface ThreadError {
  message: string;
  will_retry: boolean;
  timestamp: string;
}

export interface ThreadState {
  id: string;
  project_id: string;
  name: string;
  created_at: string;
  updated_at: string;
  settings: ThreadSettings;
  turns: TurnState[];
  session: SessionState | null;
  active_turn: string | null;
  pending_approvals: Record<string, PendingApproval>;
  pending_inputs: Record<string, PendingUserInput>;
  runtime_events: ProviderRuntimeEvent[];
  errors: ThreadError[];
  deleted: boolean;
}

export interface ProjectScript {
  name: string;
  command: string;
  icon?: string | null;
}

export interface ProjectState {
  id: string;
  name: string;
  path: string;
  default_model: string | null;
  thread_ids: string[];
}

export interface ReadModel {
  threads: Record<string, ThreadState>;
  projects: Record<string, ProjectState>;
}

export interface AppConfig {
  general: {
    theme: string;
    font_size: number;
  };
  codex: {
    binary_path: string;
    home_dir: string;
    default_model: string;
    default_reasoning_effort: CodexReasoningEffort;
    default_fast_mode: boolean;
    default_interaction_mode: InteractionMode;
    default_runtime_mode: RuntimeMode;
  };
  llama_cpp: {
    enabled: boolean;
    llama_server_binary_path: string;
    model_path: string;
    host: string;
    port: number;
    ctx_size: number;
    threads: number;
    gpu_layers: number;
    flash_attention: boolean;
    temperature: number;
    top_p: number;
    default_model: string;
    default_runtime_mode: RuntimeMode;
    default_interaction_mode: InteractionMode;
    default_local_agent_web_search_enabled: boolean;
  };
  projects: {
    entries: Array<{
      id: string;
      path: string;
      name: string;
      default_model?: string | null;
      scripts?: ProjectScript[];
    }>;
  };
}

export interface TerminalRecord {
  id: string;
  threadId: string | null;
  title: string;
  buffer: string;
  isAlive: boolean;
}

export interface AppBootstrap {
  appName: string;
  shell: string;
  migrationStage: string;
  currentProject: string | null;
  currentThreadId: string | null;
  statusMessage: string;
  codexAvailable: boolean;
  codexVersion: string | null;
  codexBinarySource: string;
  codexResolvedPath: string | null;
  codexModels: string[];
  configPath: string | null;
  config: AppConfig;
  recentProjects: string[];
}

export interface OrchestrationSnapshot {
  readModel: ReadModel;
  currentProject: string | null;
  currentThreadId: string | null;
  terminals: TerminalRecord[];
}

export interface GitFileStatus {
  path: string;
  status: "new" | "modified" | "deleted" | "renamed" | "type_change" | "conflicted";
}

export interface GitDiffLine {
  kind: "context" | "addition" | "deletion";
  content: string;
}

export interface GitDiffHunk {
  old_start: number;
  old_lines: number;
  new_start: number;
  new_lines: number;
  lines: GitDiffLine[];
}

export interface GitFileDiff {
  old_path: string | null;
  new_path: string | null;
  hunks: GitDiffHunk[];
  is_binary: boolean;
}

export interface WorktreeInfo {
  name: string;
  path: string;
  branch: string | null;
  is_main: boolean;
}

export interface GitStatusPayload {
  isGitRepo: boolean;
  currentBranch: string | null;
  statuses: GitFileStatus[];
  diffs: GitFileDiff[];
  worktrees: WorktreeInfo[];
}

export interface ProjectSearchEntry {
  path: string;
  isDirectory: boolean;
}

export type OrchestrationDispatchCommand =
  | { type: "createThread"; name: string }
  | { type: "selectThread"; threadId: string }
  | { type: "deleteThread"; threadId: string }
  | { type: "renameThread"; threadId: string; name: string }
  | { type: "sendMessage"; message: string }
  | { type: "interruptTurn" }
  | { type: "updateCurrentThreadSettings"; settings: ThreadSettings }
  | { type: "approve"; approvalId: string }
  | { type: "deny"; approvalId: string }
  | { type: "userInputResponse"; approvalId: string; response: string }
  | { type: "openProject"; path: string }
  | { type: "openTerminal" }
  | { type: "sendTerminalInput"; terminalId: string; input: string }
  | { type: "resizeTerminal"; terminalId: string; cols: number; rows: number }
  | { type: "closeTerminal"; terminalId: string }
  | { type: "clearTerminal"; terminalId: string };

export interface NativeApi {
  app: {
    getBootstrap: () => Promise<AppBootstrap>;
    pickFolder: () => Promise<string | null>;
    openExternal: (url: string) => Promise<void>;
    onDomainEvent: (listener: () => void) => Promise<() => void>;
    onTerminalEvent: (listener: () => void) => Promise<() => void>;
    onSettingsUpdated: (listener: () => void) => Promise<() => void>;
    onStatusChanged: (listener: (statusMessage: string) => void) => Promise<() => void>;
  };
  shell: {
    openInEditor: (path: string) => Promise<void>;
  };
  orchestration: {
    getSnapshot: () => Promise<OrchestrationSnapshot>;
    dispatch: (command: OrchestrationDispatchCommand) => Promise<void>;
  };
  terminal: {
    list: () => Promise<TerminalRecord[]>;
    open: () => Promise<void>;
    write: (terminalId: string, input: string) => Promise<void>;
    resize: (terminalId: string, cols: number, rows: number) => Promise<void>;
    close: (terminalId: string) => Promise<void>;
    clear: (terminalId: string) => Promise<void>;
  };
  git: {
    status: (cwd: string) => Promise<GitStatusPayload>;
    listBranches: (cwd: string) => Promise<Array<{ name: string; is_head: boolean }>>;
    diffWorkdir: (cwd: string) => Promise<GitStatusPayload>;
    createWorktree: (input: {
      cwd: string;
      name: string;
      path: string;
      branch?: string | null;
    }) => Promise<void>;
    removeWorktree: (input: { cwd: string; name: string }) => Promise<void>;
  };
  projects: {
    open: (path: string) => Promise<void>;
    searchEntries: (cwd: string, query: string, limit?: number) => Promise<ProjectSearchEntry[]>;
    writeFile: (cwd: string, relativePath: string, contents: string) => Promise<void>;
  };
  settings: {
    get: () => Promise<AppConfig>;
    save: (config: AppConfig) => Promise<void>;
  };
}

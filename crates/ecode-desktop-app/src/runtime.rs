//! Toolkit-neutral desktop runtime/controller.

use crate::state::{AppState, TerminalState};
use anyhow::Result;
use ecode_contracts::codex::{ApprovalDecision, ApprovalPolicy, CodexEvent};
use ecode_contracts::ids::*;
use ecode_contracts::orchestration::{ApprovalKind, Command, ThreadSettings, ThreadState};
use ecode_contracts::provider::{FALLBACK_CODEX_MODELS, MIN_CODEX_VERSION, ProviderKind};
use ecode_contracts::provider_runtime::{ProviderRuntimeEventKind, ProviderSessionStatus};
use ecode_contracts::terminal::TerminalEvent;
use ecode_core::codex::CodexManager;
use ecode_core::config::ConfigManager;
use ecode_core::local_agent::{
    LocalAgentDecision, LocalAgentExecutor, local_agent_system_prompt, parse_local_agent_decision,
};
use ecode_core::orchestration::OrchestrationEngine;
use ecode_core::persistence::EventStore;
use ecode_core::providers::llama_cpp::LlamaCppManager;
use ecode_core::terminal::TerminalManager;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Actions the UI can send to the background runtime.
pub enum UiAction {
    SendMessage(String),
    InterruptTurn,
    CreateThread { name: String },
    SelectThread(ThreadId),
    DeleteThread(ThreadId),
    RenameThread { id: ThreadId, name: String },
    UpdateCurrentThreadSettings(ThreadSettings),
    Approve(ApprovalRequestId),
    Deny(ApprovalRequestId),
    UserInputResponse { id: ApprovalRequestId, response: String },
    OpenProject(String),
    OpenTerminal,
    CheckCodex,
    UpdateCodexModels(Vec<String>),
    UpdateCodexConfig {
        binary_path: Option<String>,
        codex_home: Option<String>,
    },
    UpdateLlamaCppConfig {
        binary_path: Option<String>,
        default_model: String,
        host: Option<String>,
        port: Option<u16>,
        context_size: Option<u32>,
        threads: Option<u32>,
    },
    SaveSettings,
    SendTerminalInput { terminal_id: TerminalId, input: String },
    ResizeTerminal {
        terminal_id: TerminalId,
        cols: u16,
        rows: u16,
    },
    CloseTerminal(TerminalId),
    ClearTerminal(TerminalId),
}

#[derive(Clone)]
pub struct AppRuntime {
    state: Arc<AppState>,
    action_tx: mpsc::UnboundedSender<UiAction>,
}

impl AppRuntime {
    pub fn spawn_with_notifier<F>(runtime: &Runtime, notifier: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        let state = Arc::new(AppState::new());
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let state_clone = Arc::clone(&state);
        let action_tx_clone = action_tx.clone();
        let notifier: Arc<dyn Fn() + Send + Sync> = Arc::new(notifier);

        runtime.spawn(async move {
            if let Err(e) = run_background(state_clone, action_rx, action_tx_clone, notifier).await
            {
                error!("Background task error: {}", e);
            }
        });

        let _ = action_tx.send(UiAction::CheckCodex);

        Self { state, action_tx }
    }

    pub fn state(&self) -> Arc<AppState> {
        Arc::clone(&self.state)
    }

    pub fn action_tx(&self) -> mpsc::UnboundedSender<UiAction> {
        self.action_tx.clone()
    }

    pub fn send(
        &self,
        action: UiAction,
    ) -> std::result::Result<(), mpsc::error::SendError<UiAction>> {
        self.action_tx.send(action)
    }
}

async fn run_background(
    state: Arc<AppState>,
    mut action_rx: mpsc::UnboundedReceiver<UiAction>,
    action_tx: mpsc::UnboundedSender<UiAction>,
    notifier: Arc<dyn Fn() + Send + Sync>,
) -> Result<()> {
    let config = ConfigManager::load().unwrap_or_else(|e| {
        error!("Failed to load config: {}", e);
        ConfigManager::load_from(std::env::temp_dir().join("ecode_config.toml")).unwrap()
    });

    *state.config.write().unwrap() = config.config().clone();
    *state.config_path.write().unwrap() = Some(config.config_path().clone());

    let store_path = ConfigManager::event_store_path()
        .unwrap_or_else(|_| std::env::temp_dir().join("ecode_events.db"));
    let engine = initialize_engine(&store_path)?;

    {
        let snapshot = engine.snapshot();
        *state.read_model.write().unwrap() = snapshot;
    }

    let (codex_event_tx, mut codex_event_rx) = mpsc::unbounded_channel();
    let codex_binary = config.config().codex.binary_path.clone();
    let codex_home = config.config().codex.home_dir.clone();
    let resolved_codex_binary = ecode_core::codex::find_codex_binary(if codex_binary.is_empty() {
        None
    } else {
        Some(codex_binary.as_str())
    })
    .ok();
    let codex_manager = Arc::new(CodexManager::new(
        codex_event_tx,
        resolved_codex_binary.or({
            if codex_binary.is_empty() {
                None
            } else {
                Some(codex_binary)
            }
        }),
        if codex_home.is_empty() {
            None
        } else {
            Some(codex_home)
        },
    ));

    let (terminal_manager, mut terminal_rx) = TerminalManager::new();
    let terminal_manager = Arc::new(terminal_manager);
    let llama_cpp_manager = Arc::new(LlamaCppManager::new());

    let mut engine_rx = engine.subscribe();
    let engine = Arc::new(tokio::sync::Mutex::new(engine));

    let request_repaint = {
        let notifier = Arc::clone(&notifier);
        move || {
            (notifier)();
        }
    };

    info!("Background systems initialized");
    state.set_status("Ready");
    request_repaint();

    loop {
        tokio::select! {
            Some(action) = action_rx.recv() => {
                handle_action(
                    action,
                    &state,
                    &engine,
                    &codex_manager,
                    &llama_cpp_manager,
                    &terminal_manager,
                    &action_tx,
                    &request_repaint,
                ).await;
            }
            Some((thread_id, event)) = codex_event_rx.recv() => {
                handle_codex_event(thread_id, event, &state, &engine, &request_repaint).await;
            }
            Some(event) = terminal_rx.recv() => {
                handle_terminal_event(event, &state, &request_repaint);
            }
            Ok(_) = engine_rx.recv() => {
                sync_snapshot(&state, &engine).await;
                request_repaint();
            }
        }
    }
}

fn initialize_engine(store_path: &std::path::Path) -> Result<OrchestrationEngine> {
    let store = Arc::new(EventStore::open(store_path)?);
    let engine = OrchestrationEngine::new(store);
    engine.rebuild()?;
    Ok(engine)
}

async fn sync_snapshot(
    state: &Arc<AppState>,
    engine: &Arc<tokio::sync::Mutex<OrchestrationEngine>>,
) {
    let snapshot = {
        let eng = engine.lock().await;
        eng.snapshot()
    };
    *state.read_model.write().unwrap() = snapshot;
}

async fn dispatch_and_sync(
    state: &Arc<AppState>,
    engine: &Arc<tokio::sync::Mutex<OrchestrationEngine>>,
    command: Command,
) -> Result<()> {
    let snapshot = {
        let eng = engine.lock().await;
        eng.dispatch(command)?;
        eng.snapshot()
    };
    *state.read_model.write().unwrap() = snapshot;
    Ok(())
}

fn current_thread(state: &Arc<AppState>) -> Option<ThreadState> {
    state.current_thread_state()
}

fn fallback_codex_models() -> Vec<String> {
    FALLBACK_CODEX_MODELS
        .iter()
        .map(|model| (*model).to_string())
        .collect()
}

fn normalize_codex_models(models: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for model in models {
        let trimmed = model.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }
        normalized.push(trimmed.to_string());
    }

    if normalized.is_empty() {
        fallback_codex_models()
    } else {
        normalized
    }
}

fn set_codex_model_catalog(state: &Arc<AppState>, models: Vec<String>) {
    *state.codex_models.write().unwrap() = normalize_codex_models(models);
}

fn resolve_thread_turn(thread: &ThreadState, provider_turn_id: &str) -> Option<TurnId> {
    thread
        .turns
        .iter()
        .find(|turn| turn.provider_turn_id.as_deref() == Some(provider_turn_id))
        .map(|turn| turn.id)
}

fn settings_require_new_session(previous: &ThreadSettings, next: &ThreadSettings) -> bool {
    previous.provider != next.provider
        || previous.model != next.model
        || previous.runtime_mode != next.runtime_mode
        || previous.codex_fast_mode != next.codex_fast_mode
        || previous.codex_reasoning_effort != next.codex_reasoning_effort
}

async fn load_codex_model_catalog(codex_manager: &Arc<CodexManager>) -> Vec<String> {
    let discovery_thread_id = ThreadId::new();
    let result = tokio::time::timeout(Duration::from_secs(4), async {
        codex_manager.spawn_session(discovery_thread_id).await?;
        let models = codex_manager.list_models(discovery_thread_id).await?;
        Ok::<Vec<String>, anyhow::Error>(models.into_iter().map(|model| model.id).collect())
    })
    .await;
    let _ = codex_manager.kill_session(discovery_thread_id).await;

    match result {
        Ok(Ok(models)) => normalize_codex_models(models),
        Ok(Err(error)) => {
            warn!(
                ?error,
                "Failed to refresh Codex model catalog; using fallback list"
            );
            fallback_codex_models()
        }
        Err(error) => {
            warn!(
                ?error,
                "Timed out refreshing Codex model catalog; using fallback list"
            );
            fallback_codex_models()
        }
    }
}

async fn refresh_codex_model_catalog(
    state: &Arc<AppState>,
    codex_manager: &Arc<CodexManager>,
    action_tx: &mpsc::UnboundedSender<UiAction>,
) {
    if !*state.codex_available.read().unwrap() {
        set_codex_model_catalog(state, fallback_codex_models());
        let _ = action_tx.send(UiAction::UpdateCodexModels(fallback_codex_models()));
        return;
    }

    let codex_manager = Arc::clone(codex_manager);
    let action_tx = action_tx.clone();
    tokio::spawn(async move {
        let models = load_codex_model_catalog(&codex_manager).await;
        let _ = action_tx.send(UiAction::UpdateCodexModels(models));
    });
}

async fn ensure_codex_session(
    state: &Arc<AppState>,
    engine: &Arc<tokio::sync::Mutex<OrchestrationEngine>>,
    codex_manager: &Arc<CodexManager>,
    thread: &ThreadState,
) -> Result<()> {
    if codex_manager.has_session(&thread.id).await {
        return Ok(());
    }

    let session_id = codex_manager.spawn_session(thread.id).await?;
    let provider_thread_id = if let Some(session) = thread.session.as_ref() {
        codex_manager
            .resume_thread(thread.id, &session.provider_thread_id)
            .await?
    } else {
        let cwd = state
            .current_project
            .read()
            .unwrap()
            .clone()
            .unwrap_or_else(|| ".".to_string());
        let approval_policy = match thread.settings.runtime_mode {
            ecode_contracts::orchestration::RuntimeMode::ApprovalRequired => {
                ApprovalPolicy::OnRequest
            }
            ecode_contracts::orchestration::RuntimeMode::FullAccess => ApprovalPolicy::Never,
        };
        codex_manager
            .start_thread(
                thread.id,
                &thread.settings.model,
                &cwd,
                approval_policy,
                thread.settings.runtime_mode.to_sandbox_mode(),
            )
            .await?
    };

    dispatch_and_sync(
        state,
        engine,
        Command::SetSession {
            thread_id: thread.id,
            session_id,
            provider: ProviderKind::Codex,
            provider_thread_id,
        },
    )
    .await?;

    Ok(())
}

async fn ensure_local_session(
    state: &Arc<AppState>,
    engine: &Arc<tokio::sync::Mutex<OrchestrationEngine>>,
    thread: &ThreadState,
) -> Result<()> {
    if thread.session.is_some() {
        return Ok(());
    }

    dispatch_and_sync(
        state,
        engine,
        Command::SetSession {
            thread_id: thread.id,
            session_id: SessionId::new(),
            provider: ProviderKind::LlamaCpp,
            provider_thread_id: format!("llama-thread-{}", thread.id),
        },
    )
    .await
}

async fn run_llama_local_agent(
    state: &Arc<AppState>,
    engine: &Arc<tokio::sync::Mutex<OrchestrationEngine>>,
    llama_cpp_manager: &Arc<LlamaCppManager>,
    thread: &ThreadState,
    turn_id: TurnId,
    prompt: &str,
) -> Result<String> {
    let config = state.config.read().unwrap().llama_cpp.clone();
    let project_root = state
        .current_project
        .read()
        .unwrap()
        .clone()
        .unwrap_or_else(|| ".".to_string());
    let executor = LocalAgentExecutor::new(
        std::path::PathBuf::from(project_root),
        thread.settings.local_agent_web_search_enabled,
    );

    let mut messages = Vec::new();
    messages.push(serde_json::json!({
        "role": "system",
        "content": local_agent_system_prompt(thread.settings.local_agent_web_search_enabled),
    }));

    for turn in &thread.turns {
        messages.push(serde_json::json!({
            "role": "user",
            "content": turn.input,
        }));

        let assistant = turn
            .messages
            .iter()
            .filter(|message| {
                message.role == ecode_contracts::orchestration::MessageRole::Assistant
            })
            .map(|message| message.content.as_str())
            .collect::<String>();
        if !assistant.is_empty() {
            messages.push(serde_json::json!({
                "role": "assistant",
                "content": assistant,
            }));
        }
    }

    messages.push(serde_json::json!({
        "role": "user",
        "content": prompt,
    }));

    for _ in 0..6 {
        let raw = llama_cpp_manager
            .complete_messages(&config, &thread.settings.model, messages.clone())
            .await?;

        match parse_local_agent_decision(&raw) {
            Ok(LocalAgentDecision::Assistant { content }) => return Ok(content),
            Ok(LocalAgentDecision::ToolCall { tool, arguments }) => {
                let _ = dispatch_and_sync(
                    state,
                    engine,
                    Command::RecordRuntimeEvent {
                        thread_id: thread.id,
                        turn_id: Some(turn_id),
                        event_type: ProviderRuntimeEventKind::ToolStarted,
                        item_id: Some(tool.clone()),
                        request_id: None,
                        summary: Some(format!("Running {}", tool)),
                        data: arguments.clone(),
                    },
                )
                .await;

                let tool_result = executor
                    .execute(&tool, &arguments, thread.settings.runtime_mode)
                    .await;

                match tool_result {
                    Ok(result) => {
                        let _ = dispatch_and_sync(
                            state,
                            engine,
                            Command::RecordRuntimeEvent {
                                thread_id: thread.id,
                                turn_id: Some(turn_id),
                                event_type: ProviderRuntimeEventKind::ToolCompleted,
                                item_id: Some(tool.clone()),
                                request_id: None,
                                summary: Some(format!("Completed {}", tool)),
                                data: serde_json::json!({ "result": result }),
                            },
                        )
                        .await;
                        messages.push(serde_json::json!({
                            "role": "assistant",
                            "content": raw,
                        }));
                        messages.push(serde_json::json!({
                            "role": "system",
                            "content": format!("TOOL RESULT [{}]\n{}", tool, result),
                        }));
                    }
                    Err(error) => {
                        let message = format!("{} failed: {}", tool, error);
                        let _ = dispatch_and_sync(
                            state,
                            engine,
                            Command::RecordRuntimeEvent {
                                thread_id: thread.id,
                                turn_id: Some(turn_id),
                                event_type: ProviderRuntimeEventKind::RuntimeError,
                                item_id: Some(tool.clone()),
                                request_id: None,
                                summary: Some(message.clone()),
                                data: Value::Null,
                            },
                        )
                        .await;
                        messages.push(serde_json::json!({
                            "role": "assistant",
                            "content": raw,
                        }));
                        messages.push(serde_json::json!({
                            "role": "system",
                            "content": format!("TOOL ERROR [{}]\n{}", tool, message),
                        }));
                    }
                }
            }
            Err(_) => return Ok(raw),
        }
    }

    Err(anyhow::anyhow!("local agent exceeded tool iteration limit"))
}

#[allow(clippy::too_many_arguments)]
async fn handle_action<F: Fn()>(
    action: UiAction,
    state: &Arc<AppState>,
    engine: &Arc<tokio::sync::Mutex<OrchestrationEngine>>,
    codex_manager: &Arc<CodexManager>,
    llama_cpp_manager: &Arc<LlamaCppManager>,
    terminal_manager: &Arc<TerminalManager>,
    action_tx: &mpsc::UnboundedSender<UiAction>,
    repaint: &F,
) {
    match action {
        UiAction::CheckCodex => {
            let (binary_override, home_override) = {
                let cfg = state.config.read().unwrap();
                let binary = if cfg.codex.binary_path.trim().is_empty() {
                    None
                } else {
                    Some(cfg.codex.binary_path.clone())
                };
                let home = if cfg.codex.home_dir.trim().is_empty() {
                    None
                } else {
                    Some(cfg.codex.home_dir.clone())
                };
                (binary, home)
            };
            *state.codex_binary_source.write().unwrap() = if binary_override.is_some() {
                "Override".to_string()
            } else {
                "PATH".to_string()
            };

            match ecode_core::codex::find_codex_binary(binary_override.as_deref()) {
                Ok(resolved_path) => {
                    codex_manager.configure(Some(resolved_path.clone()), home_override);
                    *state.codex_resolved_path.write().unwrap() = Some(resolved_path.clone());
                    match ecode_core::codex::check_codex_version(&resolved_path, MIN_CODEX_VERSION)
                    {
                        Ok(version) => {
                            *state.codex_available.write().unwrap() = true;
                            *state.codex_version.write().unwrap() = Some(version.clone());
                            state.set_status(format!("Codex CLI v{}", version));
                            refresh_codex_model_catalog(state, codex_manager, action_tx).await;
                        }
                        Err(e) => {
                            *state.codex_available.write().unwrap() = false;
                            *state.codex_version.write().unwrap() = None;
                            set_codex_model_catalog(state, fallback_codex_models());
                            state.set_status(format!("Codex CLI not found: {}", e));
                            state.push_error(format!("Codex CLI not available: {}", e));
                        }
                    }
                }
                Err(e) => {
                    codex_manager.configure(binary_override, home_override);
                    *state.codex_available.write().unwrap() = false;
                    *state.codex_version.write().unwrap() = None;
                    *state.codex_resolved_path.write().unwrap() = None;
                    set_codex_model_catalog(state, fallback_codex_models());
                    state.set_status(format!("Codex CLI not found: {}", e));
                    state.push_error(format!("Codex CLI not available: {}", e));
                }
            }
            repaint();
        }
        UiAction::UpdateCodexModels(models) => {
            set_codex_model_catalog(state, models);
            repaint();
        }
        UiAction::CreateThread { name } => {
            let project_id = ProjectId::new();
            let thread_id = ThreadId::new();
            let mut settings = state
                .config
                .read()
                .unwrap()
                .default_thread_settings(ProviderKind::Codex);
            settings.model = state.preferred_codex_model();
            let result = dispatch_and_sync(
                state,
                engine,
                Command::CreateThread {
                    thread_id,
                    project_id,
                    name: Some(name),
                    settings,
                },
            )
            .await;
            if let Err(e) = result {
                state.push_error(format!("Failed to create thread: {}", e));
            } else {
                *state.current_thread.write().unwrap() = Some(thread_id);
            }
            repaint();
        }
        UiAction::SelectThread(id) => {
            *state.current_thread.write().unwrap() = Some(id);
            repaint();
        }
        UiAction::DeleteThread(id) => {
            if let Err(e) =
                dispatch_and_sync(state, engine, Command::DeleteThread { thread_id: id }).await
            {
                state.push_error(format!("Failed to delete thread: {}", e));
            } else if *state.current_thread.read().unwrap() == Some(id) {
                *state.current_thread.write().unwrap() = None;
            }
            repaint();
        }
        UiAction::RenameThread { id, name } => {
            if let Err(e) = dispatch_and_sync(
                state,
                engine,
                Command::RenameThread {
                    thread_id: id,
                    name,
                },
            )
            .await
            {
                state.push_error(format!("Failed to rename thread: {}", e));
            }
            repaint();
        }
        UiAction::UpdateCurrentThreadSettings(settings) => {
            let thread = match current_thread(state) {
                Some(thread) => thread,
                None => return,
            };
            let thread_id = thread.id;
            let restart_required = settings_require_new_session(&thread.settings, &settings);

            if restart_required {
                match thread.settings.provider {
                    ProviderKind::Codex => {
                        if let Err(e) = codex_manager.kill_session(thread_id).await {
                            state.push_error(format!("Failed to stop Codex session: {}", e));
                        }
                    }
                    ProviderKind::LlamaCpp => {}
                }
                let _ = dispatch_and_sync(state, engine, Command::ClearSession { thread_id }).await;
            }
            if let Err(e) = dispatch_and_sync(
                state,
                engine,
                Command::UpdateThreadSettings {
                    thread_id,
                    settings,
                },
            )
            .await
            {
                state.push_error(format!("Failed to update thread settings: {}", e));
            } else if restart_required {
                state.set_status("Thread settings updated; session will restart on next turn");
            }
            repaint();
        }
        UiAction::SendMessage(text) => {
            let thread = match current_thread(state) {
                Some(thread) => thread,
                None => {
                    state.push_error("No thread selected");
                    return;
                }
            };

            match thread.settings.provider {
                ProviderKind::Codex => {
                    if !*state.codex_available.read().unwrap() {
                        state.push_error("Codex CLI not available");
                        return;
                    }

                    if let Err(e) =
                        ensure_codex_session(state, engine, codex_manager, &thread).await
                    {
                        state.push_error(format!("Failed to establish Codex session: {}", e));
                        state.set_status("Codex session failed");
                        repaint();
                        return;
                    }

                    let turn_id = TurnId::new();
                    if let Err(e) = dispatch_and_sync(
                        state,
                        engine,
                        Command::StartTurn {
                            thread_id: thread.id,
                            turn_id,
                            input: text.clone(),
                            images: vec![],
                        },
                    )
                    .await
                    {
                        state.push_error(format!("Failed to start turn: {}", e));
                        repaint();
                        return;
                    }

                    state.set_status("Processing...");
                    let developer_instructions = match thread.settings.interaction_mode {
                        ecode_contracts::orchestration::InteractionMode::Plan => Some(
                            "Produce a plan first, then wait for approval before making changes."
                                .to_string(),
                        ),
                        ecode_contracts::orchestration::InteractionMode::Chat => None,
                    };

                    if let Err(e) = codex_manager
                        .send_turn(thread.id, &text, &[], developer_instructions)
                        .await
                    {
                        state.push_error(format!("Codex error: {}", e));
                        let _ = dispatch_and_sync(
                            state,
                            engine,
                            Command::RecordError {
                                thread_id: thread.id,
                                message: format!("Failed to send turn: {}", e),
                                will_retry: false,
                            },
                        )
                        .await;
                        let _ = dispatch_and_sync(
                            state,
                            engine,
                            Command::CompleteTurn {
                                thread_id: thread.id,
                                turn_id,
                                status: "send_failed".to_string(),
                            },
                        )
                        .await;
                        state.set_status("Send failed");
                    }
                }
                ProviderKind::LlamaCpp => {
                    let _ = ensure_local_session(state, engine, &thread).await;
                    let turn_id = TurnId::new();
                    if let Err(e) = dispatch_and_sync(
                        state,
                        engine,
                        Command::StartTurn {
                            thread_id: thread.id,
                            turn_id,
                            input: text.clone(),
                            images: vec![],
                        },
                    )
                    .await
                    {
                        state.push_error(format!("Failed to start local turn: {}", e));
                        repaint();
                        return;
                    }

                    state.set_status("Running local model...");
                    let provider_turn_id = format!("llama-turn-{}", turn_id);
                    let _ = dispatch_and_sync(
                        state,
                        engine,
                        Command::RecordTurnStarted {
                            thread_id: thread.id,
                            turn_id,
                            provider_turn_id,
                        },
                    )
                    .await;

                    match run_llama_local_agent(
                        state,
                        engine,
                        llama_cpp_manager,
                        &thread,
                        turn_id,
                        &text,
                    )
                    .await
                    {
                        Ok(response) => {
                            let _ = dispatch_and_sync(
                                state,
                                engine,
                                Command::AppendAssistantDelta {
                                    thread_id: thread.id,
                                    turn_id,
                                    item_id: format!("assistant-{}", turn_id),
                                    delta: response,
                                },
                            )
                            .await;
                            let _ = dispatch_and_sync(
                                state,
                                engine,
                                Command::CompleteTurn {
                                    thread_id: thread.id,
                                    turn_id,
                                    status: "completed".to_string(),
                                },
                            )
                            .await;
                            state.set_status("Ready");
                        }
                        Err(e) => {
                            state.push_error(format!("llama.cpp error: {}", e));
                            let _ = dispatch_and_sync(
                                state,
                                engine,
                                Command::RecordError {
                                    thread_id: thread.id,
                                    message: format!("llama.cpp send failed: {}", e),
                                    will_retry: false,
                                },
                            )
                            .await;
                            let _ = dispatch_and_sync(
                                state,
                                engine,
                                Command::CompleteTurn {
                                    thread_id: thread.id,
                                    turn_id,
                                    status: "failed".to_string(),
                                },
                            )
                            .await;
                            state.set_status("llama.cpp error");
                        }
                    }
                }
            }
            repaint();
        }
        UiAction::InterruptTurn => {
            let thread = match current_thread(state) {
                Some(thread) => thread,
                None => return,
            };
            let Some(turn_id) = thread.active_turn else {
                return;
            };

            let provider_turn_id = thread
                .turns
                .iter()
                .find(|turn| turn.id == turn_id)
                .and_then(|turn| turn.provider_turn_id.clone());

            let _ = dispatch_and_sync(
                state,
                engine,
                Command::InterruptTurn {
                    thread_id: thread.id,
                    turn_id,
                },
            )
            .await;
            if thread.settings.provider == ProviderKind::Codex
                && let Some(provider_turn_id) = provider_turn_id
                && let Err(e) = codex_manager
                    .interrupt_turn(thread.id, &provider_turn_id)
                    .await
            {
                state.push_error(format!("Failed to interrupt: {}", e));
            }
            state.set_status("Interrupted");
            repaint();
        }
        UiAction::Approve(approval_id) => {
            let thread = match current_thread(state) {
                Some(thread) => thread,
                None => return,
            };
            let rpc_id = thread
                .pending_approvals
                .get(&approval_id)
                .map(|approval| approval.rpc_id);
            if let Some(rpc_id) = rpc_id {
                if let Err(e) = codex_manager
                    .respond_approval(thread.id, rpc_id, ApprovalDecision::Accept)
                    .await
                {
                    state.push_error(format!("Failed to approve: {}", e));
                } else {
                    let _ = dispatch_and_sync(
                        state,
                        engine,
                        Command::RecordApprovalResponse {
                            thread_id: thread.id,
                            approval_id,
                            decision: ApprovalDecision::Accept,
                        },
                    )
                    .await;
                }
            }
            repaint();
        }
        UiAction::Deny(approval_id) => {
            let thread = match current_thread(state) {
                Some(thread) => thread,
                None => return,
            };
            let rpc_id = thread
                .pending_approvals
                .get(&approval_id)
                .map(|approval| approval.rpc_id);
            if let Some(rpc_id) = rpc_id {
                if let Err(e) = codex_manager
                    .respond_approval(thread.id, rpc_id, ApprovalDecision::Deny)
                    .await
                {
                    state.push_error(format!("Failed to deny: {}", e));
                } else {
                    let _ = dispatch_and_sync(
                        state,
                        engine,
                        Command::RecordApprovalResponse {
                            thread_id: thread.id,
                            approval_id,
                            decision: ApprovalDecision::Deny,
                        },
                    )
                    .await;
                }
            }
            repaint();
        }
        UiAction::UserInputResponse { id, response } => {
            let thread = match current_thread(state) {
                Some(thread) => thread,
                None => return,
            };
            let rpc_id = thread.pending_inputs.get(&id).map(|input| input.rpc_id);
            if let Some(rpc_id) = rpc_id {
                if let Err(e) = codex_manager
                    .respond_user_input(thread.id, rpc_id, Value::String(response.clone()))
                    .await
                {
                    state.push_error(format!("Failed to respond: {}", e));
                } else {
                    let _ = dispatch_and_sync(
                        state,
                        engine,
                        Command::RecordUserInputResponse {
                            thread_id: thread.id,
                            approval_id: id,
                            answers: Value::String(response),
                        },
                    )
                    .await;
                }
            }
            repaint();
        }
        UiAction::OpenProject(path) => {
            *state.current_project.write().unwrap() = Some(path.clone());
            state.set_status(format!("Project: {}", path));
            {
                let mut recent = state.recent_projects.write().unwrap();
                recent.retain(|p| p != &path);
                recent.insert(0, path.clone());
                if recent.len() > 10 {
                    recent.truncate(10);
                }
            }
            repaint();
        }
        UiAction::OpenTerminal => {
            let cwd = state
                .current_project
                .read()
                .unwrap()
                .clone()
                .unwrap_or_else(|| ".".to_string());
            let current_thread_id = *state.current_thread.read().unwrap();
            let terminal_index = state.terminals.read().unwrap().len() + 1;
            let config = ecode_contracts::terminal::TerminalConfig {
                cwd,
                ..Default::default()
            };
            match terminal_manager.open(config) {
                Ok(id) => {
                    state
                        .terminal_buffers
                        .write()
                        .unwrap()
                        .insert(id, String::new());

                    state.terminals.write().unwrap().push(TerminalState {
                        id,
                        thread_id: current_thread_id,
                        title: format!("Terminal {}", terminal_index),
                        buffer: String::new(),
                        is_alive: true,
                    });
                }
                Err(e) => {
                    state.push_error(format!("Failed to open terminal: {}", e));
                }
            }
            repaint();
        }
        UiAction::SendTerminalInput { terminal_id, input } => {
            let data = format!("{}\n", input);
            if let Err(e) = terminal_manager.write(&terminal_id, data.as_bytes()) {
                state.push_error(format!("Terminal write error: {}", e));
            }
        }
        UiAction::ResizeTerminal {
            terminal_id,
            cols,
            rows,
        } => {
            if let Err(e) = terminal_manager.resize(&terminal_id, cols, rows) {
                state.push_error(format!("Terminal resize error: {}", e));
            }
        }
        UiAction::CloseTerminal(terminal_id) => {
            if let Err(e) = terminal_manager.close(&terminal_id) {
                state.push_error(format!("Failed to close terminal: {}", e));
            }
            if let Some(terminal) = state
                .terminals
                .write()
                .unwrap()
                .iter_mut()
                .find(|terminal| terminal.id == terminal_id)
            {
                terminal.is_alive = false;
            }
            repaint();
        }
        UiAction::ClearTerminal(terminal_id) => {
            if let Err(e) = terminal_manager.write(&terminal_id, b"\x0c") {
                state.push_error(format!("Terminal clear error: {}", e));
            }
            if let Some(terminal) = state
                .terminals
                .write()
                .unwrap()
                .iter_mut()
                .find(|terminal| terminal.id == terminal_id)
            {
                terminal.buffer.clear();
            }
            repaint();
        }
        UiAction::UpdateCodexConfig {
            binary_path,
            codex_home,
        } => {
            let mut cfg = state.config.write().unwrap();
            cfg.codex.binary_path = binary_path.unwrap_or_default();
            cfg.codex.home_dir = codex_home.unwrap_or_default();
        }
        UiAction::UpdateLlamaCppConfig {
            binary_path,
            default_model,
            host,
            port,
            context_size,
            threads,
        } => {
            let mut cfg = state.config.write().unwrap();
            cfg.llama_cpp.llama_server_binary_path = binary_path.unwrap_or_default();
            cfg.llama_cpp.model_path = default_model;
            if let Some(h) = host {
                cfg.llama_cpp.host = h;
            }
            if let Some(p) = port {
                cfg.llama_cpp.port = p;
            }
            if let Some(cs) = context_size {
                cfg.llama_cpp.ctx_size = cs;
            }
            if let Some(t) = threads {
                cfg.llama_cpp.threads = t as u16;
            }
        }
        UiAction::SaveSettings => {
            let cfg = state.config.read().unwrap().clone();
            if let Some(path) = state.config_path.read().unwrap().as_ref() {
                match toml::to_string_pretty(&cfg) {
                    Ok(content) => {
                        if let Err(e) = std::fs::write(path, content) {
                            state.push_error(format!("Failed to save settings: {}", e));
                        } else {
                            state.set_status("Settings saved");
                        }
                    }
                    Err(e) => {
                        state.push_error(format!("Failed to serialize settings: {}", e));
                    }
                }
            }
        }
    }
}

async fn handle_codex_event<F: Fn()>(
    thread_id: ThreadId,
    event: CodexEvent,
    state: &Arc<AppState>,
    engine: &Arc<tokio::sync::Mutex<OrchestrationEngine>>,
    repaint: &F,
) {
    match &event {
        CodexEvent::TurnStarted { codex_turn_id } => {
            let thread = {
                state
                    .read_model
                    .read()
                    .unwrap()
                    .threads
                    .get(&thread_id)
                    .cloned()
            };
            if let Some(thread) = thread
                && let Some(turn_id) = thread.active_turn
            {
                let _ = dispatch_and_sync(
                    state,
                    engine,
                    Command::RecordTurnStarted {
                        thread_id,
                        turn_id,
                        provider_turn_id: codex_turn_id.clone(),
                    },
                )
                .await;
                let _ = dispatch_and_sync(
                    state,
                    engine,
                    Command::RecordRuntimeEvent {
                        thread_id,
                        turn_id: Some(turn_id),
                        event_type: ProviderRuntimeEventKind::TurnStarted,
                        item_id: None,
                        request_id: None,
                        summary: Some("Turn started".to_string()),
                        data: Value::Null,
                    },
                )
                .await;
            }
            state.set_status("Turn started...");
        }
        CodexEvent::AgentMessageDelta {
            codex_turn_id,
            item_id,
            delta,
        } => {
            let thread = {
                state
                    .read_model
                    .read()
                    .unwrap()
                    .threads
                    .get(&thread_id)
                    .cloned()
            };
            if let Some(thread) = thread
                && let Some(turn_id) = resolve_thread_turn(&thread, codex_turn_id)
            {
                let _ = dispatch_and_sync(
                    state,
                    engine,
                    Command::AppendAssistantDelta {
                        thread_id,
                        turn_id,
                        item_id: item_id.clone(),
                        delta: delta.clone(),
                    },
                )
                .await;
            }
        }
        CodexEvent::TurnCompleted {
            codex_turn_id,
            status,
        } => {
            let thread = {
                state
                    .read_model
                    .read()
                    .unwrap()
                    .threads
                    .get(&thread_id)
                    .cloned()
            };
            if let Some(thread) = thread
                && let Some(turn_id) = resolve_thread_turn(&thread, codex_turn_id)
            {
                let _ = dispatch_and_sync(
                    state,
                    engine,
                    Command::CompleteTurn {
                        thread_id,
                        turn_id,
                        status: status.clone(),
                    },
                )
                .await;
            }
            state.set_status("Ready");
        }
        CodexEvent::CommandApprovalRequested {
            rpc_id,
            turn_id,
            command,
            ..
        } => {
            let approval_id = ApprovalRequestId::new();
            let thread = {
                state
                    .read_model
                    .read()
                    .unwrap()
                    .threads
                    .get(&thread_id)
                    .cloned()
            };
            if let Some(thread) = thread
                && let Some(local_turn_id) = resolve_thread_turn(&thread, turn_id)
            {
                let details = command.clone().unwrap_or(Value::Null);
                let _ = dispatch_and_sync(
                    state,
                    engine,
                    Command::RecordApprovalRequest {
                        thread_id,
                        turn_id: local_turn_id,
                        approval_id,
                        rpc_id: *rpc_id,
                        kind: ApprovalKind::CommandExecution,
                        details,
                    },
                )
                .await;
            }
        }
        CodexEvent::FileChangeApprovalRequested {
            rpc_id,
            turn_id,
            file_path,
            diff,
            ..
        } => {
            let approval_id = ApprovalRequestId::new();
            let thread = {
                state
                    .read_model
                    .read()
                    .unwrap()
                    .threads
                    .get(&thread_id)
                    .cloned()
            };
            if let Some(thread) = thread
                && let Some(local_turn_id) = resolve_thread_turn(&thread, turn_id)
            {
                let details = serde_json::json!({
                    "file_path": file_path,
                    "diff": diff,
                });
                let _ = dispatch_and_sync(
                    state,
                    engine,
                    Command::RecordApprovalRequest {
                        thread_id,
                        turn_id: local_turn_id,
                        approval_id,
                        rpc_id: *rpc_id,
                        kind: ApprovalKind::FileChange,
                        details,
                    },
                )
                .await;
            }
        }
        CodexEvent::FileReadApprovalRequested {
            rpc_id,
            turn_id,
            file_path,
            ..
        } => {
            let approval_id = ApprovalRequestId::new();
            let thread = {
                state
                    .read_model
                    .read()
                    .unwrap()
                    .threads
                    .get(&thread_id)
                    .cloned()
            };
            if let Some(thread) = thread
                && let Some(local_turn_id) = resolve_thread_turn(&thread, turn_id)
            {
                let details = serde_json::json!({ "file_path": file_path });
                let _ = dispatch_and_sync(
                    state,
                    engine,
                    Command::RecordApprovalRequest {
                        thread_id,
                        turn_id: local_turn_id,
                        approval_id,
                        rpc_id: *rpc_id,
                        kind: ApprovalKind::FileRead,
                        details,
                    },
                )
                .await;
            }
        }
        CodexEvent::UserInputRequested {
            rpc_id,
            turn_id,
            questions,
            ..
        } => {
            let approval_id = ApprovalRequestId::new();
            let thread = {
                state
                    .read_model
                    .read()
                    .unwrap()
                    .threads
                    .get(&thread_id)
                    .cloned()
            };
            if let Some(thread) = thread
                && let Some(local_turn_id) = resolve_thread_turn(&thread, turn_id)
            {
                let _ = dispatch_and_sync(
                    state,
                    engine,
                    Command::RecordUserInputRequest {
                        thread_id,
                        turn_id: local_turn_id,
                        approval_id,
                        rpc_id: *rpc_id,
                        questions: questions.clone(),
                    },
                )
                .await;
            }
        }
        CodexEvent::Error {
            message,
            will_retry,
        } => {
            let _ = dispatch_and_sync(
                state,
                engine,
                Command::RecordError {
                    thread_id,
                    message: message.clone(),
                    will_retry: *will_retry,
                },
            )
            .await;
            state.push_error(format!("Codex error: {}", message));
            if !will_retry {
                let _ = dispatch_and_sync(
                    state,
                    engine,
                    Command::SetSessionStatus {
                        thread_id,
                        status: ProviderSessionStatus::Error,
                        message: Some(message.clone()),
                    },
                )
                .await;
                state.set_status("Error");
            }
        }
        CodexEvent::SessionClosed { reason } => {
            let active_turn = state
                .read_model
                .read()
                .unwrap()
                .threads
                .get(&thread_id)
                .and_then(|thread| thread.active_turn);
            let _ = dispatch_and_sync(
                state,
                engine,
                Command::SetSessionStatus {
                    thread_id,
                    status: if active_turn.is_some() {
                        ProviderSessionStatus::Error
                    } else {
                        ProviderSessionStatus::Stopped
                    },
                    message: Some(reason.clone()),
                },
            )
            .await;
            if let Some(turn_id) = active_turn {
                let _ = dispatch_and_sync(
                    state,
                    engine,
                    Command::CompleteTurn {
                        thread_id,
                        turn_id,
                        status: "session_closed".to_string(),
                    },
                )
                .await;
            }
            state.push_error(format!("Codex session closed: {}", reason));
            state.set_status("Session closed");
        }
        CodexEvent::ThreadStarted { .. } => {}
    }

    repaint();
}

fn handle_terminal_event<F: Fn()>(event: TerminalEvent, state: &Arc<AppState>, repaint: &F) {
    match event {
        TerminalEvent::Output { terminal_id, data } => {
            let mut legacy = state.terminal_buffers.write().unwrap();
            let buf = legacy.entry(terminal_id).or_default();
            buf.push_str(&data);
            if buf.len() > 1_000_000 {
                let drain = buf.len() - 500_000;
                buf.drain(..drain);
            }

            let mut terminals = state.terminals.write().unwrap();
            if let Some(term) = terminals.iter_mut().find(|t| t.id == terminal_id) {
                term.buffer.push_str(&data);
                if term.buffer.len() > 1_000_000 {
                    let drain = term.buffer.len() - 500_000;
                    term.buffer.drain(..drain);
                }
                term.is_alive = true;
            } else {
                let title = format!("Terminal {}", terminals.len() + 1);
                terminals.push(TerminalState {
                    id: terminal_id,
                    thread_id: *state.current_thread.read().unwrap(),
                    title,
                    buffer: data,
                    is_alive: true,
                });
            }
        }
        TerminalEvent::Exited {
            terminal_id,
            exit_code,
        } => {
            let msg = format!("\r\n[Process exited with code {:?}]\r\n", exit_code);

            let mut legacy = state.terminal_buffers.write().unwrap();
            if let Some(buf) = legacy.get_mut(&terminal_id) {
                buf.push_str(&msg);
            }

            let mut terminals = state.terminals.write().unwrap();
            if let Some(term) = terminals.iter_mut().find(|t| t.id == terminal_id) {
                term.buffer.push_str(&msg);
                term.is_alive = false;
            }
        }
        TerminalEvent::Resized { .. } => {}
    }
    repaint();
}

#[cfg(test)]
mod tests {
    use super::{handle_terminal_event, initialize_engine};
    use crate::state::AppState;
    use ecode_contracts::ids::{ProjectId, TerminalId, ThreadId};
    use ecode_contracts::orchestration::{Command, ThreadSettings};
    use ecode_contracts::terminal::TerminalEvent;
    use ecode_core::orchestration::OrchestrationEngine;
    use ecode_core::persistence::EventStore;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn initialize_engine_rebuilds_persisted_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.db");

        {
            let store = Arc::new(EventStore::open(&path).unwrap());
            let engine = OrchestrationEngine::new(store);
            let thread_id = ThreadId::new();
            engine
                .dispatch(Command::CreateThread {
                    thread_id,
                    project_id: ProjectId::new(),
                    name: Some("Persisted".to_string()),
                    settings: ThreadSettings::default(),
                })
                .unwrap();
        }

        let rebuilt = initialize_engine(&path).unwrap();
        let snapshot = rebuilt.snapshot();
        assert_eq!(snapshot.threads.len(), 1);
        assert_eq!(
            snapshot
                .threads
                .values()
                .next()
                .map(|thread| thread.name.as_str()),
            Some("Persisted")
        );
    }

    #[test]
    fn handle_terminal_event_creates_updates_and_exits_terminal_state() {
        let state = Arc::new(AppState::new());
        let thread_id = ThreadId::new();
        let terminal_id = TerminalId::new();
        let repaints = AtomicUsize::new(0);

        *state.current_thread.write().unwrap() = Some(thread_id);

        let repaint = || {
            repaints.fetch_add(1, Ordering::Relaxed);
        };

        handle_terminal_event(
            TerminalEvent::Output {
                terminal_id,
                data: "hello".to_string(),
            },
            &state,
            &repaint,
        );

        handle_terminal_event(
            TerminalEvent::Output {
                terminal_id,
                data: " world".to_string(),
            },
            &state,
            &repaint,
        );

        handle_terminal_event(
            TerminalEvent::Exited {
                terminal_id,
                exit_code: Some(7),
            },
            &state,
            &repaint,
        );

        let terminals = state.terminals.read().unwrap();
        let terminal = terminals
            .iter()
            .find(|terminal| terminal.id == terminal_id)
            .unwrap();
        assert_eq!(terminal.thread_id, Some(thread_id));
        assert_eq!(terminal.title, "Terminal 1");
        assert!(terminal.buffer.starts_with("hello world"));
        assert!(terminal.buffer.contains("[Process exited with code Some(7)]"));
        assert!(!terminal.is_alive);
        drop(terminals);

        let terminal_buffers = state.terminal_buffers.read().unwrap();
        assert_eq!(
            terminal_buffers.get(&terminal_id).map(String::as_str),
            Some("hello world\r\n[Process exited with code Some(7)]\r\n")
        );
        assert_eq!(repaints.load(Ordering::Relaxed), 3);
    }
}

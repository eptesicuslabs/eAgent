#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod dto;
mod events;

use anyhow::Result;
use eagent_persistence::EventStore;
use eagent_providers::api_key::{ApiKeyConfig, ApiKeyProvider};
use eagent_providers::registry::ProviderRegistry;
use eagent_runtime::engine::{RuntimeConfig, RuntimeEngine};
use eagent_tools::registry::ToolRegistry;
use ecode_desktop_app::AppRuntime;
use std::sync::Arc;
use tauri::Manager;
use tokio::runtime::Runtime;
use tracing_subscriber::{EnvFilter, fmt};

pub struct DesktopShellState {
    _runtime: Arc<Runtime>,
    pub app: AppRuntime,
}

/// State for the new eAgent multi-agent runtime, managed alongside legacy state.
pub struct EAgentState {
    pub engine: Arc<RuntimeEngine>,
    pub provider_registry: Arc<ProviderRegistry>,
    pub tool_registry: Arc<ToolRegistry>,
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).try_init();
}

fn run() -> Result<()> {
    init_logging();

    tauri::Builder::default()
        .setup(|app| {
            let runtime = Arc::new(
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| -> Box<dyn std::error::Error> { Box::new(error) })?,
            );
            let app_handle = app.handle().clone();
            let desktop_runtime = AppRuntime::spawn_with_notifier(runtime.as_ref(), {
                let app_handle = app_handle.clone();
                move || {
                    events::emit_domain_event(&app_handle);
                    events::emit_terminal_event(&app_handle);
                }
            });
            let runtime_state = desktop_runtime.state();
            events::emit_status_event(&app_handle, &runtime_state);
            app.manage(DesktopShellState {
                _runtime: runtime.clone(),
                app: desktop_runtime,
            });

            // ----- eAgent runtime setup -----
            let event_store =
                Arc::new(EventStore::in_memory().expect("failed to create event store"));

            let mut tool_registry = ToolRegistry::new();
            eagent_tools::register_builtin_tools(&mut tool_registry);
            let tool_registry = Arc::new(tool_registry);

            let provider_registry = ProviderRegistry::new();

            // Register ApiKeyProvider from environment variables if set
            if let Ok(api_key) = std::env::var("EAGENT_API_KEY") {
                let endpoint = std::env::var("EAGENT_API_ENDPOINT")
                    .unwrap_or_else(|_| "https://api.openai.com/v1".into());
                let model = std::env::var("EAGENT_MODEL")
                    .unwrap_or_else(|_| "gpt-4o".into());
                let config = ApiKeyConfig {
                    endpoint,
                    api_key,
                    default_model: model,
                    ..Default::default()
                };
                provider_registry.register(
                    "api-key".into(),
                    Arc::new(ApiKeyProvider::new(config)),
                );
                tracing::info!("registered ApiKeyProvider from environment");
            }

            let provider_registry = Arc::new(provider_registry);

            let agent_ctx = eagent_protocol::traits::AgentContext {
                workspace_root: runtime_state
                    .current_project
                    .read()
                    .unwrap()
                    .clone()
                    .unwrap_or_else(|| {
                        std::env::current_dir()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|_| "/".into())
                    }),
                project_name: None,
                project_summary: None,
            };

            let rt_config = RuntimeConfig {
                default_provider: "api-key".into(),
                ..Default::default()
            };

            let (engine, event_rx) = RuntimeEngine::new(
                provider_registry.clone(),
                tool_registry.clone(),
                event_store,
                rt_config,
                agent_ctx,
            );
            let engine = Arc::new(engine);

            // Spawn the scheduling loop
            let engine_for_run = engine.clone();
            runtime.spawn(async move { engine_for_run.run().await });

            // Spawn the event bridge (RuntimeEvent → Tauri events)
            let engine_for_bridge = engine.clone();
            let app_handle_for_bridge = app_handle.clone();
            runtime.spawn(events::eagent_event_bridge(
                event_rx,
                engine_for_bridge,
                app_handle_for_bridge,
            ));

            app.manage(EAgentState {
                engine,
                provider_registry,
                tool_registry,
            });

            tracing::info!("eAgent runtime initialized");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_get_bootstrap,
            commands::app::app_pick_folder,
            commands::app::app_open_external,
            commands::app::shell_open_in_editor,
            commands::orchestration::orchestration_get_snapshot,
            commands::orchestration::orchestration_dispatch,
            commands::terminal::terminal_list,
            commands::terminal::terminal_open,
            commands::terminal::terminal_write,
            commands::terminal::terminal_resize,
            commands::terminal::terminal_close,
            commands::terminal::terminal_clear,
            commands::git::git_status,
            commands::git::git_list_branches,
            commands::git::git_diff_workdir,
            commands::git::git_create_worktree,
            commands::git::git_remove_worktree,
            commands::projects::projects_open,
            commands::projects::projects_search_entries,
            commands::projects::projects_write_file,
            commands::settings::settings_get,
            commands::settings::settings_save,
            commands::eagent::eagent_submit_task,
            commands::eagent::eagent_cancel_graph,
            commands::eagent::eagent_get_providers,
            commands::eagent::eagent_approve_oversight,
            commands::eagent::eagent_deny_oversight,
            commands::eagent::eagent_configure_provider
        ])
        .run(tauri::generate_context!())?;

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("failed to start eCode desktop shell: {error}");
    }
}

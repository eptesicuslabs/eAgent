#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod dto;
mod events;

use anyhow::Result;
use ecode_desktop_app::AppRuntime;
use std::sync::Arc;
use tauri::Manager;
use tokio::runtime::Runtime;
use tracing_subscriber::{EnvFilter, fmt};

pub struct DesktopShellState {
    _runtime: Arc<Runtime>,
    pub app: AppRuntime,
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
                _runtime: runtime,
                app: desktop_runtime,
            });
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
            commands::settings::settings_save
        ])
        .run(tauri::generate_context!())?;

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("failed to start eCode desktop shell: {error}");
    }
}

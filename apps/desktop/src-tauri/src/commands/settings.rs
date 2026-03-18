use crate::events;
use crate::DesktopShellState;
use ecode_contracts::config::AppConfig;
use ecode_desktop_app::UiAction;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn settings_get(state: State<'_, DesktopShellState>) -> AppConfig {
    state.app.state().config.read().unwrap().clone()
}

#[tauri::command]
pub fn settings_save(
    config: AppConfig,
    app_handle: AppHandle,
    state: State<'_, DesktopShellState>,
) -> Result<(), String> {
    *state.app.state().config.write().unwrap() = config.clone();

    if let Some(path) = state.app.state().config_path.read().unwrap().clone() {
        let content = toml::to_string_pretty(&config).map_err(|error| error.to_string())?;
        std::fs::write(path, content).map_err(|error| error.to_string())?;
    }

    state
        .app
        .send(UiAction::CheckCodex)
        .map_err(|error| error.to_string())?;
    events::emit_settings_updated(&app_handle);
    Ok(())
}

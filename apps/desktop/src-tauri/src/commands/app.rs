use crate::dto::AppBootstrapPayload;
use crate::DesktopShellState;
use rfd::FileDialog;
use std::process::Command;
use tauri::State;

#[tauri::command]
pub fn app_get_bootstrap(state: State<'_, DesktopShellState>) -> AppBootstrapPayload {
    AppBootstrapPayload::from_state(state.app.state().as_ref())
}

#[tauri::command]
pub fn app_pick_folder() -> Option<String> {
    FileDialog::new()
        .pick_folder()
        .map(|path| path.display().to_string())
}

#[tauri::command]
pub fn shell_open_in_editor(path: String) -> Result<(), String> {
    open_with_system_handler(&path)
}

#[tauri::command]
pub fn app_open_external(url: String) -> Result<(), String> {
    open_with_system_handler(&url)
}

fn open_with_system_handler(target: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", target])
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(target)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Opening files is not supported on this platform".to_string())
}

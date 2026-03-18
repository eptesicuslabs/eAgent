use crate::dto::AppStatusPayload;
use ecode_desktop_app::AppState;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

pub const DOMAIN_EVENT: &str = "ecode://domain-event";
pub const TERMINAL_EVENT: &str = "ecode://terminal-event";
pub const SETTINGS_UPDATED_EVENT: &str = "ecode://settings-updated";
pub const APP_STATUS_EVENT: &str = "ecode://app-status";

pub fn emit_domain_event(app_handle: &AppHandle) {
    let _ = app_handle.emit(DOMAIN_EVENT, ());
}

pub fn emit_terminal_event(app_handle: &AppHandle) {
    let _ = app_handle.emit(TERMINAL_EVENT, ());
}

pub fn emit_settings_updated(app_handle: &AppHandle) {
    let _ = app_handle.emit(SETTINGS_UPDATED_EVENT, ());
}

pub fn emit_status_event(app_handle: &AppHandle, state: &Arc<AppState>) {
    let payload = AppStatusPayload {
        status_message: state.status_message.read().unwrap().clone(),
    };
    let _ = app_handle.emit(APP_STATUS_EVENT, payload);
}

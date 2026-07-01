//! Headroom 压缩代理的生命周期命令。

use crate::services::HeadroomStatus;
use crate::store::AppState;

#[tauri::command]
pub async fn headroom_start(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mgr = state.headroom_manager.clone();
    tauri::async_runtime::spawn_blocking(move || mgr.start())
        .await
        .map_err(|e| e.to_string())?
        .map_err(Into::into)
}

#[tauri::command]
pub async fn headroom_stop(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mgr = state.headroom_manager.clone();
    tauri::async_runtime::spawn_blocking(move || mgr.stop())
        .await
        .map_err(|e| e.to_string())?
        .map_err(Into::into)
}

#[tauri::command]
pub async fn headroom_status(state: tauri::State<'_, AppState>) -> Result<HeadroomStatus, String> {
    let mgr = state.headroom_manager.clone();
    tauri::async_runtime::spawn_blocking(move || mgr.status())
        .await
        .map_err(|e| e.to_string())
}

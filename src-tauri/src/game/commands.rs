use super::{
    discovery,
    model::{DiscoveryReport, GameInstallation, GameSource},
    platform, validation,
};
use crate::error::AppError;
use std::path::Path;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn discover_game_installations() -> Result<DiscoveryReport, AppError> {
    tauri::async_runtime::spawn_blocking(|| Ok(discovery::discover(platform::locations()?)))
        .await
        .map_err(|e| AppError::with_detail("discoveryFailed", e))?
}

#[tauri::command]
pub async fn validate_game_directory(path: String) -> Result<GameInstallation, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        validation::validate(Path::new(&path), GameSource::Manual)
    })
    .await
    .map_err(|e| AppError::with_detail("validationFailed", e))?
}

#[tauri::command]
pub async fn pick_game_directory(
    app: tauri::AppHandle,
) -> Result<Option<GameInstallation>, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app.dialog().file().blocking_pick_folder() else {
            return Ok(None);
        };
        let path = file
            .into_path()
            .map_err(|e| AppError::with_detail("directoryUnreadable", e))?;
        validation::validate(&path, GameSource::Manual).map(Some)
    })
    .await
    .map_err(|e| AppError::with_detail("validationFailed", e))?
}

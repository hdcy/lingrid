use super::{credentials, graphql, model::NexusModSummary, nxm};
use crate::{
    error::AppError,
    game::{model::GameSource, validation},
    mods::{self, model::InstalledModsReport},
};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

/// Where the user's Nexus API key is kept: application data, owner-readable only.
fn key_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("nexus").join("credentials.json"))
        .map_err(|e| AppError::with_detail("nexusKeyNotSaved", e))
}

/// Search Nexus Mods. Anonymous: only downloading needs the user's API key.
#[tauri::command]
pub async fn search_nexus_mods(query: String) -> Result<Vec<NexusModSummary>, AppError> {
    tauri::async_runtime::spawn_blocking(move || graphql::search(&query, 20))
        .await
        .map_err(|e| AppError::with_detail("nexusUnavailable", e))?
}

/// Whether a Nexus API key is stored; the key itself never reaches the UI.
#[tauri::command]
pub async fn nexus_api_key_status(app: AppHandle) -> Result<bool, AppError> {
    tauri::async_runtime::spawn_blocking(move || Ok(credentials::has_key(&key_path(&app)?)))
        .await
        .map_err(|e| AppError::with_detail("nexusKeyNotSaved", e))?
}

/// Store the Nexus API key. An empty value clears it.
#[tauri::command]
pub async fn set_nexus_api_key(app: AppHandle, key: String) -> Result<(), AppError> {
    tauri::async_runtime::spawn_blocking(move || credentials::write(&key_path(&app)?, &key))
        .await
        .map_err(|e| AppError::with_detail("nexusKeyNotSaved", e))?
}

/// Install a mod straight from an `nxm://` link.
///
/// This is the path free Nexus accounts have to take: the link carries the key and expiry
/// the website issued, which the download endpoint requires.
#[tauri::command]
pub async fn install_mod_from_nxm(
    app: AppHandle,
    path: String,
    url: String,
) -> Result<InstalledModsReport, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let installation = validation::validate(Path::new(&path), GameSource::Manual)?;
        let link = nxm::parse(&url)?;
        let key =
            credentials::read(&key_path(&app)?).ok_or_else(|| AppError::new("nexusKeyMissing"))?;
        let download = nxm::resolve(&link, &key)?;

        let workspace =
            tempfile::tempdir().map_err(|e| AppError::with_detail("modInstallFailed", e))?;
        let archive = workspace.path().join("mod.zip");
        nxm::fetch(&download, &archive)?;
        mods::install::install(&installation.path, &archive)
    })
    .await
    .map_err(|e| AppError::with_detail("modInstallFailed", e))?
}

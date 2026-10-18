use super::{
    install, manage,
    model::{InstalledMod, InstalledModsReport},
    scan,
};
use crate::{
    error::AppError,
    game::{model::GameSource, validation},
};
use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

/// Every command resolves the `Mods` folder through the game directory validator, so a
/// stray path can never be used to rename or delete something else.
fn mods_dir(path: &str) -> Result<std::path::PathBuf, AppError> {
    let installation = validation::validate(Path::new(path), GameSource::Manual)?;
    Ok(installation.path.join("Mods"))
}

#[tauri::command]
pub async fn list_mods(path: String) -> Result<Vec<InstalledMod>, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let installation = validation::validate(Path::new(&path), GameSource::Manual)?;
        scan::scan(&installation.path)
    })
    .await
    .map_err(|e| AppError::with_detail("modsUnreadable", e))?
}

/// Turn a mod on or off. SMAPI skips folders whose name starts with a dot.
#[tauri::command]
pub async fn set_mod_enabled(path: String, folder: String, enabled: bool) -> Result<(), AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = mods_dir(&path)?;
        manage::set_enabled(&dir, &folder, enabled).map(|_| ())
    })
    .await
    .map_err(|e| AppError::with_detail("modRenameFailed", e))?
}

#[tauri::command]
pub async fn remove_mod(path: String, folder: String) -> Result<(), AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = mods_dir(&path)?;
        manage::remove(&dir, &folder)
    })
    .await
    .map_err(|e| AppError::with_detail("modRemoveFailed", e))?
}

/// Ask the user for an archive to install from. Mods ship as zips; other formats are
/// rejected when opened, so the filter stays deliberately narrow.
#[tauri::command]
pub async fn pick_mod_archive(app: AppHandle) -> Result<Option<String>, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("Mod archive", &["zip"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = file
            .into_path()
            .map_err(|e| AppError::with_detail("modArchiveUnreadable", e))?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| AppError::with_detail("modArchiveUnreadable", e))?
}

/// Install every mod found in an archive into the game's `Mods` folder.
#[tauri::command]
pub async fn install_mod(path: String, archive: String) -> Result<InstalledModsReport, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let installation = validation::validate(Path::new(&path), GameSource::Manual)?;
        install::install(&installation.path, Path::new(&archive))
    })
    .await
    .map_err(|e| AppError::with_detail("modInstallFailed", e))?
}

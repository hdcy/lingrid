use super::{
    archive, install, logs,
    model::{
        InstallReport, SmapiInstallOutcome, SmapiRelease, SmapiStatus, SmapiUninstallReport,
        UninstallOptions,
    },
    progress, records, release,
};
use crate::{
    error::AppError,
    game::{
        model::{GameRuntime, GameSource},
        validation,
    },
};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

fn record_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("smapi").join("installed.json"))
        .map_err(|e| AppError::with_detail("smapiRecordFailed", e))
}

fn cache_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_cache_dir()
        .map(|dir| dir.join("smapi"))
        .map_err(|e| AppError::with_detail("smapiRecordFailed", e))
}

/// Payload folder inside the official installer for the game's own platform.
fn payload_variant(runtime: GameRuntime) -> &'static str {
    match runtime {
        GameRuntime::Windows => "windows",
        GameRuntime::Linux => "linux",
    }
}

#[tauri::command]
pub async fn smapi_status(app: AppHandle, path: String) -> Result<SmapiStatus, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let installation = validation::validate(Path::new(&path), GameSource::Manual)?;
        let mut status = install::detect(&installation.path, installation.runtime)?;
        // A manual install has no recorded version; only our own installs are versioned.
        if status.installed {
            status.version = records::version_of(&record_path(&app)?, &installation.path);
        }
        Ok(status)
    })
    .await
    .map_err(|e| AppError::with_detail("validationFailed", e))?
}

#[tauri::command]
pub async fn fetch_smapi_release() -> Result<SmapiRelease, AppError> {
    tauri::async_runtime::spawn_blocking(release::fetch_latest)
        .await
        .map_err(|e| AppError::with_detail("smapiReleaseUnavailable", e))?
}

/// Read the log SMAPI rewrites every time the game runs. A missing log is not an error:
/// it only means the game has not been started through SMAPI yet.
#[tauri::command]
pub async fn smapi_log() -> Result<logs::SmapiLog, AppError> {
    tauri::async_runtime::spawn_blocking(logs::read_latest)
        .await
        .map_err(|e| AppError::with_detail("smapiLogUnreadable", e))?
}

/// Download the latest SMAPI release and install it into the selected game directory.
///
/// Each stage reports progress to the UI, so a long download is observable and a failure
/// points at the stage it happened in. macOS is not supported: the payload would be the
/// Linux build, which is not what a native macOS install needs.
#[tauri::command]
pub async fn install_smapi(app: AppHandle, path: String) -> Result<SmapiInstallOutcome, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        if cfg!(target_os = "macos") {
            return Err(AppError::new("smapiUnsupportedPlatform"));
        }
        let reporter = progress::Reporter::new(&app);
        let installation = validation::validate(Path::new(&path), GameSource::Manual)?;
        let runtime = installation.runtime;

        let latest = reporter.run(progress::Step::Release, release::fetch_latest)?;

        let cache = cache_dir(&app)?;
        let cached = release::cached_path(&cache, &latest);
        let archive_path = if cached.is_file() {
            // A cached archive is still verified: it may have been truncated or edited.
            reporter.skipped(progress::Step::Download, "cached");
            reporter.run(progress::Step::Verify, || {
                release::verify(&cached, latest.sha256.as_deref())
            })?;
            cached
        } else {
            let path = reporter.run(progress::Step::Download, || {
                release::download(&cache, &latest, |downloaded, total| {
                    reporter.downloading(downloaded, total);
                })
            })?;
            reporter.run(progress::Step::Verify, || {
                release::verify(&path, latest.sha256.as_deref())
            })?;
            path
        };

        let workspace =
            tempfile::tempdir().map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;
        let bundle = workspace.path().join("bundle");
        reporter.run(progress::Step::Extract, || -> Result<(), AppError> {
            let payload = archive::read_entry_by_suffix(
                &archive_path,
                &format!("internal/{}/install.dat", payload_variant(runtime)),
            )?;
            archive::extract_bytes(&payload, &bundle)
        })?;

        let InstallReport { outcome, created } = reporter.run(progress::Step::Install, || {
            install::install(&installation.path, &bundle, runtime, &latest.version)
        })?;
        records::remember(
            &record_path(&app)?,
            &installation.path,
            &latest.version,
            created,
        )?;
        Ok(outcome)
    })
    .await
    .map_err(|e| AppError::with_detail("smapiInstallFailed", e))?
}

/// Remove SMAPI. `remove_mods` additionally clears the whole `Mods` folder, including
/// mods the user installed themselves.
#[tauri::command]
pub async fn uninstall_smapi(
    app: AppHandle,
    path: String,
    remove_mods: bool,
) -> Result<SmapiUninstallReport, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let reporter = progress::Reporter::new(&app);
        let installation = validation::validate(Path::new(&path), GameSource::Manual)?;
        let record = record_path(&app)?;
        let options = UninstallOptions {
            remove_mods,
            created: records::created_of(&record, &installation.path),
        };
        let report = reporter.run(progress::Step::Uninstall, || {
            install::uninstall(&installation.path, installation.runtime, &options)
        })?;
        records::forget(&record, &installation.path)?;
        Ok(report)
    })
    .await
    .map_err(|e| AppError::with_detail("smapiInstallFailed", e))?
}

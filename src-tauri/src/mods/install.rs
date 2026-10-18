use super::{model::InstalledModsReport, scan};
use crate::error::AppError;
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use zip::ZipArchive;

/// Install every mod found in an archive into the game's `Mods` folder.
///
/// An archive usually holds one mod, but packs with several mods exist, and the content
/// is often wrapped in an extra directory. Each mod is located by its `manifest.json`.
///
/// Every manifest is validated and checked against the mods already installed before any
/// file is written: SMAPI refuses to load a mod with a broken manifest, and it aborts the
/// whole load when two folders declare the same `UniqueID`.
pub fn install(game_dir: &Path, archive: &Path) -> Result<InstalledModsReport, AppError> {
    let mods_dir = game_dir.join("Mods");
    fs::create_dir_all(&mods_dir).map_err(|e| AppError::with_detail("modsUnreadable", e))?;
    let workspace =
        tempfile::tempdir().map_err(|e| AppError::with_detail("modInstallFailed", e))?;
    extract(archive, workspace.path())?;

    let roots = find_mod_roots(workspace.path())?;
    if roots.is_empty() {
        return Err(AppError::new("modArchiveInvalid"));
    }

    // Nothing is written until every target has been checked, so a half-install cannot
    // happen because the third of five mods collided.
    let installed: BTreeSet<String> = scan::scan(game_dir)?
        .into_iter()
        .filter_map(|installed| installed.unique_id)
        .map(|id| id.to_lowercase())
        .collect();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut planned = Vec::new();
    for root in roots {
        let unique_id = manifest_unique_id(&root)?;
        let folder = folder_name(&root, workspace.path(), archive)?;
        let dest = mods_dir.join(&folder);
        // A same-named folder is the more actionable problem ("remove the old version").
        if dest.exists() {
            return Err(AppError::with_detail("modFolderTaken", folder));
        }
        let key = unique_id.to_lowercase();
        if installed.contains(&key) || !seen.insert(key) {
            return Err(AppError::with_detail("modDuplicate", unique_id));
        }
        planned.push((root, folder, dest));
    }
    let mut folders = Vec::new();
    for (root, folder, dest) in planned {
        copy_entry(&root, &dest)?;
        folders.push(folder);
    }
    Ok(InstalledModsReport { folders })
}

/// Read a mod's `manifest.json` and check the fields SMAPI requires.
///
/// Field names are matched case-insensitively because manifests in the wild spell them
/// inconsistently; `Version` may be a string or an object.
fn manifest_unique_id(root: &Path) -> Result<String, AppError> {
    let path = root.join("manifest.json");
    let text =
        fs::read_to_string(&path).map_err(|e| AppError::with_detail("modManifestInvalid", e))?;
    let value: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| AppError::with_detail("modManifestInvalid", e))?;
    let object = value
        .as_object()
        .ok_or_else(|| AppError::with_detail("modManifestInvalid", path.display()))?;
    for field in ["Name", "Author", "UniqueID"] {
        let present = object
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(field))
            .and_then(|(_, value)| value.as_str())
            .is_some_and(|value| !value.trim().is_empty());
        if !present {
            return Err(AppError::with_detail(
                "modManifestInvalid",
                format!("{} has no {field}", path.display()),
            ));
        }
    }
    let version = object
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("Version"))
        .map(|(_, value)| value);
    match version {
        Some(Value::String(text)) if !text.trim().is_empty() => {}
        Some(Value::Object(_)) => {}
        _ => {
            return Err(AppError::with_detail(
                "modManifestInvalid",
                format!("{} has no Version", path.display()),
            ));
        }
    }
    object
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("UniqueID"))
        .and_then(|(_, value)| value.as_str())
        .map(|id| id.trim().to_owned())
        .ok_or_else(|| AppError::with_detail("modManifestInvalid", path.display()))
}

/// Copy a folder tree out of the temporary extraction into `Mods`.
fn copy_entry(source: &Path, dest: &Path) -> Result<(), AppError> {
    let failure = |e: std::io::Error| AppError::with_detail("modInstallFailed", e);
    if source.is_dir() {
        fs::create_dir_all(dest).map_err(failure)?;
        for entry in fs::read_dir(source).map_err(failure)? {
            let entry = entry.map_err(failure)?;
            copy_entry(&entry.path(), &dest.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(failure)?;
        }
        fs::copy(source, dest).map_err(failure)?;
        Ok(())
    }
}

/// Extract an archive, rejecting entries that escape the destination or are symlinks.
fn extract(archive: &Path, dest: &Path) -> Result<(), AppError> {
    let file =
        fs::File::open(archive).map_err(|e| AppError::with_detail("modArchiveUnreadable", e))?;
    let mut zip =
        ZipArchive::new(file).map_err(|e| AppError::with_detail("modArchiveInvalid", e))?;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|e| AppError::with_detail("modArchiveInvalid", e))?;
        let Some(relative) = entry.enclosed_name() else {
            return Err(AppError::with_detail(
                "modArchiveInvalid",
                format!("unsafe path {}", entry.name()),
            ));
        };
        if entry.is_symlink() {
            return Err(AppError::with_detail(
                "modArchiveInvalid",
                format!("unexpected symlink {}", entry.name()),
            ));
        }
        let target = dest.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target)
                .map_err(|e| AppError::with_detail("modInstallFailed", e))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| AppError::with_detail("modInstallFailed", e))?;
        }
        let mut file =
            fs::File::create(&target).map_err(|e| AppError::with_detail("modInstallFailed", e))?;
        let mut buffer = Vec::new();
        entry
            .read_to_end(&mut buffer)
            .map_err(|e| AppError::with_detail("modArchiveInvalid", e))?;
        std::io::Write::write_all(&mut file, &buffer)
            .map_err(|e| AppError::with_detail("modInstallFailed", e))?;
    }
    Ok(())
}

/// Directories holding a `manifest.json`, looking at most two levels deep.
fn find_mod_roots(dir: &Path) -> Result<Vec<PathBuf>, AppError> {
    if dir.join("manifest.json").is_file() {
        return Ok(vec![dir.to_path_buf()]);
    }
    let failure = |e: std::io::Error| AppError::with_detail("modArchiveInvalid", e);
    let mut roots = Vec::new();
    for entry in fs::read_dir(dir).map_err(failure)? {
        let entry = entry.map_err(failure)?;
        if !entry.file_type().map_err(failure)?.is_dir() {
            continue;
        }
        let path = entry.path();
        if path.join("manifest.json").is_file() {
            roots.push(path);
            continue;
        }
        for nested in fs::read_dir(&path).map_err(failure)? {
            let nested = nested.map_err(failure)?;
            if nested.file_type().map_err(failure)?.is_dir()
                && nested.path().join("manifest.json").is_file()
            {
                roots.push(nested.path());
            }
        }
    }
    roots.sort();
    Ok(roots)
}

/// Folder to create inside `Mods`.
///
/// The archive's own directory name wins, so a pack keeps the layout its author chose;
/// when the mod sits at the archive root, the file name is used instead. Nexus file names
/// carry versions and dots, so the result is sanitised into a plain, enabled name.
fn folder_name(root: &Path, workspace: &Path, archive: &Path) -> Result<String, AppError> {
    let from_archive = root
        .strip_prefix(workspace)
        .ok()
        .and_then(|relative| relative.components().next())
        .and_then(|part| part.as_os_str().to_str())
        .map(str::to_owned);
    let fallback = archive
        .file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    sanitize(from_archive.as_deref().unwrap_or(&fallback))
        .ok_or_else(|| AppError::with_detail("modArchiveInvalid", "unnamed mod"))
}

/// Turn an archive name into a folder name SMAPI will load.
fn sanitize(name: &str) -> Option<String> {
    let cleaned: String = name
        .chars()
        .map(|c| if r#"/\:*?"<>|"#.contains(c) { '_' } else { c })
        .collect();
    // A leading dot would disable the mod the moment it is installed.
    let trimmed = cleaned.trim().trim_start_matches('.').trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

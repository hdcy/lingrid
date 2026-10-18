use crate::error::AppError;
use std::{
    fs,
    path::{Component, Path},
};

/// Enable or disable a mod by renaming its folder.
///
/// SMAPI ignores folders whose name starts with a dot, so that is the switch. `folder`
/// is accepted with or without the dot. Returns the folder name that is now in use.
pub fn set_enabled(mods_dir: &Path, folder: &str, enabled: bool) -> Result<String, AppError> {
    let folder = folder.trim_start_matches('.');
    if !is_simple_name(folder) {
        return Err(AppError::with_detail("modFolderInvalid", folder));
    }
    let active = mods_dir.join(folder);
    let disabled = mods_dir.join(format!(".{folder}"));
    let (from, to) = if enabled {
        (&disabled, &active)
    } else {
        (&active, &disabled)
    };
    if !from.is_dir() {
        return Err(AppError::with_detail(
            "modNotFound",
            from.to_string_lossy().into_owned(),
        ));
    }
    // Never clobber whatever already sits at the target name.
    if to.exists() {
        return Err(AppError::with_detail(
            "modFolderTaken",
            to.to_string_lossy().into_owned(),
        ));
    }
    fs::rename(from, to).map_err(|e| AppError::with_detail("modRenameFailed", e))?;
    Ok(folder.to_owned())
}

/// Delete a mod folder, active or disabled.
pub fn remove(mods_dir: &Path, folder: &str) -> Result<(), AppError> {
    let folder = folder.trim_start_matches('.');
    if !is_simple_name(folder) {
        return Err(AppError::with_detail("modFolderInvalid", folder));
    }
    let active = mods_dir.join(folder);
    let disabled = mods_dir.join(format!(".{folder}"));
    let target = if active.is_dir() {
        active
    } else if disabled.is_dir() {
        disabled
    } else {
        return Err(AppError::with_detail("modNotFound", folder));
    };
    fs::remove_dir_all(&target).map_err(|e| AppError::with_detail("modRemoveFailed", e))
}

/// Reject anything that could reach outside the `Mods` folder.
fn is_simple_name(folder: &str) -> bool {
    if folder.is_empty() || folder == "." || folder == ".." {
        return false;
    }
    let mut parts = Path::new(folder).components();
    matches!(parts.next(), Some(Component::Normal(_))) && parts.next().is_none()
}

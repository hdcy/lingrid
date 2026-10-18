use crate::error::AppError;
use std::{
    fs,
    io::{Cursor, Read},
    path::{Component, Path, PathBuf},
};
use zip::ZipArchive;

/// Read one entry whose archive path ends with `suffix` into memory.
///
/// The installer archive nests everything under a versioned folder, so entries are
/// located by suffix instead of an exact name.
pub fn read_entry_by_suffix(path: &Path, suffix: &str) -> Result<Vec<u8>, AppError> {
    let file =
        fs::File::open(path).map_err(|e| AppError::with_detail("smapiArchiveUnreadable", e))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| AppError::with_detail("smapiArchiveInvalid", e))?;
    let mut found = None;
    for index in 0..archive.len() {
        let name = archive
            .by_index(index)
            .map_err(|e| AppError::with_detail("smapiArchiveInvalid", e))?
            .name()
            .to_owned();
        if name.ends_with(suffix) {
            found = Some(name);
            break;
        }
    }
    let name = found.ok_or_else(|| {
        AppError::with_detail("smapiArchiveInvalid", format!("missing entry {suffix}"))
    })?;
    let mut entry = archive
        .by_name(&name)
        .map_err(|e| AppError::with_detail("smapiArchiveInvalid", e))?;
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut bytes)
        .map_err(|e| AppError::with_detail("smapiArchiveInvalid", e))?;
    Ok(bytes)
}

/// Extract an in-memory zip archive, rejecting entries that escape the destination.
pub fn extract_bytes(bytes: &[u8], dest: &Path) -> Result<(), AppError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| AppError::with_detail("smapiArchiveInvalid", e))?;
    fs::create_dir_all(dest).map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| AppError::with_detail("smapiArchiveInvalid", e))?;
        // `enclosed_name` rejects absolute paths and `..` traversal.
        let Some(relative) = entry.enclosed_name() else {
            return Err(AppError::with_detail(
                "smapiArchiveInvalid",
                format!("unsafe path {}", entry.name()),
            ));
        };
        if entry.is_symlink() {
            return Err(AppError::with_detail(
                "smapiArchiveInvalid",
                format!("unexpected symlink {}", entry.name()),
            ));
        }
        let target = dest.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target)
                .map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;
        }
        let mut file = fs::File::create(&target)
            .map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;
        std::io::copy(&mut entry, &mut file)
            .map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&target, fs::Permissions::from_mode(mode & 0o777));
        }
    }
    Ok(())
}

/// Top-level entries of an extracted bundle, excluding ones the official installer
/// treats specially (`Mods` is installed separately, `mcs` is a macOS symlink).
pub fn bundle_entries(bundle: &Path) -> Result<Vec<PathBuf>, AppError> {
    let mut names = Vec::new();
    for entry in
        fs::read_dir(bundle).map_err(|e| AppError::with_detail("smapiArchiveInvalid", e))?
    {
        let entry = entry.map_err(|e| AppError::with_detail("smapiArchiveInvalid", e))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if matches!(name.as_ref(), "Mods" | "mcs") {
            continue;
        }
        names.push(PathBuf::from(name.as_ref()));
    }
    names.sort();
    Ok(names)
}

/// Whether a path is safe to join onto the game directory (no traversal, not absolute).
pub fn is_simple_name(path: &Path) -> bool {
    let mut components = path.components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

use super::{
    archive,
    model::{
        InstallReport, SmapiInstallOutcome, SmapiStatus, SmapiUninstallReport, UninstallOptions,
    },
};
use crate::{
    error::AppError,
    game::{
        model::{GameRuntime, GameSource},
        validation,
    },
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};

/// Bundled mods the official installer is willing to ship with SMAPI.
const BUNDLED_MOD_IDS: [&str; 2] = ["SMAPI.SaveBackup", "SMAPI.ConsoleCommands"];

const DEPS_JSON: &str = "Stardew Valley.deps.json";
const SMAPI_DEPS_JSON: &str = "StardewModdingAPI.deps.json";
const LAUNCHER: &str = "StardewValley";
const LAUNCHER_BACKUP: &str = "StardewValley-original";
const LAUNCHER_SCRIPT: &str = "unix-launcher.sh";
const MODS: &str = "Mods";
const USER_CONFIG: &str = "smapi-internal/config.user.json";

/// Files this application writes when installing SMAPI.
///
/// Narrower than the official uninstall list on purpose: the official installer also
/// removes legacy leftovers (`0Harmony.dll`, `Newtonsoft.Json.dll`, `steam_appid.txt`, …)
/// whose names collide with files the game itself ships. Deleting those risks damaging a
/// working installation, so they are reported rather than touched.
const MANAGED_FILES: [&str; 10] = [
    "StardewModdingAPI",
    "StardewModdingAPI.deps.json",
    "StardewModdingAPI.dll",
    "StardewModdingAPI.exe",
    "StardewModdingAPI.exe.config",
    "StardewModdingAPI.exe.mdb",
    "StardewModdingAPI.pdb",
    "StardewModdingAPI.runtimeconfig.json",
    "StardewModdingAPI.xml",
    "unix-launcher.sh",
];

/// Directories this application writes; `Mods` itself is never removed, only entries in it.
const MANAGED_DIRS: [&str; 2] = ["smapi-internal", "Mods/.cache"];

fn executable_name(runtime: GameRuntime) -> &'static str {
    match runtime {
        GameRuntime::Windows => "StardewModdingAPI.exe",
        GameRuntime::Linux => "StardewModdingAPI",
    }
}

/// Report whether SMAPI is present. The version is filled in by the caller from its records.
pub fn detect(game_dir: &Path, runtime: GameRuntime) -> Result<SmapiStatus, AppError> {
    let executable = game_dir.join(executable_name(runtime));
    let installed = executable.is_file();
    Ok(SmapiStatus {
        installed,
        version: None,
        executable: installed.then_some(executable),
    })
}

/// Copy an already extracted SMAPI bundle into the game directory.
///
/// The file set mirrors the official installer flow: back up user settings, remove files
/// from a previous install, copy the bundle, replace the launcher on Linux, derive
/// `StardewModdingAPI.deps.json` from the game's own deps file, then add bundled mods.
///
/// Alongside the outcome it reports which paths did not exist before, so a later
/// uninstall can remove every trace without touching files the game shipped.
pub fn install(
    game_dir: &Path,
    bundle_dir: &Path,
    runtime: GameRuntime,
    version: &str,
) -> Result<InstallReport, AppError> {
    let game_dir =
        fs::canonicalize(game_dir).map_err(|e| AppError::with_detail("directoryUnreadable", e))?;
    // Refuse to touch anything that is not a recognisable game directory.
    validation::validate(&game_dir, GameSource::Manual)?;

    let deps = game_dir.join(DEPS_JSON);
    if !deps.is_file() {
        return Err(AppError::with_detail(
            "smapiGameFilesMissing",
            format!("missing {}", deps.display()),
        ));
    }

    let unix = runtime == GameRuntime::Linux;
    let entries = archive::bundle_entries(bundle_dir)?;
    for name in &entries {
        if !archive::is_simple_name(name) {
            return Err(AppError::with_detail(
                "smapiArchiveInvalid",
                format!("unexpected bundle entry {}", name.display()),
            ));
        }
    }
    let bundled_mods = bundled_mod_dirs(bundle_dir)?;
    let user_config = fs::read(game_dir.join(USER_CONFIG)).ok();

    // Everything the install can touch is backed up first so a failure can be undone.
    let mut targets: BTreeSet<PathBuf> = BTreeSet::new();
    for name in MANAGED_FILES {
        targets.insert(game_dir.join(name));
    }
    for name in MANAGED_DIRS {
        targets.insert(game_dir.join(name));
    }
    for name in &entries {
        targets.insert(game_dir.join(name));
    }
    targets.insert(game_dir.join(SMAPI_DEPS_JSON));
    for name in &bundled_mods {
        targets.insert(game_dir.join(MODS).join(name));
    }
    // The launcher pair is replaced and restored by `replace_launcher`/`restore_launcher`,
    // so it is deliberately not part of the generic backup.
    let targets: Vec<PathBuf> = targets.into_iter().collect();
    // The Mods folder holds user mods: track only whether it existed, never its contents.
    let mods_existed = game_dir.join(MODS).is_dir();
    let created: Vec<String> = targets
        .iter()
        .filter(|path| !path.exists())
        .filter_map(|path| path.strip_prefix(&game_dir).ok())
        .filter(|relative| !matches!(relative.to_str(), Some(LAUNCHER) | Some(LAUNCHER_BACKUP)))
        .map(|relative| relative.to_string_lossy().into_owned())
        .collect();

    // Whether SMAPI was already installed: in that case the launcher already points at
    // SMAPI and a rollback must keep it that way instead of restoring the game launcher.
    let launcher_backup_existed = unix && game_dir.join(LAUNCHER_BACKUP).is_file();
    let backup = Backup::create(&game_dir, &targets)?;
    let mut launcher_touched = false;
    let result = apply(
        &game_dir,
        bundle_dir,
        &entries,
        &bundled_mods,
        unix,
        user_config,
        &mut launcher_touched,
    );
    match result {
        Ok(()) => {
            backup.discard();
            Ok(InstallReport {
                outcome: SmapiInstallOutcome {
                    version: version.to_owned(),
                    executable: game_dir.join(executable_name(runtime)),
                    launcher_replaced: unix,
                },
                created,
            })
        }
        Err(error) => {
            // Put the launcher back first: the backup below never contains it.
            if launcher_touched {
                let _ = rollback_launcher(&game_dir, launcher_backup_existed);
            }
            match backup.rollback() {
                Ok(()) => {
                    if !mods_existed {
                        let _ = fs::remove_dir(game_dir.join(MODS));
                    }
                    Err(error)
                }
                Err(rollback) => Err(AppError::with_detail(
                    error.code,
                    format!(
                        "{} (rollback incomplete: {})",
                        detail_of(&error),
                        detail_of(&rollback)
                    ),
                )),
            }
        }
    }
}

fn detail_of(error: &AppError) -> String {
    error.detail.clone().unwrap_or_default()
}

/// Remove SMAPI from the game directory, restoring the original launcher on Linux.
///
/// Beyond the files this application manages, it removes the bundled mods it installed
/// and the paths recorded at install time. The user's own mods are only removed when
/// `options.remove_mods` is set.
pub fn uninstall(
    game_dir: &Path,
    runtime: GameRuntime,
    options: &UninstallOptions,
) -> Result<SmapiUninstallReport, AppError> {
    let game_dir =
        fs::canonicalize(game_dir).map_err(|e| AppError::with_detail("directoryUnreadable", e))?;
    if !detect(&game_dir, runtime)?.installed {
        return Err(AppError::new("smapiNotInstalled"));
    }

    let launcher_restored = restore_launcher(&game_dir)?;
    let mut removed: u32 = 0;
    for name in MANAGED_FILES {
        removed += u32::from(remove_path(&game_dir.join(name))?);
    }
    for name in MANAGED_DIRS {
        removed += u32::from(remove_path(&game_dir.join(name))?);
    }
    // Bundled mods are recognised by their manifest ID, so a mod the user installed
    // under a similar folder name is left alone.
    for name in mods_with_ids(&game_dir.join(MODS), &BUNDLED_MOD_IDS, "smapiInstallFailed")? {
        removed += u32::from(remove_path(&game_dir.join(MODS).join(name))?);
    }
    // Paths that exist only because of the install, such as a `steam_appid.txt` the game
    // did not ship itself.
    for relative in &options.created {
        if is_safe_relative(relative) {
            removed += u32::from(remove_created(&game_dir, relative)?);
        }
    }

    let mods_removed = if options.remove_mods {
        clear_mods(&game_dir.join(MODS))?
    } else {
        0
    };
    Ok(SmapiUninstallReport {
        removed,
        mods_removed,
        launcher_restored,
    })
}

fn apply(
    game_dir: &Path,
    bundle_dir: &Path,
    entries: &[PathBuf],
    bundled_mods: &[String],
    unix: bool,
    user_config: Option<Vec<u8>>,
    launcher_touched: &mut bool,
) -> Result<(), AppError> {
    // Start from a clean slate so upgrades never leave files from an older SMAPI behind.
    for name in MANAGED_FILES {
        remove_path(&game_dir.join(name))?;
    }
    for name in MANAGED_DIRS {
        remove_path(&game_dir.join(name))?;
    }

    for name in entries {
        let source = bundle_dir.join(name);
        let dest = game_dir.join(name);
        remove_path(&dest)?;
        copy_entry(&source, &dest)?;
    }

    if unix {
        // Flagged before the replacement starts, so a failure half-way through still makes
        // the caller restore the original launcher.
        *launcher_touched = true;
        replace_launcher(game_dir)?;
    }

    // SMAPI resolves native libraries through a copy of the game's own deps file.
    fs::copy(game_dir.join(DEPS_JSON), game_dir.join(SMAPI_DEPS_JSON))
        .map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;

    if let Some(contents) = user_config {
        let path = game_dir.join(USER_CONFIG);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;
        }
        fs::write(&path, contents).map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;
    }

    fs::create_dir_all(game_dir.join(MODS))
        .map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;
    for name in bundled_mods {
        let dest = game_dir.join(MODS).join(name);
        remove_path(&dest)?;
        copy_entry(&bundle_dir.join(MODS).join(name), &dest)?;
    }
    Ok(())
}

/// Point the game launcher at SMAPI, keeping the original as `StardewValley-original`.
fn replace_launcher(game_dir: &Path) -> Result<(), AppError> {
    let launcher = game_dir.join(LAUNCHER);
    let backup = game_dir.join(LAUNCHER_BACKUP);
    let script = game_dir.join(LAUNCHER_SCRIPT);
    if !script.is_file() {
        return Err(AppError::with_detail(
            "smapiArchiveInvalid",
            format!("missing {LAUNCHER_SCRIPT}"),
        ));
    }
    if launcher.exists() {
        if backup.exists() {
            // Already installed before; the existing backup stays authoritative.
            remove_path(&launcher)?;
        } else {
            fs::rename(&launcher, &backup)
                .map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;
        }
    }
    fs::rename(&script, &launcher).map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [&launcher, &game_dir.join("StardewModdingAPI")] {
            if path.is_file() {
                fs::set_permissions(path, fs::Permissions::from_mode(0o755))
                    .map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;
            }
        }
    }
    Ok(())
}

/// Undo a launcher replacement that may have been interrupted anywhere.
///
/// `had_original` says whether `StardewValley-original` already existed before this
/// install, i.e. SMAPI was already installed and the launcher already pointed at it.
fn rollback_launcher(game_dir: &Path, had_original: bool) -> Result<(), AppError> {
    let launcher = game_dir.join(LAUNCHER);
    if !had_original {
        restore_launcher(game_dir)?;
        return Ok(());
    }
    // A reinstall: the previous launcher was SMAPI's script. If the replacement did not
    // get as far as moving it into place, put the script there instead of leaving no
    // launcher at all.
    if !launcher.exists() {
        let script = game_dir.join(LAUNCHER_SCRIPT);
        if script.is_file() {
            fs::rename(&script, &launcher)
                .map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;
        }
    }
    Ok(())
}

/// Undo `replace_launcher`, restoring the game's own launcher when a backup exists.
fn restore_launcher(game_dir: &Path) -> Result<bool, AppError> {
    let launcher = game_dir.join(LAUNCHER);
    let backup = game_dir.join(LAUNCHER_BACKUP);
    if !backup.is_file() {
        return Ok(false);
    }
    remove_path(&launcher)?;
    fs::rename(&backup, &launcher).map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;
    Ok(true)
}

/// Bundled mod folders from the installer's `Mods/`.
fn bundled_mod_dirs(bundle_dir: &Path) -> Result<Vec<String>, AppError> {
    mods_with_ids(
        &bundle_dir.join(MODS),
        &BUNDLED_MOD_IDS,
        "smapiArchiveInvalid",
    )
}

/// Folder names inside `dir` whose manifest declares one of `ids`.
fn mods_with_ids(dir: &Path, ids: &[&str], code: &'static str) -> Result<Vec<String>, AppError> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let failure = |e: std::io::Error| AppError::with_detail(code, e);
    let mut names = Vec::new();
    for entry in fs::read_dir(dir).map_err(failure)? {
        let entry = entry.map_err(failure)?;
        if !entry.file_type().map_err(failure)?.is_dir() {
            continue;
        }
        let id = read_json(&entry.path().join("manifest.json")).and_then(|value| {
            let object = value.as_object()?;
            // SMAPI manifests spell this `UniqueId`; older ones used `UniqueID`.
            object
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case("uniqueid"))
                .and_then(|(_, id)| id.as_str())
                .map(str::to_owned)
        });
        if id.is_some_and(|id| ids.contains(&id.as_str())) {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    Ok(names)
}

/// Delete one recorded path.
///
/// Directories are removed only when empty: content added after the install (for example
/// the user's own mods) is never deleted by this path.
fn remove_created(game_dir: &Path, relative: &str) -> Result<bool, AppError> {
    let path = game_dir.join(relative);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(AppError::with_detail("smapiInstallFailed", error)),
        Ok(metadata) if metadata.is_dir() => Ok(fs::remove_dir(&path).is_ok()),
        Ok(_) => {
            fs::remove_file(&path).map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;
            Ok(true)
        }
    }
}

/// Remove every entry inside `Mods`; the folder itself is kept.
fn clear_mods(mods: &Path) -> Result<u32, AppError> {
    if !mods.is_dir() {
        return Ok(0);
    }
    let failure = |e: std::io::Error| AppError::with_detail("smapiInstallFailed", e);
    let mut removed = 0;
    for entry in fs::read_dir(mods).map_err(failure)? {
        let entry = entry.map_err(failure)?;
        if remove_path(&entry.path())? {
            removed += 1;
        }
    }
    Ok(removed)
}

/// Whether a recorded path stays inside the game directory.
fn is_safe_relative(relative: &str) -> bool {
    let path = Path::new(relative);
    !path.is_absolute()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

/// Read a JSON file, tolerating a byte order mark.
fn read_json(path: &Path) -> Option<serde_json::Value> {
    let text = String::from_utf8(fs::read(path).ok()?).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

fn copy_entry(source: &Path, dest: &Path) -> Result<(), AppError> {
    let failure = |e: std::io::Error| AppError::with_detail("smapiInstallFailed", e);
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

/// Remove a path if it is there; reports whether anything was removed.
fn remove_path(path: &Path) -> Result<bool, AppError> {
    let failure = |e: std::io::Error| AppError::with_detail("smapiInstallFailed", e);
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(path).map_err(failure)?;
            Ok(true)
        }
        Ok(_) => {
            fs::remove_file(path).map_err(failure)?;
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(failure(e)),
    }
}

/// The paths an install may overwrite, moved aside so a failed attempt can be undone.
///
/// Targets are *renamed* into a scratch folder inside the game directory: that is a cheap
/// metadata operation even for a large `Mods/SMAPI.SaveBackup`, and staying on the same
/// filesystem is what makes the rename atomic. A copy-based snapshot doubled the size of
/// the largest targets and could fail on a nearly full disk before the install even ran.
struct Backup {
    dir: PathBuf,
    entries: Vec<(PathBuf, BackupKind)>,
}

enum BackupKind {
    Missing,
    Moved,
}

impl Backup {
    fn create(game_dir: &Path, targets: &[PathBuf]) -> Result<Self, AppError> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let dir = game_dir.join(format!(".lingrid-install-{stamp}"));
        fs::create_dir(&dir).map_err(|e| AppError::with_detail("smapiInstallFailed", e))?;
        let mut backup = Self {
            dir,
            entries: Vec::new(),
        };
        for (index, target) in targets.iter().enumerate() {
            let stored = backup.dir.join(index.to_string());
            match fs::symlink_metadata(target) {
                Ok(_) => {
                    if let Err(error) = fs::rename(target, &stored) {
                        // Put back whatever was already moved before giving up.
                        let _ = backup.rollback();
                        return Err(AppError::with_detail("smapiInstallFailed", error));
                    }
                    backup.entries.push((target.clone(), BackupKind::Moved));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    backup.entries.push((target.clone(), BackupKind::Missing));
                }
                Err(error) => {
                    let _ = backup.rollback();
                    return Err(AppError::with_detail("smapiInstallFailed", error));
                }
            }
        }
        Ok(backup)
    }

    fn restore(&self) -> Result<(), AppError> {
        let failure = |e: std::io::Error| AppError::with_detail("smapiInstallFailed", e);
        for (index, (target, kind)) in self.entries.iter().enumerate() {
            remove_path(target)?;
            if matches!(kind, BackupKind::Moved) {
                let stored = self.dir.join(index.to_string());
                fs::rename(&stored, target).map_err(failure)?;
            }
        }
        Ok(())
    }

    /// Restore every target, then remove the scratch directory. Used by the failure path:
    /// leaving an empty `.lingrid-install-*` folder behind would confuse the next run.
    fn rollback(self) -> Result<(), AppError> {
        let result = self.restore();
        let _ = fs::remove_dir_all(&self.dir);
        result
    }

    fn discard(self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

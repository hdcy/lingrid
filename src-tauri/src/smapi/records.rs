use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

/// Version of the on-disk record format; bump when the shape changes.
///
/// Records written by version 1 lack `created` and are still read: the field defaults to
/// an empty list, which simply means an uninstall has less to clean up.
const RECORD_VERSION: u32 = 2;

#[derive(Default, Deserialize, Serialize)]
struct RecordFile {
    version: u32,
    /// Keyed by the canonical game directory path.
    games: BTreeMap<String, GameRecord>,
}

#[derive(Deserialize, Serialize)]
struct GameRecord {
    smapi_version: String,
    /// Paths relative to the game directory that the install created.
    #[serde(default)]
    created: Vec<String>,
}

fn key(game_dir: &Path) -> String {
    fs::canonicalize(game_dir)
        .unwrap_or_else(|_| game_dir.into())
        .to_string_lossy()
        .into_owned()
}

fn load(path: &Path) -> RecordFile {
    // A missing file simply means nothing has been installed yet.
    let Ok(text) = fs::read_to_string(path) else {
        return RecordFile::default();
    };
    match serde_json::from_str(&text) {
        Ok(records) => records,
        Err(_) => {
            // Keep the damaged file instead of overwriting it: it still lists the paths an
            // earlier install created, which an uninstall would otherwise leave behind.
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default();
            let aside = path.with_extension(format!("json.corrupt-{stamp}"));
            let _ = fs::rename(path, aside);
            RecordFile::default()
        }
    }
}

fn store(path: &Path, records: &RecordFile) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::with_detail("smapiRecordFailed", e))?;
    }
    let text = serde_json::to_string_pretty(records)
        .map_err(|e| AppError::with_detail("smapiRecordFailed", e))?;
    // Write beside the target, then rename, so a failure never leaves a partial file.
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, text).map_err(|e| AppError::with_detail("smapiRecordFailed", e))?;
    fs::rename(&temporary, path).map_err(|e| AppError::with_detail("smapiRecordFailed", e))
}

pub fn version_of(path: &Path, game_dir: &Path) -> Option<String> {
    load(path)
        .games
        .get(&key(game_dir))
        .map(|record| record.smapi_version.clone())
}

/// Paths the recorded install created, used to clean up on uninstall.
pub fn created_of(path: &Path, game_dir: &Path) -> Vec<String> {
    load(path)
        .games
        .get(&key(game_dir))
        .map(|record| record.created.clone())
        .unwrap_or_default()
}

pub fn remember(
    path: &Path,
    game_dir: &Path,
    version: &str,
    created: Vec<String>,
) -> Result<(), AppError> {
    let mut records = load(path);
    records.version = RECORD_VERSION;
    records.games.insert(
        key(game_dir),
        GameRecord {
            smapi_version: version.to_owned(),
            created,
        },
    );
    store(path, &records)
}

pub fn forget(path: &Path, game_dir: &Path) -> Result<(), AppError> {
    let mut records = load(path);
    if records.games.remove(&key(game_dir)).is_none() {
        return Ok(());
    }
    store(path, &records)
}

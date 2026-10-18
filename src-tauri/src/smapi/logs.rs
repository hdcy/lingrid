use crate::error::AppError;
use serde::Serialize;
use std::{
    env, fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

/// Log file SMAPI rewrites on every launch.
const LOG_NAME: &str = "SMAPI-latest.txt";

/// The log grows while the game runs, so the UI only receives the newest slice.
const MAX_BYTES: u64 = 512 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmapiLog {
    /// Where the log is expected, whether or not it exists yet.
    pub path: PathBuf,
    pub exists: bool,
    pub modified_ms: Option<u64>,
    pub size: u64,
    pub content: String,
    /// True when the file was larger than the slice sent to the UI.
    pub truncated: bool,
}

/// The game writes its logs into the user's config directory, not the game folder.
fn logs_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env::var_os("APPDATA")
            .map(|base| PathBuf::from(base).join("StardewValley").join("ErrorLogs"))
    }
    #[cfg(not(windows))]
    {
        let base = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
        Some(base.join("StardewValley").join("ErrorLogs"))
    }
}

pub fn latest_path() -> Result<PathBuf, AppError> {
    logs_dir()
        .map(|dir| dir.join(LOG_NAME))
        .ok_or_else(|| AppError::new("smapiLogUnavailable"))
}

/// Read the newest part of the SMAPI log. A missing log is not an error: the game
/// simply has not been launched through SMAPI yet.
pub fn read_latest() -> Result<SmapiLog, AppError> {
    read(&latest_path()?, MAX_BYTES)
}

/// Exposed to the test module so the truncation rules can be checked with fixtures.
pub(super) fn read(path: &Path, max_bytes: u64) -> Result<SmapiLog, AppError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SmapiLog {
                path: path.into(),
                exists: false,
                modified_ms: None,
                size: 0,
                content: String::new(),
                truncated: false,
            });
        }
        Err(error) => return Err(AppError::with_detail("smapiLogUnreadable", error)),
    };
    let size = metadata.len();
    let truncated = size > max_bytes;
    let mut file =
        fs::File::open(path).map_err(|e| AppError::with_detail("smapiLogUnreadable", e))?;
    if truncated {
        file.seek(SeekFrom::Start(size - max_bytes))
            .map_err(|e| AppError::with_detail("smapiLogUnreadable", e))?;
    }
    let mut bytes = Vec::with_capacity(size.min(max_bytes) as usize);
    file.read_to_end(&mut bytes)
        .map_err(|e| AppError::with_detail("smapiLogUnreadable", e))?;
    // Cutting at an arbitrary byte can split a multi-byte character; lossy is fine here.
    Ok(SmapiLog {
        path: path.into(),
        exists: true,
        modified_ms: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|since| since.as_millis() as u64),
        size,
        content: String::from_utf8_lossy(&bytes).into_owned(),
        truncated,
    })
}

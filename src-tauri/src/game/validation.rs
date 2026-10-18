use super::model::{GameInstallation, GameRuntime, GameSource};
use crate::error::AppError;
use std::{fs, path::Path};

pub fn validate(path: &Path, source: GameSource) -> Result<GameInstallation, AppError> {
    if !path.is_absolute() {
        return Err(AppError::new("absolutePathRequired"));
    }
    let path =
        fs::canonicalize(path).map_err(|e| AppError::with_detail("directoryUnreadable", e))?;
    let entries =
        fs::read_dir(&path).map_err(|e| AppError::with_detail("directoryUnreadable", e))?;
    // Check readability rather than treating permission failures as an empty installation.
    for entry in entries {
        entry.map_err(|e| AppError::with_detail("directoryUnreadable", e))?;
    }
    if !path.join("Content").is_dir() {
        return Err(AppError::new("invalidGameDirectory"));
    }

    // Prefer the native Linux launcher when a managed .exe also exists beside it.
    let (runtime, executable) = if path.join("StardewValley").is_file()
        && [
            "Stardew Valley.dll",
            "StardewValley.dll",
            "StardewValley.exe",
            "StardewValley.bin.x86_64",
        ]
        .iter()
        .any(|name| path.join(name).is_file())
    {
        (GameRuntime::Linux, path.join("StardewValley"))
    } else if path.join("Stardew Valley.exe").is_file() {
        (GameRuntime::Windows, path.join("Stardew Valley.exe"))
    } else {
        return Err(AppError::new("invalidGameDirectory"));
    };

    let smapi = match runtime {
        GameRuntime::Linux => "StardewModdingAPI",
        GameRuntime::Windows => "StardewModdingAPI.exe",
    };
    let smapi_path = path.join(smapi);
    Ok(GameInstallation {
        smapi_path: smapi_path.is_file().then_some(smapi_path),
        executable,
        path,
        runtime,
        sources: vec![source],
    })
}

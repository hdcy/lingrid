use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Default, Deserialize, Serialize)]
struct Credentials {
    #[serde(default)]
    api_key: String,
}

/// Read the stored Nexus API key, if one was saved.
pub fn read(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let credentials: Credentials = serde_json::from_str(&text).ok()?;
    (!credentials.api_key.trim().is_empty()).then_some(credentials.api_key)
}

/// Whether a key is stored, without exposing it to the UI.
pub fn has_key(path: &Path) -> bool {
    read(path).is_some()
}

/// Store the key. An empty value clears it.
pub fn write(path: &Path, key: &str) -> Result<(), AppError> {
    let key = key.trim();
    if key.is_empty() {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(AppError::with_detail("nexusKeyNotSaved", error)),
        };
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::with_detail("nexusKeyNotSaved", e))?;
    }
    let text = serde_json::to_string_pretty(&Credentials {
        api_key: key.to_owned(),
    })
    .map_err(|e| AppError::with_detail("nexusKeyNotSaved", e))?;
    // Write beside the target and rename, so a crash cannot leave a partial file.
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, text).map_err(|e| AppError::with_detail("nexusKeyNotSaved", e))?;
    fs::rename(&temporary, path).map_err(|e| AppError::with_detail("nexusKeyNotSaved", e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // The key grants download access, so keep it readable by its owner only.
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|e| AppError::with_detail("nexusKeyNotSaved", e))?;
    }
    Ok(())
}

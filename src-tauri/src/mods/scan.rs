use super::model::InstalledMod;
use crate::error::AppError;
use serde_json::{Map, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Folders SMAPI itself skips; they are never mods.
const IGNORED: [&str; 3] = [".cache", "__MACOSX", ".DS_Store"];

/// A manifest as far as this application needs it.
#[derive(Default)]
struct Manifest {
    name: String,
    author: String,
    version: String,
    unique_id: Option<String>,
    description: Option<String>,
    nexus_id: Option<u64>,
}

/// List the mods installed in the game's `Mods` folder.
///
/// Disabled mods are folders whose name starts with a dot; they are reported with
/// `enabled: false` so the UI can offer to turn them back on again.
pub fn scan(game_dir: &Path) -> Result<Vec<InstalledMod>, AppError> {
    let mods = game_dir.join("Mods");
    if !mods.is_dir() {
        return Ok(Vec::new());
    }
    let failure = |e: std::io::Error| AppError::with_detail("modsUnreadable", e);
    let mut result = Vec::new();
    for entry in fs::read_dir(&mods).map_err(failure)? {
        let entry = entry.map_err(failure)?;
        if !entry.file_type().map_err(failure)?.is_dir() {
            continue;
        }
        let raw = entry.file_name().to_string_lossy().into_owned();
        if IGNORED.contains(&raw.as_str()) {
            continue;
        }
        let folder = raw.trim_start_matches('.').to_owned();
        if folder.is_empty() {
            continue;
        }
        let (manifest_path, manifest) = find_manifest(&entry.path());
        // A dot folder without a manifest is a leftover, not a disabled mod.
        if manifest.is_none() && raw.starts_with('.') {
            continue;
        }
        let manifest = manifest.unwrap_or_default();
        let name = if manifest.name.is_empty() {
            folder.clone()
        } else {
            manifest.name.clone()
        };
        result.push(InstalledMod {
            path: manifest_path
                .as_deref()
                .and_then(Path::parent)
                .unwrap_or(&entry.path())
                .to_path_buf(),
            folder,
            enabled: !raw.starts_with('.'),
            name,
            author: manifest.author,
            version: manifest.version,
            unique_id: manifest.unique_id,
            description: manifest.description,
            nexus_id: manifest.nexus_id,
            manifest_missing: manifest_path.is_none(),
        });
    }
    result.sort_by_key(|item| item.folder.to_lowercase());
    Ok(result)
}

/// Find a manifest in the folder itself or one level below, as SMAPI does when a pack
/// wraps its content in an extra directory.
fn find_manifest(dir: &Path) -> (Option<PathBuf>, Option<Manifest>) {
    let direct = dir.join("manifest.json");
    if let Some(manifest) = read_manifest(&direct) {
        return (Some(direct), Some(manifest));
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return (None, None);
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let nested = entry.path().join("manifest.json");
        if let Some(manifest) = read_manifest(&nested) {
            return (Some(nested), Some(manifest));
        }
    }
    (None, None)
}

fn read_manifest(path: &Path) -> Option<Manifest> {
    let text = String::from_utf8(fs::read(path).ok()?).ok()?;
    let value: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let object = value.as_object()?;
    Some(Manifest {
        name: text_field(object, "Name"),
        author: text_field(object, "Author"),
        version: version_field(object),
        unique_id: Some(text_field(object, "UniqueID")).filter(|value| !value.is_empty()),
        description: Some(text_field(object, "Description")).filter(|value| !value.is_empty()),
        nexus_id: nexus_id(object),
    })
}

/// Manifests spell fields inconsistently across versions, so lookups ignore case.
fn field<'a>(object: &'a Map<String, Value>, name: &str) -> Option<&'a Value> {
    object
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value)
}

fn text_field(object: &Map<String, Value>, name: &str) -> String {
    field(object, name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// `Version` is normally a string, but objects appear in the wild; render them readably.
fn version_field(object: &Map<String, Value>) -> String {
    match field(object, "Version") {
        Some(Value::String(text)) => text.trim().to_owned(),
        Some(Value::Object(map)) => map
            .iter()
            .filter_map(|(key, value)| value.as_str().map(|text| format!("{key} {text}")))
            .collect::<Vec<_>>()
            .join(", "),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

/// `UpdateKeys: ["Nexus:2400", …]` links a folder back to its Nexus page.
fn nexus_id(object: &Map<String, Value>) -> Option<u64> {
    field(object, "UpdateKeys")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .find_map(|key| {
            let (kind, id) = key.split_once(':')?;
            kind.eq_ignore_ascii_case("nexus")
                .then(|| id.trim().parse().ok())
                .flatten()
        })
}

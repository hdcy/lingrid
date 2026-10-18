use serde::Serialize;
use std::path::PathBuf;

/// One folder inside `Mods/`.
///
/// SMAPI ignores folders whose name starts with a dot, which is the convention this
/// application uses to disable a mod, so `folder` never carries that dot.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledMod {
    /// Folder name without the leading dot used for disabling.
    pub folder: String,
    pub path: PathBuf,
    /// Whether SMAPI will load it.
    pub enabled: bool,
    /// Manifest fields; empty strings when the folder has no readable manifest.
    pub name: String,
    pub author: String,
    pub version: String,
    pub unique_id: Option<String>,
    pub description: Option<String>,
    /// Nexus mod id taken from the manifest's update keys, when it declares one.
    pub nexus_id: Option<u64>,
    /// True for content packs, XNB mods and anything else without a readable manifest.
    pub manifest_missing: bool,
}

/// Result of installing an archive: one archive can hold several mods.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModsReport {
    /// Folder names that were created inside `Mods`.
    pub folders: Vec<String>,
}

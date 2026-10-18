use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GameSource {
    Steam,
    Gog,
    Common,
    Manual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GameRuntime {
    Windows,
    Linux,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInstallation {
    pub path: PathBuf,
    pub executable: PathBuf,
    pub runtime: GameRuntime,
    pub sources: Vec<GameSource>,
    /// Presence only: this does not claim SMAPI is working or version-compatible.
    pub smapi_path: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryWarning {
    pub code: &'static str,
    pub path: PathBuf,
    pub detail: String,
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryReport {
    pub installations: Vec<GameInstallation>,
    pub warnings: Vec<DiscoveryWarning>,
}

#[derive(Default)]
pub struct DiscoveryLocations {
    pub steam_roots: Vec<PathBuf>,
    pub candidates: Vec<(PathBuf, GameSource)>,
    pub warnings: Vec<DiscoveryWarning>,
}

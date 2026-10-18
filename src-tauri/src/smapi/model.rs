use serde::Serialize;
use std::path::PathBuf;

/// Latest published SMAPI release resolved from GitHub Releases.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmapiRelease {
    pub version: String,
    pub asset_name: String,
    pub asset_url: String,
    /// Lowercase SHA-256 reported by the GitHub API; absent for older releases.
    pub sha256: Option<String>,
    pub size: u64,
}

/// SMAPI state inside one game directory.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmapiStatus {
    pub installed: bool,
    /// Only known for installations this application performed; a manually installed
    /// SMAPI is reported as present without a version.
    pub version: Option<String>,
    pub executable: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmapiInstallOutcome {
    pub version: String,
    pub executable: PathBuf,
    /// Whether the game launcher was replaced (Linux only; Windows keeps the original).
    pub launcher_replaced: bool,
}

/// What an install wrote, kept so a later uninstall can remove every trace of it.
#[derive(Debug)]
pub struct InstallReport {
    pub outcome: SmapiInstallOutcome,
    /// Paths relative to the game directory that did not exist before the install.
    pub created: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmapiUninstallReport {
    /// Files and folders removed from the game directory.
    pub removed: u32,
    /// Mods deleted from `Mods`; only non-zero when the user asked for it.
    pub mods_removed: u32,
    /// Whether the original game launcher was restored (Linux).
    pub launcher_restored: bool,
}

/// How thoroughly an uninstall should clean up.
pub struct UninstallOptions {
    /// Also delete everything inside `Mods`, including the user's own mods.
    pub remove_mods: bool,
    /// Paths recorded at install time that did not exist before; removed as well.
    pub created: Vec<String>,
}

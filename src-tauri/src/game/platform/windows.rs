use super::super::model::{DiscoveryLocations, DiscoveryWarning, GameSource};
use std::{env, io, path::PathBuf};
use winreg::{
    enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY},
    RegKey,
};

pub fn locations() -> DiscoveryLocations {
    let mut result = DiscoveryLocations::default();
    let user = RegKey::predef(HKEY_CURRENT_USER);
    let machine = RegKey::predef(HKEY_LOCAL_MACHINE);
    // Steam and GOG installers can write either registry view, irrespective of our architecture.
    for hive in [&user, &machine] {
        for view in [KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
            if let Some(key) = open(hive, r"SOFTWARE\Valve\Steam", view, &mut result) {
                for name in ["SteamPath", "InstallPath"] {
                    if let Ok(path) = key.get_value::<String, _>(name) {
                        if !path.is_empty() {
                            result.steam_roots.push(path.into());
                        }
                    }
                }
            }
            if let Some(key) = open(
                hive,
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Steam App 413150",
                view,
                &mut result,
            ) {
                if let Ok(path) = key.get_value::<String, _>("InstallLocation") {
                    if !path.is_empty() {
                        result.candidates.push((path.into(), GameSource::Steam));
                    }
                }
            }
            if let Some(games) = open(hive, r"SOFTWARE\GOG.com\Games", view, &mut result) {
                for id in games.enum_keys() {
                    let id = match id {
                        Ok(id) => id,
                        Err(e) => {
                            warning(&mut result, r"SOFTWARE\GOG.com\Games", e);
                            continue;
                        }
                    };
                    if let Some(key) = open(&games, &id, view, &mut result) {
                        let name = key.get_value::<String, _>("gameName").unwrap_or_default();
                        if name.eq_ignore_ascii_case("Stardew Valley") {
                            if let Ok(path) = key.get_value::<String, _>("path") {
                                if !path.is_empty() {
                                    result.candidates.push((path.into(), GameSource::Gog));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    for variable in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(base) = env::var_os(variable).map(PathBuf::from) {
            result.steam_roots.push(base.join("Steam"));
            result.candidates.push((
                base.join("GOG Galaxy/Games/Stardew Valley"),
                GameSource::Gog,
            ));
            result.candidates.push((
                base.join("ModifiableWindowsApps/Stardew Valley"),
                GameSource::Common,
            ));
        }
    }
    if let Some(local) = env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        result.steam_roots.push(local.join("Steam"));
    }
    if let Some(drive) = env::var_os("SystemDrive") {
        let root = PathBuf::from(format!("{}\\", drive.to_string_lossy()));
        result
            .candidates
            .push((root.join("GOG Games/Stardew Valley"), GameSource::Gog));
        result.candidates.push((
            root.join("XboxGames/Stardew Valley/Content"),
            GameSource::Common,
        ));
    }
    result
}

fn open(hive: &RegKey, path: &str, view: u32, result: &mut DiscoveryLocations) -> Option<RegKey> {
    match hive.open_subkey_with_flags(path, KEY_READ | view) {
        Ok(key) => Some(key),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => {
            warning(result, path, e);
            None
        }
    }
}

fn warning(result: &mut DiscoveryLocations, path: &str, error: io::Error) {
    result.warnings.push(DiscoveryWarning {
        code: "registryUnreadable",
        path: path.into(),
        detail: error.to_string(),
    });
}

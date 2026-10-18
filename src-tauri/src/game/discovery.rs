use super::{
    model::{DiscoveryLocations, DiscoveryReport, DiscoveryWarning, GameSource},
    validation,
    vdf::{self, Value},
};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};

const MAX_VDF_BYTES: u64 = 1024 * 1024;

fn read_vdf(path: &Path, report: &mut DiscoveryReport) -> Option<Value> {
    let read = || -> Result<String, io::Error> {
        let mut text = String::new();
        File::open(path)?
            .take(MAX_VDF_BYTES + 1)
            .read_to_string(&mut text)?;
        if text.len() as u64 > MAX_VDF_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Steam metadata exceeds 1 MiB",
            ));
        }
        Ok(text)
    };
    match read() {
        Ok(text) => match vdf::parse(&text) {
            Ok(value) => Some(value),
            Err(detail) => {
                report.warnings.push(DiscoveryWarning {
                    code: "steamMetadataInvalid",
                    path: path.into(),
                    detail,
                });
                None
            }
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => {
            report.warnings.push(DiscoveryWarning {
                code: "steamMetadataUnreadable",
                path: path.into(),
                detail: e.to_string(),
            });
            None
        }
    }
}

fn path_key(path: &Path) -> PathBuf {
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.into());
    #[cfg(windows)]
    {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    }
    #[cfg(not(windows))]
    {
        path
    }
}

fn add_library(path: PathBuf, libraries: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
    if path.is_absolute() && seen.insert(path_key(&path)) {
        libraries.push(path);
    }
}

pub fn discover(locations: DiscoveryLocations) -> DiscoveryReport {
    let mut report = DiscoveryReport {
        warnings: locations.warnings,
        ..Default::default()
    };
    let mut libraries = Vec::new();
    let mut seen = HashSet::new();
    for root in locations.steam_roots {
        // A Steam root is itself a library, including older Steam installations.
        add_library(root.clone(), &mut libraries, &mut seen);
        for file in [
            root.join("steamapps/libraryfolders.vdf"),
            root.join("config/libraryfolders.vdf"),
        ] {
            if let Some(value) = read_vdf(&file, &mut report) {
                if let Some(entries) = value.get("libraryfolders").and_then(Value::entries) {
                    for (index, value) in entries {
                        if !index.is_empty() && index.chars().all(|c| c.is_ascii_digit()) {
                            // Old format: "1" "D:\\SteamLibrary". New format: "1" { "path" ... }.
                            if let Some(path) = value
                                .text()
                                .or_else(|| value.get("path").and_then(Value::text))
                            {
                                add_library(path.into(), &mut libraries, &mut seen);
                            }
                        }
                    }
                } else {
                    report.warnings.push(DiscoveryWarning {
                        code: "steamMetadataInvalid",
                        path: file,
                        detail: "Missing libraryfolders object".into(),
                    });
                }
            }
        }
    }
    let mut candidates = locations.candidates;
    for library in libraries {
        let common = library.join("steamapps/common");
        let manifest_path = library.join("steamapps/appmanifest_413150.acf");
        if let Some(manifest) = read_vdf(&manifest_path, &mut report) {
            let state = manifest.get("AppState");
            let app_id = state.and_then(|v| v.get("appid")).and_then(Value::text);
            let directory = state
                .and_then(|v| v.get("installdir"))
                .and_then(Value::text);
            match directory {
                Some(name) if app_id == Some("413150") && safe_install_dir(name) => {
                    candidates.push((common.join(name), GameSource::Steam));
                }
                _ => report.warnings.push(DiscoveryWarning {
                    code: "steamMetadataInvalid",
                    path: manifest_path,
                    detail: "Invalid Stardew Valley app ID or install directory".into(),
                }),
            }
        }
        candidates.push((common.join("Stardew Valley"), GameSource::Steam));
    }
    let mut checked = HashSet::new();
    for (path, source) in candidates {
        let key = path_key(&path);
        if let Some(existing) = report
            .installations
            .iter_mut()
            .find(|i| path_key(&i.path) == key)
        {
            if !existing.sources.contains(&source) {
                existing.sources.push(source);
            }
            continue;
        }
        if !checked.insert(key) {
            continue;
        }
        // Missing default locations are normal; inaccessible candidates deserve a warning.
        match fs::metadata(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => {
                report.warnings.push(DiscoveryWarning {
                    code: "directoryUnreadable",
                    path,
                    detail: e.to_string(),
                });
                continue;
            }
            Ok(_) => {}
        }
        match validation::validate(&path, source) {
            Ok(game) => report.installations.push(game),
            Err(error) => report.warnings.push(DiscoveryWarning {
                code: error.code,
                path,
                detail: error.detail.unwrap_or_default(),
            }),
        }
    }
    report.installations.sort_by(|a, b| a.path.cmp(&b.path));
    report
}

fn safe_install_dir(name: &str) -> bool {
    !name.trim().is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', ':', '\0'])
}

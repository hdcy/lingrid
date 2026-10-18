use super::{
    discovery,
    model::{DiscoveryLocations, GameRuntime, GameSource},
    validation, vdf,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn game(path: &Path, runtime: GameRuntime) {
    fs::create_dir_all(path.join("Content")).unwrap();
    match runtime {
        GameRuntime::Windows => write(&path.join("Stardew Valley.exe"), "fixture"),
        GameRuntime::Linux => {
            write(&path.join("StardewValley"), "fixture");
            write(&path.join("Stardew Valley.dll"), "fixture");
        }
    }
}

fn quoted(path: &Path) -> String {
    path.to_str().unwrap().replace('\\', "\\\\")
}

#[test]
fn discovers_secondary_steam_library_and_manifest_directory_with_unicode() {
    let temp = TempDir::new().unwrap();
    let steam = temp.path().join("Steam");
    let library = temp.path().join("游戏库 日本語");
    let install = library.join("steamapps/common/Custom Stardew");
    game(&install, GameRuntime::Windows);
    write(
        &steam.join("steamapps/libraryfolders.vdf"),
        &format!(
            r#"
        // Steam metadata
        "libraryfolders" {{ "0" {{ "path" "{}" "apps" {{ "413150" "1" }} }} }}
    "#,
            quoted(&library)
        ),
    );
    write(
        &library.join("steamapps/appmanifest_413150.acf"),
        r#""AppState" { "appid" "413150" "installdir" "Custom Stardew" }"#,
    );
    let report = discovery::discover(DiscoveryLocations {
        steam_roots: vec![steam],
        ..Default::default()
    });
    assert!(report.warnings.is_empty());
    assert_eq!(report.installations.len(), 1);
    assert_eq!(
        report.installations[0].path,
        fs::canonicalize(install).unwrap()
    );
    assert_eq!(report.installations[0].runtime, GameRuntime::Windows);
}

#[test]
fn old_library_format_and_default_folder_work_without_manifest() {
    let temp = TempDir::new().unwrap();
    let steam = temp.path().join("Steam");
    let library = temp.path().join("OtherLibrary");
    game(
        &library.join("steamapps/common/Stardew Valley"),
        GameRuntime::Linux,
    );
    write(
        &steam.join("config/libraryfolders.vdf"),
        &format!(r#""LibraryFolders" {{ "1" "{}" }}"#, quoted(&library)),
    );
    let report = discovery::discover(DiscoveryLocations {
        steam_roots: vec![steam],
        ..Default::default()
    });
    assert_eq!(report.installations.len(), 1);
}

#[test]
fn corrupt_metadata_does_not_hide_valid_default_installation() {
    let temp = TempDir::new().unwrap();
    game(
        &temp.path().join("steamapps/common/Stardew Valley"),
        GameRuntime::Linux,
    );
    write(
        &temp.path().join("steamapps/libraryfolders.vdf"),
        "\"libraryfolders\" { broken",
    );
    let report = discovery::discover(DiscoveryLocations {
        steam_roots: vec![temp.path().into()],
        ..Default::default()
    });
    assert_eq!(report.installations.len(), 1);
    assert_eq!(report.warnings[0].code, "steamMetadataInvalid");
}

#[test]
fn rejects_manifest_path_traversal_and_wrong_app_id() {
    let temp = TempDir::new().unwrap();
    for (app_id, name) in [("413150", "../../outside"), ("999", "Stardew Valley")] {
        write(
            &temp.path().join("steamapps/appmanifest_413150.acf"),
            &format!(r#""AppState" {{ "appid" "{app_id}" "installdir" "{name}" }}"#),
        );
        let report = discovery::discover(DiscoveryLocations {
            steam_roots: vec![temp.path().into()],
            ..Default::default()
        });
        assert!(report.installations.is_empty());
        assert_eq!(report.warnings[0].code, "steamMetadataInvalid");
    }
}

#[test]
fn ignores_uninstalled_records_and_reports_invalid_existing_folder() {
    let temp = TempDir::new().unwrap();
    let invalid = temp.path().join("Not a game");
    fs::create_dir(&invalid).unwrap();
    let report = discovery::discover(DiscoveryLocations {
        candidates: vec![
            (temp.path().join("Removed game"), GameSource::Gog),
            (invalid, GameSource::Gog),
        ],
        ..Default::default()
    });
    assert!(report.installations.is_empty());
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(report.warnings[0].code, "invalidGameDirectory");
}

#[test]
fn validates_both_platforms_and_does_not_require_smapi() {
    let temp = TempDir::new().unwrap();
    for runtime in [GameRuntime::Windows, GameRuntime::Linux] {
        let path = temp.path().join(format!("{runtime:?}"));
        game(&path, runtime);
        let result = validation::validate(&path, GameSource::Manual).unwrap();
        assert_eq!(result.runtime, runtime);
        assert!(result.smapi_path.is_none());
        let smapi = if runtime == GameRuntime::Windows {
            "StardewModdingAPI.exe"
        } else {
            "StardewModdingAPI"
        };
        write(&path.join(smapi), "fixture");
        assert!(validation::validate(&path, GameSource::Manual)
            .unwrap()
            .smapi_path
            .is_some());
    }
    assert_eq!(
        validation::validate(Path::new("relative/path"), GameSource::Manual)
            .unwrap_err()
            .code,
        "absolutePathRequired"
    );
    assert_eq!(
        validation::validate(temp.path(), GameSource::Manual)
            .unwrap_err()
            .code,
        "invalidGameDirectory"
    );
}

#[test]
fn an_executable_without_content_is_not_a_valid_game() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("Stardew Valley.exe"), "fixture");
    assert!(validation::validate(temp.path(), GameSource::Manual).is_err());
}

#[test]
fn returns_multiple_copies_and_merges_duplicate_sources() {
    let temp = TempDir::new().unwrap();
    let first = temp.path().join("A");
    let second = temp.path().join("B");
    game(&first, GameRuntime::Linux);
    game(&second, GameRuntime::Windows);
    let report = discovery::discover(DiscoveryLocations {
        candidates: vec![
            (first.clone(), GameSource::Steam),
            (first, GameSource::Gog),
            (second, GameSource::Manual),
        ],
        ..Default::default()
    });
    assert_eq!(report.installations.len(), 2);
    assert_eq!(
        report.installations[0].sources,
        [GameSource::Steam, GameSource::Gog]
    );
}

#[cfg(unix)]
#[test]
fn canonicalizes_steam_symlinks_without_duplicate_results() {
    let temp = TempDir::new().unwrap();
    let real = temp.path().join("real");
    let link = temp.path().join("link");
    game(
        &real.join("steamapps/common/Stardew Valley"),
        GameRuntime::Linux,
    );
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let report = discovery::discover(DiscoveryLocations {
        steam_roots: vec![real, link],
        ..Default::default()
    });
    assert_eq!(report.installations.len(), 1);
}

#[test]
fn vdf_handles_bom_comments_escapes_and_nested_objects() {
    let value = vdf::parse(
        "\u{feff}// comment\n\"LibraryFolders\" { \"1\" { \"path\" \"D:\\\\游戏 库\" } }",
    )
    .unwrap();
    assert_eq!(
        value
            .get("libraryfolders")
            .unwrap()
            .get("1")
            .unwrap()
            .get("path")
            .unwrap()
            .text(),
        Some("D:\\游戏 库")
    );
    assert_eq!(
        vdf::parse(r#"key "C:\Games""#)
            .unwrap()
            .get("key")
            .unwrap()
            .text(),
        Some("C:\\Games")
    );
    for bad in ["{", "key", "key {", "key \"unterminated", "}"] {
        assert!(vdf::parse(bad).is_err(), "{bad}");
    }
    let deep = format!("{}{}", "x {".repeat(34), "}".repeat(34));
    assert!(vdf::parse(&deep).is_err());
}

#[test]
fn bounds_metadata_size() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("steamapps/libraryfolders.vdf"),
        &" ".repeat(1024 * 1024 + 1),
    );
    let report = discovery::discover(DiscoveryLocations {
        steam_roots: vec![PathBuf::from(temp.path())],
        ..Default::default()
    });
    assert_eq!(report.warnings[0].code, "steamMetadataUnreadable");
}

/// Opt-in smoke check; only reads local installation metadata and game directories.
#[test]
#[ignore = "requires host installation metadata"]
fn discovers_host_installations() {
    let report = discovery::discover(super::platform::locations().unwrap());
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

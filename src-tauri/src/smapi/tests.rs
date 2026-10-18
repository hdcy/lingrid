use super::{
    archive, install, logs,
    model::{SmapiUninstallReport, UninstallOptions},
    records, release,
};
use crate::game::model::GameRuntime;
use std::{
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};
use tempfile::TempDir;
use zip::{write::SimpleFileOptions, ZipWriter};

const DEPS: &str = r#"{"runtimeTarget":{"name":".NETCoreApp,Version=v6.0"}}"#;
const ORIGINAL_LAUNCHER: &str = "#!/bin/sh\nexec ./Stardew Valley.dll\n";
const SMAPI_LAUNCHER: &str = "#!/bin/sh\nexec ./StardewModdingAPI\n";

fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut buffer);
        for (name, contents) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap();
    }
    buffer.into_inner()
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// A minimal but recognisable game directory, as `validation` expects it.
fn game_dir(path: &Path, runtime: GameRuntime) {
    fs::create_dir_all(path.join("Content")).unwrap();
    fs::write(path.join("Stardew Valley.deps.json"), DEPS).unwrap();
    match runtime {
        GameRuntime::Windows => write(&path.join("Stardew Valley.exe"), "game"),
        GameRuntime::Linux => {
            write(&path.join("StardewValley"), ORIGINAL_LAUNCHER);
            write(&path.join("Stardew Valley.dll"), "game");
        }
    }
}

/// An extracted `install.dat`, mirroring the real file set (including one mod that the
/// official installer would refuse to ship).
fn bundle(path: &Path, runtime: GameRuntime) {
    write(&path.join("smapi-internal/config.json"), "{}");
    write(&path.join("steam_appid.txt"), "413150");
    write(&path.join("StardewModdingAPI.runtimeconfig.json"), "{}");
    // Real SMAPI manifests spell the field `UniqueId`; older ones used `UniqueID`.
    write(
        &path.join("Mods/SaveBackup/manifest.json"),
        r#"{"UniqueId":"SMAPI.SaveBackup"}"#,
    );
    write(
        &path.join("Mods/ConsoleCommands/manifest.json"),
        r#"{"UniqueID":"SMAPI.ConsoleCommands"}"#,
    );
    write(
        &path.join("Mods/NotAllowed/manifest.json"),
        r#"{"UniqueId":"Someone.Else"}"#,
    );
    match runtime {
        GameRuntime::Windows => write(&path.join("StardewModdingAPI.exe"), "smapi"),
        GameRuntime::Linux => {
            write(&path.join("StardewModdingAPI"), "smapi");
            write(&path.join("unix-launcher.sh"), SMAPI_LAUNCHER);
        }
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

fn uninstall(
    game: &Path,
    runtime: GameRuntime,
    created: &[String],
    remove_mods: bool,
) -> SmapiUninstallReport {
    install::uninstall(
        game,
        runtime,
        &UninstallOptions {
            remove_mods,
            created: created.to_vec(),
        },
    )
    .unwrap()
}

#[test]
fn linux_install_replaces_launcher_and_derives_deps_json() {
    let temp = TempDir::new().unwrap();
    let game = temp.path().join("game");
    let bundle_dir = temp.path().join("bundle");
    game_dir(&game, GameRuntime::Linux);
    bundle(&bundle_dir, GameRuntime::Linux);

    let report = install::install(&game, &bundle_dir, GameRuntime::Linux, "4.5.2").unwrap();
    let outcome = &report.outcome;

    assert_eq!(outcome.version, "4.5.2");
    assert!(outcome.launcher_replaced);
    // `install` canonicalizes the directory; on Windows that adds a `\\?\` prefix.
    assert_eq!(
        outcome.executable,
        fs::canonicalize(game.join("StardewModdingAPI")).unwrap()
    );
    assert_eq!(
        read(&game.join("StardewValley-original")),
        ORIGINAL_LAUNCHER
    );
    assert_eq!(read(&game.join("StardewValley")), SMAPI_LAUNCHER);
    assert_eq!(read(&game.join("StardewModdingAPI")), "smapi");
    assert!(game.join("smapi-internal/config.json").is_file());
    // Native libraries are resolved through a copy of the game's own deps file.
    assert_eq!(read(&game.join("StardewModdingAPI.deps.json")), DEPS);
    // The launcher script is moved into place, not left behind.
    assert!(!game.join("unix-launcher.sh").exists());
}

#[test]
fn only_allowlisted_bundled_mods_are_installed() {
    let temp = TempDir::new().unwrap();
    let game = temp.path().join("game");
    let bundle_dir = temp.path().join("bundle");
    game_dir(&game, GameRuntime::Linux);
    bundle(&bundle_dir, GameRuntime::Linux);

    install::install(&game, &bundle_dir, GameRuntime::Linux, "4.5.2").unwrap();

    assert!(game.join("Mods/SaveBackup/manifest.json").is_file());
    assert!(game.join("Mods/ConsoleCommands/manifest.json").is_file());
    assert!(!game.join("Mods/NotAllowed").exists());
}

#[test]
fn windows_install_keeps_the_game_launcher() {
    let temp = TempDir::new().unwrap();
    let game = temp.path().join("game");
    let bundle_dir = temp.path().join("bundle");
    game_dir(&game, GameRuntime::Windows);
    bundle(&bundle_dir, GameRuntime::Windows);

    let report = install::install(&game, &bundle_dir, GameRuntime::Windows, "4.5.2").unwrap();

    // Windows cannot be redirected by the installer; the game launcher stays untouched.
    assert!(!report.outcome.launcher_replaced);
    assert_eq!(
        report.outcome.executable,
        fs::canonicalize(game.join("StardewModdingAPI.exe")).unwrap()
    );
    assert_eq!(read(&game.join("Stardew Valley.exe")), "game");
    assert_eq!(read(&game.join("StardewModdingAPI.deps.json")), DEPS);
    assert!(!game.join("StardewValley-original").exists());
}

#[test]
fn uninstall_restores_the_original_launcher() {
    let temp = TempDir::new().unwrap();
    let game = temp.path().join("game");
    let bundle_dir = temp.path().join("bundle");
    game_dir(&game, GameRuntime::Linux);
    bundle(&bundle_dir, GameRuntime::Linux);
    let report = install::install(&game, &bundle_dir, GameRuntime::Linux, "4.5.2").unwrap();

    uninstall(&game, GameRuntime::Linux, &report.created, false);

    assert_eq!(read(&game.join("StardewValley")), ORIGINAL_LAUNCHER);
    assert!(!game.join("StardewValley-original").exists());
    for leftover in [
        "StardewModdingAPI",
        "StardewModdingAPI.deps.json",
        "smapi-internal",
    ] {
        assert!(
            !game.join(leftover).exists(),
            "{leftover} should be removed"
        );
    }
    // The game's own files survive.
    assert_eq!(read(&game.join("Stardew Valley.dll")), "game");
    assert!(game.join("Stardew Valley.deps.json").is_file());
}

#[test]
fn uninstall_removes_bundled_mods_and_created_files() {
    let temp = TempDir::new().unwrap();
    let game = temp.path().join("game");
    let bundle_dir = temp.path().join("bundle");
    game_dir(&game, GameRuntime::Linux);
    bundle(&bundle_dir, GameRuntime::Linux);

    let report = install::install(&game, &bundle_dir, GameRuntime::Linux, "4.5.2").unwrap();

    // The game fixture ships no `steam_appid.txt`, so SMAPI's install created it.
    assert!(report.created.contains(&"steam_appid.txt".to_owned()));
    // The launcher pair is handled by the backup flow, not recorded as created.
    assert!(!report.created.iter().any(|path| path == "StardewValley"));

    let removed = uninstall(&game, GameRuntime::Linux, &report.created, false);

    assert!(removed.launcher_restored);
    assert!(removed.mods_removed == 0, "user mods are kept by default");
    assert!(
        !game.join("Mods/SaveBackup").exists(),
        "bundled mods go too"
    );
    assert!(!game.join("Mods/ConsoleCommands").exists());
    assert!(
        !game.join("steam_appid.txt").exists(),
        "files created by the install are cleaned up"
    );
    assert!(game.join("StardewValley").is_file());
}

#[test]
fn uninstall_keeps_files_the_game_shipped() {
    let temp = TempDir::new().unwrap();
    let game = temp.path().join("game");
    let bundle_dir = temp.path().join("bundle");
    game_dir(&game, GameRuntime::Linux);
    bundle(&bundle_dir, GameRuntime::Linux);
    // A game that ships its own app id file: the install must not claim it as created.
    write(&game.join("steam_appid.txt"), "413150");

    let report = install::install(&game, &bundle_dir, GameRuntime::Linux, "4.5.2").unwrap();
    assert!(!report.created.contains(&"steam_appid.txt".to_owned()));

    uninstall(&game, GameRuntime::Linux, &report.created, false);

    assert!(
        game.join("steam_appid.txt").is_file(),
        "a file the game shipped must survive an uninstall"
    );
}

#[test]
fn uninstall_can_clear_every_mod() {
    let temp = TempDir::new().unwrap();
    let game = temp.path().join("game");
    let bundle_dir = temp.path().join("bundle");
    game_dir(&game, GameRuntime::Linux);
    bundle(&bundle_dir, GameRuntime::Linux);
    write(
        &game.join("Mods/MyMod/manifest.json"),
        r#"{"UniqueID":"My.Mod"}"#,
    );
    write(&game.join("Mods/MyMod/MyMod.dll"), "mod");
    let report = install::install(&game, &bundle_dir, GameRuntime::Linux, "4.5.2").unwrap();

    let removed = uninstall(&game, GameRuntime::Linux, &report.created, true);

    // Only the entries cleared by the option are counted here; the bundled mods the
    // install added are already counted in `removed`.
    assert_eq!(removed.mods_removed, 1);
    assert!(removed.removed >= 3, "got {}", removed.removed);
    assert!(!game.join("Mods/MyMod").exists());
    assert!(!game.join("Mods/SaveBackup").exists());
    assert!(game.join("Mods").is_dir(), "the Mods folder itself is kept");
}

#[test]
fn a_failed_install_rolls_back_completely() {
    let temp = TempDir::new().unwrap();
    let game = temp.path().join("game");
    let bundle_dir = temp.path().join("bundle");
    game_dir(&game, GameRuntime::Linux);
    bundle(&bundle_dir, GameRuntime::Linux);
    // Linux installs must replace the launcher; without the script the install cannot finish.
    fs::remove_file(bundle_dir.join("unix-launcher.sh")).unwrap();

    let error = install::install(&game, &bundle_dir, GameRuntime::Linux, "4.5.2").unwrap_err();

    assert_eq!(error.code, "smapiArchiveInvalid");
    assert_eq!(read(&game.join("StardewValley")), ORIGINAL_LAUNCHER);
    for leftover in [
        "StardewModdingAPI",
        "StardewModdingAPI.deps.json",
        "smapi-internal",
        "StardewValley-original",
        "Mods",
    ] {
        assert!(
            !game.join(leftover).exists(),
            "{leftover} should be rolled back"
        );
    }
    assert!(
        fs::read_dir(&game).unwrap().flatten().all(|entry| !entry
            .file_name()
            .to_string_lossy()
            .starts_with(".lingrid-install-")),
        "the scratch backup directory is removed"
    );
}

#[test]
fn reinstalling_keeps_the_original_launcher_and_user_settings() {
    let temp = TempDir::new().unwrap();
    let game = temp.path().join("game");
    let bundle_dir = temp.path().join("bundle");
    game_dir(&game, GameRuntime::Linux);
    bundle(&bundle_dir, GameRuntime::Linux);
    install::install(&game, &bundle_dir, GameRuntime::Linux, "4.5.1").unwrap();
    write(
        &game.join("smapi-internal/config.user.json"),
        r#"{"VerboseLogging":true}"#,
    );

    install::install(&game, &bundle_dir, GameRuntime::Linux, "4.5.2").unwrap();

    // The backup captured on the first install stays authoritative on upgrade.
    assert_eq!(
        read(&game.join("StardewValley-original")),
        ORIGINAL_LAUNCHER
    );
    assert_eq!(read(&game.join("StardewValley")), SMAPI_LAUNCHER);
    assert_eq!(
        read(&game.join("smapi-internal/config.user.json")),
        r#"{"VerboseLogging":true}"#
    );
}

#[test]
fn existing_user_mods_are_never_removed() {
    let temp = TempDir::new().unwrap();
    let game = temp.path().join("game");
    let bundle_dir = temp.path().join("bundle");
    game_dir(&game, GameRuntime::Linux);
    bundle(&bundle_dir, GameRuntime::Linux);
    write(
        &game.join("Mods/MyMod/manifest.json"),
        r#"{"UniqueID":"My.Mod"}"#,
    );

    let report = install::install(&game, &bundle_dir, GameRuntime::Linux, "4.5.2").unwrap();
    uninstall(&game, GameRuntime::Linux, &report.created, false);

    assert!(game.join("Mods/MyMod/manifest.json").is_file());
}

#[test]
fn detect_reports_presence_per_platform() {
    let temp = TempDir::new().unwrap();
    let game = temp.path().join("game");
    game_dir(&game, GameRuntime::Windows);

    let missing = install::detect(&game, GameRuntime::Windows).unwrap();
    assert!(!missing.installed);
    assert!(missing.executable.is_none());

    write(&game.join("StardewModdingAPI.exe"), "smapi");
    let present = install::detect(&game, GameRuntime::Windows).unwrap();
    assert!(present.installed);
    assert_eq!(
        present.version, None,
        "manual installs have no known version"
    );
    assert_eq!(
        present.executable.unwrap(),
        game.join("StardewModdingAPI.exe")
    );
}

#[test]
fn uninstalling_something_that_is_not_installed_is_an_error() {
    let temp = TempDir::new().unwrap();
    let game = temp.path().join("game");
    game_dir(&game, GameRuntime::Linux);
    assert_eq!(
        install::uninstall(
            &game,
            GameRuntime::Linux,
            &UninstallOptions {
                remove_mods: false,
                created: Vec::new(),
            },
        )
        .unwrap_err()
        .code,
        "smapiNotInstalled"
    );
}

#[test]
fn installer_payload_is_read_from_the_versioned_folder() {
    let temp = TempDir::new().unwrap();
    let inner = zip_bytes(&[("StardewModdingAPI", b"smapi")]);
    let installer = temp.path().join("installer.zip");
    fs::write(
        &installer,
        zip_bytes(&[
            (
                "SMAPI 4.5.2 installer/internal/linux/install.dat",
                inner.as_slice(),
            ),
            ("SMAPI 4.5.2 installer/README.txt", b"readme"),
        ]),
    )
    .unwrap();

    let payload = archive::read_entry_by_suffix(&installer, "internal/linux/install.dat").unwrap();
    assert_eq!(payload, inner);
    assert_eq!(
        archive::read_entry_by_suffix(&installer, "internal/macOS/install.dat")
            .unwrap_err()
            .code,
        "smapiArchiveInvalid"
    );
}

#[test]
fn extraction_rejects_traversal_and_reads_content() {
    let temp = TempDir::new().unwrap();
    let destination = temp.path().join("out");
    let bytes = zip_bytes(&[("smapi-internal/config.json", b"{}")]);
    archive::extract_bytes(&bytes, &destination).unwrap();
    assert_eq!(read(&destination.join("smapi-internal/config.json")), "{}");

    let malicious = zip_bytes(&[("../../escaped.txt", b"boom")]);
    assert_eq!(
        archive::extract_bytes(&malicious, &temp.path().join("bad"))
            .unwrap_err()
            .code,
        "smapiArchiveInvalid"
    );
    assert!(!temp.path().parent().unwrap().join("escaped.txt").exists());
}

#[test]
fn release_parsing_prefers_the_installer_asset() {
    let body = r#"{
        "tag_name": "4.5.2",
        "assets": [
            {"name": "SMAPI-4.5.2-installer-double-zipped.zip", "browser_download_url": "https://example/double", "size": 1, "digest": "sha256:aa"},
            {"name": "SMAPI-4.5.2-installer.zip", "browser_download_url": "https://example/installer", "size": 41766494, "digest": "sha256:DD01DDCA"}
        ]
    }"#;
    let release = release::parse_release(body).unwrap();
    assert_eq!(release.version, "4.5.2");
    assert_eq!(release.asset_name, "SMAPI-4.5.2-installer.zip");
    assert_eq!(release.asset_url, "https://example/installer");
    assert_eq!(release.sha256.as_deref(), Some("dd01ddca"));
    assert_eq!(release.size, 41766494);

    assert_eq!(
        release::parse_release(r#"{"tag_name":"1.0.0","assets":[]}"#)
            .unwrap_err()
            .code,
        "smapiReleaseInvalid"
    );
    assert_eq!(
        release::parse_release("not json").unwrap_err().code,
        "smapiReleaseInvalid"
    );
}

#[test]
fn install_records_survive_round_trips_and_damage() {
    let temp = TempDir::new().unwrap();
    let record = temp.path().join("smapi/installed.json");
    let game = temp.path().join("game");
    fs::create_dir_all(&game).unwrap();

    assert_eq!(records::version_of(&record, &game), None);
    records::remember(&record, &game, "4.5.2", vec!["steam_appid.txt".to_owned()]).unwrap();
    assert_eq!(
        records::version_of(&record, &game).as_deref(),
        Some("4.5.2")
    );
    assert_eq!(
        records::created_of(&record, &game),
        ["steam_appid.txt".to_owned()]
    );

    // A damaged file degrades to "unknown version" instead of failing, but is kept aside
    // rather than silently overwritten: it may still list paths an uninstall must remove.
    fs::write(&record, "{not json").unwrap();
    assert_eq!(records::version_of(&record, &game), None);
    assert!(records::created_of(&record, &game).is_empty());
    assert!(
        fs::read_dir(record.parent().unwrap())
            .unwrap()
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().contains("corrupt-")),
        "the damaged record is preserved"
    );

    records::remember(&record, &game, "4.5.2", Vec::new()).unwrap();
    records::forget(&record, &game).unwrap();
    assert_eq!(records::version_of(&record, &game), None);
}

#[test]
fn records_written_before_created_paths_existed_still_load() {
    let temp = TempDir::new().unwrap();
    let record = temp.path().join("installed.json");
    let game = temp.path().join("game");
    fs::create_dir_all(&game).unwrap();
    let key = fs::canonicalize(&game)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    // Build the JSON through serde: a hand-written Windows path contains backslashes that
    // would produce invalid JSON escapes.
    let mut games = serde_json::Map::new();
    games.insert(key, serde_json::json!({ "smapi_version": "4.5.0" }));
    let body = serde_json::json!({ "version": 1, "games": games });
    fs::write(&record, serde_json::to_string(&body).unwrap()).unwrap();

    assert_eq!(
        records::version_of(&record, &game).as_deref(),
        Some("4.5.0")
    );
    assert!(records::created_of(&record, &game).is_empty());
}

#[test]
fn log_reading_handles_missing_and_oversized_files() {
    let temp = TempDir::new().unwrap();
    let log_path = temp.path().join("SMAPI-latest.txt");

    // No log yet: the game has not been launched through SMAPI.
    let missing = logs::read(&log_path, 1024).unwrap();
    assert!(!missing.exists);
    assert!(missing.content.is_empty());
    assert_eq!(missing.size, 0);

    fs::write(&log_path, "0123456789").unwrap();
    let full = logs::read(&log_path, 1024).unwrap();
    assert!(full.exists);
    assert_eq!(full.content, "0123456789");
    assert!(!full.truncated);
    assert_eq!(full.size, 10);
    assert!(full.modified_ms.is_some());

    // Only the newest slice reaches the UI.
    let tail = logs::read(&log_path, 4).unwrap();
    assert!(tail.truncated);
    assert_eq!(tail.content, "6789");
    assert_eq!(tail.size, 10);
}

/// Opt-in end-to-end check against a real SMAPI installer and the host game.
///
/// Needs `SMAPI_INSTALLER` (the official installer zip) and `STARDEW_GAME` (a real game
/// directory). The game is copied into a temporary directory; the original is only read.
#[test]
#[ignore = "requires a downloaded SMAPI installer and host game files"]
fn installs_the_real_release_into_a_copy_of_the_host_game() {
    let installer = PathBuf::from(std::env::var("SMAPI_INSTALLER").expect("SMAPI_INSTALLER"));
    let host = PathBuf::from(std::env::var("STARDEW_GAME").expect("STARDEW_GAME"));
    let temp = TempDir::new().unwrap();
    let game = temp.path().join("game");
    fs::create_dir_all(game.join("Content")).unwrap();
    for name in [
        "StardewValley",
        "Stardew Valley.dll",
        "StardewValley.dll",
        "Stardew Valley.deps.json",
        "steam_appid.txt",
    ] {
        let source = host.join(name);
        if source.is_file() {
            fs::copy(&source, game.join(name)).unwrap();
        }
    }

    let payload = archive::read_entry_by_suffix(&installer, "internal/linux/install.dat").unwrap();
    let bundle = temp.path().join("bundle");
    archive::extract_bytes(&payload, &bundle).unwrap();

    let report = install::install(&game, &bundle, GameRuntime::Linux, "probe").unwrap();

    assert!(report.outcome.launcher_replaced);
    assert!(game.join("smapi-internal/0Harmony.dll").is_file());
    assert!(game.join("Mods/SaveBackup/manifest.json").is_file());
    assert!(game.join("Mods/ConsoleCommands/manifest.json").is_file());
    assert_eq!(
        fs::read(game.join("StardewModdingAPI.deps.json")).unwrap(),
        fs::read(game.join("Stardew Valley.deps.json")).unwrap()
    );
    // The launcher now starts SMAPI, and the original is preserved byte for byte.
    let launcher = fs::read_to_string(game.join("StardewValley")).unwrap();
    assert!(launcher.contains("StardewModdingAPI"), "{launcher}");
    assert_eq!(
        fs::read(game.join("StardewValley-original")).unwrap(),
        fs::read(host.join("StardewValley")).unwrap()
    );

    // The install round-trips: uninstalling restores the launcher and removes everything
    // it added, including the bundled mods.
    let removed = uninstall(&game, GameRuntime::Linux, &report.created, false);
    assert!(removed.launcher_restored);
    assert!(!game.join("StardewModdingAPI").exists());
    assert!(!game.join("smapi-internal").exists());
    assert!(!game.join("Mods/SaveBackup").exists());
    assert_eq!(
        fs::read(game.join("StardewValley")).unwrap(),
        fs::read(host.join("StardewValley")).unwrap()
    );
}

/// Opt-in network check: resolve the latest release and verify the downloaded hash.
#[test]
#[ignore = "requires network access and downloads about 42 MiB"]
fn downloads_and_verifies_the_latest_release() {
    let release = release::fetch_latest().unwrap();
    assert!(
        release.version.split('.').count() >= 2,
        "unexpected version {}",
        release.version
    );
    assert!(
        release.sha256.is_some(),
        "GitHub stopped reporting a digest"
    );
    assert!(release.size > 1_000_000);

    let cache = TempDir::new().unwrap();
    let target = release::cached_path(cache.path(), &release);
    assert!(!target.is_file(), "the probe cache should start empty");

    // Progress must be reported repeatedly and end exactly at the published size.
    let mut seen: Vec<(u64, Option<u64>)> = Vec::new();
    let path = release::download(cache.path(), &release, |done, total| {
        seen.push((done, total))
    })
    .unwrap();

    assert_eq!(path, target);
    assert!(seen.len() > 1, "expected incremental progress");
    assert_eq!(seen.last().unwrap().0, release.size);
    assert_eq!(seen.last().unwrap().1, Some(release.size));
    release::verify(&path, release.sha256.as_deref()).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().len(), release.size);

    // A truncated archive is caught by verification instead of being installed.
    fs::write(&path, b"truncated").unwrap();
    assert_eq!(
        release::verify(&path, release.sha256.as_deref())
            .unwrap_err()
            .code,
        "smapiChecksumMismatch"
    );
}

#[test]
fn verify_checks_digests_without_network_access() {
    let temp = TempDir::new().unwrap();
    let archive = temp.path().join("archive.bin");
    fs::write(&archive, "smapi").unwrap();

    release::verify(
        &archive,
        Some("aa24ead4341ffe368cc5c73145c150ba8ec6c922fe249b54512cf7d93a3a1243"),
    )
    .unwrap();
    assert_eq!(
        release::verify(&archive, Some("00")).unwrap_err().code,
        "smapiChecksumMismatch"
    );
    // A release without a published digest is refused: the installer runs as code in the
    // game directory, so an unverifiable download must not be copied there.
    assert_eq!(
        release::verify(&archive, None).unwrap_err().code,
        "smapiDigestMissing"
    );
    assert_eq!(
        release::verify(&temp.path().join("missing"), Some("00"))
            .unwrap_err()
            .code,
        "smapiArchiveUnreadable"
    );
}

#[test]
fn cache_paths_use_only_the_asset_file_name() {
    // The asset name comes from the API response; a name carrying separators must not be
    // able to place the cached installer outside the cache directory.
    let release = super::model::SmapiRelease {
        version: "4.5.2".to_owned(),
        asset_name: "../../escape.zip".to_owned(),
        asset_url: "https://example/installer".to_owned(),
        sha256: None,
        size: 1,
    };
    assert_eq!(
        release::cached_path(Path::new("/cache"), &release),
        PathBuf::from("/cache/escape.zip")
    );

    let nameless = super::model::SmapiRelease {
        asset_name: String::new(),
        ..release
    };
    assert_eq!(
        release::cached_path(Path::new("/cache"), &nameless),
        PathBuf::from("/cache/smapi-installer.zip")
    );
}

#[test]
fn a_successful_install_leaves_no_scratch_directory() {
    let temp = TempDir::new().unwrap();
    let game = temp.path().join("game");
    let bundle_dir = temp.path().join("bundle");
    game_dir(&game, GameRuntime::Linux);
    bundle(&bundle_dir, GameRuntime::Linux);

    install::install(&game, &bundle_dir, GameRuntime::Linux, "4.5.2").unwrap();

    assert!(
        fs::read_dir(&game).unwrap().flatten().all(|entry| !entry
            .file_name()
            .to_string_lossy()
            .starts_with(".lingrid-install-")),
        "the scratch backup directory is removed after a successful install"
    );
}

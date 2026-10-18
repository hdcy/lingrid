use super::{install, manage, scan};
use std::{fs, path::Path};
use tempfile::TempDir;

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// A game directory holding just a `Mods` folder.
fn game() -> TempDir {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("Mods")).unwrap();
    temp
}

#[test]
fn reads_manifest_fields_and_the_nexus_id() {
    let temp = game();
    write(
        &temp.path().join("Mods/Alpha/manifest.json"),
        r#"{
            "Name": "Alpha Mod",
            "Author": "Someone",
            "Version": "1.2.3",
            "Description": "Does things",
            "UniqueID": "Someone.Alpha",
            "UpdateKeys": ["Nexus:1234", "GitHub:someone/alpha"]
        }"#,
    );

    let mods = scan::scan(temp.path()).unwrap();

    assert_eq!(mods.len(), 1);
    let alpha = &mods[0];
    assert_eq!(alpha.folder, "Alpha");
    assert_eq!(alpha.name, "Alpha Mod");
    assert_eq!(alpha.author, "Someone");
    assert_eq!(alpha.version, "1.2.3");
    assert_eq!(alpha.unique_id.as_deref(), Some("Someone.Alpha"));
    assert_eq!(alpha.description.as_deref(), Some("Does things"));
    assert_eq!(alpha.nexus_id, Some(1234));
    assert!(alpha.enabled);
    assert!(!alpha.manifest_missing);
}

#[test]
fn disabled_mods_are_reported_without_the_leading_dot() {
    let temp = game();
    write(
        &temp.path().join("Mods/.Sleepy/manifest.json"),
        r#"{"Name":"Sleepy","Author":"A","Version":"1.0.0","UniqueID":"A.Sleepy"}"#,
    );

    let mods = scan::scan(temp.path()).unwrap();

    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].folder, "Sleepy");
    assert!(!mods[0].enabled);
    assert_eq!(mods[0].name, "Sleepy");
}

#[test]
fn wrapped_packs_and_manifest_less_folders_are_handled() {
    let temp = game();
    // A pack that wraps its content in an extra directory (common on Nexus).
    write(
        &temp.path().join("Mods/Wrapped/inner/manifest.json"),
        r#"{"Name":"Wrapped","Author":"A","Version":"2.0.0"}"#,
    );
    // An XNB-style folder with no manifest at all.
    write(&temp.path().join("Mods/Plain/content.xnb"), "binary");
    // SMAPI's own scratch folder, which must never show up as a mod.
    write(&temp.path().join("Mods/.cache/whatever"), "cache");

    let mods = scan::scan(temp.path()).unwrap();

    let folders: Vec<&str> = mods.iter().map(|item| item.folder.as_str()).collect();
    assert_eq!(folders, ["Plain", "Wrapped"]);
    let wrapped = mods.iter().find(|item| item.folder == "Wrapped").unwrap();
    assert_eq!(wrapped.name, "Wrapped");
    assert!(!wrapped.manifest_missing);
    let plain = mods.iter().find(|item| item.folder == "Plain").unwrap();
    assert!(plain.manifest_missing);
    assert_eq!(plain.name, "Plain", "falls back to the folder name");
}

#[test]
fn a_missing_mods_folder_is_not_an_error() {
    let temp = TempDir::new().unwrap();
    assert!(scan::scan(temp.path()).unwrap().is_empty());
}

#[test]
fn toggling_renames_the_folder_and_never_clobbers() {
    let temp = game();
    let mods = temp.path().join("Mods");
    fs::create_dir_all(mods.join("Alpha")).unwrap();

    manage::set_enabled(&mods, "Alpha", false).unwrap();
    assert!(!mods.join("Alpha").exists());
    assert!(mods.join(".Alpha").is_dir());

    // The name is accepted with or without the dot, both ways.
    manage::set_enabled(&mods, ".Alpha", true).unwrap();
    assert!(mods.join("Alpha").is_dir());
    assert!(!mods.join(".Alpha").exists());

    // If something already sits at the target name, refuse instead of overwriting it.
    fs::create_dir_all(mods.join(".Alpha")).unwrap();
    assert_eq!(
        manage::set_enabled(&mods, "Alpha", false).unwrap_err().code,
        "modFolderTaken"
    );
    assert!(mods.join("Alpha").is_dir(), "the original stays put");
}

#[test]
fn removal_accepts_both_names_and_rejects_traversal() {
    let temp = game();
    let mods = temp.path().join("Mods");
    fs::create_dir_all(mods.join("Alpha")).unwrap();
    fs::create_dir_all(mods.join(".Beta")).unwrap();
    fs::create_dir_all(temp.path().join("Outside")).unwrap();

    manage::remove(&mods, "Alpha").unwrap();
    manage::remove(&mods, ".Beta").unwrap();
    assert!(!mods.join("Alpha").exists());
    assert!(!mods.join(".Beta").exists());

    for bad in ["../Outside", "..", "", ".", "nested/Path"] {
        assert_eq!(
            manage::remove(&mods, bad).unwrap_err().code,
            "modFolderInvalid",
            "{bad} must be rejected"
        );
    }
    assert!(temp.path().join("Outside").is_dir(), "nothing escaped");

    assert_eq!(
        manage::set_enabled(&mods, "../Outside", false)
            .unwrap_err()
            .code,
        "modFolderInvalid"
    );
    assert_eq!(
        manage::remove(&mods, "Missing").unwrap_err().code,
        "modNotFound"
    );
}

fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    use zip::{write::SimpleFileOptions, ZipWriter};

    let mut buffer = std::io::Cursor::new(Vec::new());
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

fn archive(temp: &TempDir, name: &str, entries: &[(&str, &[u8])]) -> std::path::PathBuf {
    let path = temp.path().join(name);
    fs::write(&path, zip_bytes(entries)).unwrap();
    path
}

const ALPHA: &[u8] = br#"{"Name":"Alpha","Author":"A","Version":"1.0.0","UniqueID":"A.Alpha"}"#;
const BETA: &[u8] = br#"{"Name":"Beta","Author":"B","Version":"2.0.0","UniqueID":"B.Beta"}"#;

#[test]
fn installs_a_pack_that_wraps_its_mod_in_a_folder() {
    let temp = game();
    let file = archive(
        &temp,
        "SomeMod-1.0.zip",
        &[
            ("SomeMod/manifest.json", ALPHA),
            ("SomeMod/Mod.dll", b"binary"),
            ("readme.txt", b"hello"),
        ],
    );

    let report = install::install(temp.path(), &file).unwrap();

    assert_eq!(report.folders, ["SomeMod"]);
    assert!(temp.path().join("Mods/SomeMod/manifest.json").is_file());
    assert!(temp.path().join("Mods/SomeMod/Mod.dll").is_file());
    // The mod list picks it up straight away.
    let mods = scan::scan(temp.path()).unwrap();
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].name, "Alpha");
    assert_eq!(mods[0].unique_id.as_deref(), Some("A.Alpha"));
}

#[test]
fn installs_a_mod_that_sits_at_the_archive_root() {
    let temp = game();
    let file = archive(
        &temp,
        "Root Mod-2.0.0.zip",
        &[("manifest.json", BETA), ("Beta.dll", b"binary")],
    );

    let report = install::install(temp.path(), &file).unwrap();

    // The archive name is used, with the extension stripped.
    assert_eq!(report.folders, ["Root Mod-2.0.0"]);
    assert!(temp
        .path()
        .join("Mods/Root Mod-2.0.0/manifest.json")
        .is_file());
}

#[test]
fn installs_every_mod_in_a_multi_mod_archive() {
    let temp = game();
    let file = archive(
        &temp,
        "Pack.zip",
        &[
            ("First/manifest.json", ALPHA),
            ("Second/nested/manifest.json", BETA),
        ],
    );

    let report = install::install(temp.path(), &file).unwrap();

    assert_eq!(report.folders, ["First", "Second"]);
    assert_eq!(scan::scan(temp.path()).unwrap().len(), 2);
}

#[test]
fn refuses_archives_without_a_manifest_and_colliding_names() {
    let temp = game();
    let not_a_mod = archive(&temp, "Notes.zip", &[("readme.txt", b"nothing here")]);
    assert_eq!(
        install::install(temp.path(), &not_a_mod).unwrap_err().code,
        "modArchiveInvalid"
    );

    let file = archive(&temp, "Mod.zip", &[("Alpha/manifest.json", ALPHA)]);
    install::install(temp.path(), &file).unwrap();
    assert_eq!(
        install::install(temp.path(), &file).unwrap_err().code,
        "modFolderTaken"
    );
    assert!(
        temp.path().join("Mods/Alpha/manifest.json").is_file(),
        "the installed copy is untouched"
    );
}

#[test]
fn install_rejects_traversal_and_never_disables_what_it_installs() {
    let temp = game();
    let escaping = archive(&temp, "Evil.zip", &[("../escaped/manifest.json", ALPHA)]);
    assert_eq!(
        install::install(temp.path(), &escaping).unwrap_err().code,
        "modArchiveInvalid"
    );
    assert!(!temp.path().join("escaped").exists());

    // A leading dot in the archive name must not produce a disabled folder.
    let dotted = archive(&temp, ".Hidden.zip", &[("manifest.json", BETA)]);
    let report = install::install(temp.path(), &dotted).unwrap();
    assert_eq!(report.folders, ["Hidden"]);
    assert!(scan::scan(temp.path()).unwrap()[0].enabled);
}

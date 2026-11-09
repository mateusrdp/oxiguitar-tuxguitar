// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Reading and writing: TuxGuitar's 2.x test files and our own fixture
//! read, write and read back as they were.

use std::path::Path;

use oxiguitar_model::{Score, wire};

fn fixture(path: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(path),
    )
    .unwrap()
}

fn read(data: &[u8]) -> Score {
    wire::from_wire2(oxiguitar_tuxguitar_plugin::read(data).unwrap().score).unwrap()
}

fn write(score: &Score) -> (Vec<u8>, Vec<oxiguitar_plugin::LogEntry>) {
    let written = oxiguitar_tuxguitar_plugin::write(&wire::to_wire2(score)).unwrap();
    (written.data, written.losses)
}

#[test]
fn tuxguitars_files_round_trip() {
    for name in [
        "reference_20.tg",
        "Untitled_20.tg",
        "altRepeatLoop_20.tg",
        "fix_20.tg",
        "freeEditionMode_20.tg",
        "getNextTied_20.tg",
        "letRing_20.tg",
        "tempo_20.tg",
        "test_midi_20.tg",
        "tiedNotes_20.tg",
        "track_pitch_20.tg",
    ] {
        let first = read(&fixture(&format!("tuxguitar/{name}")));
        let (data, losses) = write(&first);
        assert!(losses.is_empty(), "{name}: {losses:#?}");
        let second = read(&data);
        assert_eq!(first, second, "{name}");
    }
}

/// What the plugin writes validates against TuxGuitar's schema of the 2.0
/// format (`fixtures/tuxguitar/tuxguitar_20.xsd`), checked with xmllint
/// where it is installed (as on CI's Ubuntu); skipped elsewhere.
#[test]
fn what_is_written_validates_against_tuxguitars_schema() {
    use std::io::Read;
    use std::process::Command;
    if Command::new("xmllint").arg("--version").output().is_err() {
        println!("no xmllint: skipped");
        return;
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("schema");
    std::fs::create_dir_all(&dir).unwrap();
    let schema = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/tuxguitar/tuxguitar_20.xsd");
    for name in [
        "tuxguitar/reference_20.tg",
        "tuxguitar/test_midi_20.tg",
        "oxiguitar-test.tg",
    ] {
        let (data, _) = write(&read(&fixture(name)));
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(data)).unwrap();
        let mut xml = Vec::new();
        zip.by_name("content.xml")
            .unwrap()
            .read_to_end(&mut xml)
            .unwrap();
        let path = dir.join(name.replace('/', "-") + ".xml");
        std::fs::write(&path, xml).unwrap();
        let out = Command::new("xmllint")
            .arg("--noout")
            .arg("--schema")
            .arg(&schema)
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// The core's test song, saved by this plugin (`oxiguitar-cli convert`),
/// reads back exactly.
#[test]
fn our_fixture_round_trips() {
    let first = read(&fixture("oxiguitar-test.tg"));
    assert_eq!(first.info.title, "OxiGuitar Test Song");
    let (data, losses) = write(&first);
    assert!(losses.is_empty(), "{losses:#?}");
    assert_eq!(first, read(&data));
}

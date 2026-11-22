// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The test corpus (`tests/support/corpus.rs`: `OXIGUITAR_CORPUS`, or
//! `corpus/files/`; never committed): every TuxGuitar 2 file in it,
//! recognised by its contents as the manifest's signature does (a ZIP
//! holding `version.txt`), reads, writes and reads back: exact from the
//! first trip where the writer names no losses, stable from the second
//! otherwise. Any other file is left alone. `cargo test --test corpus --
//! --nocapture`; with no corpus it checks nothing (unless
//! `OXIGUITAR_CORPUS_REQUIRED` is set).

#[path = "support/corpus.rs"]
mod corpus;

use std::path::Path;

use corpus::Outcome;
use oxiguitar_model::{Score, wire};

/// A ZIP holding `version.txt` (the manifest's `zip_member` signature).
fn is_tuxguitar(data: &[u8]) -> bool {
    data.starts_with(b"PK")
        && zip::ZipArchive::new(std::io::Cursor::new(data))
            .is_ok_and(|mut z| z.by_name("version.txt").is_ok())
}

fn read(data: &[u8], what: &str) -> Result<Score, String> {
    let read = oxiguitar_tuxguitar_plugin::read(data)
        .map_err(|e| format!("{what} doesn't read: {}", e.message))?;
    wire::from_wire2(read.score).map_err(|e| e.to_string())
}

fn write(score: &Score, what: &str) -> Result<(Vec<u8>, usize), String> {
    let written = oxiguitar_tuxguitar_plugin::write(&wire::to_wire2(score))
        .map_err(|e| format!("{what} doesn't write: {}", e.message))?;
    Ok((written.data, written.losses.len()))
}

fn round_trip(data: &[u8]) -> Result<Outcome, String> {
    if !is_tuxguitar(data) {
        return Ok(Outcome::NotTaken);
    }
    let first = read(data, "the file")?;
    let (data, losses) = write(&first, "it")?;
    let second = read(&data, "what it wrote")?;
    if losses == 0 {
        if first != second {
            return Err("doesn't round-trip, though the writer named no losses".into());
        }
    } else {
        let (data, _) = write(&second, "it a second time")?;
        let third = read(&data, "what it wrote the second time")?;
        if second != third {
            return Err(format!(
                "doesn't settle after one trip ({losses} kinds of loss named)"
            ));
        }
    }
    Ok(Outcome::Passed)
}

#[test]
fn every_corpus_tuxguitar_file_round_trips() {
    let fallback = Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/files");
    corpus::check("tuxguitar round trip", &fallback, |_, data| {
        round_trip(data)
    });
}

/// The picking itself: what the signature takes and what it leaves.
#[test]
fn tuxguitar_files_are_recognised_by_their_contents() {
    let ours =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/oxiguitar-test.tg"))
            .expect("the fixture");
    assert!(is_tuxguitar(&ours));
    assert!(!is_tuxguitar(b"PK\x03\x04 not a zip"));
    assert!(!is_tuxguitar(b"FICHIER GUITAR PRO v3.00"));
    assert!(!is_tuxguitar(b""));
}

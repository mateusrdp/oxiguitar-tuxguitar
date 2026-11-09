// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The TuxGuitar format plugin (`tuxguitar.oxp`, id `oxiguitar.tuxguitar`):
//! reads and writes TuxGuitar 2's files (`.tg`), #223 and #171.
//!
//! A TuxGuitar 2 file is a ZIP holding `version.txt` (the format version,
//! `TuxGuitar_file_format 2.0`) and `content.xml` (the song). No Rust
//! library reads or writes them, so this is our own code, ported from
//! TuxGuitar's (LGPL-2.1), on the roxmltree, quick-xml and zip
//! crates. The binary `.tg` of TuxGuitar 0.7 to 1.5 is not read: TuxGuitar
//! 2 opens those and saves them in the newer form. The host recognises the
//! files from the signature in `manifest.json` (a ZIP holding
//! `version.txt`), without loading the plugin. Design and decisions:
//! `docs/design.md`.

mod container;
mod log;
mod read;
mod spell;
mod tg;
mod time;
mod write;

use oxiguitar_model::wire;
use oxiguitar_plugin::{
    Error, ErrorKind, FormatReader2, FormatWriter2, Host, Plugin, Read2, Written, score2,
};

/// The id of TuxGuitar files in `manifest.json`.
pub const TG: &str = "tg";

/// Reads a TuxGuitar 2 file into OxiGuitar's score and its import log.
pub fn read(data: &[u8]) -> Result<Read2, Error> {
    let imported = read::read(data).map_err(invalid)?;
    imported.score.check_structure().map_err(invalid)?;
    Ok(Read2 {
        score: wire::to_wire2(&imported.score),
        log: imported.log.into_entries(),
    })
}

/// Writes `score` as a TuxGuitar 2 file, with what the file can't hold.
pub fn write(score: &score2::Score2) -> Result<Written, Error> {
    let score = wire::from_wire2(score.clone()).map_err(invalid)?;
    let (data, losses) = write::write(&score).map_err(invalid)?;
    Ok(Written {
        data,
        losses: losses.into_entries(),
    })
}

/// A file or score that can't be read or written, as the interface carries
/// it.
fn invalid(message: String) -> Error {
    Error {
        kind: ErrorKind::Invalid,
        message,
        offset: None,
        context: String::new(),
    }
}

/// The plugin.
#[derive(Debug)]
pub struct TuxGuitar;

impl Plugin for TuxGuitar {
    fn start(_host: Host) -> Result<TuxGuitar, Error> {
        Ok(TuxGuitar)
    }

    // The score version 2 (interface 1.2 and later), which holds the
    // palette's marks (1.6), so that the writer names them as losses.
    fn reader2(&self, format: &str) -> Option<&dyn FormatReader2> {
        (format == TG).then_some(self as &dyn FormatReader2)
    }

    fn writer2(&self, format: &str) -> Option<&dyn FormatWriter2> {
        (format == TG).then_some(self as &dyn FormatWriter2)
    }
}

impl FormatReader2 for TuxGuitar {
    fn read(&self, data: &[u8]) -> Result<Read2, Error> {
        read(data)
    }
}

impl FormatWriter2 for TuxGuitar {
    fn write(&self, score: &score2::Score2) -> Result<Written, Error> {
        write(score)
    }
}

oxiguitar_plugin::export_plugin!(TuxGuitar);

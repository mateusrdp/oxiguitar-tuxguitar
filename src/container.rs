// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The ZIP container of a TuxGuitar 2 file: `version.txt`, one line naming
//! the format version, and `content.xml`, the song. Opened and written with
//! the zip crate, as in the MusicXML plugin.

use std::io::{Cursor, Read, Write};

use zip::write::SimpleFileOptions;

use crate::tg::{CONTENT_FILE, MAJOR, MINOR, VERSION_FILE, VERSION_PREFIX};

/// The largest `content.xml` accepted, uncompressed: far beyond any real
/// song (a 300-bar, 8-track one is a few megabytes), and small enough that
/// a file claiming more can't exhaust memory.
const MAX_CONTENT: u64 = 256 * 1024 * 1024;

/// The largest `version.txt` read: one short line.
const MAX_VERSION: u64 = 1024;

/// A file's format version, from `version.txt`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Version {
    /// The major version: 2 for every file this plugin reads.
    pub(crate) major: u32,
    /// The minor version.
    pub(crate) minor: u32,
}

/// What a TuxGuitar 2 file holds: its version and its song's XML.
#[derive(Debug)]
pub(crate) struct Opened {
    pub(crate) version: Version,
    pub(crate) content: Vec<u8>,
}

/// Opens a TuxGuitar file's container and checks its version: major 2 is
/// read, any minor (a newer one with a note, as TuxGuitar does); another
/// major, or no version, is refused. Members other than the two are
/// ignored (TuxGuitar refuses such a file; nothing in them is read).
pub(crate) fn open(data: &[u8]) -> Result<Opened, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(data)).map_err(|e| {
        format!(
            "not a TuxGuitar 2 file ({e}); a file from TuxGuitar 1.5 or older opens in \
             TuxGuitar 2, which saves it in the newer form"
        )
    })?;
    let version = read_member(&mut archive, VERSION_FILE, MAX_VERSION)?;
    let version = parse_version(&version)?;
    if version.major != MAJOR {
        return Err(format!(
            "TuxGuitar file format {}.{}: OxiGuitar reads format {MAJOR}.x, the one \
             TuxGuitar 2 writes",
            version.major, version.minor
        ));
    }
    let content = read_member(&mut archive, CONTENT_FILE, MAX_CONTENT)?;
    Ok(Opened { version, content })
}

/// `version.txt`'s first line: `TuxGuitar_file_format MAJOR.MINOR`, a
/// revision after a second dot allowed.
fn parse_version(data: &[u8]) -> Result<Version, String> {
    let text = String::from_utf8_lossy(data);
    let line = text.lines().next().unwrap_or("").trim_end_matches('\r');
    let bad = || format!("{VERSION_FILE} doesn't name a TuxGuitar file format: {line:?}");
    let mut words = line.split(' ');
    let (Some(prefix), Some(number), None) = (words.next(), words.next(), words.next()) else {
        return Err(bad());
    };
    if prefix != VERSION_PREFIX {
        return Err(bad());
    }
    let mut parts = number.split('.');
    let major = parts.next().and_then(|p| p.parse().ok()).ok_or_else(bad)?;
    let minor = match parts.next() {
        Some(p) => p.parse().map_err(|_| bad())?,
        None => 0,
    };
    Ok(Version { major, minor })
}

/// One member's contents, at most `max` bytes.
fn read_member(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    name: &str,
    max: u64,
) -> Result<Vec<u8>, String> {
    let file = archive
        .by_name(name)
        .map_err(|e| format!("not a TuxGuitar 2 file: its {name}: {e}"))?;
    let mut out = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("the file's {name}: {e}"))?;
    if out.len() as u64 > max {
        return Err(format!("the file's {name} is too large"));
    }
    Ok(out)
}

/// A TuxGuitar 2 file holding `xml` as its song: `version.txt` then
/// `content.xml`, deflated, as TuxGuitar writes them.
pub(crate) fn archive(xml: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let deflated =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let version = format!("{VERSION_PREFIX} {MAJOR}.{MINOR}");
    let mut put = |name: &str, data: &[u8]| -> Result<(), String> {
        out.start_file(name, deflated).map_err(|e| e.to_string())?;
        out.write_all(data).map_err(|e| e.to_string())
    };
    put(VERSION_FILE, version.as_bytes())?;
    put(CONTENT_FILE, xml)?;
    let cursor = out.finish().map_err(|e| e.to_string())?;
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        let v = |s: &str| parse_version(s.as_bytes());
        assert_eq!(
            v("TuxGuitar_file_format 2.0"),
            Ok(Version { major: 2, minor: 0 })
        );
        assert_eq!(
            v("TuxGuitar_file_format 2.1\n"),
            Ok(Version { major: 2, minor: 1 })
        );
        assert_eq!(
            v("TuxGuitar_file_format 3.0.1\r\n"),
            Ok(Version { major: 3, minor: 0 })
        );
        assert!(v("lsmdhmofdnmcx 2.0\n").is_err());
        assert!(v("TuxGuitar_file_format").is_err());
        assert!(v("TuxGuitar_file_format 2.x").is_err());
        assert!(v("").is_err());
    }

    #[test]
    fn an_archive_opens_again() {
        let xml = b"<TuxGuitarFile/>";
        let opened = open(&archive(xml).unwrap()).unwrap();
        assert_eq!(opened.version, Version { major: 2, minor: 0 });
        assert_eq!(opened.content, xml);
        assert!(open(b"PK\x03\x04 cut short").is_err());
        assert!(open(b"").is_err());
    }
}

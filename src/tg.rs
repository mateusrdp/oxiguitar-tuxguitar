// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
//
// The numbers and names below are TuxGuitar's (LGPL-2.1,
// github.com/helge17/tuxguitar: common/TuxGuitar-lib/src/main/java/app/
// tuxguitar/io/tg/TGStream.java and song/models/), ported to Rust.

//! What TuxGuitar 2's file format fixes: its version line, its unit of
//! time, its velocities, its harmonic table and its value names.

use oxiguitar_model::{Dynamic, NoteValue};

/// The member of the ZIP that names the format version.
pub(crate) const VERSION_FILE: &str = "version.txt";

/// The member of the ZIP that holds the song.
pub(crate) const CONTENT_FILE: &str = "content.xml";

/// The first word of `version.txt`.
pub(crate) const VERSION_PREFIX: &str = "TuxGuitar_file_format";

/// The format version this plugin reads fully and writes.
pub(crate) const MAJOR: u32 = 2;
/// See [`MAJOR`].
pub(crate) const MINOR: u32 = 0;

/// A whole note in TuxGuitar's precise time (`TGDuration`'s
/// `WHOLE_PRECISE_DURATION`): four times the least common multiple of the
/// shortest value (a sixty-fourth) and every tuplet it knows (3, 5, 6, 7,
/// 9, 10, 11, 12 and 13 in the time of 2, 4 or 8), so that every duration
/// it can write is a whole number of units.
pub(crate) const WHOLE: i64 = 4 * 64 * 9 * 5 * 7 * 11 * 13;

/// Where a song starts in precise time: a quarter note in, by TuxGuitar's
/// convention.
pub(crate) const START: i64 = WHOLE / 4;

/// TuxGuitar's velocities: the dynamics from ppp to fff, 15 to 127 in
/// steps of 16 (`TGVelocities`).
pub(crate) const VELOCITIES: [(i64, Dynamic); 8] = [
    (15, Dynamic::PPP),
    (31, Dynamic::PP),
    (47, Dynamic::P),
    (63, Dynamic::MP),
    (79, Dynamic::MF),
    (95, Dynamic::F),
    (111, Dynamic::FF),
    (127, Dynamic::FFF),
];

/// The velocity a note has when the file gives none (forte).
pub(crate) const DEFAULT_VELOCITY: i64 = 95;

/// The dynamic nearest a velocity, and whether it was exact.
pub(crate) fn dynamic(velocity: i64) -> (Dynamic, bool) {
    let (v, d) = VELOCITIES
        .iter()
        .copied()
        .min_by_key(|(v, _)| (v - velocity).abs())
        .unwrap_or((DEFAULT_VELOCITY, Dynamic::F));
    (d, v == velocity)
}

/// A dynamic's velocity.
pub(crate) fn velocity(dynamic: Dynamic) -> i64 {
    VELOCITIES
        .iter()
        .find(|(_, d)| *d == dynamic)
        .map_or(DEFAULT_VELOCITY, |(v, _)| *v)
}

/// TuxGuitar's harmonics other than natural ones (`TGEffectHarmonic`'s
/// `NATURAL_FREQUENCIES`): the file stores an index into this table. Each
/// row is the frets above the fretted note where the string is touched
/// (AH12, AH9 …) and the semitones the harmonic sounds above the fretted
/// note.
pub(crate) const HARMONICS: [(u8, u8); 6] = [(12, 12), (9, 28), (5, 24), (7, 19), (4, 28), (3, 31)];

/// A bend point's position: twelfths of the note.
pub(crate) const BEND_POSITIONS: i64 = 12;

/// A bend's height unit: a quarter tone (TuxGuitar reads a Guitar Pro
/// bend's 25 units, 50 cents, as one).
pub(crate) const BEND_CENTS: i32 = 50;

/// A tremolo bar's height unit: a semitone (TuxGuitar reads a Guitar Pro
/// whammy's 50 units, 100 cents, as one).
pub(crate) const WHAMMY_CENTS: i32 = 100;

/// The largest bend and dive TuxGuitar's editors make, in their units.
pub(crate) const MAX_BEND: i32 = 12;

/// TuxGuitar's offset range, in semitones (`TGTrack`).
pub(crate) const MAX_OFFSET: i64 = 24;

/// The most strings a TuxGuitar track has.
pub(crate) const MAX_STRINGS: usize = 25;

/// A note value from TuxGuitar's number for it (1 whole … 64 sixty-fourth).
pub(crate) fn value(n: i64) -> Option<NoteValue> {
    u16::try_from(n).ok().and_then(NoteValue::from_denominator)
}

/// The tuplets TuxGuitar knows (`TGDivisionType`), as notes in the time
/// of.
pub(crate) const TUPLETS: [(u8, u8); 10] = [
    (1, 1),
    (3, 2),
    (5, 4),
    (6, 4),
    (7, 4),
    (9, 8),
    (10, 8),
    (11, 8),
    (12, 8),
    (13, 8),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unit_of_time_is_tuxguitars() {
        // TuxGuitar's reference file starts at 2882880 and its second bar,
        // after one of 4/4, at 14414400.
        assert_eq!(START, 2_882_880);
        assert_eq!(START + WHOLE, 14_414_400);
        for (enters, times) in TUPLETS {
            let shortest = WHOLE * i64::from(times) / (i64::from(enters) * 64);
            assert_eq!(shortest * i64::from(enters) * 64, WHOLE * i64::from(times));
        }
    }

    #[test]
    fn velocities_and_dynamics() {
        assert_eq!(dynamic(95), (Dynamic::F, true));
        assert_eq!(dynamic(100), (Dynamic::F, false));
        assert_eq!(dynamic(0), (Dynamic::PPP, false));
        assert_eq!(dynamic(200), (Dynamic::FFF, false));
        for (v, d) in VELOCITIES {
            assert_eq!(velocity(d), v);
        }
    }
}

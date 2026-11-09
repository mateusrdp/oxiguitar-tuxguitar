// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! TuxGuitar's "alternative enharmonic": a note spelled with its other
//! name (`TGMusicKeyUtils`: its spelling in C sharp major where that
//! differs from the usual one, else in C flat major). OxiGuitar keeps a
//! spelling only where it differs from its own ([`Spelling::automatic`]),
//! so a note marked in TuxGuitar gets the other name of OxiGuitar's: the
//! flat of a sharp, the sharp of a flat, and for E, F, B and C the names
//! F flat, E sharp, C flat and B sharp.

use oxiguitar_model::{KeySignature, Pitch, Spelling};

/// The other name of written `pitch` in `key`, if it has one (D, G and A
/// have none with a single accidental).
pub(crate) fn alternative(pitch: Pitch, key: KeySignature) -> Option<Spelling> {
    let usual = Spelling::automatic(pitch, key);
    if usual.alter != 0 {
        Spelling::with_alter(pitch, -usual.alter)
    } else {
        Spelling::with_alter(pitch, 1).or_else(|| Spelling::with_alter(pitch, -1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(step: u8, alter: i8) -> Option<Spelling> {
        Some(Spelling { step, alter })
    }

    #[test]
    fn the_other_names() {
        let c = KeySignature::default();
        assert_eq!(
            alternative(Pitch(61), c),
            s(1, -1),
            "C sharp becomes D flat"
        );
        assert_eq!(alternative(Pitch(64), c), s(3, -1), "E becomes F flat");
        assert_eq!(alternative(Pitch(65), c), s(2, 1), "F becomes E sharp");
        assert_eq!(alternative(Pitch(60), c), s(6, 1), "C becomes B sharp");
        assert_eq!(alternative(Pitch(71), c), s(0, -1), "B becomes C flat");
        assert_eq!(alternative(Pitch(62), c), None, "D has no other name");
        let f = KeySignature {
            fifths: -1,
            ..KeySignature::default()
        };
        assert_eq!(alternative(Pitch(70), f), s(5, 1), "B flat becomes A sharp");
    }
}

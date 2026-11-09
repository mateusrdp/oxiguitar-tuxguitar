# The TuxGuitar plugin: design

TuxGuitar is the free tablature editor OxiGuitar takes as its functional starting point. This plugin (#223 and #171 in github.com/mateusrdp/oxiguitar) reads and writes TuxGuitar's own files (`.tg`), so a TuxGuitar user's songs open in OxiGuitar and OxiGuitar's open in TuxGuitar. It is the last of the format plugins planned before v1.0 (MuseScore and ABC were dropped on 9 Nov 2026).

Status: planned (session 38, 9 Nov 2026). Nothing is built yet.

## Decided

- **Versions** (session 38; Mateus had no preference, so the recommended choice): read the binary `.tg` of TuxGuitar 1.0 to 1.5 and the ZIP-and-XML `.tg` of TuxGuitar 2.x; write the 2.x form, as TuxGuitar 2.1 writes it. TuxGuitar 0.7 to 0.9 files (its compatibility readers still open them) only if they turn out cheap once 1.0 reads.
- **No library**: there is no Rust library for `.tg`, so the reader and writer are our own code, ported from TuxGuitar's Java (`common/TuxGuitar-lib/.../io/tg/` for 2.x, `common/TuxGuitar-compat/.../io/tg/v10` to `v15` for 1.x, in github.com/helge17/tuxguitar). TuxGuitar is LGPL-2.1; code ported from it may go into this GPL-3.0-or-later plugin, with a notice in the files that hold it. Mateus's standing rule (9 Nov 2026): always delegate to a mature existing library when one exists and doesn't cause trouble; general crates do the rest here: **roxmltree** to read `content.xml`, **quick-xml** to write it, **zip** (no default features, `deflate-flate2-zlib-rs`, as the MusicXML plugin) for the container.
- **One plugin, one id** (`oxiguitar.tuxguitar`, file `tuxguitar.oxp`), with two formats in its manifest so each form has its own signature: `tg` (2.x, read and write) and `tg1` (1.x, read only), both extension `tg`.

## Recognising the files

Both forms can be told apart with interface 1.7's tests, so the plugin needs no core change:

- **2.x**: a ZIP holding `version.txt` and `content.xml`. A signature names one ZIP member, so `{"zip_member": "version.txt"}`, and the reader checks the rest. `version.txt` is one line, `TuxGuitar_file_format MAJOR.MINOR` (with or without a newline): major 2 is read, a newer minor is read with a note in the log (TuxGuitar does the same), another major is refused with a message naming the version.
- **1.x**: a length byte, then `TuxGuitar File Format - 1.N` in UTF-16BE (`1b 00 54 00 75 ...`): `{"at": 1, "hex": "005400750078004700750069007400610072002000460069006c006500200046006f0072006d00610074"}` ("TuxGuitar File Format"). The reader takes the version from the rest of the string.

## Reading

TuxGuitar's model follows Guitar Pro's, as OxiGuitar's does, so most of it maps one to one:

- **Song**: name → title, artist, album, author → music, writer → words, copyright, transcriber, comments → notices (one a line), date → a notice ("Date: ...").
- **Channels** (`TGChannel`, shared by tracks): bank 128 = percussion; program, volume, balance, chorus, reverb, phaser, tremolo → the track's `Instrument` and `Mix` (TuxGuitar's 0–127 scales as Guitar Pro's).
- **Measure headers** → master bars: time signature, tempo (quarter notes a minute; 2.x's `base` and `dotted` convert), repeat open, repeat close (TuxGuitar counts repeats, OxiGuitar counts plays: close + 1), alternatives (bit set), marker with colour, triplet feel, line breaks (Design Mode's bars per system, `Score::layout`). A tempo that changes from the bar before becomes a tempo change at the bar's start.
- **Tracks**: name, colour, strings (highest first, as OxiGuitar's), offset → capo (TuxGuitar's offset transposes the whole track; a positive offset is a capo), max fret, solo/mute, lyrics (`TGLyric from` → a lyric line from that bar).
- **Measures**: clef, key signature (TuxGuitar's 0–14: 1–7 sharps, 8–14 flats), beats by `preciseStart`. A track's beats become up to two voices (TuxGuitar has two), each beat's duration (value, dotted, double dotted, `divisionType enters/times` → tuplet), rests from empty voices, gaps filled with rests where a voice starts late.
- **Beats**: strokes (direction and speed), chords (name, first fret, a fret per string), text, pick strokes.
- **Notes**: string, fret, velocity → dynamic, tied notes, and the effects: dead and ghost notes, accents (normal and heavy), palm mute, staccato, let ring, tapping, slapping, popping, fade in, vibrato, hammer-on/pull-off, slides, bends and tremolo-bar dives (points with position and value), harmonics (natural, artificial, tapped, pinch, semi), grace notes (fret, duration, dynamic, on-beat, dead, transition), trills, tremolo picking, alternative enharmonics → spellings.
- Anything left out is named in the import log, as the other format plugins do.

## Writing

The 2.x form exactly as TuxGuitar 2.1 writes it: `version.txt` (`TuxGuitar_file_format 2.0`, as current TuxGuitar writes it, so TuxGuitar 2.0 opens it too) and `content.xml`, the reverse of reading. One channel a track (TuxGuitar's channels are shared, OxiGuitar's tracks own theirs). Losses, named before anything is written, for what `.tg` can't hold: voices past the second, staves past the first (a grand staff), the palette's marks of v0.17 (slurs, hairpins, ottavas, fermatas, pedal, ornaments, rasgueado, golpe, barres, simile signs, beaming), navigation signs and jumps (TuxGuitar has none), standard-notation transposition, chord diagrams' extra fingering, lyrics past the first line, a track's sound settings in `FILE.oxg`.

## Tests

- **TuxGuitar's own test files** (`reference_15.tg`, `reference_20.tg`, `Untitled_15.tg`, `Untitled_extended_21.tg`, `Untitled_extended_30.tg` for the refused major version, `invalidContent.tg`, `noVersion.tg`, `notTuxGuitar.tg` and the feature files `letRing_20.tg`, `tempo_20.tg`, `tiedNotes_20.tg`, `altRepeatLoop_20.tg`, `track_pitch_20.tg`, `test_midi_20.tg`) as fixtures, with their LGPL-2.1 notice (`fixtures/tuxguitar/NOTICE`), checked note by note against what TuxGuitar's tests say they hold.
- **Our own fixture**: the core's test song written by the plugin (`fixtures/oxiguitar-test.tg`), read back exactly (`tests/read_write.rs`).
- **The private corpus**, locally (`tests/corpus.rs`, files in the ignored `corpus/files/`): all 104 Guitar Pro files converted to `.tg` with `oxiguitar-cli convert`, read back with the same tracks, note counts, bars and bar check; exact from the first round trip where the writer names no losses, stable from the second otherwise.
- The contract suite (`tests/contract.rs`), damaged and hostile input, and fuzz targets for reading both forms and for writing (`fuzz/`).
- Still wanted from Mateus: `.tg` files of his own, and a look at what the plugin writes in TuxGuitar 2.1.

## Building it

From the MIDI file plugin's repository (START-HERE, "A new format plugin"): `.github/workflows/{ci,fuzz,release,publish}.yml`, `build-oxp.sh` (`NAME=tuxguitar`, `LIBRARY=oxiguitar_tuxguitar_plugin`), `fuzz/`, `src/log.rs`, `tests/contract.rs`; Cargo.toml and manifest.json at 0.0.0, interface 1.7 (score version 2, so the palette's marks are named as losses rather than dropped silently); a README with a picture of our fixture; then a line in oxiguitar-packages' `repositories.txt`, and the core's manual (official-plugins.md, opening.md) and ROADMAP.

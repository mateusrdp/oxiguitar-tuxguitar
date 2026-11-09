// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! TuxGuitar's own test files (fixtures/tuxguitar, LGPL-2.1, see
//! its NOTICE), read as TuxGuitar's tests say they hold: what
//! TestFileFormat20.java asserts of `test_20.xml` and of `reference_20.tg`,
//! in OxiGuitar's terms, and the files TuxGuitar refuses.

use std::io::Write;
use std::path::Path;

use oxiguitar_model::{
    Accent, BeatKind, BeatTechnique, Clef, Dynamic, Fade, GraceKind, Harmonic, NotePlace,
    NoteValue, Pitch, Ratio, Score, Slide, StrokeDirection, TripletFeel, Tuplet, Vibrato, wire,
};
use oxiguitar_plugin::{Level, LogEntry};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/tuxguitar")
            .join(name),
    )
    .unwrap()
}

fn read(data: &[u8]) -> (Score, Vec<LogEntry>) {
    let read = oxiguitar_tuxguitar_plugin::read(data).unwrap();
    (wire::from_wire2(read.score).unwrap(), read.log)
}

/// A TuxGuitar 2 file around `xml`, as TuxGuitar's tests make one.
fn tg(xml: &[u8]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("version.txt", options).unwrap();
    zip.write_all(b"TuxGuitar_file_format 2.0").unwrap();
    zip.start_file("content.xml", options).unwrap();
    zip.write_all(xml).unwrap();
    zip.finish().unwrap().into_inner()
}

fn r(n: i64, d: i64) -> Ratio {
    Ratio::new(n, d).unwrap()
}

fn logged(log: &[LogEntry], text: &str) -> bool {
    log.iter().any(|e| e.message.contains(text))
}

#[test]
fn test_20_reads_as_tuxguitar_says() {
    let (score, log) = read(&tg(&fixture("test_20.xml")));
    let info = &score.info;
    assert_eq!(info.title, "TGSong.name");
    assert_eq!(info.artist, "TGSong.artist");
    assert_eq!(info.album, "TGSong.album");
    assert_eq!(info.music, "TGSong.author");
    assert_eq!(info.words, "TGSong.writer");
    assert_eq!(info.copyright, "TGSong.copyright");
    assert_eq!(info.transcriber, "TGSong.transcriber");
    assert_eq!(
        info.notices,
        ["TGSong.comments", "2nd line", "Date: TGSong.date"]
    );

    // The measure headers.
    let mb = &score.master_bars;
    assert_eq!(mb.len(), 4);
    assert_eq!(score.initial_tempo, 110.0);
    assert_eq!(
        (
            mb[0].time_signature.numerator,
            mb[0].time_signature.denominator
        ),
        (3, 8)
    );
    assert!(mb[0].repeat_start && mb[0].repeat_end.is_none());
    assert_eq!(mb[0].alternate_endings, 0);
    assert!(mb[0].marker.is_none() && mb[0].tempo_changes.is_empty());
    assert_eq!(mb[1].time_signature, mb[0].time_signature, "carried on");
    assert_eq!(mb[1].alternate_endings, 0b101, "endings 1 and 3");
    let marker = mb[1].marker.as_ref().unwrap();
    assert_eq!(marker.text, "header2.marker");
    assert_eq!(
        (marker.color.r, marker.color.g, marker.color.b),
        (255, 1, 2)
    );
    assert_eq!(mb[2].tempo_changes[0].bpm, 40.0);
    assert_eq!(mb[2].tempo_changes[0].position, Ratio::ZERO);
    assert_eq!(mb[2].repeat_end, Some(2), "goes back once: plays twice");
    assert_eq!(mb[2].triplet_feel, TripletFeel::Triplet8th);
    assert_eq!(mb[3].tempo_changes[0].bpm, 110.0);
    assert_eq!(mb[0].key_signature.fifths, -1, "TuxGuitar's 8 is one flat");

    // Track 1.
    let t = &score.tracks[0];
    assert_eq!(t.name, "Track1.name");
    assert!(t.mix.solo && !t.mix.mute);
    assert_eq!((t.color.r, t.color.g, t.color.b), (10, 20, 30));
    let staff = &t.staves[0];
    // Offset -1 tunes the strings down a semitone (200 is past MIDI's top).
    assert_eq!(staff.tuning, [Pitch(127), Pitch(99), Pitch(49)]);
    assert_eq!(staff.capo, 0);
    assert!(logged(&log, "tuned lower"));
    assert_eq!(t.lyrics.len(), 1);
    assert_eq!(
        (t.lyrics[0].start_bar, t.lyrics[0].text.as_str()),
        (1, "test lyrics")
    );
    let bar = &staff.bars[0];
    assert_eq!(bar.clef, Clef::Treble8vb);
    assert_eq!(bar.voices.len(), 1, "the second voice is empty");
    let beats = &bar.voices[0].beats;
    assert_eq!(beats.len(), 1);
    let beat = &beats[0];
    let stroke = beat.effects.stroke.unwrap();
    assert_eq!(
        (stroke.direction, stroke.speed),
        (StrokeDirection::Up, NoteValue::Half)
    );
    let chord = &t.chords[beat.chord.unwrap()];
    assert_eq!(chord.name, "Bm");
    assert_eq!(chord.first_fret, 2);
    assert_eq!(
        chord.frets,
        [Some(2), Some(3), Some(4), Some(4), Some(2), None]
    );
    assert_eq!(beat.text, "Beat1.text");
    assert_eq!(beat.duration.value, NoteValue::Eighth);
    assert_eq!(beat.duration.dots, 2);
    assert!(beat.duration.tuplets.is_empty());
    // Track 1 has three strings: the notes on strings 4 to 6 (with a
    // tremolo bar, a harmonic and a grace note) are left out, and said.
    assert_eq!(beat.notes.len(), 3);
    let n = &beat.notes[0];
    assert_eq!(
        n.place,
        NotePlace::Fretted {
            string: 1,
            fret: 12
        }
    );
    assert_eq!(n.dynamic, Dynamic::F, "velocity 90 is nearest f's 95");
    assert!(!n.tie);
    assert_eq!(n.effects.vibrato, Some(Vibrato::Slight));
    assert!(n.effects.dead && n.effects.ghost);
    assert!(!n.effects.hammer_pull && n.effects.slides.is_empty() && n.effects.bend.is_none());
    let n = &beat.notes[1];
    assert_eq!(n.place, NotePlace::Fretted { string: 2, fret: 1 });
    assert_eq!(n.dynamic, Dynamic::F, "the velocity carries on");
    assert!(n.tie);
    assert_eq!(n.effects.accent, Some(Accent::Heavy));
    let bend = beat.notes[2].effects.bend.as_ref().unwrap();
    let points: Vec<_> = bend.points.iter().map(|p| (p.offset, p.cents)).collect();
    assert_eq!(points, [(r(1, 12), 100), (r(3, 12), 200)], "quarter tones");
    assert!(logged(&log, "string the track doesn't have"));
    assert!(logged(&log, "stem directions"));

    // Track 2.
    let t = &score.tracks[1];
    assert_eq!(t.name, "Track2.name");
    assert!(!t.mix.solo && !t.mix.mute);
    assert_eq!(t.staves[0].bars[0].clef, Clef::Bass8vb);
    let beat = &t.staves[0].bars[0].voices[0].beats[0];
    let trill = beat.notes[0].effects.trill.unwrap();
    assert_eq!((trill.fret, trill.speed), (3, NoteValue::ThirtySecond));
    // Its second note is on a third string the track doesn't have.
    assert_eq!(beat.notes.len(), 1);
    assert!(
        log.iter()
            .all(|e| e.level == Level::Warning || e.level == Level::Info)
    );
}

#[test]
fn reference_20_reads_as_tuxguitar_says() {
    let (score, log) = read(&fixture("reference_20.tg"));
    assert_eq!(score.info.title, "songName");
    assert_eq!(
        score.info.notices,
        [
            "songComments line 1",
            "songComments line 2",
            "Date: songDate"
        ]
    );
    assert_eq!(score.tracks.len(), 3);
    assert_eq!(score.master_bars.len(), 31);
    assert_eq!(score.initial_tempo, 80.0);
    let mb = &score.master_bars;
    assert_eq!(mb[2].tempo_changes[0].bpm, 100.0);
    assert!(mb[3].repeat_start);
    assert_eq!(mb[4].repeat_end, Some(4), "repeatClose 3: plays four times");
    assert_eq!(mb[4].marker.as_ref().unwrap().text, "testMarker");
    assert_eq!(mb[7].alternate_endings, 0b0101);
    assert_eq!(mb[9].alternate_endings, 0b1010);
    assert_eq!(mb[14].triplet_feel, TripletFeel::Triplet8th);
    assert!(score.layout.is_empty(), "no line breaks");

    let guitar = &score.tracks[0];
    assert!(!guitar.instrument.percussion);
    assert_eq!(guitar.instrument.midi_program, 25);
    assert_eq!(guitar.staves[0].fret_count, 29);
    assert_eq!(guitar.lyrics[0].start_bar, 0);
    assert_eq!(guitar.lyrics[0].text, "ly- rics track one");
    let bars = &guitar.staves[0].bars;
    assert_eq!(bars[17].clef, Clef::Tenor);
    assert_eq!(bars[20].clef, Clef::Treble8vb);
    assert_eq!(bars[21].voices[0].beats[0].duration.dots, 2);
    assert_eq!(
        bars[22].voices[0].beats[0].duration.tuplets,
        [Tuplet {
            notes: 5,
            in_time_of: 4
        }]
    );
    let notes = |bar: usize| -> Vec<&oxiguitar_model::Note> {
        bars[bar]
            .voices
            .iter()
            .flat_map(|v| &v.beats)
            .filter(|b| b.grace.is_none())
            .flat_map(|b| &b.notes)
            .collect()
    };
    let fx: Vec<_> = notes(23).iter().map(|n| n.effects.clone()).collect();
    assert!(fx[0].vibrato.is_some() && fx[1].dead && fx[2].slides == [Slide::Shift]);
    assert!(fx[4].hammer_pull && fx[6].ghost && fx[7].accent == Some(Accent::Normal));
    let beats = |bar: usize| &bars[bar].voices[0].beats;
    assert!(
        beats(24)
            .iter()
            .any(|b| b.effects.technique == Some(BeatTechnique::Tap))
    );
    assert!(
        beats(24)
            .iter()
            .any(|b| b.effects.technique == Some(BeatTechnique::Slap))
    );
    assert!(
        beats(24)
            .iter()
            .any(|b| b.effects.technique == Some(BeatTechnique::Pop))
    );
    assert!(beats(24).iter().any(|b| b.effects.fade == Some(Fade::In)));
    assert!(notes(24).iter().any(|n| n.effects.let_ring));
    assert!(notes(24).iter().any(|n| n.effects.palm_mute));
    assert!(notes(24).iter().any(|n| n.effects.staccato));
    let h: Vec<_> = notes(25)
        .iter()
        .filter_map(|n| n.effects.harmonic)
        .collect();
    assert_eq!(
        h,
        [
            Harmonic::Natural,
            Harmonic::Artificial { semitones: 12 },
            Harmonic::Tapped { fret: 11 }, // AH9 over fret 2
            Harmonic::Pinch,
            Harmonic::Semi,
        ]
    );
    let bend = notes(25)[0].effects.bend.as_ref().unwrap();
    assert_eq!(bend.points[1].cents, 200, "4 quarter tones: a whole tone");
    let graces: Vec<_> = beats(26).iter().filter(|b| b.grace.is_some()).collect();
    assert_eq!(graces.len(), 4);
    assert_eq!(graces[1].grace, Some(GraceKind::OnBeat));
    assert!(graces[3].notes[0].effects.hammer_pull);
    let n = notes(27);
    assert_eq!(n[0].effects.trill.unwrap().speed, NoteValue::ThirtySecond);
    assert_eq!(n[1].effects.tremolo_picking, Some(NoteValue::Sixteenth));
    assert!(notes(29).iter().any(|n| n.tie));

    let drums = &score.tracks[1];
    assert!(drums.instrument.percussion && drums.mix.solo);
    assert_eq!(drums.staves[0].bars[0].clef, Clef::Neutral);
    let bass = &score.tracks[2];
    assert!(bass.mix.mute);
    assert_eq!(bass.instrument.midi_program, 33);
    assert_eq!(bass.staves[0].capo, 2, "offset 2 is a capo");
    assert_eq!(bass.staves[0].bars[0].clef, Clef::Bass8vb);
    assert!(logged(&log, "chorus, reverb"));
    assert!(!logged(&log, "left out of"), "{log:#?}");
    let rests = bars[2]
        .voices
        .iter()
        .flat_map(|v| &v.beats)
        .filter(|b| b.kind == BeatKind::Rest)
        .count();
    assert_eq!(rests, 4);
}

#[test]
fn the_files_tuxguitar_refuses_are_refused() {
    for (name, why) in [
        ("Untitled_extended_30.tg", "format 3.0"),
        ("noVersion.tg", "version.txt"),
        ("invalidVersion.tg", "version.txt"),
        ("notTuxGuitar.tg", "DummyNode"),
        ("reference_15.tg", "TuxGuitar 1.5"),
        ("Untitled_15.tg", "TuxGuitar 1.5"),
    ] {
        let e = oxiguitar_tuxguitar_plugin::read(&fixture(name)).unwrap_err();
        assert!(e.message.contains(why), "{name}: {}", e.message);
    }
}

#[test]
fn a_newer_minor_version_reads_with_a_note() {
    let (score, log) = read(&fixture("Untitled_extended_21.tg"));
    assert_eq!(score.tracks.len(), 1);
    assert!(
        log.iter()
            .any(|e| e.level == Level::Info && e.message.contains("2.1"))
    );
    let (_, log) = read(&fixture("Untitled_20.tg"));
    assert!(log.is_empty(), "{log:#?}");
}

#[test]
fn every_2x_file_reads() {
    for name in [
        "altRepeatLoop_20.tg",
        "fix_20.tg",
        "freeEditionMode_20.tg",
        "getNextTied_20.tg",
        "letRing_20.tg",
        "tempo_20.tg",
        "test_midi_20.tg",
        "tiedNotes_20.tg",
        "track_pitch_20.tg",
        // An extra member TuxGuitar refuses the file for; the song is read.
        "invalidContent.tg",
        "invalidContent2.tg",
    ] {
        read(&fixture(name));
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
//
// Ported from TuxGuitar's writer of its 2.x format (LGPL-2.1,
// github.com/helge17/tuxguitar: common/TuxGuitar-lib/src/main/java/app/
// tuxguitar/io/tg/TGSongWriterImpl.java): the elements, their order and
// what each value means.

//! Writing OxiGuitar's score as a TuxGuitar 2 file.
//!
//! The reverse of reading (`read.rs`), as TuxGuitar 2 writes its files: a
//! channel per track, a measure header per bar, and per track a measure per
//! bar whose beats each hold two voices that start together. OxiGuitar's
//! first two voices are merged into such beats by their starts, a voice
//! with nothing at a beat's start written empty; grace beats fold into the
//! notes of the beat after them, and what OxiGuitar keeps on a beat
//! (tapping, fade-in, the tremolo bar, let ring) goes on each of its notes.
//! What the format can't hold is named in the losses before anything is
//! written.

use std::collections::HashMap;

use oxiguitar_model::{
    Bar, Beat, BeatKind, BeatTechnique, Bend, ChordDiagram, Clef, Duration, Fade, GraceKind,
    Harmonic, KeyMode, MasterBar, Note, NotePlace, NoteValue, Ratio, Score, Slide, Staff,
    StrokeDirection, Track, TripletFeel, Vibrato,
};

use crate::container;
use crate::log::{Log, place};
use crate::read::DATE;
use crate::spell;
use crate::tg;

/// Writes `score` as a TuxGuitar 2 file, with what the file can't hold.
pub(crate) fn write(score: &Score) -> Result<(Vec<u8>, Log), String> {
    let mut w = Writer {
        score,
        log: Log::default(),
        at: String::from("the song"),
        xml: Xml::default(),
    };
    w.song();
    let data = container::archive(w.xml.0.as_bytes())?;
    Ok((data, w.log))
}

struct Writer<'s> {
    score: &'s Score,
    log: Log,
    /// Where the writer is, for the losses.
    at: String,
    xml: Xml,
}

/// How a track's notes are placed on TuxGuitar's strings.
struct Plan {
    /// The strings written, highest first.
    tuning: Vec<u8>,
    capo: u8,
    drums: bool,
}

/// A beat of one voice, ready to be written in a TuxGuitar beat.
struct Placed<'b> {
    beat: &'b Beat,
    /// The grace beat before it, if any.
    grace: Option<&'b Beat>,
}

impl Writer<'_> {
    fn lost(&mut self, message: &str) {
        let at = self.at.clone();
        self.log.lost(message, at);
    }

    fn kept_as(&mut self, message: &str) {
        let at = self.at.clone();
        self.log.note(message, at);
    }

    // ---- the song ------------------------------------------------------

    fn song(&mut self) {
        let score = self.score;
        self.xml
            .0
            .push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>");
        self.xml.open("TuxGuitarFile", &[]);
        self.xml.empty(
            "TGVersion",
            &[("major", "2"), ("minor", "0"), ("revision", "0")],
        );
        self.xml.open("TGSong", &[]);
        let info = &score.info;
        let mut notices: Vec<&str> = info.notices.iter().map(String::as_str).collect();
        let date = notices
            .iter()
            .rposition(|n| n.starts_with(DATE))
            .map(|i| notices.remove(i))
            .and_then(|n| n.strip_prefix(DATE))
            .unwrap_or("");
        if !info.subtitle.is_empty() {
            self.lost("the subtitle is left out");
        }
        if !info.instructions.is_empty() {
            self.lost("the playing instructions are left out");
        }
        let comments = notices.join("\n");
        for (tag, text) in [
            ("name", info.title.as_str()),
            ("artist", &info.artist),
            ("album", &info.album),
            ("author", &info.music),
            ("date", date),
            ("copyright", &info.copyright),
            ("writer", &info.words),
            ("transcriber", &info.transcriber),
            ("comments", &comments),
        ] {
            self.text(tag, &[], text);
        }
        if !score.tempo_label.is_empty() {
            self.lost("the tempo's text is left out");
        }
        for (ti, track) in score.tracks.iter().enumerate() {
            self.at = format!("track {}", ti + 1);
            self.channel(ti, track);
        }
        let mut breaks = vec![false; score.master_bars.len()];
        let mut bar = 0usize;
        for bars in &score.layout {
            bar += usize::from(*bars);
            if let Some(b) = breaks.get_mut(bar) {
                *b = true;
            }
        }
        let mut bpm = score.initial_tempo;
        for (bi, mb) in score.master_bars.iter().enumerate() {
            self.at = format!("bar {}", bi + 1);
            bpm = self.header(mb, bpm, breaks.get(bi).copied().unwrap_or(false));
        }
        for (ti, track) in score.tracks.iter().enumerate() {
            self.at = format!("track {}", ti + 1);
            self.track(ti, track);
        }
        self.xml.close("TGSong");
        self.xml.close("TuxGuitarFile");
    }

    fn channel(&mut self, ti: usize, track: &Track) {
        let id = (ti + 1).to_string();
        let bank = if track.instrument.percussion {
            "128"
        } else {
            "0"
        };
        let program = track.instrument.midi_program.min(127).to_string();
        let volume = ((track.mix.volume.clamp(0.0, 1.0) * 127.0).round() as i32).to_string();
        let balance = ((track.mix.pan.clamp(-1.0, 1.0) * 64.0 + 64.0)
            .round()
            .clamp(0.0, 127.0) as i32)
            .to_string();
        self.xml.open("TGChannel", &[]);
        for (tag, text) in [
            ("id", id.as_str()),
            ("bank", bank),
            ("program", &program),
            ("volume", &volume),
            ("balance", &balance),
            ("chorus", "0"),
            ("reverb", "0"),
            ("phaser", "0"),
            ("tremolo", "0"),
        ] {
            self.xml.leaf(tag, &[], text);
        }
        self.text("name", &[], &track.name);
        self.xml.close("TGChannel");
    }

    /// A measure header; answers the tempo in force after it.
    fn header(&mut self, mb: &MasterBar, previous_bpm: f64, line_break: bool) -> f64 {
        self.xml.open("TGMeasureHeader", &[]);
        let ts = mb.time_signature;
        let denominator = if matches!(ts.denominator, 1 | 2 | 4 | 8 | 16 | 32 | 64) {
            ts.denominator
        } else {
            self.lost("time signatures count in whole notes to sixty-fourths: another is written in quarters");
            4
        };
        let numerator = ts.numerator.max(1).to_string();
        let denominator = denominator.to_string();
        self.xml.empty(
            "timeSignature",
            &[("numerator", &numerator), ("denominator", &denominator)],
        );
        let mut bpm = previous_bpm;
        for t in &mb.tempo_changes {
            if t.position != Ratio::ZERO {
                self.lost(
                    "tempo changes inside a bar are left out: TuxGuitar changes tempo at bar lines",
                );
                continue;
            }
            if !t.label.is_empty() {
                self.lost("the texts of tempo changes are left out");
            }
            if t.linear {
                self.lost("a gradual tempo change is written as a sudden one");
            }
            bpm = t.bpm;
        }
        // Quarters a minute, or eighths where that makes a whole number.
        let (raw, base) = if bpm.fract() == 0.0 {
            (bpm, None)
        } else if (bpm * 2.0).fract() == 0.0 {
            (bpm * 2.0, Some("8"))
        } else {
            self.lost("tempos are whole numbers of quarters or eighths a minute");
            (bpm.round(), None)
        };
        let raw = (raw.clamp(1.0, 100_000.0) as i64).to_string();
        let mut attrs = Vec::new();
        if let Some(b) = base {
            attrs.push(("base", b));
        }
        self.xml.leaf("tempo", &attrs, &raw);
        if mb.repeat_start {
            self.xml.empty("repeatOpen", &[]);
        }
        match mb.repeat_end {
            Some(plays) if plays >= 2 => {
                let back = (plays - 1).to_string();
                self.xml.leaf("repeatClose", &[], &back);
            }
            Some(_) => self.lost("a repeat played once is left out"),
            None => {}
        }
        let mut endings = mb.alternate_endings;
        if endings >> 7 != 0 {
            // TuxGuitar keeps them in a byte (TGSongReaderImpl).
            self.lost("alternate endings go up to the seventh");
            endings &= 0x7f;
        }
        if endings != 0 {
            self.xml.open("repeatAlternative", &[]);
            for bit in 0..7 {
                if endings & (1 << bit) != 0 {
                    let n = (bit + 1).to_string();
                    self.xml.leaf("alternative", &[], &n);
                }
            }
            self.xml.close("repeatAlternative");
        }
        if let Some(m) = &mb.marker {
            if !m.letter.is_empty() {
                self.lost("section letters are left out");
            }
            let (r, g, b) = (
                m.color.r.to_string(),
                m.color.g.to_string(),
                m.color.b.to_string(),
            );
            self.text("marker", &[("R", &r), ("G", &g), ("B", &b)], &m.text);
        }
        match mb.triplet_feel {
            TripletFeel::None => {}
            TripletFeel::Triplet8th => self.xml.leaf("tripletFeel", &[], "eighth"),
            TripletFeel::Triplet16th => self.xml.leaf("tripletFeel", &[], "sixteenth"),
            _ => self.lost("only the triplet feels of eighths and sixteenths are kept"),
        }
        if line_break {
            self.xml.empty("lineBreak", &[]);
        }
        self.xml.close("TGMeasureHeader");
        if !mb.directions.is_empty() {
            self.lost("signs and jumps (segno, coda, da capo …) are left out");
        }
        if !mb.fermatas.is_empty() {
            self.lost("fermatas are left out");
        }
        if mb.free_time {
            self.lost("free time is left out");
        }
        if mb.double_bar {
            self.lost("double bar lines are left out");
        }
        if mb.key_signature.mode == KeyMode::Minor {
            self.kept_as("minor keys are written as their key signature alone");
        }
        bpm
    }

    // ---- tracks --------------------------------------------------------

    fn plan(&mut self, track: &Track) -> Plan {
        let staff = track.staves.first();
        let drums = track.instrument.percussion;
        let mut tuning: Vec<u8> =
            staff.map_or_else(Vec::new, |s| s.tuning.iter().map(|p| p.0).collect());
        if drums {
            // Every drum on a string tuned to 0, its fret the drum.
            tuning = vec![0; tuning.len().clamp(6, tg::MAX_STRINGS)];
        } else if tuning.is_empty() {
            self.lost(
                "a track without strings is written as a guitar's, its notes placed by pitch",
            );
            tuning = vec![64, 59, 55, 50, 45, 40];
        }
        if tuning.len() > tg::MAX_STRINGS {
            self.lost("a track has 25 strings at most; the others are left out");
            tuning.truncate(tg::MAX_STRINGS);
        }
        let mut capo = staff.map_or(0, |s| s.capo);
        if i64::from(capo) > tg::MAX_OFFSET {
            self.lost("a capo above fret 24 is left out");
            capo = 0;
        }
        Plan {
            tuning,
            capo: if drums { 0 } else { capo },
            drums,
        }
    }

    fn track(&mut self, ti: usize, track: &Track) {
        let plan = self.plan(track);
        // The string each drum was last struck on, for ties.
        let mut drum_strings: HashMap<u8, u8> = HashMap::new();
        let staff = track.staves.first();
        if track.staves.len() > 1 {
            self.lost("only the first staff of a track is kept");
        }
        if !track.short_name.is_empty() {
            self.lost("a track's short name is left out");
        }
        if let Some(s) = staff {
            if s.transposition != 0 {
                self.lost("a staff's transposition is left out");
            }
            if !s.show_standard || !s.show_tablature {
                self.kept_as("TuxGuitar shows standard notation and tablature for every track");
            }
        }
        if !track.layout.is_empty() && track.layout != self.score.layout {
            self.lost("the layout of a track shown alone is left out");
        }
        let max_fret = staff.map_or(24, |s| s.fret_count).to_string();
        let attrs: Vec<(&str, &str)> = if plan.drums {
            Vec::new()
        } else {
            vec![("maxFret", max_fret.as_str())]
        };
        self.xml.open("TGTrack", &attrs);
        self.text("name", &[], &track.name);
        if track.mix.mute {
            self.xml.leaf("soloMute", &[], "mute");
        } else if track.mix.solo {
            self.xml.leaf("soloMute", &[], "solo");
        }
        let id = (ti + 1).to_string();
        self.xml.leaf("channelId", &[], &id);
        if plan.capo != 0 {
            let capo = plan.capo.to_string();
            self.xml.leaf("offset", &[], &capo);
        }
        let c = track.color;
        let (r, g, b) = (c.r.to_string(), c.g.to_string(), c.b.to_string());
        self.xml.empty("color", &[("R", &r), ("G", &g), ("B", &b)]);
        for p in &plan.tuning {
            let p = p.to_string();
            self.xml.leaf("TGString", &[], &p);
        }
        if track.lyrics.len() > 1 {
            self.lost("only the first line of lyrics is kept");
        }
        let (from, lyrics) = track.lyrics.first().map_or((1, ""), |l| {
            (l.start_bar.saturating_add(1), l.text.as_str())
        });
        let from = from.to_string();
        self.text("TGLyric", &[("from", &from)], lyrics);
        let mut bar_start = Ratio::ZERO;
        let mut previous: Option<(Clef, i64)> = None;
        for (bi, mb) in self.score.master_bars.iter().enumerate() {
            self.at = place(bi, ti);
            let bar = staff.and_then(|s| s.bars.get(bi));
            let clef = bar.map_or(Clef::Treble8vb, |b| b.clef);
            let key = i64::from(mb.key_signature.fifths.clamp(-7, 7));
            let key = if key < 0 { 7 - key } else { key };
            self.xml.open("TGMeasure", &[]);
            if previous.is_none_or(|(c, _)| c != clef) {
                let name = match clef {
                    Clef::Treble8vb | Clef::Neutral => "treble",
                    Clef::Treble => {
                        if !plan.drums {
                            self.kept_as(
                                "a treble clef is written as TuxGuitar's, which sounds an octave lower",
                            );
                        }
                        "treble"
                    }
                    Clef::Bass8vb => "bass",
                    Clef::Bass => {
                        self.kept_as(
                            "a bass clef is written as TuxGuitar's, which sounds an octave lower",
                        );
                        "bass"
                    }
                    Clef::Tenor => "tenor",
                    Clef::Alto => "alto",
                };
                self.xml.leaf("clef", &[], name);
            }
            if previous.is_none_or(|(_, k)| k != key) {
                let k = key.to_string();
                self.xml.leaf("keySignature", &[], &k);
            }
            previous = Some((clef, key));
            if let Some(bar) = bar {
                self.beats(
                    track,
                    staff,
                    bar,
                    mb,
                    bar_start,
                    bi == 0,
                    &plan,
                    &mut drum_strings,
                );
            } else {
                self.empty_beat(bar_start);
            }
            self.xml.close("TGMeasure");
            bar_start = mb
                .expected_length()
                .and_then(|l| bar_start.checked_add(l))
                .unwrap_or(bar_start);
        }
        self.xml.close("TGTrack");
    }

    // ---- beats ---------------------------------------------------------

    /// A voice's beats by their start in the bar, grace beats folded into
    /// the beat after them, empty beats left out (TuxGuitar places beats by
    /// their start, so gaps need nothing).
    fn placed<'b>(&mut self, voice: &'b oxiguitar_model::Voice) -> Vec<(Ratio, Placed<'b>)> {
        let mut out = Vec::new();
        let mut position = Ratio::ZERO;
        let mut graces: Vec<&Beat> = Vec::new();
        for beat in &voice.beats {
            if beat.grace.is_some() {
                graces.push(beat);
                continue;
            }
            if graces.len() > 1 {
                self.lost("only one grace beat before a beat is kept");
            }
            let grace = graces.pop();
            graces.clear();
            if beat.kind != BeatKind::Empty {
                out.push((position, Placed { beat, grace }));
            } else if grace.is_some() {
                self.lost("a grace note needs a note after it to belong to");
            }
            position = beat
                .duration
                .length()
                .and_then(|l| position.checked_add(l))
                .unwrap_or(position);
        }
        if !graces.is_empty() {
            self.lost("a grace note needs a note after it to belong to");
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn beats(
        &mut self,
        track: &Track,
        staff: Option<&Staff>,
        bar: &Bar,
        mb: &MasterBar,
        bar_start: Ratio,
        first_bar: bool,
        plan: &Plan,
        drum_strings: &mut HashMap<u8, u8>,
    ) {
        if bar.voices.len() > 2 {
            self.lost("only two voices of a bar are kept");
        }
        if bar.simile.is_some() {
            self.kept_as("repeat signs are written out as the bars they repeat");
        }
        let voices: Vec<Vec<(Ratio, Placed<'_>)>> =
            bar.voices.iter().take(2).map(|v| self.placed(v)).collect();
        let mut starts: Vec<Ratio> = voices.iter().flatten().map(|(s, _)| *s).collect();
        starts.sort();
        starts.dedup();
        if starts.is_empty() || (first_bar && starts.first() != Some(&Ratio::ZERO)) {
            // TuxGuitar refuses a file whose first beat doesn't start the
            // song, and every bar has a beat: one with both voices empty.
            starts.insert(0, Ratio::ZERO);
        }
        for start in starts {
            let at_start: Vec<Option<&Placed<'_>>> = (0..2)
                .map(|vi| {
                    voices
                        .get(vi)
                        .and_then(|v| v.iter().find(|(s, _)| *s == start))
                        .map(|(_, p)| p)
                })
                .collect();
            self.xml.open("TGBeat", &[]);
            let precise = bar_start
                .checked_add(start)
                .and_then(|s| s.checked_mul(Ratio::from_integer(tg::WHOLE)));
            let precise = match precise {
                Some(p) if p.denom() == 1 => p.numer(),
                Some(p) => {
                    self.lost("a beat between TuxGuitar's units of time is moved to the nearest");
                    p.to_f64().round() as i64
                }
                None => 0,
            };
            let precise = (tg::START + precise).to_string();
            self.xml.leaf("preciseStart", &[], &precise);
            // What the voices share: the first voice's, else the second's.
            let shared: Vec<&Beat> = at_start.iter().flatten().map(|p| p.beat).collect();
            if let [a, b] = shared.as_slice()
                && (b.effects.stroke.is_some()
                    || b.effects.pick_stroke.is_some()
                    || b.chord.is_some()
                    || !b.text.is_empty())
                && (a.effects.stroke != b.effects.stroke
                    || a.effects.pick_stroke != b.effects.pick_stroke
                    || a.chord != b.chord
                    || a.text != b.text)
            {
                self.lost(
                    "the second voice's strum, pick stroke, chord or text at the first's beat is left out",
                );
            }
            if let Some(beat) = shared.first() {
                self.beat_shared(track, beat);
            }
            for p in &at_start {
                match p {
                    Some(p) => self.voice(staff, mb, plan, p, drum_strings),
                    None => {
                        self.xml.open("voice", &[("empty", "true")]);
                        self.xml.empty("duration", &[("value", "4")]);
                        self.xml.close("voice");
                    }
                }
            }
            self.xml.close("TGBeat");
        }
    }

    /// A beat with both voices empty, at the start of a bar.
    fn empty_beat(&mut self, bar_start: Ratio) {
        let precise = bar_start
            .checked_mul(Ratio::from_integer(tg::WHOLE))
            .map_or(0, |p| p.to_f64().round() as i64);
        self.xml.open("TGBeat", &[]);
        let precise = (tg::START + precise).to_string();
        self.xml.leaf("preciseStart", &[], &precise);
        for _ in 0..2 {
            self.xml.open("voice", &[("empty", "true")]);
            self.xml.empty("duration", &[("value", "4")]);
            self.xml.close("voice");
        }
        self.xml.close("TGBeat");
    }

    /// What a TuxGuitar beat's voices share: strum, pick stroke, chord and
    /// text.
    fn beat_shared(&mut self, track: &Track, beat: &Beat) {
        if let Some(s) = beat.effects.stroke {
            let direction = match s.direction {
                StrokeDirection::Up => "up",
                StrokeDirection::Down => "down",
            };
            let value = self.value(s.speed, "a strum's speed").to_string();
            self.xml
                .empty("stroke", &[("direction", direction), ("value", &value)]);
        }
        if let Some(p) = beat.effects.pick_stroke {
            let direction = match p {
                StrokeDirection::Up => "up",
                StrokeDirection::Down => "down",
            };
            self.xml.leaf("pickStroke", &[], direction);
        }
        if let Some(i) = beat.chord {
            match track.chords.get(i) {
                Some(c) => self.chord(c),
                None => self.lost("a chord name without a diagram is left out"),
            }
        }
        if !beat.text.is_empty() {
            self.text("text", &[], &beat.text);
        }
    }

    fn chord(&mut self, c: &ChordDiagram) {
        self.xml.open("chord", &[]);
        self.text("name", &[], &c.name);
        let first = c.first_fret.to_string();
        self.xml.leaf("firstFret", &[], &first);
        for f in &c.frets {
            match f {
                Some(f) => {
                    let f = f.to_string();
                    self.xml.leaf("string", &[], &f);
                }
                None => self.xml.empty("string", &[]),
            }
        }
        self.xml.close("chord");
    }

    fn voice(
        &mut self,
        staff: Option<&Staff>,
        mb: &MasterBar,
        plan: &Plan,
        p: &Placed<'_>,
        drum_strings: &mut HashMap<u8, u8>,
    ) {
        let beat = p.beat;
        self.unheld_beat_marks(beat);
        if beat.effects.dynamic.is_some() {
            self.kept_as("dynamic markings are written as the notes' velocities");
        }
        if beat.kind == BeatKind::Rest && !beat.notes.is_empty() {
            self.lost("notes in a rest are left out");
        }
        // The notes, each on its string.
        let mut notes: Vec<(u8, u8, &Note)> = Vec::new();
        if beat.kind == BeatKind::Notes {
            let mut taken: Vec<u8> = Vec::new();
            for note in &beat.notes {
                if let Some((string, fret)) = self.place(plan, note, &taken, drum_strings) {
                    if taken.contains(&string) {
                        self.lost("a second note on the same string in one beat is left out");
                        continue;
                    }
                    taken.push(string);
                    notes.push((string, fret, note));
                }
            }
        }
        let attrs: Vec<(&str, &str)> = if notes.is_empty() {
            vec![("empty", "false")]
        } else {
            Vec::new()
        };
        self.xml.open("voice", &attrs);
        self.duration(&beat.duration);
        let graces = p
            .grace
            .map(|g| self.graces(plan, g, &notes))
            .unwrap_or_default();
        let mut previous_velocity = None;
        for (string, fret, note) in &notes {
            let grace = graces.iter().find(|(s, _)| s == string).map(|(_, g)| g);
            let velocity = tg::velocity(note.dynamic);
            self.note(
                staff,
                mb,
                beat,
                *string,
                *fret,
                note,
                grace,
                (previous_velocity != Some(velocity)).then_some(velocity),
            );
            previous_velocity = Some(velocity);
        }
        self.xml.close("voice");
    }

    /// The string and fret a note is written on, or why it is left out.
    fn place(
        &mut self,
        plan: &Plan,
        note: &Note,
        taken: &[u8],
        drum_strings: &mut HashMap<u8, u8>,
    ) -> Option<(u8, u8)> {
        let strings = u8::try_from(plan.tuning.len()).unwrap_or(0);
        match (plan.drums, note.place) {
            (true, NotePlace::Pitched(p)) => {
                // The string it was last struck on, for a tie; else the
                // first free one.
                let free = |s: &u8| !taken.contains(s);
                let string = drum_strings
                    .get(&p.0)
                    .copied()
                    .filter(|s| note.tie && free(s) && *s <= strings)
                    .or_else(|| (1..=strings).find(free));
                match string {
                    Some(s) => {
                        drum_strings.insert(p.0, s);
                        Some((s, p.0))
                    }
                    None => {
                        self.lost("a beat of drums holds as many drums as the track has strings");
                        None
                    }
                }
            }
            (true, NotePlace::Fretted { fret, .. }) => {
                self.lost("a drum on a string is written as the drum its fret names");
                (1..=strings)
                    .find(|s| !taken.contains(s))
                    .map(|s| (s, fret))
            }
            (false, NotePlace::Pitched(p)) => {
                // The string with the lowest fret that reaches the pitch.
                let placed = plan
                    .tuning
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| u8::try_from(i + 1).is_ok_and(|s| !taken.contains(&s)))
                    .filter_map(|(i, open)| {
                        let base = u16::from(*open) + u16::from(plan.capo);
                        Some((i, u16::from(p.0).checked_sub(base)?))
                    })
                    .min_by_key(|(_, fret)| *fret)
                    .and_then(|(i, fret)| {
                        Some((u8::try_from(i + 1).ok()?, u8::try_from(fret).ok()?))
                    });
                if placed.is_none() {
                    self.lost("a note below the lowest string is left out");
                }
                placed
            }
            (false, NotePlace::Fretted { string, fret }) => {
                if string == 0 || string > strings {
                    self.lost("a note on a string the track does not have is left out");
                    None
                } else {
                    Some((string, fret))
                }
            }
        }
    }

    fn duration(&mut self, d: &Duration) {
        let value = self.value(d.value, "a beat").to_string();
        let mut attrs = vec![("value", value.as_str())];
        match d.dots {
            0 => {}
            1 => attrs.push(("dotted", "dotted")),
            2 => attrs.push(("dotted", "doubleDotted")),
            _ => {
                self.lost("notes have two dots at most: a third is left out");
                attrs.push(("dotted", "doubleDotted"));
            }
        }
        if d.tuplets.len() > 1 {
            self.lost("a tuplet inside another is written as the outer one");
        }
        let tuplet = d.tuplets.first().and_then(|t| {
            if tg::TUPLETS.contains(&(t.notes, t.in_time_of)) {
                Some(t)
            } else {
                self.lost(
                    "a tuplet other than 3:2, 5 to 7 in the time of 4, and 9 to 13 in the time of 8 is left out",
                );
                None
            }
        });
        match tuplet {
            Some(t) => {
                self.xml.open("duration", &attrs);
                let (enters, times) = (t.notes.to_string(), t.in_time_of.to_string());
                self.xml
                    .empty("divisionType", &[("enters", &enters), ("times", &times)]);
                self.xml.close("duration");
            }
            None => self.xml.empty("duration", &attrs),
        }
    }

    /// TuxGuitar's number for a note value: 1 for a whole to 64 for a
    /// sixty-fourth.
    fn value(&mut self, v: NoteValue, what: &str) -> u16 {
        match v {
            NoteValue::DoubleWhole => {
                self.lost(&format!(
                    "{what} as long as a double whole note is written as a whole note"
                ));
                1
            }
            NoteValue::OneTwentyEighth | NoteValue::TwoFiftySixth => {
                self.lost(&format!(
                    "{what} shorter than a sixty-fourth is written as a sixty-fourth"
                ));
                64
            }
            v => v.denominator(),
        }
    }

    /// Names what TuxGuitar can't hold of a beat's marks.
    fn unheld_beat_marks(&mut self, beat: &Beat) {
        let e = &beat.effects;
        let unheld = [
            (beat.slur, "slurs are left out"),
            (beat.hairpin.is_some(), "hairpins are left out"),
            (beat.ottava.is_some(), "ottava lines are left out"),
            (beat.slash, "slash notation is left out"),
            (
                beat.beaming != oxiguitar_model::Beaming::Automatic,
                "beaming changes are left out",
            ),
            (beat.tuplet_group, "forced tuplet groups are left out"),
            (beat.barre.is_some(), "barres are left out"),
            (e.pedal.is_some(), "sustain pedal marks are left out"),
            (e.wah.is_some(), "wah-wah marks are left out"),
            (e.arpeggio.is_some(), "arpeggios are left out"),
            (e.rasgueado.is_some(), "rasgueados are left out"),
            (e.golpe.is_some(), "golpes are left out"),
            (e.pick_scrape.is_some(), "pick scrapes are left out"),
            (e.dead_slap, "dead slaps are left out"),
            (e.bridge_vibrato.is_some(), "whammy-bar vibrato is left out"),
            (
                matches!(e.fade, Some(Fade::Out | Fade::Swell)),
                "fade-outs and volume swells are left out",
            ),
        ];
        for (here, what) in unheld {
            if here {
                self.lost(what);
            }
        }
    }

    /// A grace beat folded into the notes after it: per string, the grace
    /// note's attributes.
    fn graces(&mut self, plan: &Plan, g: &Beat, notes: &[(u8, u8, &Note)]) -> Vec<(u8, Grace)> {
        let duration = match g.duration.value {
            NoteValue::SixtyFourth => "64",
            NoteValue::ThirtySecond => "32",
            NoteValue::Sixteenth => "16",
            _ => {
                self.lost("a grace note is a sixteenth, thirty-second or sixty-fourth");
                "32"
            }
        };
        let on_beat = g.grace == Some(GraceKind::OnBeat);
        let mut out = Vec::new();
        let mut taken = Vec::new();
        let mut no_ties = HashMap::new();
        for note in &g.notes {
            let Some((string, fret)) = self.place(plan, note, &taken, &mut no_ties) else {
                continue;
            };
            taken.push(string);
            if !notes.iter().any(|(s, _, _)| *s == string) {
                self.lost("a grace note needs a note on its string after it");
                continue;
            }
            let fx = &note.effects;
            let mut plain = fx.clone();
            plain.dead = false;
            plain.hammer_pull = false;
            plain.slides.retain(|s| *s != Slide::Legato);
            if plain != oxiguitar_model::NoteEffects::default() {
                self.lost("a grace note holds only dead, slide and hammer-on effects");
            }
            let transition = if fx.slides.contains(&Slide::Legato) {
                "slide"
            } else if fx.hammer_pull {
                "hammer"
            } else {
                "none"
            };
            out.push((
                string,
                Grace {
                    fret: fret.to_string(),
                    duration,
                    dynamic: tg::velocity(note.dynamic).to_string(),
                    transition,
                    on_beat,
                    dead: fx.dead,
                },
            ));
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn note(
        &mut self,
        staff: Option<&Staff>,
        mb: &MasterBar,
        beat: &Beat,
        string: u8,
        fret: u8,
        note: &Note,
        grace: Option<&Grace>,
        velocity: Option<i64>,
    ) {
        let fx = &note.effects;
        let be = &beat.effects;
        let value = fret.to_string();
        let string_text = string.to_string();
        let velocity = velocity.map(|v| v.to_string());
        let mut attrs = vec![("value", value.as_str()), ("string", &string_text)];
        if note.tie {
            attrs.push(("tiedNote", "true"));
        }
        if let Some(v) = &velocity {
            attrs.push(("velocity", v));
        }
        // Element order as TuxGuitar writes it.
        let mut children: Vec<&str> = Vec::new();
        if let Some(v) = fx.vibrato {
            if v == Vibrato::Wide {
                self.kept_as("wide vibrato is written as TuxGuitar's one vibrato");
            }
            children.push("vibrato");
        }
        if fx.dead {
            children.push("deadNote");
        }
        let mut slides = fx.slides.iter();
        if slides.any(|s| matches!(s, Slide::Shift | Slide::Legato)) {
            children.push("slide");
        }
        if fx.slides.contains(&Slide::Legato) && !fx.slides.contains(&Slide::Shift) {
            self.kept_as("a legato slide is written as TuxGuitar's one slide");
        }
        if fx
            .slides
            .iter()
            .any(|s| !matches!(s, Slide::Shift | Slide::Legato))
        {
            self.lost("slides into and out of a note are left out");
        }
        if fx.hammer_pull {
            children.push("hammer");
        }
        if fx.ghost {
            children.push("ghostNote");
        }
        match fx.accent {
            Some(oxiguitar_model::Accent::Normal) => children.push("accentuatedNote"),
            Some(oxiguitar_model::Accent::Heavy) => children.push("heavyAccentuatedNote"),
            None => {}
        }
        if fx.palm_mute {
            children.push("palmMute");
        }
        if fx.staccato {
            children.push("staccato");
        }
        match be.technique {
            Some(BeatTechnique::Tap) => children.push("tapping"),
            Some(BeatTechnique::Slap) => children.push("slapping"),
            Some(BeatTechnique::Pop) => children.push("popping"),
            None => {}
        }
        if be.fade == Some(Fade::In) {
            children.push("fadeIn");
        }
        if fx.let_ring || be.let_ring {
            children.push("letRing");
        }
        let unheld = [
            (
                fx.left_finger.is_some() || fx.right_finger.is_some(),
                "fingerings are left out",
            ),
            (fx.ornament.is_some(), "ornaments are left out"),
            (fx.string_number, "string numbers are left out"),
            (fx.left_hand_tap, "left-hand tapping on a note is left out"),
        ];
        for (here, what) in unheld {
            if here {
                self.lost(what);
            }
        }
        let open = !children.is_empty()
            || fx.bend.is_some()
            || be.whammy.is_some()
            || fx.harmonic.is_some()
            || grace.is_some()
            || fx.trill.is_some()
            || fx.tremolo_picking.is_some()
            || be.tremolo_picking.is_some()
            || note.spelling.is_some();
        if !open {
            self.xml.empty("note", &attrs);
            return;
        }
        self.xml.open("note", &attrs);
        for c in children {
            self.xml.empty(c, &[]);
        }
        if let Some(b) = &fx.bend {
            self.curve("bend", b, tg::BEND_CENTS, "a bend");
        }
        if let Some(b) = &be.whammy {
            self.curve("tremoloBar", b, tg::WHAMMY_CENTS, "a tremolo bar dive");
        }
        if let Some(h) = fx.harmonic {
            self.harmonic(h, fret);
        }
        if let Some(g) = grace {
            self.xml.empty(
                "grace",
                &[
                    ("fret", &g.fret),
                    ("duration", g.duration),
                    ("dynamic", &g.dynamic),
                    ("transition", g.transition),
                    ("onBeat", if g.on_beat { "true" } else { "false" }),
                    ("dead", if g.dead { "true" } else { "false" }),
                ],
            );
        }
        if let Some(t) = fx.trill {
            let fret = t.fret.to_string();
            let speed = self.value(t.speed, "a trill's speed").to_string();
            self.xml
                .empty("trill", &[("fret", &fret), ("duration", &speed)]);
        }
        if let Some(v) = fx.tremolo_picking.or(be.tremolo_picking) {
            let speed = self.value(v, "tremolo picking").to_string();
            self.xml.empty("tremoloPicking", &[("duration", &speed)]);
        }
        if let Some(s) = note.spelling {
            let key = mb.key_signature;
            let transposition = staff.map_or(0, |s| i32::from(s.transposition));
            let sounding = staff
                .and_then(|st| match note.place {
                    NotePlace::Fretted { string, fret } => st.pitch_of(string, fret),
                    NotePlace::Pitched(p) => Some(p),
                })
                .and_then(|p| u8::try_from(i32::from(p.0) + transposition).ok())
                .map(oxiguitar_model::Pitch);
            if let Some(written) = sounding
                && s != oxiguitar_model::Spelling::automatic(written, key)
            {
                if spell::alternative(written, key) == Some(s) {
                    self.xml.empty("alternativeEnharmonic", &[]);
                } else {
                    self.lost(
                        "a spelling other than a note's usual or alternative name is left out",
                    );
                }
            }
        }
        self.xml.close("note");
    }

    fn curve(&mut self, tag: &str, b: &Bend, cents: i32, what: &str) {
        self.xml.open(tag, &[]);
        for p in &b.points {
            let position = p
                .offset
                .checked_mul(Ratio::from_integer(tg::BEND_POSITIONS))
                .map_or(0.0, Ratio::to_f64);
            if position.fract() != 0.0 {
                self.lost(&format!(
                    "{what}'s points are moved to twelfths of the note"
                ));
            }
            let height = f64::from(p.cents) / f64::from(cents);
            if height.fract() != 0.0 {
                self.lost(&format!(
                    "{what}'s heights are kept to {}",
                    if cents == tg::BEND_CENTS {
                        "quarter tones"
                    } else {
                        "semitones"
                    }
                ));
            }
            let position = (position.round().clamp(0.0, 12.0) as i64).to_string();
            let height = (height.round() as i64).to_string();
            self.xml
                .empty("point", &[("position", &position), ("value", &height)]);
        }
        self.xml.close(tag);
    }

    fn harmonic(&mut self, h: Harmonic, fret: u8) {
        let index = |f: &dyn Fn(&(u8, u8)) -> bool| tg::HARMONICS.iter().position(f);
        let (kind, data) = match h {
            Harmonic::Natural => ("N.H", 0),
            Harmonic::Artificial { semitones } => match index(&|r| r.1 == semitones) {
                Some(i) => ("A.H", i),
                None => {
                    self.lost("an artificial harmonic is written as the nearest of AH12, AH7, AH5, AH4 and AH3");
                    let nearest = (0..tg::HARMONICS.len())
                        .min_by_key(|i| {
                            tg::HARMONICS
                                .get(*i)
                                .map_or(u8::MAX, |r| r.1.abs_diff(semitones))
                        })
                        .unwrap_or(0);
                    ("A.H", nearest)
                }
            },
            Harmonic::Tapped { fret: touched } => {
                let above = touched.checked_sub(fret);
                match above.and_then(|a| index(&|r| r.0 == a)) {
                    Some(i) => ("T.H", i),
                    None => {
                        self.lost("a tapped harmonic is touched 12, 9, 7, 5, 4 or 3 frets above its note: another is written at 12");
                        ("T.H", 0)
                    }
                }
            }
            Harmonic::Pinch => ("P.H", 0),
            Harmonic::Semi => ("S.H", 0),
            Harmonic::Feedback => {
                self.lost("feedback harmonics are left out");
                return;
            }
        };
        let data = data.to_string();
        self.xml
            .empty("harmonic", &[("type", kind), ("data", &data)]);
    }

    /// An element holding text, the text cleaned of what XML can't hold.
    fn text(&mut self, tag: &str, attrs: &[(&str, &str)], text: &str) {
        let clean: String = text.chars().filter(|c| xml_char(*c)).collect();
        if clean.len() != text.len() {
            self.lost("characters XML can't hold are left out of texts");
        }
        self.xml.leaf(tag, attrs, &clean);
    }
}

/// A grace note's attributes.
struct Grace {
    fret: String,
    duration: &'static str,
    dynamic: String,
    transition: &'static str,
    on_beat: bool,
    dead: bool,
}

/// Whether XML 1.0 can hold the character.
fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..)
}

/// The XML being written, as TuxGuitar writes it: no indentation.
#[derive(Default)]
struct Xml(String);

impl Xml {
    fn start(&mut self, tag: &str, attrs: &[(&str, &str)]) {
        self.0.push('<');
        self.0.push_str(tag);
        for (name, value) in attrs {
            self.0.push(' ');
            self.0.push_str(name);
            self.0.push_str("=\"");
            self.0.push_str(&quick_xml::escape::escape(*value));
            self.0.push('"');
        }
    }

    fn open(&mut self, tag: &str, attrs: &[(&str, &str)]) {
        self.start(tag, attrs);
        self.0.push('>');
    }

    fn close(&mut self, tag: &str) {
        self.0.push_str("</");
        self.0.push_str(tag);
        self.0.push('>');
    }

    fn empty(&mut self, tag: &str, attrs: &[(&str, &str)]) {
        self.start(tag, attrs);
        self.0.push_str("/>");
    }

    fn leaf(&mut self, tag: &str, attrs: &[(&str, &str)], text: &str) {
        if text.is_empty() {
            self.empty(tag, attrs);
            return;
        }
        self.open(tag, attrs);
        self.0.push_str(&quick_xml::escape::escape(text));
        self.close(tag);
    }
}

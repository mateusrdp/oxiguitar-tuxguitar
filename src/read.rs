// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
//
// Ported from TuxGuitar's reader of its 2.x format (LGPL-2.1,
// github.com/helge17/tuxguitar: common/TuxGuitar-lib/src/main/java/app/
// tuxguitar/io/tg/TGSongReaderImpl.java and TGStream.java): the elements
// it reads, the defaults it gives what a file leaves out, and what each
// value means.

//! Reading a TuxGuitar 2 file's `content.xml` into OxiGuitar's score.
//!
//! TuxGuitar's song follows Guitar Pro's, as OxiGuitar's does: a list of
//! measure headers every track shares (time signature, tempo, repeats,
//! markers) and, per track, a measure per header holding beats. Each beat
//! has a precise start and two voices with their own durations; a voice
//! marked empty is not there. The voices become OxiGuitar's first two,
//! each beat placed at its start. Note effects that OxiGuitar keeps on the
//! beat (tapping, slapping, popping, fade-in, the tremolo bar) move there,
//! and grace notes become a grace beat before their beat, as the Guitar
//! Pro reader does.

use oxiguitar_model::{
    Bar, Beat, BeatKind, BeatTechnique, Bend, BendPoint, ChordDiagram, Clef, Color, Duration, Fade,
    GraceKind, Harmonic, Instrument, KeyMode, KeySignature, LyricLine, Marker, MasterBar, Mix,
    Note, NotePlace, NoteValue, Pitch, Ratio, Score, Slide, Staff, Stroke, StrokeDirection,
    TempoChange, TimeSignature, Track, Trill, TripletFeel, Tuplet, Vibrato, Voice,
};
use roxmltree::Node;

use crate::container;
use crate::log::{Log, place};
use crate::spell;
use crate::tg;
use crate::time;

/// A file read: the score and what was lost or changed on the way.
pub(crate) struct Imported {
    pub(crate) score: Score,
    pub(crate) log: Log,
}

/// The most bars and tracks a file may have: far beyond any real song,
/// and small enough that a damaged file can't make the reader allocate
/// what it claims.
const MAX_BARS: usize = 4096;
const MAX_TRACKS: usize = 256;

/// Reads a TuxGuitar 2 file.
pub(crate) fn read(data: &[u8]) -> Result<Imported, String> {
    let opened = container::open(data)?;
    let mut log = Log::default();
    if opened.version.minor > tg::MINOR {
        log.note(
            format!(
                "the file is in TuxGuitar's format {}.{}, newer than the {}.{} OxiGuitar knows: \
                 what it adds is left out",
                opened.version.major,
                opened.version.minor,
                tg::MAJOR,
                tg::MINOR
            ),
            "the file",
        );
    }
    let text = std::str::from_utf8(&opened.content)
        .map_err(|e| format!("the song (content.xml) is not UTF-8 text: {e}"))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let doc = roxmltree::Document::parse(text)
        .map_err(|e| format!("the song (content.xml) is not readable XML: {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "TuxGuitarFile" {
        return Err(format!(
            "content.xml is not a TuxGuitar song (its first element is <{}>)",
            root.tag_name().name()
        ));
    }
    let song = child(root, "TGSong").ok_or("content.xml holds no song (no <TGSong>)")?;
    let mut reader = Reader {
        log,
        score: Score::new(),
    };
    reader.song(song)?;
    Ok(Imported {
        score: reader.score,
        log: reader.log,
    })
}

struct Reader {
    log: Log,
    score: Score,
}

/// A TuxGuitar channel: the sound a track plays with.
#[derive(Clone, Default)]
struct Channel {
    id: i64,
    bank: i64,
    program: i64,
    volume: i64,
    balance: i64,
    effects: bool,
    parameters: bool,
}

/// What every voice of a TuxGuitar beat shares.
struct BeatShared {
    stroke: Option<Stroke>,
    pick_stroke: Option<StrokeDirection>,
    chord: Option<ChordDiagram>,
    text: String,
}

/// A voice of a TuxGuitar beat, at its place in the bar.
struct Placed {
    start: Ratio,
    /// Which TuxGuitar beat it belongs to, for what the voices share.
    beat: usize,
    graces: Option<Beat>,
    main: Beat,
}

impl Reader {
    fn song(&mut self, song: Node<'_, '_>) -> Result<(), String> {
        let info = &mut self.score.info;
        info.title = text_of(song, "name");
        info.artist = text_of(song, "artist");
        info.album = text_of(song, "album");
        info.music = text_of(song, "author");
        info.words = text_of(song, "writer");
        info.copyright = text_of(song, "copyright");
        info.transcriber = text_of(song, "transcriber");
        let comments = text_of(song, "comments");
        if !comments.is_empty() {
            info.notices = comments.split('\n').map(str::to_owned).collect();
        }
        let date = text_of(song, "date");
        if !date.is_empty() {
            info.notices.push(format!("{DATE}{date}"));
        }
        let channels = children(song, "TGChannel")
            .map(channel)
            .collect::<Result<Vec<_>, _>>()?;
        let headers: Vec<Node<'_, '_>> = children(song, "TGMeasureHeader").collect();
        if headers.len() > MAX_BARS {
            return Err(format!(
                "the song has {} bars; OxiGuitar reads at most {MAX_BARS}",
                headers.len()
            ));
        }
        self.headers(&headers)?;
        let tracks: Vec<Node<'_, '_>> = children(song, "TGTrack").collect();
        if tracks.len() > MAX_TRACKS {
            return Err(format!(
                "the song has {} tracks; OxiGuitar reads at most {MAX_TRACKS}",
                tracks.len()
            ));
        }
        let mut keys: Vec<Option<KeySignature>> = vec![None; headers.len()];
        for (ti, node) in tracks.into_iter().enumerate() {
            let track = self.track(ti, node, &channels, &mut keys)?;
            self.score.tracks.push(track);
        }
        for (mb, key) in self.score.master_bars.iter_mut().zip(keys) {
            mb.key_signature = key.unwrap_or_default();
        }
        Ok(())
    }

    // ---- measure headers ------------------------------------------------

    fn headers(&mut self, headers: &[Node<'_, '_>]) -> Result<(), String> {
        let mut time = TimeSignature::COMMON;
        let mut bpm = 120.0;
        let mut previous_bpm = None;
        let mut system = 0usize;
        for (bi, header) in headers.iter().enumerate() {
            let at = format!("bar {}", bi + 1);
            if let Some(node) = child(*header, "timeSignature") {
                let numerator = attr_int(node, "numerator", &at)?;
                let denominator = attr_int(node, "denominator", &at)?;
                let ok = (1..=255).contains(&numerator)
                    && matches!(denominator, 1 | 2 | 4 | 8 | 16 | 32 | 64);
                if !ok {
                    return Err(format!(
                        "{at} has the time signature {numerator}/{denominator}"
                    ));
                }
                time = TimeSignature {
                    numerator: u8::try_from(numerator).unwrap_or(4),
                    denominator: u8::try_from(denominator).unwrap_or(4),
                };
            }
            if let Some(node) = child(*header, "tempo") {
                bpm = tempo(node, &at)?;
            }
            let mut mb = MasterBar {
                time_signature: time,
                ..MasterBar::default()
            };
            match previous_bpm {
                None => self.score.initial_tempo = bpm,
                Some(p) if p != bpm => mb.tempo_changes.push(TempoChange {
                    position: Ratio::ZERO,
                    bpm,
                    label: String::new(),
                    linear: false,
                }),
                Some(_) => {}
            }
            previous_bpm = Some(bpm);
            mb.repeat_start = child(*header, "repeatOpen").is_some();
            if let Some(node) = child(*header, "repeatClose") {
                let times = int(node, &at)?;
                // TuxGuitar counts the times it goes back; OxiGuitar the
                // times the section plays.
                if times > 0 {
                    mb.repeat_end =
                        Some(u8::try_from(times.saturating_add(1)).unwrap_or_else(|_| {
                            self.log
                                .lost("repeats are played up to 255 times", at.clone());
                            u8::MAX
                        }));
                }
            }
            if let Some(node) = child(*header, "repeatAlternative") {
                for alt in children(node, "alternative") {
                    let n = int(alt, &at)?;
                    match u32::try_from(n.saturating_sub(1)).ok().filter(|n| *n < 16) {
                        Some(bit) => mb.alternate_endings |= 1 << bit,
                        None => self
                            .log
                            .lost("alternate endings go up to the sixteenth", &at),
                    }
                }
            }
            if let Some(node) = child(*header, "marker") {
                mb.marker = Some(Marker {
                    letter: String::new(),
                    text: node.text().unwrap_or("").to_owned(),
                    color: color(node, &at)?,
                });
            }
            if let Some(node) = child(*header, "tripletFeel") {
                mb.triplet_feel = match node.text().unwrap_or("") {
                    "eighth" => TripletFeel::Triplet8th,
                    "sixteenth" => TripletFeel::Triplet16th,
                    "none" => TripletFeel::None,
                    other => {
                        return Err(format!("{at} has the triplet feel {other:?}"));
                    }
                };
            }
            // A line break starts a new system at its bar.
            if child(*header, "lineBreak").is_some() && bi > 0 {
                let bars = bi - system;
                match u8::try_from(bars) {
                    Ok(b) => self.score.layout.push(b),
                    Err(_) => {
                        self.log
                            .lost("line breaks after a line of more than 255 bars", &at);
                        self.score.layout.clear();
                    }
                }
                system = bi;
            }
            self.score.master_bars.push(mb);
        }
        Ok(())
    }

    // ---- tracks ------------------------------------------------------------

    fn track(
        &mut self,
        ti: usize,
        node: Node<'_, '_>,
        channels: &[Channel],
        keys: &mut [Option<KeySignature>],
    ) -> Result<Track, String> {
        let at = format!("track {}", ti + 1);
        let channel_id = child(node, "channelId")
            .map(|n| int(n, &at))
            .transpose()?
            .ok_or_else(|| format!("{at} names no channel"))?;
        let ch = match channels.iter().find(|c| c.id == channel_id) {
            Some(c) => c.clone(),
            None => {
                self.log.lost(
                    "a track's missing channel is replaced by a guitar's",
                    at.clone(),
                );
                Channel {
                    program: 25,
                    volume: 127,
                    balance: 64,
                    ..Channel::default()
                }
            }
        };
        if ch.effects {
            self.log.lost(
                "chorus, reverb, phaser and tremolo levels are left out",
                &at,
            );
        }
        if ch.parameters {
            self.log.lost(
                "the settings of TuxGuitar's sound plugins on a channel are left out",
                &at,
            );
        }
        let percussion = ch.bank == 128;
        let solo_mute = text_of(node, "soloMute");
        let mut track = Track {
            name: text_of(node, "name"),
            color: child(node, "color")
                .map(|n| color(n, &at))
                .transpose()?
                .unwrap_or_default(),
            instrument: Instrument {
                midi_program: u8::try_from(ch.program.clamp(0, 127)).unwrap_or(0),
                percussion,
            },
            mix: Mix {
                volume: ch.volume.clamp(0, 127) as f32 / 127.0,
                pan: ((ch.balance.clamp(0, 127) - 64) as f32 / 64.0).clamp(-1.0, 1.0),
                mute: solo_mute == "mute",
                solo: solo_mute == "solo",
            },
            ..Track::default()
        };
        let offset = child(node, "offset")
            .map(|n| int(n, &at))
            .transpose()?
            .unwrap_or(0)
            .clamp(-tg::MAX_OFFSET, tg::MAX_OFFSET);
        let strings = children(node, "TGString")
            .map(|n| int(n, &at))
            .collect::<Result<Vec<_>, _>>()?;
        if strings.len() > tg::MAX_STRINGS {
            return Err(format!("{at} has {} strings", strings.len()));
        }
        // A positive offset is a capo (TuxGuitar reads Guitar Pro's capo
        // into it); a negative one tunes every string down, as OxiGuitar
        // has no capo below the nut.
        let lower = if percussion { 0 } else { (-offset).max(0) };
        if lower > 0 {
            self.log.note(
                "a track transposed down is read as its strings tuned lower",
                &at,
            );
        }
        let tuning = strings
            .iter()
            .map(|p| Pitch(u8::try_from(p.saturating_sub(lower).clamp(0, 127)).unwrap_or(0)))
            .collect();
        let mut staff = Staff {
            tuning,
            capo: if percussion {
                0
            } else {
                u8::try_from(offset.max(0)).unwrap_or(0)
            },
            ..Staff::default()
        };
        if let Some(max) = node.attribute("maxFret") {
            let max: i64 = parse(max, "maxFret", &at)?;
            staff.fret_count = u8::try_from(max.clamp(1, 255)).unwrap_or(24);
        } else if !percussion {
            staff.fret_count = 29; // TuxGuitar's default
        }
        if let Some(lyric) = child(node, "TGLyric") {
            let text = lyric.text().unwrap_or("").to_owned();
            if !text.is_empty() {
                let from = lyric
                    .attribute("from")
                    .map(|f| parse::<i64>(f, "from", &at))
                    .transpose()?
                    .unwrap_or(1);
                track.lyrics.push(LyricLine {
                    start_bar: usize::try_from(from.saturating_sub(1)).unwrap_or(0),
                    text,
                });
            }
        }
        let measures: Vec<Node<'_, '_>> = children(node, "TGMeasure").collect();
        let bars = self.score.master_bars.len();
        if measures.len() != bars {
            self.log.lost(
                "a track whose bars don't match the song's is cut or filled with empty bars",
                &at,
            );
        }
        let mut clef = if percussion {
            Clef::Neutral
        } else {
            Clef::Treble8vb
        };
        let mut key = 0i64;
        let pitch_offset = if percussion { offset } else { 0 };
        let mut bar_start = Ratio::ZERO;
        for bi in 0..bars {
            let mb_len = self
                .score
                .master_bars
                .get(bi)
                .and_then(MasterBar::expected_length)
                .unwrap_or(Ratio::ONE);
            let bar = match measures.get(bi) {
                Some(m) => {
                    let at = place(bi, ti);
                    if let Some(c) = child(*m, "clef") {
                        clef = match (percussion, c.text().unwrap_or("")) {
                            (true, _) => Clef::Neutral,
                            (_, "treble") => Clef::Treble8vb,
                            (_, "bass") => Clef::Bass8vb,
                            (_, "tenor") => Clef::Tenor,
                            (_, "alto") => Clef::Alto,
                            (_, other) => return Err(format!("{at} has the clef {other:?}")),
                        };
                    }
                    if let Some(k) = child(*m, "keySignature") {
                        key = int(k, &at)?;
                        if !(0..=14).contains(&key) {
                            return Err(format!("{at} has the key signature {key}"));
                        }
                    }
                    let key = KeySignature {
                        fifths: i8::try_from(if key > 7 { 7 - key } else { key }).unwrap_or(0),
                        mode: KeyMode::Major,
                    };
                    if let Some(slot) = keys.get_mut(bi) {
                        match slot {
                            None => *slot = Some(key),
                            Some(k) if *k != key => self.log.lost(
                                "key signatures that differ between tracks follow the first track's",
                                &at,
                            ),
                            Some(_) => {}
                        }
                    }
                    self.bar(ti, bi, *m, bar_start, &staff, key, pitch_offset, &mut track)?
                }
                None => Bar::default(),
            };
            staff.bars.push(Bar { clef, ..bar });
            bar_start = bar_start
                .checked_add(mb_len)
                .ok_or("the song is too long")?;
        }
        track.staves.push(staff);
        Ok(track)
    }

    // ---- measures, beats and voices --------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn bar(
        &mut self,
        ti: usize,
        bi: usize,
        measure: Node<'_, '_>,
        bar_start: Ratio,
        staff: &Staff,
        key: KeySignature,
        pitch_offset: i64,
        track: &mut Track,
    ) -> Result<Bar, String> {
        let at = place(bi, ti);
        let percussion = track.instrument.percussion;
        let mut shared: Vec<BeatShared> = Vec::new();
        let mut voices: [Vec<Placed>; 2] = [Vec::new(), Vec::new()];
        for node in children(measure, "TGBeat") {
            let precise: i64 = child(node, "preciseStart")
                .ok_or_else(|| format!("{at}: a beat has no start"))
                .and_then(|n| int(n, &at))?;
            let start = precise
                .checked_sub(tg::START)
                .and_then(|p| Ratio::new(p, tg::WHOLE))
                .and_then(|s| s.checked_sub(bar_start))
                .ok_or_else(|| format!("{at}: a beat's start is out of range"))?;
            let bi_shared = shared.len();
            shared.push(BeatShared {
                stroke: child(node, "stroke")
                    .map(|n| stroke(n, &at))
                    .transpose()?
                    .flatten(),
                pick_stroke: match child(node, "pickStroke").and_then(|n| n.text()) {
                    Some("up") => Some(StrokeDirection::Up),
                    Some("down") => Some(StrokeDirection::Down),
                    _ => None,
                },
                chord: child(node, "chord").map(|n| chord(n, &at)).transpose()?,
                text: text_of(node, "text"),
            });
            for (vi, vnode) in children(node, "voice").enumerate() {
                if vi >= 2 {
                    self.log.lost("voices past the second are left out", &at);
                    break;
                }
                if vnode.attribute("direction").is_some() {
                    self.log.lost("stem directions are left out", &at);
                }
                if let Some((graces, main)) =
                    self.voice(vnode, &at, staff, key, pitch_offset, percussion)?
                    && let Some(list) = voices.get_mut(vi)
                {
                    list.push(Placed {
                        start,
                        beat: bi_shared,
                        graces,
                        main,
                    });
                }
            }
        }
        let mut bar = Bar {
            voices: Vec::new(),
            ..Bar::default()
        };
        let mut used = vec![false; shared.len()];
        for (vi, mut placed) in voices.into_iter().enumerate() {
            placed.sort_by_key(|p| p.start);
            if vi > 0 && placed.is_empty() {
                continue;
            }
            let mut voice = Voice::default();
            let mut position = Ratio::ZERO;
            for p in placed {
                if p.start < Ratio::ZERO {
                    self.log
                        .lost("a beat placed before the start of its bar is left out", &at);
                    continue;
                }
                if p.start < position {
                    self.log.lost(
                        "a beat that starts before the one before it ends is left out",
                        &at,
                    );
                    continue;
                }
                if p.start > position {
                    let gap = p.start.checked_sub(position).unwrap_or(Ratio::ZERO);
                    let pieces = time::fill(gap);
                    if pieces.is_empty() {
                        self.log
                            .lost("a gap in a voice that no rests fill is closed up", &at);
                    }
                    for duration in pieces {
                        voice.beats.push(Beat {
                            kind: BeatKind::Empty,
                            duration,
                            ..Beat::default()
                        });
                    }
                    position = p.start;
                }
                let mut main = p.main;
                if let Some(slot) = used.get_mut(p.beat)
                    && !*slot
                {
                    *slot = true;
                    if let Some(s) = shared.get_mut(p.beat) {
                        main.effects.stroke = s.stroke;
                        main.effects.pick_stroke = s.pick_stroke;
                        main.text = std::mem::take(&mut s.text);
                        if let Some(c) = s.chord.take() {
                            main.chord = Some(add_chord(track, c));
                        }
                    }
                }
                position = main
                    .duration
                    .length()
                    .and_then(|l| position.checked_add(l))
                    .ok_or_else(|| format!("{at}: a beat's duration is out of range"))?;
                voice.beats.extend(p.graces);
                voice.beats.push(main);
            }
            bar.voices.push(voice);
        }
        if shared
            .iter()
            .zip(&used)
            .any(|(s, used)| !used && (s.chord.is_some() || !s.text.is_empty()))
        {
            self.log.lost(
                "a chord or text on a beat with no note or rest is left out",
                &at,
            );
        }
        Ok(bar)
    }

    /// A voice of a beat: its grace beat, if its notes have grace notes,
    /// and its beat; `None` when the voice is marked empty.
    fn voice(
        &mut self,
        node: Node<'_, '_>,
        at: &str,
        staff: &Staff,
        key: KeySignature,
        pitch_offset: i64,
        percussion: bool,
    ) -> Result<Option<(Option<Beat>, Beat)>, String> {
        let notes: Vec<Node<'_, '_>> = children(node, "note").collect();
        let empty = match node.attribute("empty") {
            Some(e) => e == "true",
            None => notes.is_empty(),
        };
        if empty && notes.is_empty() {
            return Ok(None);
        }
        let duration_node =
            child(node, "duration").ok_or_else(|| format!("{at}: a beat has no duration"))?;
        let mut beat = Beat {
            kind: if notes.is_empty() {
                BeatKind::Rest
            } else {
                BeatKind::Notes
            },
            duration: duration(duration_node, at)?,
            ..Beat::default()
        };
        let mut graces: Vec<(Note, NoteValue, bool)> = Vec::new();
        let mut velocity = tg::DEFAULT_VELOCITY;
        for n in notes {
            if let Some(v) = n.attribute("velocity") {
                velocity = parse(v, "velocity", at)?;
            }
            let string: i64 = parse(n.attribute("string").unwrap_or(""), "string", at)?;
            let fret: i64 = parse(n.attribute("value").unwrap_or(""), "value", at)?;
            let Some(place) = self.place(staff, string, fret, pitch_offset, percussion, at) else {
                continue;
            };
            let (dynamic, exact) = tg::dynamic(velocity);
            if !exact {
                self.log.note(
                    "velocities between TuxGuitar's dynamics are read as the nearest",
                    at,
                );
            }
            let mut note = Note {
                place,
                tie: n.attribute("tiedNote") == Some("true"),
                dynamic,
                ..Note::fretted(1, 0)
            };
            self.note_effects(n, &mut note, &mut beat, at)?;
            if has(n, "alternativeEnharmonic")
                && let Some(p) = sounding(staff, &note)
            {
                note.spelling = spell::alternative(p, key);
            }
            if let Some(g) = child(n, "grace") {
                graces.push(self.grace(g, string, staff, pitch_offset, percussion, at)?);
            }
            beat.notes.push(note);
        }
        if beat.kind == BeatKind::Notes && beat.notes.is_empty() {
            // Every note was left out: the beat still takes its time.
            beat.kind = BeatKind::Rest;
        }
        let grace = (!graces.is_empty()).then(|| {
            let value = graces.first().map_or(NoteValue::ThirtySecond, |g| g.1);
            let on_beat = graces.iter().any(|g| g.2);
            Beat {
                kind: BeatKind::Notes,
                duration: Duration::new(value),
                notes: graces.into_iter().map(|g| g.0).collect(),
                grace: Some(if on_beat {
                    GraceKind::OnBeat
                } else {
                    GraceKind::BeforeBeat
                }),
                ..Beat::default()
            }
        });
        Ok(Some((grace, beat)))
    }

    /// Where a note is: on its string and fret, or, for drums, the pitch
    /// it plays.
    fn place(
        &mut self,
        staff: &Staff,
        string: i64,
        fret: i64,
        pitch_offset: i64,
        percussion: bool,
        at: &str,
    ) -> Option<NotePlace> {
        let open = usize::try_from(string.saturating_sub(1))
            .ok()
            .and_then(|s| staff.tuning.get(s));
        let Some(open) = open else {
            self.log
                .lost("a note on a string the track doesn't have is left out", at);
            return None;
        };
        if percussion {
            let pitch = i64::from(open.0)
                .saturating_add(fret)
                .saturating_add(pitch_offset);
            return match u8::try_from(pitch).ok().filter(|p| *p <= 127) {
                Some(p) => Some(NotePlace::Pitched(Pitch(p))),
                None => {
                    self.log
                        .lost("a drum outside the MIDI range is left out", at);
                    None
                }
            };
        }
        match (u8::try_from(string), u8::try_from(fret)) {
            (Ok(string), Ok(fret)) => Some(NotePlace::Fretted { string, fret }),
            _ => {
                self.log
                    .lost("a note on a fret below 0 or above 255 is left out", at);
                None
            }
        }
    }

    fn note_effects(
        &mut self,
        n: Node<'_, '_>,
        note: &mut Note,
        beat: &mut Beat,
        at: &str,
    ) -> Result<(), String> {
        let fx = &mut note.effects;
        if has(n, "vibrato") {
            fx.vibrato = Some(Vibrato::Slight);
        }
        fx.dead = has(n, "deadNote");
        if has(n, "slide") {
            fx.slides.push(Slide::Shift);
        }
        fx.hammer_pull = has(n, "hammer");
        fx.ghost = has(n, "ghostNote");
        if has(n, "accentuatedNote") {
            fx.accent = Some(oxiguitar_model::Accent::Normal);
        }
        if has(n, "heavyAccentuatedNote") {
            fx.accent = Some(oxiguitar_model::Accent::Heavy);
        }
        fx.palm_mute = has(n, "palmMute");
        fx.staccato = has(n, "staccato");
        fx.let_ring = has(n, "letRing");
        // OxiGuitar keeps these on the beat.
        let technique = [
            ("tapping", BeatTechnique::Tap),
            ("slapping", BeatTechnique::Slap),
            ("popping", BeatTechnique::Pop),
        ]
        .into_iter()
        .find(|(name, _)| has(n, name))
        .map(|(_, t)| t);
        if let Some(t) = technique {
            if beat.effects.technique.is_some_and(|b| b != t) {
                self.log.lost(
                    "notes of one beat tapped, slapped or popped differently are read as the first's",
                    at,
                );
            } else {
                beat.effects.technique = Some(t);
            }
        }
        if has(n, "fadeIn") {
            beat.effects.fade = Some(Fade::In);
        }
        if let Some(b) = child(n, "bend") {
            note.effects.bend = Some(curve(b, tg::BEND_CENTS, at)?);
        }
        if let Some(b) = child(n, "tremoloBar") {
            let whammy = curve(b, tg::WHAMMY_CENTS, at)?;
            if beat.effects.whammy.as_ref().is_some_and(|w| *w != whammy) {
                self.log.lost(
                    "notes of one beat with different tremolo bar dives are read as the first's",
                    at,
                );
            } else {
                beat.effects.whammy = Some(whammy);
            }
        }
        let fx = &mut note.effects;
        if let Some(h) = child(n, "harmonic") {
            let data: i64 = parse(h.attribute("data").unwrap_or("0"), "data", at)?;
            let row = usize::try_from(data)
                .ok()
                .and_then(|d| tg::HARMONICS.get(d))
                .copied();
            let row = row.unwrap_or_else(|| {
                self.log.lost(
                    "a harmonic of a kind TuxGuitar doesn't have is read as AH12",
                    at,
                );
                tg::HARMONICS[0]
            });
            fx.harmonic = match h.attribute("type").unwrap_or("") {
                "N.H" => Some(Harmonic::Natural),
                "A.H" => Some(Harmonic::Artificial { semitones: row.1 }),
                "T.H" => match note.place {
                    NotePlace::Fretted { fret, .. } => Some(Harmonic::Tapped {
                        fret: fret.saturating_add(row.0),
                    }),
                    NotePlace::Pitched(_) => None,
                },
                "P.H" => Some(Harmonic::Pinch),
                "S.H" => Some(Harmonic::Semi),
                other => return Err(format!("{at}: a harmonic of the kind {other:?}")),
            };
            if matches!(h.attribute("type"), Some("P.H" | "S.H")) && data != 0 {
                self.log.lost(
                    "the pitch a pinch or semi-harmonic sounds at is left out",
                    at,
                );
            }
        }
        if let Some(t) = child(n, "trill") {
            let fret: i64 = parse(t.attribute("fret").unwrap_or(""), "fret", at)?;
            let speed: i64 = parse(t.attribute("duration").unwrap_or(""), "duration", at)?;
            note.effects.trill = Some(Trill {
                fret: u8::try_from(fret.clamp(0, 255)).unwrap_or(0),
                speed: tg::value(speed).unwrap_or(NoteValue::ThirtySecond),
            });
        }
        if let Some(t) = child(n, "tremoloPicking") {
            let v: i64 = parse(t.attribute("duration").unwrap_or(""), "duration", at)?;
            note.effects.tremolo_picking = Some(tg::value(v).unwrap_or(NoteValue::Sixteenth));
        }
        Ok(())
    }

    /// A note's grace note: the note, its value, and whether it is played
    /// on the beat.
    fn grace(
        &mut self,
        g: Node<'_, '_>,
        string: i64,
        staff: &Staff,
        pitch_offset: i64,
        percussion: bool,
        at: &str,
    ) -> Result<(Note, NoteValue, bool), String> {
        let fret: i64 = parse(g.attribute("fret").unwrap_or(""), "fret", at)?;
        let velocity: i64 = parse(g.attribute("dynamic").unwrap_or("95"), "dynamic", at)?;
        let value: i64 = parse(g.attribute("duration").unwrap_or("32"), "duration", at)?;
        let place = self
            .place(staff, string, fret, pitch_offset, percussion, at)
            .unwrap_or(NotePlace::Fretted { string: 1, fret: 0 });
        let mut note = Note {
            place,
            dynamic: tg::dynamic(velocity).0,
            ..Note::fretted(1, 0)
        };
        note.effects.dead = g.attribute("dead") == Some("true");
        match g.attribute("transition").unwrap_or("none") {
            "slide" => note.effects.slides.push(Slide::Legato),
            "hammer" => note.effects.hammer_pull = true,
            "bend" => self
                .log
                .lost("a grace note's bend into its note is left out", at),
            _ => {}
        }
        let value = match value {
            16 => NoteValue::Sixteenth,
            64 => NoteValue::SixtyFourth,
            _ => NoteValue::ThirtySecond,
        };
        Ok((note, value, g.attribute("onBeat") == Some("true")))
    }
}

/// The text TuxGuitar adds a song's date to its notices with.
pub(crate) const DATE: &str = "Date: ";

/// The pitch a note sounds, if it is on a string.
fn sounding(staff: &Staff, note: &Note) -> Option<Pitch> {
    match note.place {
        NotePlace::Fretted { string, fret } => staff.pitch_of(string, fret),
        NotePlace::Pitched(p) => Some(p),
    }
}

fn channel(node: Node<'_, '_>) -> Result<Channel, String> {
    let at = "a channel";
    let get = |name: &str, default: i64| -> Result<i64, String> {
        child(node, name).map_or(Ok(default), |n| int(n, at))
    };
    let effects = ["chorus", "reverb", "phaser", "tremolo"]
        .iter()
        .map(|n| get(n, 0))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Channel {
        id: child(node, "id")
            .map(|n| int(n, at))
            .transpose()?
            .ok_or("a channel has no id")?,
        bank: get("bank", 0)?,
        program: get("program", 25)?,
        volume: get("volume", 127)?,
        balance: get("balance", 64)?,
        effects: effects.iter().any(|v| *v != 0),
        parameters: children(node, "TGChannelParameter").next().is_some(),
    })
}

/// A tempo in quarter notes a minute: TuxGuitar 2 counts in any note value
/// (`base`), dotted or not.
fn tempo(node: Node<'_, '_>, at: &str) -> Result<f64, String> {
    let raw = int(node, at)?;
    let base: i64 = node
        .attribute("base")
        .map_or(Ok(4), |b| parse(b, "base", at))?;
    let dotted = node.attribute("dotted") == Some("true");
    if raw <= 0 || tg::value(base).is_none() {
        return Err(format!("{at} has the tempo {raw} (base {base})"));
    }
    let mut bpm = raw as f64 * 4.0 / base as f64;
    if dotted {
        bpm *= 1.5;
    }
    Ok(bpm)
}

fn duration(node: Node<'_, '_>, at: &str) -> Result<Duration, String> {
    let v: i64 = parse(node.attribute("value").unwrap_or(""), "value", at)?;
    let value = tg::value(v).ok_or_else(|| format!("{at}: a beat lasts a 1/{v}"))?;
    let dots = match node.attribute("dotted") {
        Some("dotted") => 1,
        Some("doubleDotted") => 2,
        _ => 0,
    };
    let mut tuplets = Vec::new();
    if let Some(d) = child(node, "divisionType") {
        let enters = attr_int(d, "enters", at)?;
        let times = attr_int(d, "times", at)?;
        let (Ok(notes), Ok(in_time_of)) = (u8::try_from(enters), u8::try_from(times)) else {
            return Err(format!("{at}: a tuplet of {enters} in the time of {times}"));
        };
        if notes == 0 || in_time_of == 0 {
            return Err(format!("{at}: a tuplet of {enters} in the time of {times}"));
        }
        if notes != in_time_of {
            tuplets.push(Tuplet { notes, in_time_of });
        }
    }
    Ok(Duration {
        value,
        dots,
        tuplets,
    })
}

fn stroke(node: Node<'_, '_>, at: &str) -> Result<Option<Stroke>, String> {
    let direction = match node.attribute("direction") {
        Some("up") => StrokeDirection::Up,
        Some("down") => StrokeDirection::Down,
        _ => return Ok(None),
    };
    let v: i64 = parse(node.attribute("value").unwrap_or("16"), "value", at)?;
    Ok(Some(Stroke {
        direction,
        speed: tg::value(v).unwrap_or(NoteValue::Sixteenth),
    }))
}

fn chord(node: Node<'_, '_>, at: &str) -> Result<ChordDiagram, String> {
    let first: i64 = child(node, "firstFret").map_or(Ok(0), |n| int(n, at))?;
    let frets = children(node, "string")
        .map(
            |s| match s.text().map(str::trim).filter(|t| !t.is_empty()) {
                None => Ok(None),
                Some(t) => parse::<i64>(t, "string", at).map(|f| u8::try_from(f).ok()),
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ChordDiagram {
        name: text_of(node, "name"),
        first_fret: u8::try_from(first.clamp(0, 255)).unwrap_or(0),
        frets,
    })
}

/// A track's chord diagram, added once.
fn add_chord(track: &mut Track, chord: ChordDiagram) -> usize {
    if let Some(i) = track.chords.iter().position(|c| *c == chord) {
        return i;
    }
    track.chords.push(chord);
    track.chords.len() - 1
}

/// A bend or tremolo bar curve: positions in twelfths of the note, heights
/// in `cents` a unit.
fn curve(node: Node<'_, '_>, cents: i32, at: &str) -> Result<Bend, String> {
    let points = children(node, "point")
        .take(64)
        .map(|p| {
            let position = attr_int(p, "position", at)?.clamp(0, tg::BEND_POSITIONS);
            let value = attr_int(p, "value", at)?.clamp(-1000, 1000);
            Ok(BendPoint {
                offset: Ratio::new(position, tg::BEND_POSITIONS).unwrap_or(Ratio::ZERO),
                cents: i32::try_from(value).unwrap_or(0).saturating_mul(cents),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Bend { points })
}

fn color(node: Node<'_, '_>, at: &str) -> Result<Color, String> {
    let c = |name: &str| -> Result<u8, String> {
        Ok(u8::try_from(attr_int(node, name, at)?.clamp(0, 255)).unwrap_or(0))
    };
    Ok(Color {
        r: c("R")?,
        g: c("G")?,
        b: c("B")?,
    })
}

// ---- XML helpers ---------------------------------------------------------

/// The first child element named `name`.
fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == name)
}

/// Every child element named `name`.
fn children<'a, 'i>(node: Node<'a, 'i>, name: &str) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children()
        .filter(move |n| n.is_element() && n.tag_name().name() == name)
}

/// Whether there is a child element named `name`.
fn has(node: Node<'_, '_>, name: &str) -> bool {
    child(node, name).is_some()
}

/// The text of the child named `name`; empty if it or its text is missing.
fn text_of(node: Node<'_, '_>, name: &str) -> String {
    child(node, name)
        .and_then(|n| n.text())
        .unwrap_or("")
        .to_owned()
}

/// An element's text as a number.
fn int(node: Node<'_, '_>, at: &str) -> Result<i64, String> {
    parse(node.text().unwrap_or(""), node.tag_name().name(), at)
}

/// An attribute as a number.
fn attr_int(node: Node<'_, '_>, name: &str, at: &str) -> Result<i64, String> {
    parse(node.attribute(name).unwrap_or(""), name, at)
}

fn parse<T: std::str::FromStr>(text: &str, what: &str, at: &str) -> Result<T, String> {
    text.trim()
        .parse()
        .map_err(|_| format!("{at}: {what} {text:?} is not a number"))
}

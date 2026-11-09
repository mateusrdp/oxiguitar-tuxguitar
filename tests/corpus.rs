// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The private test corpus, when it is here (never committed): every
//! TuxGuitar file in `corpus/files/` reads, writes and reads back the same:
//! from the first trip where the writer names no losses, from the second
//! otherwise (the first leaves out what TuxGuitar can't hold). `cargo test
//! --test corpus -- --nocapture` locally; with no corpus it checks nothing.
//! The corpus's Guitar Pro files become TuxGuitar files with `oxiguitar-cli
//! convert` (see docs/design.md, "Tests").

use oxiguitar_model::{Score, wire};

fn read(data: &[u8], what: &str) -> Score {
    let read =
        oxiguitar_tuxguitar_plugin::read(data).unwrap_or_else(|e| panic!("{what}: {}", e.message));
    wire::from_wire2(read.score).unwrap()
}

fn write(score: &Score) -> (Vec<u8>, usize) {
    let written = oxiguitar_tuxguitar_plugin::write(&wire::to_wire2(score)).unwrap();
    (written.data, written.losses.len())
}

#[test]
fn every_corpus_file_round_trips() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/files");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        println!("no corpus in {}", dir.display());
        return;
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("tg")))
        .collect();
    paths.sort();
    let (mut exact, mut stable) = (0, 0);
    for path in &paths {
        let what = path.display().to_string();
        let first = read(&std::fs::read(path).unwrap(), &what);
        let (data, losses) = write(&first);
        let second = read(&data, &what);
        if losses == 0 {
            assert!(first == second, "{what} doesn't round-trip");
            exact += 1;
        } else {
            let (data, _) = write(&second);
            let third = read(&data, &what);
            assert!(second == third, "{what} doesn't settle after one trip");
            stable += 1;
        }
        println!(
            "{what}: {} bars, {} tracks, {losses} kinds of loss",
            first.master_bars.len(),
            first.tracks.len()
        );
    }
    println!(
        "{} files: {exact} exact from the first trip, {stable} from the second",
        paths.len()
    );
}

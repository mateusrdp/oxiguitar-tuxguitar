# Features and their tests

Everything the TuxGuitar plugin does, and the tests that check it. The test names are the ones `cargo test` prints; a `stage:` is a step of `scripts/test-all.py`. The script fails when a feature here names no test, or names a test that doesn't exist, so a new feature gets its row and its tests in the same pull request (the [Testing](testing.md) page says how to run them, on your own files too).

Many of the tests use TuxGuitar's own test files (`fixtures/tuxguitar/`, LGPL-2.1, with their notice).

| Feature | What it does | Tests |
| --- | --- | --- |
| Recognises TuxGuitar 2 files | A ZIP holding `version.txt` and `content.xml`; major 2 is read, a newer minor with a note; other versions, the old binary `.tg` and files TuxGuitar itself refuses are refused. | `contract::its_signature_recognises_tuxguitar_2_files_only`, `corpus::tuxguitar_files_are_recognised_by_their_contents`, `container::tests::versions`, `tuxguitar_files::a_newer_minor_version_reads_with_a_note`, `tuxguitar_files::the_files_tuxguitar_refuses_are_refused` |
| Reads songs as TuxGuitar means them | The song's texts, channels as instruments and mix, measure headers (time, tempo, repeats, alternatives, markers, line breaks), tracks with strings, capo and lyrics, both voices, notes and their effects; what TuxGuitar's own tests assert holds. | `tuxguitar_files::test_20_reads_as_tuxguitar_says`, `tuxguitar_files::reference_20_reads_as_tuxguitar_says`, `tuxguitar_files::every_2x_file_reads`, `tg::tests::the_unit_of_time_is_tuxguitars`, `tg::tests::velocities_and_dynamics`, `time::tests::fills_lengths`, `spell::tests::the_other_names` |
| Writes TuxGuitar 2 files | What it reads is written and reads back exactly; what it writes validates against TuxGuitar's schema (where xmllint is installed). | `read_write::tuxguitars_files_round_trip`, `read_write::our_fixture_round_trips`, `read_write::what_is_written_validates_against_tuxguitars_schema`, `container::tests::an_archive_opens_again` |
| Names what it can't write | Voices past the second, slurs, hairpins, segno and coda, fingerings and the rest: each kind of loss once, counted, before saving. | `log::tests::one_entry_per_kind_counted` |
| Your TuxGuitar files | Every TuxGuitar 2 file of the corpus reads and round-trips: exactly where no losses are named, stable from the second trip otherwise. | `corpus::every_corpus_tuxguitar_file_round_trips` |
| Through the newest OxiGuitar | Every corpus file OxiGuitar opens (TuxGuitar by this plugin, Guitar Pro by the released Guitar Pro plugins) is written as `.tg` three times and read back by this plugin, settling after the first trip. | `stage:corpus through the core` |
| A working plugin file | The `.oxp` file keeps the plugin contract, with this repository's SDK version and with the newest release of OxiGuitar. | `contract::the_plugin_file_keeps_the_contract`, `oxiguitar_plugin_checks::the_manifest_matches_the_plugin`, `stage:contract (latest core)` |

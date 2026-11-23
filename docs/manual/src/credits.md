# Credits

Thank you to the people whose work this plugin is built on, and above all to the TuxGuitar project.

## Code we use

| Project | Authors | What it does here | Licence |
| --- | --- | --- | --- |
| [OxiGuitar's plugin SDK](https://github.com/mateusrdp/oxiguitar) (`oxiguitar-plugin`, `oxiguitar-model`, and `oxiguitar-plugin-test` for the tests) | Mateus Rocha de Paula and the OxiGuitar contributors | The plugin interface and the score model | MIT OR Apache-2.0 for the interface and SDK; GPL-3.0-or-later for the model |
| [roxmltree](https://github.com/RazrFalcon/roxmltree) | Yevhenii Reizner and contributors | Reading `content.xml` | MIT OR Apache-2.0 |
| [quick-xml](https://github.com/tafia/quick-xml) | Johann Tuffe, Mingun and contributors | Writing `content.xml` | MIT |
| [zip](https://github.com/zip-rs/zip2) | Mathijs van de Nes, Marli Frost, Ryan Levick, Chris Hennick and contributors | The `.tg` archive | MIT |
| [zlib-rs](https://github.com/trifectatechfoundation/zlib-rs) | The Trifecta Tech Foundation | Deflate compression, in pure Rust, under zip | Zlib |
| [serde_json](https://github.com/serde-rs/json) | David Tolnay, Erick Tryzelaar and contributors | Tests only | MIT OR Apache-2.0 |
| [Rust](https://www.rust-lang.org/) and its standard library | The Rust Project | Everything else | MIT OR Apache-2.0 |

## Ported from

| Project | Authors | What we took | Licence |
| --- | --- | --- | --- |
| [TuxGuitar](https://github.com/helge17/tuxguitar) | Julian Casadesus, helge17 and contributors | The `.tg` reader and writer, ported from its Java (`TGSongReaderImpl`, `TGSongWriterImpl` and friends), its test files to check against, and its schema to check what we write | LGPL-2.1 |

Each ported file says so at its top, with TuxGuitar's copyright line.

## Tools

| Tool | Use |
| --- | --- |
| [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) and [libfuzzer-sys](https://github.com/rust-fuzz/libfuzzer) | Fuzzing the reader and the writer (`fuzz/`) |
| [mdBook](https://github.com/rust-lang/mdBook) | Builds this manual |
| [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | Checks the licences of everything we build with |

## Thanks

To Julian Casadesus, who started TuxGuitar, and to helge17 and everyone who keeps it going: for a free tab editor that has served guitarists for years, and for code clear enough to port.

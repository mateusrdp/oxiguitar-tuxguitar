# Architecture and strategy

No Rust library reads TuxGuitar files, so the format code is our own, a port of TuxGuitar's Java code (LGPL-2.1) onto the `roxmltree`, `quick-xml` and `zip` libraries. A TuxGuitar 2 file is a ZIP archive with a `content.xml` inside, and the plugin reads and writes that.

The strategy is to match TuxGuitar's own meaning for each value: its unit of time, velocities, harmonics, bends in quarter tones, and the capo. Tests read TuxGuitar's own test files and compare them to what TuxGuitar's tests say they hold. What we write is checked against TuxGuitar's schema.

The plugin recognises a TuxGuitar 2 file by the ZIP archive holding `version.txt`, not by the extension.

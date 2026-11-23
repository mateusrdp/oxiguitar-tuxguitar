# Architecture and strategy

No Rust library reads TuxGuitar files, so the format code is our own, a port of TuxGuitar's Java code (LGPL-2.1) onto the `roxmltree`, `quick-xml` and `zip` libraries. A TuxGuitar 2 file is a ZIP archive with a `content.xml` inside, and the plugin reads and writes that.

The strategy is to match TuxGuitar's own meaning for each value: its unit of time, velocities, harmonics, bends in quarter tones, and the capo. Tests read TuxGuitar's own test files and compare them to what TuxGuitar's tests say they hold. What we write is checked against TuxGuitar's schema.

The plugin recognises a TuxGuitar 2 file by the ZIP archive holding `version.txt`, not by the extension.

## Current limitations

- **TuxGuitar 2 files only.** The binary `.tg` of TuxGuitar 0.7 to 1.5 isn't read: open it in TuxGuitar 2 and save it there first.
- **TuxGuitar 1.x can't open what we write.** Some Linux distributions still ship TuxGuitar 1.6; its users need TuxGuitar 2.
- **Two voices.** TuxGuitar has two voices a track; OxiGuitar has four. Voices three and four are lost when saving, and OxiGuitar says so first.
- **Stem directions** and a few layout details are named in the import log and dropped.

## Left out on purpose

- **The old binary format.** TuxGuitar is free, and TuxGuitar 2 converts every older file, so reading the old form would be a lot of code for files anyone can convert in a minute (Mateus, 9 November 2026).
- **Guessing from the extension.** A `.tg` file is recognised by the ZIP archive holding `version.txt`.

## Where you can help

- **Files from real TuxGuitar 2 users** to test with, especially with unusual tunings, many tracks or lots of effects. Never attach files you aren't allowed to share.
- **Checking what we write in TuxGuitar 2**, on your system, and telling us what looks wrong.
- **Following TuxGuitar's changes**: when its format moves on, the port here needs to follow (`src/`, `docs/design.md`).

Every change needs its tests and a row on the [Features](features.md) page; the [Testing](testing.md) page says how to run them.


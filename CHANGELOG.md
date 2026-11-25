# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project
follows [Semantic Versioning](https://semver.org/). A release's number is
the plugin's own; the plugin interface it uses is in `manifest.json` and in
the release notes.

## [Unreleased]

## [0.2.0] - 2026-11-25

### Changed

- **The plugin file is a tar archive** (mateusrdp/oxiguitar#347): `build-oxp.sh` packs `manifest.json` first and the library second (`plugin.so`, `plugin.dll` or `plugin.dylib`), as OxiGuitar 0.23 and later read plugin files, which older OxiGuitars can't load. The SDK is at OxiGuitar v0.23.0 (interface 1.16). `scripts/test-all.py` 1.1.0, as in every repository.

### Added

- **Every test with one command** (mateusrdp/oxiguitar#273): `scripts/test-all.py` builds and runs all of this repository's tests on your own corpus (`--corpus DIR` or `OXIGUITAR_CORPUS`), and tests the plugin against the newest OxiGuitar release, with a progress bar, a summary to paste into an issue and a log to diagnose from. The manual has a Features page naming the tests of every feature (CI checks it) and a Testing page.

### Changed

- **CI outlasts a slow Ubuntu mirror**: it installs packages with `scripts/apt-install.sh`, the same file as in every OxiGuitar repository. apt is stopped only when its downloads have not grown for 2 minutes, then tried again on the next mirror; while they grow, it may take as long as it needs (24 Nov 2026: fixed 5-minute tries failed jobs on mirrors at 80 to 185 kB/s).
- **The manual explains the ideas behind the plugin** (its overview, with pictures), shows how to use it with screenshots, says in Architecture and strategy what it can't do yet, what is left out on purpose and where help is wanted, and has a Credits page listing every project it uses, with links and thanks. The README has a plain-words introduction and a Credits section.

## [0.1.1] - 2026-11-16

### Added

- **A Flatpak extension**, `io.github.mateusrdp.OxiGuitar.Plugin.tuxguitar`, in
  the release and in OxiGuitar's Flatpak repository, for OxiGuitar
  installed as a Flatpak. 0.1.0 was released without it: its release was
  built before the packaging gained Flatpak extensions.

## [0.1.0] - 2026-11-09

### Added

- **The TuxGuitar plugin** (`tuxguitar.oxp`, id `oxiguitar.tuxguitar`, #223 and #171 in OxiGuitar): opens and saves TuxGuitar 2's files (`.tg`, a ZIP holding `version.txt` and `content.xml`), recognised by the `version.txt` they hold. Our own code, ported from TuxGuitar's (LGPL-2.1), on roxmltree, quick-xml and zip. Format 2.x is read (a newer minor version with a note in the import log) and 2.0 written, as TuxGuitar 2 writes it; TuxGuitar 1.x's binary files are not read. What either side can't hold is named in the import log or before saving. Needs OxiGuitar 0.18 or later (plugin interface 1.7).
- TuxGuitar's own test files and its schema of the format as fixtures, under their LGPL-2.1 notice, checked against what TuxGuitar's tests say they hold; what the plugin writes validates against the schema.

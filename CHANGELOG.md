# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project
follows [Semantic Versioning](https://semver.org/). A release's number is
the plugin's own; the plugin interface it uses is in `manifest.json` and in
the release notes.

## [Unreleased]

## [0.1.0] - 2026-11-09

### Added

- **The TuxGuitar plugin** (`tuxguitar.oxp`, id `oxiguitar.tuxguitar`, #223 and #171 in OxiGuitar): opens and saves TuxGuitar 2's files (`.tg`, a ZIP holding `version.txt` and `content.xml`), recognised by the `version.txt` they hold. Our own code, ported from TuxGuitar's (LGPL-2.1), on roxmltree, quick-xml and zip. Format 2.x is read (a newer minor version with a note in the import log) and 2.0 written, as TuxGuitar 2 writes it; TuxGuitar 1.x's binary files are not read. What either side can't hold is named in the import log or before saving. Needs OxiGuitar 0.18 or later (plugin interface 1.7).
- TuxGuitar's own test files and its schema of the format as fixtures, under their LGPL-2.1 notice, checked against what TuxGuitar's tests say they hold; what the plugin writes validates against the schema.

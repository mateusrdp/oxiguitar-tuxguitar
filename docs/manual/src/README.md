# TuxGuitar plugin

This plugin lets OxiGuitar open songs made in TuxGuitar 2, and save songs TuxGuitar 2 opens. If you have been writing tabs in TuxGuitar, you can bring them over, or go back and forth between the two programs.

It reads and writes `.tg` files made by TuxGuitar 2. It only handles files. It makes no sound.

## What TuxGuitar is

[TuxGuitar](https://github.com/helge17/tuxguitar) is a free, open-source tablature editor and player, written in Java, that has been around for many years. For many Linux users it was *the* tab editor, and it still reads Guitar Pro files and plays songs with a General MIDI synthesiser. It was started by Julian Casadesus and is maintained today by helge17 and contributors.

TuxGuitar has its own file format, `.tg`, and it changed shape over the years:

- **TuxGuitar 0.7 to 1.5** wrote a compact binary format.
- **TuxGuitar 2** writes a ZIP archive holding two files: `version.txt` with the format's version, and `content.xml`, the song itself in XML.

This plugin reads and writes the TuxGuitar 2 form. An older file opens fine in TuxGuitar 2, which can save it in the new form; OxiGuitar can then read the result.

## What comes across

Tracks, tunings, repeats and alternate endings, lyrics, chord diagrams, both of TuxGuitar's voices and the note effects all come along. What OxiGuitar has no place for, such as stem directions, is listed in the import log. When saving, OxiGuitar tells you first what a TuxGuitar file can't hold, such as voices past the second, slurs, hairpins, segno and coda signs, and fingerings.

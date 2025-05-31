## MKV Edit
A command line utility to modify the metadata of mkv files, for the purpose of organization in a music player.

This is actually an abstraction layer over `mkvextract`, `mkvinfo` and `mkvpropedit`,
using your system editor (defined in `$EDITOR`) and `grep`.  As such, you will need
to have all the tools installed.  If you use arch linux, just install the `mediainfo`
package and set `$EDITOR` to your editor of choice.


usage:
`mkv_edit <files> ...`


This will open a file in your system editor where you can modify some metadata fields.
If provided multiple mkv files at the same time, it will open one at a time.  The mkv
files will only be modified once you close the editor.

Do not modify the headers, as they are how the data gets detected.

Note: this uses temporarary files in your system's temporary folder, and this might
come with some privacy concerns if there are untrusted users on your system.

Planned:
- asyncronous modification of mkv files (not waiting until editor is closed)
- open multiple files in a single editor session
- modifiable config for what properties to change

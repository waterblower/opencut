# OpenCut

OpenCut is an experimental desktop video tool written in Rust with
[GPUI](https://gpui.rs/). The `rust` package contains two applications:

- `opencut-player`: a local MP4/MOV player using FFmpeg, CPAL audio, and GPUI rendering.
- `opencut-editor`: a non-destructive, folder-based multi-track editor with
  FFmpeg video/audio file previews and model-based timeline editing. Timeline
  previews are currently black and silent; timeline export is unavailable.

The project is an active prototype rather than a production-ready editor.

Devlog: https://www.youtube.com/playlist?list=PLRz1nfZl0jMU

## Requirements

- Latest stable Rust (edition 2024)
- Existing FFmpeg development libraries in `rust/vendor/ffmpeg-8.1.2/`
- Xcode command line tools and `pkg-config` on macOS

Use the existing vendored FFmpeg libraries; do not build FFmpeg locally.
On macOS, install `pkg-config` with `brew install pkg-config`, then run the
commands below from `rust`.

## Player

Build, run, and test commands select a platform explicitly. On macOS:

```sh
cargo build-player-mac
cargo build-editor-mac
cargo player-mac
cargo editor-mac
cargo test-mac
```

Run these from `rust`. Extra Cargo flags such as `--release` are forwarded;
application arguments follow `--`. On Windows, use the corresponding `-win`
commands, such as `cargo editor-win` and `cargo test-win`. These are native host
commands, not cross-compilation commands. Editor aliases load `.cargo/macos.toml`
or `.cargo/windows.toml`; player aliases use the FFmpeg-only `.cargo/cli.toml`
or `.cargo/ffmpeg-windows.toml`. Small shell runners
set runtime library paths for applications and tests. No Rust launcher
is compiled, and the commands do not change the parent shell's environment.

Both applications use the existing vendored FFmpeg libraries. Platform runners
set their runtime library paths without requiring a separately installed media
runtime.

For other Cargo operations, select the platform configuration from `rust`:

```powershell
cargo check --config .cargo/windows.toml --no-default-features --features editor --bin opencut-editor
```

```sh
cargo check --config .cargo/macos.toml --no-default-features --features editor --bin opencut-editor
```

```sh
cargo player-mac # Windows: cargo player-win
```

The player uses vendored FFmpeg for MP4/MOV decoding and history thumbnails,
CPAL for audio output, and GPUI for rendering. Playback is independent of the timeline editor.
It supports playback, scrubbing, approximate frame stepping using the average
frame rate, volume/mute, fullscreen, resizable playback history, and render FPS
inspection. Playback speed, looping, and audio-device selection are not yet
supported by this backend.

| Shortcut | Action |
| --- | --- |
| `Space` | Play or pause |
| `Left` / `Right` | Step backward or forward one frame |
| `M` | Mute or unmute |
| `Command-B` | Toggle playback history |
| `F` / `Esc` | Enter / exit fullscreen |
| `Option-Command-I` | Toggle the GPUI inspector |

## Editor

```sh
cargo editor-mac # Windows: cargo editor-win
```

Current editor capabilities:

- Open any ordinary folder as a project; supported media appears in a live file
  tree without an import or copy step. Filter the complete folder tree, preview
  media in place, or drag media directly onto compatible timeline tracks.
- Timelines are ordinary `*.timeline.json` files located at the project root or
  inside project subdirectories. The editor does not create one automatically.
  Use **New Timeline** and click a timeline entry in the Explorer to switch to it.
- A new timeline starts with no tracks. Create video and audio tracks manually;
  video tracks accept video and still images, while audio tracks accept audio
  files.
- Select clips individually, with Command-click, or by drawing a selection
  rectangle. Command-A selects every clip on unlocked tracks. Multi-clip move,
  duplicate, copy, cut, paste, and delete operations preserve relative timing
  and are recorded as single undo steps within the active timeline.
- Selection, blade, and trim tools support positioning clips, moving them between
  compatible tracks, splitting them, and trimming their source ranges without
  changing source files. Invalid moves show collision or compatibility feedback.
- Track visibility, mute, lock, reorder, creation, and deletion update the saved
  timeline model. Timeline previews currently remain black and silent. Ruler and
  track interactions continue to work, including scrubbing and frame stepping.
- The frame-based timeline supports horizontal scroll and zoom (including macOS
  trackpad pinch), vertical track scrolling, frame ticks at high zoom, frame
  stepping, a draggable playhead, and optional snapping with visible guides for
  the playhead and clip edges. Playhead position, scroll, zoom, snapping, and
  track-magnet settings are stored per timeline.
- Multiresolution waveform peaks are generated in the background and retained
  in memory by media path. Each clip renders only its selected source range.
- Undo/redo, clip metadata, fullscreen preview, and a docked GPUI element
  inspector with render FPS are available in the editor UI.
- Selected visual clips expose position and scale controls. These values remain
  editable and persisted while timeline rendering is unavailable.
- Clips retain audio gain and mute values in the document for future playback.
- Individual audio/video files support Generate SRT. Timeline transcription and
  timeline MP4 export are unavailable while their new backends are developed.

Supported file extensions:

- Video: `.mp4`, `.mov`, `.m4v`, `.mkv`, `.webm`, `.avi`
- Images: `.png`, `.jpg`, `.jpeg` (added as five-second still clips)
- Audio: `.aac`, `.flac`, `.m4a`, `.mp3`, `.ogg`, `.wav`

| Shortcut | Action |
| --- | --- |
| `Space` | Play or pause a media-file preview |
| `Left` / `Right` | Move the playhead backward or forward one project frame |
| `V` / `B` / `T` | Activate the selection, blade, or trim tool |
| `Command-click` | Add or remove a clip from the current selection |
| `Command-B` | Split every compatible selected clip at the playhead |
| `Backspace` / `Delete` | Delete the selected clips |
| `Command-D` | Duplicate the selected clips |
| `Command-C` / `Command-X` / `Command-V` | Copy / cut / paste selected clips |
| `Command-A` | Select all clips on unlocked tracks |
| `Command-Z` / `Command-Shift-Z` | Undo / redo |
| `F` / `Esc` | Enter / exit fullscreen preview |
| `Option-Command-I` | Toggle the GPUI inspector |
| `Option-Command-R` | Reveal the selected project entry in Finder |
| `Control-Shift-Enter` | Open the selected entry in its default app |

## Project data

Each timeline is saved automatically as a JSON document such as
`<project folder>/main.timeline.json` or
`<project folder>/timelines/opening.timeline.json`. A project can contain
multiple timeline files. Each currently stores its own timeline settings, media
metadata, tracks, clips, and view state. Media paths are relative to the project
folder, so a project folder can be moved, backed up, or committed as one unit.

The last opened project folder is stored locally in
`rust/data/editor-settings.json`. This location is temporary while OpenCut is a
prototype. On startup, the editor restores the saved active timeline and playhead; a missing
saved timeline leaves the editor with no active timeline.

Source media is referenced in place and never rewritten. Waveform peaks are
regenerated in memory when the editor opens a project and are not written to
the project folder.

## Development

```sh
cargo test-mac # Windows: cargo test-win
cargo check --config .cargo/macos.toml --all-targets --all-features
cargo clippy --config .cargo/macos.toml --all-features
cargo loc
```

`cargo loc` reports the Rust source line count for this project.
Use `.cargo/windows.toml` for check and clippy on Windows. Plain `cargo test`
does not load a platform configuration; use `test-mac` or `test-win` when testing
the editor and its native media dependencies.

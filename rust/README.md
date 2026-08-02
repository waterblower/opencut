# OpenCut

OpenCut is an experimental desktop video tool written in Rust with
[GPUI](https://gpui.rs/). This directory contains two applications:

- `opencut-player`: a focused local MP4 player powered by GStreamer.
- `opencut-editor`: a non-destructive, folder-based multi-track editor with
  GStreamer preview and in-process FFmpeg export.

The project is an active prototype rather than a production-ready editor.

## Requirements

- Latest stable Rust (edition 2024)
- GStreamer and its base, good, bad, and libav plugins
- FFmpeg development libraries

On macOS with Homebrew:

```sh
brew install gstreamer ffmpeg
```

Run commands below from this `rust` directory.

## Player

```sh
cargo player
```

The player supports local MP4 playback, scrubbing, frame stepping, volume and
speed controls, looping, fullscreen, resizable playback history, media
metadata, render FPS inspection, PNG frame capture, and the GPUI element
inspector.

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
cargo editor
```

Current editor capabilities:

- Open any ordinary folder as a project; supported media appears in a live file
  tree without an import or copy step. Filter the complete folder tree, preview
  media in place, or drag media directly onto compatible timeline tracks.
- Video tracks accept video and still images; audio tracks accept video or audio.
- Select clips individually, with Command-click, or by drawing a selection
  rectangle. Multi-clip move, duplicate, copy, cut, paste, and delete operations
  preserve relative timing and are recorded as single undo steps.
- Selection, blade, and trim tools support positioning clips, moving them between
  compatible tracks, splitting them, and trimming their source ranges without
  changing source files. Invalid moves show collision or compatibility feedback.
- Multi-track preview includes layered video/images and synchronized overlapping
  audio, with per-track visibility, mute, lock, reorder, creation, and deletion.
- The frame-based timeline supports horizontal scroll and zoom (including macOS
  trackpad pinch), vertical track scrolling, frame ticks at high zoom, frame
  stepping, a draggable playhead, and optional snapping with visible guides for
  the playhead and clip edges.
- First-frame thumbnails and multiresolution waveform peak caches are generated
  in the background. Each clip renders only its selected source range.
- Undo/redo, clip metadata, fullscreen preview, and a docked GPUI element
  inspector with render FPS are available in the editor UI.
- The project model stores default video transform properties (position, scale,
  rotation, and opacity) and audio properties (gain, mute, and stereo pan).
  Property controls and preview/export application are still under development.
- Export composites visible visual tracks, mixes unmuted audio, and writes an
  H.264/AAC MP4 directly through `ffmpeg-next`.

Supported file extensions:

- Video: `.mp4`, `.mov`, `.m4v`, `.mkv`, `.webm`, `.avi`
- Images: `.png`, `.jpg`, `.jpeg` (added as five-second still clips)
- Audio: `.aac`, `.flac`, `.m4a`, `.mp3`, `.ogg`, `.wav`

| Shortcut | Action |
| --- | --- |
| `Space` | Play or pause |
| `Left` / `Right` | Move the playhead backward or forward one project frame |
| `V` / `B` / `T` | Activate the selection, blade, or trim tool |
| `Command-click` | Add or remove a clip from the current selection |
| `Command-B` | Split every compatible selected clip at the playhead |
| `Backspace` / `Delete` | Delete the selected clips |
| `Command-D` | Duplicate the selected clips |
| `Command-C` / `Command-X` / `Command-V` | Copy / cut / paste selected clips |
| `Command-Z` / `Command-Shift-Z` | Undo / redo |
| `F` / `Esc` | Enter / exit fullscreen preview |
| `Option-Command-I` | Toggle the GPUI inspector |
| `Option-Command-R` | Reveal the selected project entry in Finder |
| `Control-Shift-Enter` | Open the selected entry in its default app |

## Project data

Editor state is saved automatically to
`<project folder>/.opencut/project.json`. Media paths are relative to the
project folder, so the media and project file can be moved, backed up, or
committed together. The last opened folder is stored locally in
`data/editor-settings.json`.

Disposable thumbnails and multiresolution `.ocwf` waveform peak caches are stored in
`<project folder>/.opencut/cache`; its generated `.gitignore` keeps the cache
out of version control. Source media is referenced in place and never rewritten.

## Development

```sh
cargo test --features editor
cargo check --all-targets --all-features
cargo loc
```

`cargo loc` reports the Rust source line count for this project.

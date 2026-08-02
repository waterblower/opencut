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
  tree without an import or copy step.
- Video tracks accept video and still images; audio tracks accept video or audio.
- Clips can be positioned, moved between compatible tracks, trimmed, split,
  duplicated, and deleted without changing source files.
- Multi-track preview includes layered video/images and synchronized overlapping
  audio, with per-track visibility, mute, lock, reorder, creation, and deletion.
- The timeline provides horizontal zoom and scrolling, vertical track scrolling,
  a draggable playhead and snapping to the playhead and clip
  edges.
- First-frame thumbnails and audio waveforms are generated in the background.
- Undo/redo, clip metadata, fullscreen preview, and the GPUI element inspector
  are available in the editor UI.
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
| `Command-B` | Split the selected clip at the playhead |
| `Backspace` / `Delete` | Delete the selected clip |
| `Command-D` | Duplicate the selected clip |
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

Disposable thumbnails and waveforms are stored in
`<project folder>/.opencut/cache`; its generated `.gitignore` keeps the cache
out of version control. Source media is referenced in place and never rewritten.

## Development

```sh
cargo test --features editor
cargo check --all-targets --all-features
cargo loc
```

`cargo loc` reports the Rust source line count for this project.

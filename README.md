# OpenCut

OpenCut is an experimental desktop video tool written in Rust with
[GPUI](https://gpui.rs/). The `rust` package contains two applications:

- `opencut-player`: a focused local MP4 player powered by GStreamer.
- `opencut-editor`: a non-destructive, folder-based multi-track editor with
  GStreamer preview and GStreamer Editing Services export.

The project is an active prototype rather than a production-ready editor.

## Requirements

- Latest stable Rust (edition 2024)
- GStreamer with Editing Services and the base, good, bad, ugly, and libav
  plugins
- FFmpeg development libraries

On macOS with Homebrew:

```sh
brew install gstreamer ffmpeg
```

Run the commands below from the Rust package:

```sh
cd rust
```

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
- Multi-track preview includes layered video/images and synchronized overlapping
  audio, with per-track visibility, mute, lock, reorder, creation, and deletion.
- Clicking the timeline preview selects the visible clip at the playhead. A
  selected visual clip can be dragged on the preview canvas and snapped by its
  edges or center to the canvas edges and center lines.
- The frame-based timeline supports horizontal scroll and zoom (including macOS
  trackpad pinch), vertical track scrolling, frame ticks at high zoom, frame
  stepping, a draggable playhead, and optional snapping with visible guides for
  the playhead and clip edges. Playhead position, scroll, zoom, snapping, and
  track-magnet settings are stored per timeline.
- Multiresolution waveform peak caches are generated in the background. Each
  clip renders only its selected source range.
- Undo/redo, clip metadata, fullscreen preview, and a docked GPUI element
  inspector with render FPS are available in the editor UI.
- A selected video or image clip exposes position, scale, opacity, and crop
  controls. These transforms are used by timeline preview and export, and a clip
  context-menu command can copy its transforms to the other visual clips on the
  same track.
- Clips store audio gain and mute values, which preview and export apply. Their
  properties-panel controls are not implemented yet.
- Export maps editor tracks and clips to a GStreamer Editing Services timeline,
  composites visible visual tracks, mixes unmuted audio, and writes an
  H.264/AAC MP4 with configurable resolution, frame rate, bitrate, and hardware
  (when available) or software H.264 encoding.

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

The last opened project folder and active timeline path are stored locally in
`rust/data/editor-settings.json`. This location is temporary while OpenCut is a
prototype.

Disposable multiresolution `.ocwf` waveform peak caches are stored in `<project
folder>/.opencut/cache`; its generated `.gitignore` keeps the cache out of
version control. Source media is referenced in place and never rewritten.

## Development

```sh
cargo test --features editor
cargo check --all-targets --all-features
cargo clippy --all-features
cargo loc
```

`cargo loc` reports the Rust source line count for this project.

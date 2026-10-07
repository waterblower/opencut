# OpenCut

OpenCut is an experimental desktop video tool written in Rust with
[GPUI](https://gpui.rs/). The `rust` workspace contains shared libraries under
`rust/libs` and these applications under `rust/apps`:

- `editor`: a non-destructive, folder-based multi-track editor.
- `player`: a standalone player for a video, audio, or timeline file.
- `cli`: a headless tool for probing, validating, and exporting timelines; see
  [its README](rust/apps/cli/README.md).

The project is an active prototype rather than a production-ready editor.

Devlog: https://www.youtube.com/playlist?list=PLRz1nfZl0jMU

## Requirements

- Latest stable Rust (edition 2024)
- Existing FFmpeg development libraries in `rust/vendor/ffmpeg-8.1.2/`; do not
  build FFmpeg locally
- On macOS: Xcode command line tools and `pkg-config` (`brew install pkg-config`)

## Build and run

Run commands from `rust`. Each command selects a platform explicitly:

```sh
cargo editor-mac                 # Windows: cargo editor-win
cargo player-mac -- <file>       # Windows: cargo player-win -- <file>
cargo build-editor-mac
cargo build-player-mac
cargo test-mac                   # Windows: cargo test-win
```

Extra Cargo flags such as `--release` are forwarded; application arguments
follow `--`. These are native host commands, not cross-compilation commands.
Editor aliases load `.cargo/macos.toml` or `.cargo/windows.toml`; player aliases
use the FFmpeg-only `.cargo/cli.toml` or `.cargo/ffmpeg-windows.toml`. Small
shell runners set the runtime library paths for the vendored FFmpeg.

For other Cargo operations, pass the platform configuration:

```sh
cargo check --config .cargo/macos.toml -p editor   # Windows: .cargo/windows.toml
```

## Editor

- Open any folder as a project. Supported media appears in a live file tree
  without an import step; filter it, preview media in place, or drag media onto
  compatible tracks.
- Timelines are JSON files named `*.timeline` or `*.timeline.json` anywhere in
  the project. **New Timeline** creates a `.timeline` file; click it in the
  Explorer to open it.
- A new timeline has no tracks. Video tracks accept video and still images;
  audio tracks accept audio files.
- Select clips by clicking, Command-clicking, or drawing a selection rectangle.
  Move, duplicate, copy, cut, paste, and delete work on multiple clips, keep
  their relative timing, and are single undo steps.
- The selection tool moves clips within and between compatible tracks; the
  blade tool splits clips at the playhead. Invalid moves show feedback.
- Drag either edge of a video, audio, image, or text clip to change its duration.
  Trimming respects neighboring clips, source limits, and a one-frame minimum;
  each drag is one undo step and saves on release.
- Tracks can be created, deleted, reordered, hidden, muted, and locked.
- Select and drag subtitles directly in the preview. With **Snap on**, text
  aligns to canvas centers and edges with visible guides. Right-click a text
  clip and choose **Apply style to track** to copy its font, size, color, and
  position to other text clips on the same track, preserving their words and timing.
- The frame-based timeline supports horizontal zoom (including trackpad pinch),
  scrolling, frame ticks at high zoom, and optional snapping to the playhead and
  clip edges. Click the ruler to move the playhead. Playhead, scroll, zoom, and
  snapping are saved per timeline.
- The timeline preview plays composited video, including clip position and
  scale, with mixed audio that respects track and clip mute and clip gain. It
  plays a snapshot of the timeline; edits made after it opens are not yet
  reflected, except for frame rate changes and live text properties.
- Waveforms are generated in the background and kept in memory.
- **Generate SRT** on an audio or video file opens a transcription window.
  Press **Start** to transcribe; the window shows stages and offers **Cancel**.
- The top-right **Export** button opens a separate window for exporting a
  snapshot of the active timeline to MP4. It shows progress and provides
  **Stop export**; the window cannot close while exporting.
- To remove pauses based on subtitles, use the CLI's
  [`timeline keep-text-sections`](rust/apps/cli/README.md#keep-text-covered-sections)
  command. It removes time without text clips across all tracks and closes gaps;
  choose a new output file or `--write-inplace`.

Supported file extensions:

- Video: `.mp4`, `.mov`, `.m4v`, `.mkv`, `.webm`, `.avi`
- Images: `.png`, `.jpg`, `.jpeg` (added as five-second still clips)
- Audio: `.aac`, `.flac`, `.m4a`, `.mp3`, `.ogg`, `.wav`

| Shortcut | Action |
| --- | --- |
| `Space` | Play or pause the preview |
| `Left` / `Right` | Move the playhead one frame |
| `V` / `B` | Selection / blade tool |
| `Command-click` | Add or remove a clip from the selection |
| `Command-A` | Select all clips on unlocked tracks |
| `]` | Select clips intersecting the mouse position and all clips to its right on unlocked tracks (pointer over timeline) |
| `Command-B` | Split the selected clips at the playhead |
| `Backspace` / `Delete` | Delete the selected clips |
| `Command-D` | Duplicate the selected clips |
| `Command-C` / `Command-X` / `Command-V` | Copy / cut / paste clips |
| `Command-Z` / `Command-Shift-Z` | Undo / redo |
| `F` / `Esc` | Enter / exit fullscreen preview |
| `Option-Command-I` | Toggle the GPUI inspector |
| `Option-Command-R` | Reveal the selected entry in Finder |
| `Control-Shift-Enter` | Open the selected entry in its default app |

## Player

The player opens the file given after `--`: a video, an audio file, or a
`*.timeline` / `*.timeline.json` file. Press `Space` to play or pause, and click
the seek bar to jump.

## Project data

Each timeline is saved automatically as its own JSON file with its settings,
media metadata, tracks, clips, and view state. Media paths are relative to the
directory containing the timeline file. For example, `timelines/edit.timeline.json`
references `media/clip.mp4` as `../media/clip.mp4`. Move or back up timelines and
media together while preserving their relative locations. Absolute media paths
are also supported.
Source media is never rewritten, and waveforms are not written to the project.

The last opened project is stored in `~/.opencut/editor-settings.json`. On
startup the editor restores the active timeline and its playhead.

## Development

```sh
cargo test-mac # Windows: cargo test-win
cargo check --config .cargo/macos.toml --all-targets --all-features
cargo clippy --config .cargo/macos.toml --all-features
cargo loc
```

`cargo loc` reports the project's Rust line count. Use `.cargo/windows.toml` on
Windows. Plain `cargo test` loads no platform configuration; use `test-mac` or
`test-win` for the editor and its native media dependencies.

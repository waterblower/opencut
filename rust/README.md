# OpenCut for GPUI

OpenCut is a Rust video player and basic non-destructive video editor built with
[GPUI](https://gpui.rs/).

The project contains three applications:

- `opencut-player`: the primary GStreamer player.
- `opencut-player-ffmpeg`: an experimental player that decodes audio and video
  with FFmpeg.
- `opencut-editor`: a basic single-track editor with GStreamer preview and
  FFmpeg export.

## Prerequisites

- Latest stable Rust
- macOS, Linux, or Windows supported by GPUI
- GStreamer with the base, good, bad, and libav plugins for the primary player
  and editor preview
- FFmpeg libraries for the FFmpeg player and editor media inspection
- The `ffmpeg` executable on `PATH` for editor export

On macOS with Homebrew:

```sh
brew install gstreamer ffmpeg
```

## GStreamer player

From this directory, run:

```sh
cargo run
```

The player includes playback controls, frame-by-frame seeking, timeline
scrubbing, volume and speed controls, fullscreen playback, playback history,
media metadata, an FPS inspector, and PNG frame capture.

Player keyboard controls:

- `Space`: play or pause
- `Left` / `Right`: move backward or forward by one frame
- `M`: mute or unmute
- `Command` + `B`: toggle playback history
- `F`: toggle fullscreen
- `Esc`: exit fullscreen
- `Option` + `Command` + `I`: toggle the inspector

## FFmpeg player

Run the experimental FFmpeg playback implementation with:

```sh
cargo ffmpeg
```

It uses FFmpeg for demuxing and decoding both video and audio. Decoded PCM is
sent through Rodio/CPAL to the operating system's audio output. Its interface
and controls match the GStreamer player.

## Video editor

Run the editor with:

```sh
cargo editor
```

The editor currently provides:

- Importing multiple source videos
- One sequential timeline track with attached video and audio
- GStreamer timeline preview and clip-to-clip playback
- A scrollable, zoomable timeline and ruler-based seeking
- Non-destructive left and right trimming
- Splitting at the playhead
- Deleting and reordering clips
- Undo and redo
- Clip metadata in the inspector
- Automatic project persistence
- H.264/AAC MP4 export through FFmpeg

Editor keyboard controls:

- `Space`: play or pause
- `Command` + `B`: split the selected clip at the playhead
- `Backspace` / `Delete`: delete the selected clip
- `Command` + `Z`: undo
- `Command` + `Shift` + `Z`: redo

Edits do not modify or duplicate the source videos. The editor stores media
references, clip order, and source in/out points in `data/editor-project.json`.
Playback history and editor data under `data/` are local runtime files and are
ignored by Git.

During export, clips are trimmed from their original files, normalized to the
first clip's output dimensions and frame rate, concatenated, and encoded as an
H.264/AAC MP4.

## Utilities

Count Rust source lines in this project with:

```sh
cargo loc
```

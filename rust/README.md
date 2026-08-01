# OpenCut for GPUI

OpenCut is a Rust video player and basic non-destructive video editor built with
[GPUI](https://gpui.rs/).

The project contains two applications:

- `opencut-player`: the primary GStreamer player.
- `opencut-editor`: a non-destructive multi-track editor with GStreamer preview
  and FFmpeg export.

## Prerequisites

- Latest stable Rust
- macOS, Linux, or Windows supported by GPUI
- GStreamer with the base, good, bad, and libav plugins for the primary player
  and editor preview
- FFmpeg libraries for editor media inspection, cache generation, and export

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

## Video editor

Run the editor with:

```sh
cargo editor
```

The editor currently provides:

- An IDE-style asset panel backed directly by an ordinary project folder
- Automatic discovery of video, PNG/JPEG image, and common audio files copied
  into the project folder
- Freely positioned text, video/image, and audio tracks with overlapping clips
- Layered image and text preview, synchronized overlapping audio preview, and
  per-track visibility and mute controls
- A scrollable, zoomable timeline with a time ruler, draggable playhead, and
  markers
- Dragging clips in time and between compatible tracks
- Edge and playhead snapping while moving and trimming clips
- Non-destructive left/right trimming, splitting, duplication, and deletion
- Track creation, reordering, locking, visibility, muting, and deletion
- Cached first-frame thumbnails and audio waveforms
- Undo and redo
- Clip metadata in the inspector
- Versioned automatic project persistence with migration from the original
  sequential timeline format
- Layered H.264/AAC MP4 export with composited visual tracks and mixed audio

Editor keyboard controls:

- `Space`: play or pause
- `Command` + `B`: split the selected clip at the playhead
- `Backspace` / `Delete`: delete the selected clip
- `Command` + `D`: duplicate the selected clip
- `Shift` + `M`: add a marker at the playhead
- `Command` + `Z`: undo
- `Command` + `Shift` + `Z`: redo

Edits do not modify or duplicate the source videos. The editor stores media
references, clip order, and source in/out points in
`<project folder>/.opencut/project.json`. Media paths are relative to the
project folder, so the folder—including `.opencut`—can be committed to version
control or backed up as a unit.

Use **Open Folder** to choose a project. The left panel displays that folder as
a live file tree; expand directories and use the `+` beside a media file to add
it at the end of a compatible timeline track. There is no separate import or
media copy step: placing supported media anywhere inside the project folder
makes it appear in the panel. The last opened folder is remembered locally in
`data/editor-settings.json`, which is ignored by Git.

Images appear with thumbnails in the asset tree and open in the preview when
clicked. Adding one creates a five-second still-image clip that supports
preview playback, trimming, splitting, positioning, and mixed video/image
export.

Generated thumbnails and waveforms live in `<project folder>/.opencut/cache`.
The editor writes a nested `.gitignore` there so disposable cache files do not
enter version control while `project.json` remains portable and commit-friendly.
Cache generation and export call the FFmpeg libraries through `ffmpeg-next`;
the editor never launches the `ffmpeg` command-line executable.

Right-click any project-tree entry to reveal it in Finder or open it with the
operating system's default application. The same actions are available with
`Option` + `Command` + `R` and `Control` + `Shift` + `Enter`.

During export, clips are trimmed directly from their original files. Visible
visual tracks are composited in track order, unmuted audio is mixed, and the
result is encoded as an H.264/AAC MP4. Source media is never rewritten.

## Utilities

Count Rust source lines in this project with:

```sh
cargo loc
```

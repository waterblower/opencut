# OpenCut GPUI player

A small Rust replacement for the video playback portion of `../zig`. GPUI owns
the window and controls; `gpui-video-player` uses GStreamer for video decoding,
audio, seeking, and playback state.

## Prerequisites

- Latest stable Rust
- macOS or Linux (the platforms currently supported by GPUI)
- GStreamer with the base, good, bad, and libav plugins

On macOS with Homebrew:

```sh
brew install gstreamer
```

## Run

```sh
cd rust
cargo run --release
```

Use **Open MP4** in the window to choose a video with the operating system's
file picker. A local path or URL can still be supplied at launch:

```sh
cargo run --release -- ../zig/test-videos/test1.mp4
```

Pass `--loop` (or `-l`) to repeat the video:

```sh
cargo run --release -- --loop /path/to/video.mp4
```

The player accepts MP4 files from the OS picker and local paths plus `file:`,
`http:`, and `https:` URLs on the command line. It has play/pause, timeline and
five-second seeking, mute, playback speed, fullscreen, resize-aware aspect-fit
rendering, and audio playback.

Keyboard controls:

- `Space`: play or pause
- `Left` / `Right`: seek five seconds
- `M`: mute or unmute
- `F`: toggle fullscreen
- `Esc`: exit fullscreen

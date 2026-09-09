# video2 — WIP

Experimental FFmpeg-based local-file playback backend with CPAL audio output
and an optional GPUI video element. This is work in progress, not the active
player backend: `cargo player` uses the [GStreamer implementation](../video/).

Implemented functionality includes opening paused with the first frame ready,
play/pause, volume and mute, asynchronous and synchronous seeking, independent
audio/video workers, and retained frame snapshots. Seeking preserves play/pause
state and supersedes older seek requests.

## Current limitations

- **Local video files only.** Input must be a regular file containing a video
  stream. There is no URL/live-stream or audio-only input API. FFmpeg chooses
  the best video and audio streams; callers cannot select tracks or subtitles.
- **Limited playback controls.** There is no playback-rate, looping, reverse
  playback, or export API. Resuming after end-of-file does not restart playback;
  seek back before unpausing. `framerate()` reports an average, not individual
  variable-frame-rate intervals, and duration can be zero when undeclared.
- **Audio requires a usable default device.** Files with audio require a CPAL
  default output device supporting `f32` samples, even when initially paused or
  later muted. There is no device-selection, video-only override, or automatic
  device-reconnection API; audio failures can fail the entire backend.
- **Hardware decoding is narrow.** VideoToolbox is used only on macOS for
  eligible 8-bit YUV420P H.264/HEVC streams in MP4/MOV with suitable codec
  configuration. Other inputs and recognized hardware-unavailability cases
  use software decoding. Not every hardware error triggers fallback, and
  there is no midstream hardware-to-software recovery. Other platforms use
  FFmpeg software decoding.
- **Some operations block.** `open_sync()` and `seek_sync()` wait on their
  calling thread, and dropping a backend joins its workers. Avoid these on the
  UI thread when responsiveness matters. The API has no operation timeout;
  dropping a seek future does not cancel a submitted seek. `seek_sync()` also
  still writes timing diagnostics to stderr.
- **Synchronization needs broader validation.** Presentation follows a shared
  `Instant` clock rather than an audio-device master clock. Audio alignment
  skips stale samples or leaves silence for gaps. Long-running drift, device
  changes, and behavior under sustained load need more validation; the current
  tests do not establish production-quality A/V synchronization.
- **Rendering has conversion costs.** The macOS path prepares NV12/CoreVideo
  surfaces, with a BGRA image fallback; other platforms use BGRA images. Do not
  assume zero-copy rendering. A stable GPUI element ID is needed to reuse the
  converted-frame cache.

## Development and validation

Enable `ffmpeg-backend` for the backend, `ffmpeg-video` for the GPUI element,
or `ffmpeg-video-tests` for GPUI cache/lifetime tests. From the Rust package:

```sh
cargo test --no-default-features --features ffmpeg-backend --lib
cargo test --no-default-features --features ffmpeg-video-tests --lib
```

Tests cover frame snapshots, pause/end behavior, seeking and supersession,
variable frame rates, audio buffering, and rendering/cache behavior. Generated
media fixtures currently require the repository's
[vendored FFmpeg executable](../../vendor/ffmpeg-8.1.2/bin/ffmpeg).

The real audio-device test is ignored by default and needs a working output
device. The synchronous seek benchmark is also ignored and requires
`VIDEO2_BENCH_PATH` pointing to a local video. Passing the default suite does
not validate audible output, representative seek latency, or cross-platform
hardware/device behavior. These limitations are based on the implementation
and test coverage, not a new performance or listening evaluation.

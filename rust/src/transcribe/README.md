# Audio transcription

The `opencut_player::transcribe` module provides async MiniMax speech recognition
for local audio and video. Enable the parent package's `transcribe` feature to
use it without CLI or GUI features. The CLI already enables it; future editor
integration can enable the same feature.

Call `transcribe(path, api_key, &Options).await` from a Tokio runtime. Credentials
are supplied by application initialization. The result is a JSON object for JSON
formats or a JSON string for SRT and VTT. FFmpeg decoding runs on the blocking
pool; HTTP uses async reqwest. File publication remains the caller's job.

`subtitles::merge_srt_sections(&srt)` optionally merges cues with gaps under
100 ms. `audio::transcription_wav(path)` exposes synchronous normalization, and
`audio::AudioReader` is shared with timeline audio mixing.

Errors carry a code, message, file, and line. Applications decide how to display
them or map them to exit codes. Clap derives are enabled only with the CLI feature.

Link against the existing vendored FFmpeg libraries; do not build FFmpeg.
From the repository root, run module tests with:

```sh
bash rust/scripts/cargo-cli.sh test --locked --no-default-features --features transcribe --lib
```

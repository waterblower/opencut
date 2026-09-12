# Audio transcription

The `opencut_player::transcribe` module provides async MiniMax speech recognition
for local audio and video. Enable the parent package's `transcribe` feature to
use it without CLI or GUI features. Both the CLI and editor enable it.

In the editor, open **Settings** in the top bar to save a MiniMax API key in the
global settings JSON. Right-click an audio or video file in the project
explorer and choose **Generate SRT**. Output is saved in the project root as
`<source-stem>.srt`, replacing any existing file at that path.
This sends the extracted audio to MiniMax; it does not insert subtitle clips.

Audio must be at most 500 seconds. Files without audio are rejected.
Timeline transcription is not currently supported.

Call `transcribe(path, api_key, &Options).await` from a Tokio runtime. Credentials
are supplied by application initialization. The result is a parsed `SRT`; this
function always requests SRT. Use `transcribe_response` for raw JSON/VTT formats. FFmpeg decoding runs synchronously on
the caller's background thread; HTTP uses async reqwest. File publication remains
the caller's job.

`subtitles::merge_srt_sections(&srt)` optionally merges cues with gaps under
100 ms. `audio::extract_audio_as_wav(path)` exposes full-file synchronous normalization, and
`audio::AudioReader` is shared with timeline audio mixing.

Errors use anyhow with a reason and source location. Applications decide how to display
them or map them to exit codes. Clap derives are enabled only with the CLI feature.

Link against the existing vendored FFmpeg libraries; do not build FFmpeg.
From the repository root, run module tests with:

```sh
bash rust/scripts/cargo-cli.sh test --locked --no-default-features --features transcribe --lib
```

The editor also supports Generate SRT for saved timeline files. It resolves media
against the event's project root and renders only audio through GES into an
in-memory mono 16 kHz PCM WAV. Track/clip mute, gain, trims, overlaps, and timeline
silence are preserved. Empty timelines, timelines without enabled audio, and
timelines longer than 500 seconds are rejected before rendering.

Call `transcribe_wav(wav, api_key, &Options).await` to transcribe normalized WAV
bytes directly. It validates the WAV format and duration before uploading and
returns parsed SRT. The editor merges cues and writes only the final SRT to the
project root; it creates no intermediate audio files.

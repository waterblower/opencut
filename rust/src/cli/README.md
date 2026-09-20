# OpenCut CLI

A headless application that reads the same timeline JSON as the OpenCut editor.
The CLI probes media with FFmpeg; the editor updates the same timeline model.
The shared [timeline module](../timeline/mod.rs) defines the format and serialization
rules and file I/O, without backend state. CLI-specific services live beneath this
application module; the shared FFmpeg engine lives in `src/engine`.

## Build and run

From the Rust directory, `cargo cli` runs with the vendored FFmpeg environment:

```sh
cargo cli --help
cargo cli probe /path/to/media.mp4 --json
cargo cli new episode.timeline.json --fps 30000/1001 --json
cargo cli schema --json
cargo cli validate scenes/episode.timeline.json --json
cargo cli probe scenes/episode.timeline.json --json
```

Timeline and output arguments resolve from the working directory. Relative media
paths inside a timeline resolve from the directory containing that timeline file,
including `..` references. Absolute asset paths are unchanged.

From the repository root:

```sh
bash rust/scripts/cargo-cli.sh build --no-default-features --features cli --bin opencut
```

The wrapper links the existing `rust/vendor/ffmpeg-8.1.2` libraries. It never builds
FFmpeg or invokes Python. Disable default features to build only the CLI; its demo renderer uses GPUI.

## Probe media and timelines

`probe <file>` accepts video, audio, image, and timeline files. Media paths must
be absolute; relative media paths are rejected. Files with a `.json`
extension (case-insensitive, including `.timeline.json`) are loaded as timelines
and summarized with duration, settings, clips, tracks, gaps, and asset usage.
Other files report media streams, codecs, duration, dimensions, and keyframe spacing
where applicable. Timeline probing does not open referenced media; use `validate`
to check those files. The separate `inspect` command has been removed.

## Shared timeline contract

The authoritative schema is generated from the actual shared Rust types by
`opencut schema`. See the [complete example](../../tests/fixtures/shared.timeline.json).

- Root fields are `editing_state` and `view_state`. Editing state contains
  `settings`, `assets`, `tracks`, and `clips`. Existing flat documents remain readable.
  There is no CLI-specific `version` field. Existing GUI defaults and deserialization aliases
  remain supported; canonical serialization uses the GUI field names and tags.
- Settings include width, height, rational `frame_rate`, and `audio_sample_rate`.
- Assets include ULID, path, media kind, display name, duration, dimensions,
  frame-rate metadata, codec, and `has_audio`.
- Tracks use `Video`, `Audio`, or `Text`, with `locked`, `muted`, and `visible`.
- Clips use `{"kind":"Video"|"Audio"|"Text","data":{...}}`. Media data includes
  IDs, `timeline_start`, `source_in`, `source_out`, and video/audio properties.
  Images are video clips referencing an asset whose kind is `Image`.
- Media source bounds and timeline positions are integer frames at the timeline
  rate. Text length uses `{ "secs": ..., "nanos": ... }`. Ranges are end-exclusive.
- Video position is a **pixel offset from the centered placement** in project
  dimensions. Scale 1 fits the source inside the frame. Text positions specify a
  normalized center, clamped to the canvas; text color is big-endian ARGB.
- Earlier video tracks appear above later video tracks. Text tracks appear above
  video tracks, also respecting earlier-track priority.
- Visibility controls visual output; hiding a video track does not mute its audio.
  Track and clip muting control audio. Locking affects editing, not export.
- View state survives serialization and is preserved by CLI document operations.

`new` creates a document with visible video and audio tracks. All input timelines
are read without rewriting them. Subtitle import is not yet available in the CLI; existing GUI import/edit
operations remain available.

**Legacy CLI timelines and the `edit` command have been removed.** Documents using
the old `type` clip tags, root `version`, or `transitions` are rejected with
`legacy_cli_format`. CLI-only effects, opacity, background color, and transitions
are not part of this shared format. There is no legacy conversion layer.

## Transcription

`transcribe` uploads audio to the [MiniMax speech-to-text API](https://platform.minimax.cn/docs/api-reference/speech-to-text).
Set `MINIMAX_API_KEY` in the environment before running it:

```sh
cargo cli transcribe recording.mp4 --json
cargo cli transcribe recording.wav --format srt -o subtitles.srt
cargo cli transcribe recording.mp4 --format verbose_json --language zh
```

Local audio and video inputs are decoded with the vendored FFmpeg libraries. The
best audio stream is normalized to mono 16 kHz PCM WAV in memory before upload.
Source-relative timing is preserved, including silence before speech and timestamp
gaps. The audio must have a known positive duration and fit within 500 seconds;
overlong audio is rejected, never truncated or split. The normalized WAV is at
most about 16 MB, below MiniMax's 50 MB upload limit. Video-only files are rejected.
Input and output paths resolve from the working directory.

`--format` accepts `json`, `verbose_json` (default), `srt`, or `vtt`.
With `--format srt`, add `--post-merge` to merge consecutive cues whose gap is
less than 100 ms before printing or saving. This groups character/word cues. Text is concatenated and cues are renumbered.
Verbose JSON includes speaker labels, timestamps, and the provider trace ID.
All requests use word-level timestamps (ignored by MiniMax for plain `json`). Optional `--language` supplies a BCP-47 hint such as `zh`, `yue`,
or `en`; omitting it enables mixed-language recognition. Requests use `asr-1.0`
with `stream=false`; streaming and automatic subtitle insertion are not included.
SRT output can be imported using the editor's existing SRT support.

Without `-o`, the result goes to stdout. Global `--json` prints a JSON object for
JSON formats and a JSON string for subtitle formats. With `-o`, the selected
format is written atomically and stdout reports its path and format. Existing
files require `--overwrite`, and the input cannot be used as the output.

HTTP uses async reqwest on Tokio, with a 30-second connection timeout and a
10-minute request timeout. There are no automatic retries. FFmpeg decoding and
the existing atomic file writer run on Tokio's blocking pool. Errors use exit 1
for HTTP/service/response failures, 4 for unusable media, 2 for missing credentials
or invalid arguments, and 6 for output I/O. Provider HTTP errors include the
status and request ID when available; keys are not included in diagnostics.

The shared [transcription module](../transcribe/mod.rs) provides
`opencut_player::transcribe::transcribe(path, api_key, &options).await` without CLI or GUI
dependencies. The CLI retains a re-export for existing callers.

```rust,no_run
use opencut_player::transcribe::{self as transcribe, Format, Options};
use std::path::Path;

use anyhow::Result;

async fn example(api_key: &str) -> Result<()> {
    let result = transcribe::transcribe(Path::new("recording.mp4"), api_key, &Options {
        format: Format::Srt,
        language: Some("zh".into()),
    }).await?;
    // The result is a parsed SRT; Display serializes it to subtitle text.
    println!("{result}");
    Ok(())
}
```

The library accepts credentials explicitly and requires a Tokio runtime. It does
not read environment variables. Automated tests use local mock HTTP responses;
they require no API key and do not make paid MiniMax requests.

## Output and errors

The `still` and `render` commands, CPU compositor, and text rasterizer have been
removed ahead of the shared GPUI renderer refactor. CLI video export and still
preview generation are currently unavailable. FFmpeg decoding, encoding, and
audio mixing services remain available for reuse.

All commands accept `--json`. Results go to stdout and diagnostics go to stderr.
Runtime failures use anyhow and exit code 1; Clap usage errors use exit code 2.
JSON failures have the shape `{"error":{"message":"..."}}`. Diagnostic labels,
paths, and source locations are included in the message. `validate` uses the shared timeline validator and stops at the first document
or media probe error. Media probe errors include the asset path.

## Verification and packaging

```sh
# Shared format alone, without either media backend:
cargo test --manifest-path rust/Cargo.toml --no-default-features --features timeline-schema --lib --test timeline
# CLI and FFmpeg integration:
bash rust/scripts/cargo-cli.sh test --no-default-features --features cli --lib --test cli --test timeline --test transcribe
# Optional VideoToolbox encoder tests:
bash rust/scripts/cargo-cli.sh test --no-default-features --features cli --test cli -- --ignored
# Local macOS package:
bash rust/scripts/package-cli.sh
```

The package is a local unsigned artifact linked against the existing vendored
FFmpeg installation and its transitive libraries, not a relocatable distribution.
`OPENCUT_GPL=1` selects libx264; it does not build codecs. FFmpeg licensing still
depends on the vendored build. Signing and publishing remain separate operations.

Generate an agent-friendly Markdown usage guide with `opencut doc`.
It includes workflows, examples, and command options generated from the CLI
definitions. The full timeline schema is available separately via `opencut schema`.
The `docs` alias is also supported;
`--json` returns the guide as a JSON string.

```sh
opencut doc > llms.txt
```

Release packaging generates this file from the built executable.

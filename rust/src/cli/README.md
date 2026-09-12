# OpenCut CLI

A headless application that reads the same timeline JSON as the OpenCut editor.
The CLI renders with FFmpeg; the editor edits and renders with GStreamer.
The shared [timeline module](../timeline/mod.rs) defines the format and serialization
rules, without file I/O or backend state. CLI-specific services and the FFmpeg
engine live beneath this application module.

## Build and run

From the Rust directory, `cargo cli` runs with the vendored FFmpeg environment:

```sh
cargo cli --help
cargo cli probe media.mp4 --json
cargo cli new episode.timeline.json --fps 30000/1001 --json
cargo cli schema --json
cargo cli --project-root /path/to/project validate scenes/episode.timeline.json --json
cargo cli --project-root /path/to/project inspect scenes/episode.timeline.json --json
cargo cli --project-root /path/to/project still scenes/episode.timeline.json --at 50% -o preview.png
cargo cli --project-root /path/to/project render scenes/episode.timeline.json -o output.mp4 --dry-run --json
cargo cli --project-root /path/to/project render scenes/episode.timeline.json -o output.mp4 --progress json --json
```

Timeline and output arguments resolve from the working directory. `--project-root`
controls the base of relative **asset paths**, matching the editor. It defaults to
the working directory, not the timeline's parent directory. Absolute asset paths
are unchanged. A timeline saved in a nested project folder uses the same project
root as a timeline at the top level.

From the repository root:

```sh
bash rust/scripts/cargo-cli.sh build --no-default-features --features cli --bin opencut
```

The wrapper links the existing `rust/vendor/ffmpeg-8.1.2` libraries. It never builds
FFmpeg or invokes Python. Disable default features to build the CLI without GPUI
or GStreamer dependencies.

## Shared timeline contract

The authoritative schema is generated from the actual shared Rust types by
`opencut schema`. See the [complete example](../../tests/fixtures/shared.timeline.json).

- Root fields are `settings`, `assets`, `tracks`, `clips`, and `view`. There is no
  CLI-specific `version` field. Existing GUI defaults and deserialization aliases
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
- View state survives serialization and is ignored by the CLI renderer.

`new` creates a document with visible video and audio tracks. All input timelines
are read without rewriting them. Use `assemble` to build a podcast from supplied
decisions. Subtitle import is not yet available in the CLI; existing GUI import/edit
operations remain available.

**Legacy CLI timelines and the `edit` command have been removed.** Documents using
the old `type` clip tags, root `version`, or `transitions` are rejected with
`legacy_cli_format`. CLI-only effects, opacity, background color, and transitions
are not part of this shared format. There is no legacy conversion layer.

## Podcast assembly

An external agent supplies editing decisions. `assemble` does not call models or
transcription services; it compiles those decisions into the same editable document
used by the GUI. See the [example recipe](../../tests/fixtures/podcast.recipe.json)
and generate its authoritative schema with `schema --kind recipe`.

```sh
cargo cli schema --kind recipe --json
cargo cli --project-root /project assemble recipe.json --dry-run --json
cargo cli --project-root /project assemble recipe.json -o episode.timeline.json --json
cargo cli --project-root /project render episode.timeline.json --range 10s..15s -o review.mp4
```

The recipe supplies output `settings`, a `time_base` in seconds per tick, named
`sources`, `master_audio`, optional `gain_db` (default 0), `retained` intervals,
and `cameras`. All start/end/offset values are signed integer ticks in that common
time base. For example, `{ "numerator": 1, "denominator": 1000 }` means milliseconds.
Source paths resolve against `--project-root`, independently of the recipe location.
The example references recordings you must supply; it is not a bundled media set.

The master source must contain audio and have zero `source_offset`. It defines the
uncut episode clock. Other sources obey `source_time = episode_time + source_offset`:
a +2000 ms offset maps episode time 10 seconds to camera time 12 seconds. Offsets
and boundaries round independently once to the nearest project frame (ties away
from zero); the report lists every adjustment. Source offsets therefore preserve
clip lengths on the same frame grid. Sample-accurate cuts and drift correction
are not supported by this assembly interface.

Retained intervals and camera selections must each be chronological, nonoverlapping,
nonnegative, and nonempty. Ranges are end-exclusive. Camera choices refer to uncut
episode time and must cover every retained frame. Removed intervals disappear from
both video and audio. Camera switches never split the master audio; camera clips
are explicitly muted. Missing source coverage is an error. Files with more than
one audio or video stream are rejected because the shared format cannot yet select
a stream. Coverage checks use the selected stream's duration when available,
falling back to the recording duration when the container does not report stream
duration. Unknown recipe fields (including captions) are rejected.

`--dry-run` probes and validates without writing a timeline, and does not require
`-o`. Its report includes total frames, the project frame rate, episode-to-output
intervals, per-clip source/output bounds, and rounding adjustments. Every reported
time is in project frames except the explicitly named original `ticks` fields.
Normal assembly returns the same report and writes atomically. Existing output
requires `--overwrite`; neither source media nor the input recipe can be replaced.
Reassembly does not merge manual GUI edits: use a new output file for revisions.
Captions, extraction commands, audio analysis, and cut fades are later milestones.

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
Input and output paths resolve from the working directory, not `--project-root`.

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
the existing atomic file writer run on Tokio's blocking pool. Errors use exit 5
for HTTP/service/response failures, 4 for unusable media, 2 for missing credentials
or invalid arguments, and 6 for output I/O. Provider HTTP errors include the
status and request ID when available; keys are not included in diagnostics.

The shared [transcription module](../transcribe/mod.rs) provides
`opencut_player::transcribe::transcribe(path, api_key, &options).await` without CLI or GUI
dependencies. The CLI retains a re-export for existing callers.

```rust,no_run
use opencut_player::transcribe::{self as transcribe, Format, Options};
use std::path::Path;

async fn example(api_key: &str) -> anyhow::Result<()> {
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

## Rendering and output

H.264/HEVC use macOS VideoToolbox; `gpl` selects libx264 for H.264. ProRes uses
FFmpeg's native encoder and requires MOV. H.264/HEVC accept MP4 or MOV. AAC stereo
is included, including silence for timelines without audio. HEVC uses `hvc1` with
out-of-band parameter sets for Apple playback compatibility.

Without `--bitrate`, video bitrate is the duration-weighted source bitrate of
visible video clips contributing to the selected range. Unused assets and audio
tracks are excluded. Preset-derived bitrate is the fallback. `--bitrate` accepts
bits/s and decimal `k`/`M` suffixes. ProRes is profile-controlled.

The FFmpeg compositor uses deterministic 8-bit RGBA, an opaque black background,
and BT.709 output. Text uses embedded IBM Plex Sans with fallback for unavailable
font names. Backend font rasterization and encoded bytes may differ; clip timing,
source selection, layering, transforms, and audio inclusion use the shared
contract. HDR, wide-gamut management, and animated effects are outside this format.
Audio gain is clamped to the editor's -96..24 dB range, and summed samples are
hard-clipped to [-1,1].

`new` and render outputs refuse existing destinations unless supported overwrite
flags are supplied. Source media cannot be output targets. Render outputs are
staged next to the destination and committed only on success. The input timeline
is embedded under `opencut.timeline` unless `--no-metadata` is supplied.

CLI times accept seconds, `s`, frames with `f`, and `HH:MM:SS`. They round to the
nearest timeline frame using rational arithmetic. Only `still --at` accepts
percentages; `100%` selects the final frame.

All commands accept `--json`. Results go to stdout; progress goes to stderr at
most once every five seconds, plus final completion. Errors include codes, JSON
pointers, and Rust file/line locations. `validate` returns all findings.

## Verification and packaging

```sh
# Shared format alone, without either media backend:
cargo test --manifest-path rust/Cargo.toml --no-default-features --features timeline-schema --lib --test timeline
# CLI and FFmpeg integration:
bash rust/scripts/cargo-cli.sh test --no-default-features --features cli --lib --test cli --test assemble --test timeline --test transcribe
# Optional VideoToolbox encoder tests:
bash rust/scripts/cargo-cli.sh test --no-default-features --features cli --test cli -- --ignored
# Local macOS package:
bash rust/scripts/package-cli.sh
```

From the Rust directory, the cross-backend GUI round-trip test is:

```sh
cargo test-mac --no-default-features --features editor,cli --bin opencut-editor shared_timeline
```

This generates synthetic media, saves and edits a GUI timeline, renders it through
both backends, and checks cut boundaries, transforms, caption placement/timing,
and audio gain/muting. Its GStreamer export uses software AAC for headless testing.

The package is a local unsigned artifact linked against the existing vendored
FFmpeg installation and its transitive libraries, not a relocatable distribution.
`OPENCUT_GPL=1` selects libx264; it does not build codecs. FFmpeg licensing still
depends on the vendored build. Signing and publishing remain separate operations.

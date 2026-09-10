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
are read without rewriting them. Document assembly and subtitle-import interfaces
are deferred; existing GUI import/edit operations remain available.

**Legacy CLI timelines and the `edit` command have been removed.** Documents using
the old `type` clip tags, root `version`, or `transitions` are rejected with
`legacy_cli_format`. CLI-only effects, opacity, background color, and transitions
are not part of this shared format. There is no legacy conversion layer.

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
bash rust/scripts/cargo-cli.sh test --no-default-features --features cli --test cli --test timeline
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

# OpenCut CLI

A standalone, headless video editor driven by a versioned JSON timeline. This
application has its own format and does not migrate or modify GUI editor files.

## Build and run

From the Rust directory, `cargo cli` builds and runs the CLI with the vendored
FFmpeg environment. Arguments are forwarded directly:

```sh
cd rust
cargo cli --help
cargo cli probe media.mp4 --json
```

From the repository root, using the existing development FFmpeg installation:

```sh
bash rust/scripts/cargo-cli.sh build --no-default-features --features cli --bin opencut
rust/target/debug/opencut --help
rust/target/debug/opencut docs
rust/target/debug/opencut schema
```

The wrapper links the existing `rust/vendor/ffmpeg-8.1.2` installation and sets
only libav discovery and library paths, with no GStreamer environment setup.
Always disable default Cargo features when building the CLI alone.

The [agent guide](llms.txt) describes the command and document contracts. All
commands accept `--json`; render progress is written only to stderr.
Progress is throttled to one update per five seconds, plus a final update
after the output is saved. `--progress none` suppresses progress entirely. Errors
include stable codes, JSON pointers, and Rust source locations. `validate`
returns a findings array. Read-modify-write edits preserve unknown fields and
commit only after validation, using a temporary file in the target directory.

CLI time inputs round to the nearest timeline frame using integer rational
arithmetic. Render ranges are end-exclusive. `still --at 100%` means the final
frame. Text durations accept integer frames or `{secs,nanos}`. `add-clip --asset`
resolves its argument from the working directory and stores a document-relative
path when the asset is beneath the document directory, otherwise an absolute path.

## Rendering contracts

- H.264/HEVC use macOS VideoToolbox; `gpl` selects libx264 for H.264. ProRes uses
  the native FFmpeg encoder and requires MOV. H.264/HEVC accept MP4 or MOV. AAC
  stereo is always included, including silence for timelines without audio.
  HEVC uses the `hvc1` sample entry and out-of-band parameter sets for Apple
  playback compatibility. `probe` reports `codec_tag` as well as the codec name.
- When `--bitrate` is omitted, video encoding targets the input video's bitrate.
  With multiple input videos, it uses their duration-weighted average within the
  render range; audio-only tracks and unused assets are excluded. `--bitrate`
  (alias `--video-bitrate`) overrides this in bits/s, with decimal `k` and `M`
  suffixes (for example `1500000`, `1500k`, or `1.5M`). Preset-derived bitrate is
  used only when no contributing source reports a video bitrate (including
  image/text-only timelines). Dry-run reports `video_bitrate` and `bitrate_source`.
  These are average bitrate targets, not exact file-size limits. ProRes remains
  profile-controlled and does not honor a target bitrate like H.264/HEVC.
- Draft/standard/high select encoder settings and fallback bitrate targets. Scaled video
  dimensions must be even; unsupported encoders produce a capability error.
- Compositing is deterministic 8-bit RGBA in track order. Coordinates are
  normalized center anchors; scale is relative to fitting inside the frame.
  RGB is treated as sRGB, with BT.709 matrix/range and color tags on video output.
  HDR and wide-gamut color management are outside v1.
- Text uses the embedded IBM Plex Sans font (SIL OFL). Sans, sans-serif, and Inter
  resolve to that fallback; unavailable named fonts also fall back. No system
  font discovery is used, keeping output repeatable across machines.
- Crop uses normalized source coordinates and then refits the cropped image.
  Blur radius is in source pixels. Alpha is preserved through composition.
- Transitions center on adjacent cuts; odd lengths allocate the extra frame
  after the cut. Media clips require source handles, while still/text content
  extends across the window. Transition windows may not overlap. Audio uses
  equal-power sine/cosine curves for every visual transition type.
- Audio is resampled to the configured rate and mixed with per-clip gain/muting
  and track muting. Sum overflow is hard-clipped to [-1,1]; no limiter is applied.
  AAC is lossy and may contain codec padding; presentation timestamps determine
  the intended duration. Frame selection uses nearest PTS, with earlier frames
  winning ties. Stills use exactly the same compositor as video encoding.
- Render outputs are staged next to the destination and committed after success.
  Existing outputs require `--overwrite`; source media cannot be output targets.
  Provenance is stored under the `opencut.timeline` metadata key by default.

## Test and package

```sh
bash rust/scripts/cargo-cli.sh test --no-default-features --features cli --test cli
```

Tests generate tiny source media, images, and SVGs in isolated temporary
directories. They exercise exact time parsing, edits, validation, decoding,
compositing, transitions, audio, native encoding, metadata, and command outputs.
The separately marked VideoToolbox test requires an unrestricted macOS session:
run the same test command with `-- --ignored` to verify H.264/HEVC and fractional
frame rates. A sandbox can deny VideoToolbox sessions even when encoders exist.

For a local macOS release using the same vendored libraries:

```sh
bash rust/scripts/package-cli.sh
```

FFmpeg is never built by the application build, and Python is not used. Both
development and release binaries link the vendored FFmpeg dynamically and need
that installation and its transitive libraries at runtime. `OPENCUT_GPL=1`
selects the Cargo `gpl` feature for libx264; it does not build any codecs.
The vendored FFmpeg may contain GPL components regardless of that feature.

The package includes the binary, usage documentation, FFmpeg license texts, and
the font's SIL OFL notice. It is a local unsigned artifact for this installation,
not a relocatable standalone distribution. The script checks the 100 MB binary
size ceiling (target: 40 MB). CI requires a provisioned macOS runner with the
vendored libraries and their dependencies. Signing and publishing are separate
release operations.

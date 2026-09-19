# OpenCut rendering backend refactor

## Goal and constraints

Build a Rust-owned editing engine that uses **the same GPUI composition for preview and export**, with FFmpeg handling media decoding, encoding, and muxing.

Deliver the migration on **macOS first, then Windows, then Linux**. Each platform must support installation and use without a separate multimedia runtime setup.

Preserve the current timeline format, editing operations, undo/redo, audio controls, and CLI command contracts. Visual output will follow the shared GPUI renderer.

Use the existing vendored FFmpeg 8.1.2 libraries. Do not build FFmpeg, invoke Python in the build, or introduce custom macros. Keep native operations behind small Rust interfaces with explicit ownership and error propagation.

## Shared architecture

```text
Timeline + exact timestamp + project dimensions
                      │
               Timeline evaluation
                      │
           Decoded frames + text + images
                      │
              GPUI composition
                      │
             Project-size GPU texture
                 ┌────┴────┐
                 │         │
              Preview    Export
                 │         │
          Scale to panel   Read pixels → FFmpeg encoder
          Add edit handles                  │
                                      Mux with audio

Timeline audio → Shared mixer → Playback device or export encoder
```

- **Timeline evaluation:** pure logic selects active clips, source timestamps, layer order, transforms, and audio ranges. Preserve rational frame rates and end-exclusive clip boundaries.
- **Media workers:** own FFmpeg contexts and decoded buffers. Decode sequentially during export; seek only when the requested source position requires it. Reuse suitable code from the existing CLI engine and FFmpeg playback backend.
- **Composition:** one GPUI element tree renders video, images, and text in project-resolution coordinates. Text layout and hit-test bounds come from that same composition. Editor handles and selection outlines are separate.
- **Preview:** display the rendered project texture scaled to the panel. Moving text updates composition properties and redraws with the currently decoded video frames; it does not seek the media.
- **Export:** evaluate every output frame at its exact timestamp and wait for its required assets before rendering. Export speed is independent of wall-clock playback.
- **Audio:** share sample selection, gain, mute, resampling, and mixing between preview and export. Preview uses the audio playback clock when available and a monotonic clock otherwise.

Introduce shared interfaces for evaluating a frame, requesting decoded media, rendering a prepared composition, mixing an audio interval, and submitting encoded output. Keep these independent of CLI argument parsing and editor state.

Use bounded worker queues and generation identifiers for superseded preview requests. Export applies backpressure and drops no frames. Blocking decode, encode, and device operations stay off the UI thread; GPUI remains on its required platform thread.

## Current state and next steps

- The editor updates the timeline model without a media pipeline. Timeline preview
  is a static black surface; timeline playback, export, and transcription are
  temporarily unavailable. Ruler and track editing remain functional.
- File previews and metadata probing use FFmpeg; waveform generation uses FFmpeg;
  audio playback uses CPAL. Build/run scripts use the vendored FFmpeg libraries.
- The CLI demo renders GPUI text to a five-second video using the existing
  `test-support` headless APIs. It does not modify the vendored Zed source.

### 1. Implement the timeline renderer

Evaluate the timeline at an explicit time and produce a GPUI element from prepared
media. Support video, images, text, transforms, clipping, and track visibility.
Decode on workers, keeping I/O out of GPUI render callbacks. Share composition
geometry with future preview interaction overlays.

### 2. Restore timeline playback

Use one timeline clock and a shared audio mixer. Support seeking, source trims,
overlaps, silence, gain/mute, and end-of-timeline behavior. Avoid independently
clocked players for individual clips. Preserve all existing editing operations.

### 3. Implement export using the same composition

Render each frame at its exact timestamp and encode through FFmpeg. Restore the
editor export UI and timeline transcription only when their rendering services
are available. Preview and export must use identical project-space geometry.

### 4. Package and port

Bundle the existing FFmpeg libraries with application-relative loading paths.
Deliver macOS first, then Windows and Linux with the same composition contract.
Do not build FFmpeg locally or modify vendored Zed to add headless APIs.

## Validation and acceptance

- **Composition parity:** compare preview’s project-size texture with offscreen export pixels before encoding, using identical assets, fonts, frame numbers, and renderer settings. Expect byte equality for deterministic fixtures on the same platform.
- **Layout stability:** resizing the editor or changing display scale must not change composition geometry, font size, wrapping, or layer placement.
- **Media correctness:** test fractional frame rates, variable-rate sources, nonzero source in-points, cuts, gaps, overlaps, images, multiline text, transparency, and hidden tracks.
- **Editing correctness:** text typing and dragging update the next rendered composition without media seeks; scrubbing rejects stale results; undo/redo restores both model and output.
- **Audio correctness:** test mute, gain, overlaps, resampling, export ranges, seek resets, and long-duration A/V synchronization. Preview without an audio device remains usable with silent playback.
- **Export correctness:** verify frame counts, timestamps, duration, codec/container options, cancellation, and failure cleanup. Only publish the final output after successful encoder draining and mux finalization.
- **Performance instrumentation:** measure evaluation, decode, layout/paint, GPU submission/completion, readback, encoding, audio mixing, and queue waits separately. Report preview latency and export throughput; do not claim a Remotion speedup without an equivalent benchmark.
- **Distribution:** launch and export from a clean machine without developer environment variables or a separately installed multimedia runtime.

## Defaults and boundaries

- Initial output is SDR, using a shared color-conversion path and explicit output metadata. HDR support and direct hardware encoder surface transfer are follow-up work.
- Preview/export identity means identical composition pixels before encoding on the same platform and font environment. Lossy encoding, chroma subsampling, and cross-platform font rasterization can produce differences.
- Existing project files retain their schema and coordinate semantics. No new effects, transition system, or animation language is introduced by this refactor.
- Complete each stage with working tests before expanding scope; shared-renderer correctness and zero-setup distribution are required outcomes, while performance gains must be measured.

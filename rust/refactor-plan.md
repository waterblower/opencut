# OpenCut rendering backend refactor

## Goal and constraints

Replace GStreamer with a Rust-owned editing engine that uses **the same GPUI composition for preview and export**, with FFmpeg handling media decoding, encoding, and muxing.

Deliver the migration on **macOS first, then Windows, then Linux**. Each platform must support installation and use without a separate multimedia runtime setup.

Preserve the current timeline format, editing operations, undo/redo, audio controls, and CLI command contracts. Visual output will follow the shared GPUI renderer; reproducing GStreamer’s exact text rasterization is not a requirement.

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

## Implementation sequence

### 1. Establish a macOS offscreen rendering path

- Extend vendored GPUI with a production `offscreen-rendering` capability based on its existing Metal renderer.
- Provide an offscreen host that performs real GPUI layout, text shaping, painting, and GPU rendering without a visible window.
- Use production scheduling and asset loading. Do not ship the test dispatcher or fake platform as the export runtime.
- Expose rendering to a reusable project-size texture and explicit RGBA readback. Reuse textures and buffers between frames.
- Start with CPU pixel readback into the existing FFmpeg encoder. Hardware texture-to-encoder integration is a later optimization, not a migration prerequisite.

### 2. Prove the shared renderer through OpenCut’s CLI

Use OpenCut’s [CLI entry point](/Users/mac/Documents/GitHub/OpenCut/rust/src/cli/main.rs:1), rather than modifying Zed’s vendored CLI.

- Add a temporary opt-in GPUI rendering backend to existing `still` and `render` commands.
- First render a short composition containing a decoded video and GPUI text, then cover images, multiple tracks, transforms, clipping, and audio.
- Preserve existing output options, project-root resolution, render ranges, progress reporting, and overwrite protections.
- Move reusable decoding, encoding, probing, and mixing services out of CLI ownership into the shared engine.
- Replace the CLI’s independent text rasterization and CPU visual composition once GPUI parity tests pass.
- Keep non-rendering CLI commands usable without initializing a GPU.

### 3. Migrate preview, playback, and editing

- Replace GES timeline runtime state with the shared evaluator, media workers, mixer, and composition renderer.
- Reuse the existing FFmpeg playback implementation selectively; do not create an independently clocked player for each timeline clip.
- Support play/pause, scrubbing, seek completion, end-of-timeline behavior, audio-only timelines, and simultaneous visible clips.
- Apply edits to the existing timeline model. Invalidate affected layout, decoded frames, or audio buffers according to the changed data.
- Text and transform edits reuse decoded frames; source-position edits request new media.
- Preserve locking, snapping, selection, resizing, track visibility, mute, gain, and undo/redo.
- Route editor export through the same service used by CLI export.
- Replace remaining GStreamer consumers, including standalone playback, audio preview, waveforms, metadata probing, and backend-specific debug checks.

### 4. Cut over macOS and remove its runtime dependency

- Make the shared renderer the macOS default after feature and parity checks pass.
- Remove GStreamer from macOS application features, launch scripts, packaging, environment setup, and runtime discovery.
- Bundle required FFmpeg libraries with application-relative loading paths.
- Retain legacy implementation only where required by platforms awaiting migration; do not ship a second rendering backend in the migrated macOS application.
- Remove obsolete diagnostic code and update development and distribution documentation.

### 5. Port Windows, then Linux

- Implement the same offscreen host and texture/readback contract using each platform’s GPUI graphics backend.
- Reuse the shared evaluator, composition, worker protocols, audio mixer, and export service.
- Add platform audio-device integration and package compatible prebuilt FFmpeg libraries; do not build FFmpeg locally.
- Require the same functional and parity gates before each platform cuts over.
- After Linux migration, remove remaining GStreamer dependencies, implementations, setup scripts, and obsolete tests from OpenCut.

## Validation and acceptance

- **Composition parity:** compare preview’s project-size texture with offscreen export pixels before encoding, using identical assets, fonts, frame numbers, and renderer settings. Expect byte equality for deterministic fixtures on the same platform.
- **Layout stability:** resizing the editor or changing display scale must not change composition geometry, font size, wrapping, or layer placement.
- **Media correctness:** test fractional frame rates, variable-rate sources, nonzero source in-points, cuts, gaps, overlaps, images, multiline text, transparency, and hidden tracks.
- **Editing correctness:** text typing and dragging update the next rendered composition without media seeks; scrubbing rejects stale results; undo/redo restores both model and output.
- **Audio correctness:** test mute, gain, overlaps, resampling, export ranges, seek resets, and long-duration A/V synchronization. Preview without an audio device remains usable with silent playback.
- **Export correctness:** verify frame counts, timestamps, duration, codec/container options, cancellation, and failure cleanup. Only publish the final output after successful encoder draining and mux finalization.
- **Performance instrumentation:** measure evaluation, decode, layout/paint, GPU submission/completion, readback, encoding, audio mixing, and queue waits separately. Report preview latency and export throughput; do not claim a Remotion speedup without an equivalent benchmark.
- **Distribution:** launch and export from a clean machine without GStreamer, developer environment variables, or a separately installed FFmpeg.

## Defaults and boundaries

- Initial output is SDR, using a shared color-conversion path and explicit output metadata. HDR support and direct hardware encoder surface transfer are follow-up work.
- Preview/export identity means identical composition pixels before encoding on the same platform and font environment. Lossy encoding, chroma subsampling, and cross-platform font rasterization can produce differences.
- Existing project files retain their schema and coordinate semantics. No new effects, transition system, or animation language is introduced by this refactor.
- Complete each stage with working tests before expanding scope; shared-renderer correctness and zero-setup distribution are required outcomes, while performance gains must be measured.

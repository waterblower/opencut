# OpenCut TODO

## Confirmed bugs — recommended priority

### P0 — UI responsiveness and resource safety

- [ ] Move GES timeline construction, source discovery, and preview preroll off
      the UI thread. Coalesce edits into one pending rebuild and discard a
      completed graph when its project snapshot is no longer current.
- [ ] Replace history thumbnail `std::thread::spawn` calls with a bounded worker
      queue. Track in-flight paths so the same video cannot start two jobs, and
      give each job an exclusive temporary output path.
- [ ] Make the editor responsive below 724 px tall. The top bar, preview, and
      timeline must fit the available height without fixed-size children
      overlapping or clipping the playback controls.

### P1 — visible correctness and resilience

- [ ] Fix `format_speed` so it removes only trailing fractional zeros. It must
      render `0.5×`, `1.25×`, `1.5×`, and `1.05×` without changing the value.
- [ ] Make file-tree traversal tolerate an unreadable or disconnected
      subdirectory. Preserve readable entries and report the failing directory
      inline instead of aborting the complete tree refresh.
- [ ] Store application-global history, thumbnail cache, sidebar settings, and
      last-project settings in the platform application-data/config directory
      instead of the compile-time `CARGO_MANIFEST_DIR`. Keep project-owned state
      in `<project>/.opencut`.

### P2 — cleanup

#### Deduplicate editor domain logic

- [ ] Define video-property normalization once on `VideoClipProperties` and use
      it for property editing, project loading, preview rendering, and export.
      Keep finite-value, scale, opacity, and paired crop limits consistent so
      stored values cannot behave differently between UI and rendering paths.
- [ ] Define media-to-track compatibility once and use it for placement,
      append-target selection, and property availability. Keep audio restricted
      to audio tracks and video/images restricted to video tracks.
- [ ] Centralize asset-probe scheduling for explorer drag-and-drop and
      add-to-timeline actions. Share cached results and one in-flight path set,
      reject results from stale projects, and let each caller handle the
      completed asset without launching a duplicate probe.
- [ ] Add one narrowly scoped timeline-playback pause operation for the GES
      preview pipeline and standalone file previews. Use it
      when moving or trimming clips, scrubbing, changing project settings, and
      switching preview targets without combining unrelated UI reset behavior.

## Performance

Keep the debug build responsive enough for rapid POC development. Measure each
change in both debug and release builds; release already reaches the display's
120 Hz refresh rate, so optimize proven debug hot paths without weakening the
release path.

### P0 — per-frame rendering

- [ ] Make the shared video renderer upload a video surface only when a new decoded
      frame arrives. Retain and repaint the previous `CVPixelBuffer` otherwise,
      and stop requesting full frame copies merely because playback is active.
- [ ] Drain buffered video samples without packing every discarded frame into a
      new NV12 `Vec`. Pack or map only the newest frame that will actually be
      presented, and pursue a zero-copy GStreamer-to-CoreVideo path on macOS.

### P1 — main-thread scheduling

- [ ] Cache standalone audio duration instead of querying its GStreamer pipeline
      again while rendering the properties panel.
- [ ] Move periodic file-tree scanning off the UI thread or replace it with a
      filesystem watcher plus a background fallback scan for the project root
      and expanded directories.
- [ ] Move media-cache readiness validation off the one-second UI tick. Track
      cache completion and source-file changes explicitly instead of reopening
      and validating cache files for every referenced asset on each scan.
- [ ] Avoid rebuilding timeline elements for clips and tracks completely outside
      the visible horizontal or vertical viewport. Use GPUI virtualization or a
      viewport-indexed visible-item query for large projects.

### P2 — scaling and measurement

- [ ] Reduce multiresolution waveform-cache disk and memory usage if it becomes
      material for long projects. Measure first, then consider increasing the
      finest level from 64 to 128 samples per peak, packing peaks as `i8`, or
      compressing levels independently without sacrificing efficient range access.

### Export throughput

Baselines from `gst-launch-1.0` pipelines matching the export graph, on 120 s of
1280×720 24 fps rendered to 1920×1080. Decode plus convert costs 1.5 s and the
compositor 0.7 s; scaling is the dominant stage at 10.8 s single-threaded and
3.1 s once threaded. A full export lands near 15 s on either encoder, so once
scaling is threaded the encoder is the wall — the remaining wins come from doing
less work rather than encoding faster.

- [ ] Skip the scaler when the source already matches the export resolution.
      `build_timeline` calls `track.set_restriction_caps` unconditionally, so
      every frame passes through videoscale even when the dimensions are
      identical. Skipping the stage costs 1.5 s instead of 3.1 s threaded, and
      is the largest guaranteed win available.
- [ ] Skip the compositor on tracks that do not need mixing. `build_timeline`
      calls `track.set_mixing(true)` unconditionally, which routes every frame
      through a software blend and its conversions. Mixing is only required when
      clips overlap in time or carry a non-identity transform; `posx`, `posy`,
      `width`, `height`, and `alpha` are compositor pad properties, so gate this
      on `resolve_visual_clip_render_plan` returning identity for every clip on
      the track.
- [ ] Pass qualifying timeline segments through with
      `ges::PipelineFlags::SMART_RENDER` instead of re-encoding them. Export uses
      `PipelineFlags::RENDER`, which decodes, scales, and encodes every frame.
      Smart rendering copies the encoded stream for plain cuts whose source codec,
      resolution, and frame rate already match the export profile, collapsing
      those segments to demux and remux — decoding alone costs 1.5 s and a stream
      copy does not decode at all. Detect eligibility per clip from an identity
      `resolve_visual_clip_render_plan` plus the asset's `codec`, `width`,
      `height`, and frame-rate fields against `ExportOptions`, then re-encode only
      the segments that fail the check and the cuts that do not land on a
      keyframe boundary. Validate against `data/tests/mini测试` before designing
      around it; GES smart-render support has historically been incomplete.

## Clip properties

Add static per-clip transforms and audio controls before introducing keyframes
or an effects system.

- [ ] Add audio gain and mute controls to the properties panel, reusing the
      existing numeric-field, slider, and reset-button patterns from the video
      controls.
- [ ] Add stereo pan controls and apply pan consistently across simultaneous
      timeline audio previews and export.
- [ ] Edit the complete compatible clip selection or make no change. Disable a
      property when any selected clip is incompatible, and display mixed-value
      states when compatible clips have different values.
- [ ] Add integration tests for multi-selection compatibility, preview/export
      parity, timeline audio mixing, and atomic undo behavior.
- [ ] Add keyframes and interpolation for transform and audio properties after
      static properties work end-to-end.
- [ ] Add anchor-point controls, compositing modes, color correction, and
      extensible effects after the core property pipeline is stable.

## Editing modes

Expand the current move, trim, split, duplicate, and delete operations into a
consistent editing toolset.

- [ ] Add linked video/audio clip groups so related clips select, move, trim,
      split, and delete together.
- [ ] Add explicit link and unlink commands.
- [ ] Add ripple trim, shifting later clips by the trim delta.
- [ ] Add insert editing, shifting existing clips to make room.
- [ ] Add overwrite editing, replacing content in the destination range.
- [ ] Add roll editing between adjacent clips without changing their combined
      timeline duration.
- [ ] Add slip editing that changes a clip's source range without moving it.
- [ ] Add slide editing that moves a clip while adjusting its neighbours.
- [ ] Add collision, locked-track, track-compatibility, and minimum-duration tests
      for every editing mode.

# OpenCut TODO

## Confirmed bugs — recommended priority

### P0 — UI responsiveness and resource safety

- [ ] Move `AudioPreview` construction and GStreamer preroll off the UI thread.
      Deduplicate pending loads and discard a completed load when its clip is no
      longer requested.
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

- [ ] Remove unreachable missing-asset and image branches from
      `load_timeline_position`, or change `visual_clip_at_time` so those cases are
      intentionally returned and handled there.

#### Deduplicate editor domain logic

- [ ] Extract one pure clip-splitting operation that preserves source ranges and
      clip properties. Keep checkpointing, multi-clip batching, selection, and UI
      feedback in the existing editing commands.
- [ ] Replace the separate explorer-drop, clip-move, and clipboard-paste
      validators with one pure clip-placement validator. Cover timeline bounds,
      minimum duration, missing or locked tracks, track compatibility, overlap
      within the proposed placements, and overlap with existing clips; return a
      reason enum so each UI can supply its own localized message.
- [ ] Centralize asset-probe scheduling for explorer drag-and-drop and
      add-to-timeline actions. Share cached results and one in-flight path set,
      reject results from stale projects, and let each caller handle the
      completed asset without launching a duplicate probe.

## Performance

Keep the debug build responsive enough for rapid POC development. Measure each
change in both debug and release builds; release already reaches the display's
120 Hz refresh rate, so optimize proven debug hot paths without weakening the
release path.

### P0 — per-frame rendering

- [ ] Make `gpui-video-player` upload a video surface only when a new decoded
      frame arrives. Retain and repaint the previous `CVPixelBuffer` otherwise,
      and stop requesting full frame copies merely because playback is active.
- [ ] Drain buffered video samples without packing every discarded frame into a
      new NV12 `Vec`. Pack or map only the newest frame that will actually be
      presented, and pursue a zero-copy GStreamer-to-CoreVideo path on macOS.

### P1 — main-thread scheduling

- [ ] Complete the P0 `AudioPreview` background-preroll task above; timeline
      playback currently constructs a pipeline and can wait up to two seconds
      from the main-thread update path.
- [ ] Stop querying every active GStreamer audio pipeline position every 33 ms.
      Use a master playback clock and throttle drift checks or react to pipeline
      timing messages. Cache standalone audio duration instead of querying it
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

- [ ] Add reusable numeric fields, sliders, and reset buttons for editing one
      clip in the video and audio properties panels.
- [ ] Apply audio gain and mute during standalone audio preview playback.
- [ ] Add tests for property defaults, serialization, controls, and preview value
      mapping.
- [ ] Add stereo pan and apply audio properties consistently across simultaneous
      timeline audio previews.
- [ ] Edit the complete compatible clip selection or make no change. Disable a
      property when any selected clip is incompatible, and display mixed-value
      states when compatible clips have different values.
- [ ] Coalesce continuous slider and pointer gestures into one snapshot-based
      undo checkpoint instead of creating an undo entry for every update.
- [ ] Apply the same video and audio properties during export so preview and
      exported output remain consistent.
- [ ] Add integration tests for multi-selection compatibility, preview/export
      parity, timeline audio mixing, and atomic undo behavior.
- [ ] Add keyframes and interpolation for transform and audio properties after
      static properties work end-to-end.
- [ ] Add cropping, anchor-point controls, compositing modes, color correction,
      and extensible effects after the core property pipeline is stable.

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

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
- [x] Replace fixed full-asset waveform images with a multiresolution peak-data
      cache, and render each clip's `source_in` to `source_out` range dynamically.
- [ ] Make file-tree traversal tolerate an unreadable or disconnected
      subdirectory. Preserve readable entries and report the failing directory
      inline instead of aborting the complete tree refresh.
- [ ] Store application-global history, thumbnail cache, sidebar settings, and
      last-project settings in the platform application-data/config directory
      instead of the compile-time `CARGO_MANIFEST_DIR`. Keep project-owned state
      in `<project>/.opencut`.
- [ ] Make explorer metadata consistent for audio and video files. Probe media
      duration in the background and cache it for files that have not been added
      to the timeline, rather than showing duration only for registered assets
      and file size for everything else. Bound and deduplicate probe jobs, and
      discard stale results when the project changes.

### P2 — performance and cleanup

- [ ] Reduce multiresolution waveform-cache disk and memory usage if it becomes
      material for long projects. Measure first, then consider increasing the
      finest level from 64 to 128 samples per peak, packing peaks as `i8`, or
      compressing levels independently without sacrificing efficient range access.
- [ ] Stop querying every active GStreamer audio pipeline position every 33 ms.
      Use a master playback clock and throttle drift checks or react to pipeline
      timing messages.
- [ ] Move periodic file-tree scanning off the UI thread or replace it with a
      filesystem watcher plus a background fallback scan for the project root
      and expanded directories.
- [ ] Remove unreachable missing-asset and image branches from
      `load_timeline_position`, or change `visual_clip_at_time` so those cases are
      intentionally returned and handled there.

### P3 — future scalability

- [ ] Revisit snapshot-based undo history only if project snapshots become
      materially expensive. Measure memory usage and checkpoint latency first;
      then consider structural sharing, deltas, or an operation log while
      preserving atomic undo for compound edits.

## Clip properties

Add static per-clip transforms and audio controls before introducing keyframes
or an effects system.

- [ ] Extend `TimelineClip` with defaulted video and audio property structures.
      Video properties should include position X/Y, scale, rotation, and opacity;
      audio properties should include gain in dB, mute, and stereo pan.
- [ ] Add reusable numeric fields, sliders, and reset buttons for editing one
      clip in the video and audio properties panels.
- [ ] Apply audio gain and mute during standalone audio preview playback.
- [ ] Apply video position, scale, rotation, and opacity in the preview renderer.
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

- [x] Add tool modes and cursor feedback for selection, blade, and trim.
- [x] Add copy, cut, and paste for multiple selected clips. Preserve relative
      timing and track placement, and keep paste as one atomic undo operation.
- [x] Allow project files to be dragged from the asset explorer onto compatible
      tracks at a chosen timeline position, with a visible placement preview and
      collision feedback.
- [ ] Add linked video/audio clip groups so related clips select, move, trim,
      split, and delete together.
- [ ] Add explicit link and unlink commands.
- [ ] Add ripple delete for single and multi-clip selections, closing the
      resulting timeline gaps without introducing overlaps.
- [ ] Add ripple trim, shifting later clips by the trim delta.
- [ ] Add insert editing, shifting existing clips to make room.
- [ ] Add overwrite editing, replacing content in the destination range.
- [ ] Add roll editing between adjacent clips without changing their combined
      timeline duration.
- [ ] Add slip editing that changes a clip's source range without moving it.
- [ ] Add slide editing that moves a clip while adjusting its neighbours.
- [ ] Add collision, locked-track, track-compatibility, and minimum-duration tests
      for every editing mode.

# OpenCut TODO

Scope: the editor (`src/editor/`) is the product. The standalone player
(`src/player/`) is an experiment kept for trying GPUI and GStreamer ideas in
isolation, so its bugs are deliberately not tracked here. Shared code that both
binaries use — `src/video.rs`, `src/playback_view.rs` — is in scope.

## Confirmed bugs — recommended priority

### P0 — UI responsiveness and resource safety

- [ ] Move GES timeline construction, source discovery, and preview preroll off
      the UI thread. Coalesce edits into one pending rebuild and discard a
      completed graph when its project snapshot is no longer current.
      `preview.rs` calls `create_timeline_video` synchronously inside
      `load_timeline_position_with_options`. Clean seeks reuse the existing
      pipeline, but initial loading and edits that mark the preview dirty rebuild
      the whole GES timeline on the main thread, with cost growing by clip count.

### P1 — visible correctness and resilience

- [ ] Store the editor's application-global settings in the platform
      application-data/config directory instead of the compile-time
      `CARGO_MANIFEST_DIR`. `workspace.rs` writes the last project, active
      timeline, timeline zoom, scroll positions, snapping, and magnet settings
      to `data/editor-settings.json`, and falls back to `CARGO_MANIFEST_DIR` as
      the initial project root. A copied or installed binary therefore reads
      and writes paths from the machine that built it. Keep project-owned
      timeline data in the project folder.

### P2 — cleanup

#### Deduplicate editor domain logic

- [ ] Define video-property normalization once on `VideoClipProperties` and use
      it for property editing, project loading, preview rendering, and export.
      Keep finite-value, scale, opacity, and paired crop limits consistent so
      stored values cannot behave differently between UI and rendering paths.
- [ ] Finish centralizing asset probing. `media_probe::probe_asset` is now the
      single probe, but scheduling is still duplicated: the explorer drag path
      guards with `explorer_drag_probe_jobs`, while the add-to-timeline path
      spawns `probe_asset` with no in-flight check and no shared cache. Share
      cached results and one in-flight path set across both, and reject results
      from stale projects.
- [ ] Add one narrowly scoped timeline-playback pause operation for the GES
      preview pipeline and standalone file previews. There are twelve
      `set_paused(true)` sites across `preview.rs`, `timeline_interactions.rs`,
      and `settings.rs`. Use the shared operation when moving or trimming clips,
      scrubbing, changing project settings, and switching preview targets,
      without combining unrelated UI reset behavior.

### P3 — deferred while this is a POC

Real failure modes that a shipped editor would need to handle, deliberately
parked because they do not occur on a development machine editing local media.

- [ ] Make file-tree traversal tolerate an unreadable or disconnected
      subdirectory. Preserve readable entries and report the failing directory
      inline instead of aborting the complete tree refresh. `read_directory`
      propagates any `read_dir` failure out of `visible_tree`, so one permission
      error or an unplugged external drive discards the whole tree and leaves the
      explorer frozen behind a per-second error banner. Revisit when footage on
      external or network volumes becomes a supported workflow.

## Performance

Keep the debug build responsive enough for rapid POC development. Measure each
change in both debug and release builds; release already reaches the display's
120 Hz refresh rate, so optimize proven debug hot paths without weakening the
release path.

### P0 — per-frame rendering

- [ ] Make the shared video renderer in `src/video.rs` upload a video surface
      only when a new decoded frame arrives. Retain and repaint the previous
      `CVPixelBuffer` otherwise, and stop requesting full frame copies merely
      because playback is active.
- [ ] Drain buffered video samples without packing every discarded frame into a
      new NV12 `Vec`. Pack or map only the newest frame that will actually be
      presented, and pursue a zero-copy GStreamer-to-CoreVideo path on macOS.

### P1 — main-thread scheduling

- [ ] Move periodic file-tree scanning off the UI thread or replace it with a
      filesystem watcher plus a background fallback scan for the project root
      and expanded directories.
- [ ] Avoid rebuilding timeline elements for clips and tracks completely outside
      the visible horizontal or vertical viewport. Use GPUI virtualization or a
      viewport-indexed visible-item query for large projects.

### P2 — scaling and measurement

- [ ] Measure multiresolution waveform memory usage with long media before
      changing its representation. If it becomes material, compare coarser peak
      levels and packed samples without sacrificing efficient range access.

### Export throughput

Recorded baselines from `gst-launch-1.0` pipelines matching the export graph, on
120 s of 1280×720 24 fps rendered to 1920×1080. Decode plus convert costs 1.5 s
and the compositor 0.7 s; scaling is the dominant stage at 10.8 s
single-threaded and 3.1 s once threaded. A full export lands near 15 s on either
encoder, so once scaling is threaded the encoder is the wall — the remaining
wins come from doing less work rather than encoding faster. Rerun these
measurements before making a pipeline change; the source code alone cannot
verify the figures.

`configure_export_elements` already sets `n-threads = 0` on the converters, so
the 3.1 s figure describes the current measured configuration rather than a
target. The items below are investigations, not confirmed optimizations.

- [ ] Avoid unnecessary video scaling when decoded frames already satisfy the
      export dimensions and frame rate. `build_timeline` must retain output
      restriction caps so the export format remains correct; measure the actual
      GStreamer graph and determine whether matching sources already negotiate a
      passthrough path before changing pipeline construction.
- [ ] Avoid the compositor when the complete exported video timeline does not
      need mixing. `build_timeline` calls `set_mixing(true)` unconditionally.
      Eligibility must account for simultaneous content across all GES layers,
      as well as non-identity position, scale, opacity, and crop transforms; it
      cannot be decided independently for each project track.
- [ ] Experiment with `ges::PipelineFlags::SMART_RENDER` on plain cuts before
      treating it as an optimization plan. Export currently uses
      `PipelineFlags::RENDER`. Validate codec/profile matching, keyframe-boundary
      behavior, mixed eligible and ineligible segments, transforms, and output
      correctness against `data/tests/mini测试`; keep normal rendering as the
      fallback because GES smart-render support may not cover the required graph.

## Clip properties

Add static per-clip transforms and audio controls before introducing keyframes
or an effects system.

- [ ] Add audio gain and mute controls to the properties panel, reusing the
      existing numeric-field, slider, and reset-button patterns from the video
      controls. This is UI-only work: `AudioClipProperties` already carries the
      values and `clip_render_plan` already resolves gain and mute for preview
      and export, but nothing in `properties.rs` references them yet.
- [ ] Add stereo pan to `AudioClipProperties`, then add controls and apply it
      consistently across timeline preview and export. The model currently
      stores only gain and mute despite its stale documentation mentioning pan.
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

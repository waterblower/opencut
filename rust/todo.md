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

- [ ] Keep the existing GES preview pipeline and timeline alive while editing.
      Apply each committed model change to its corresponding GES layer, clip,
      source, or child property, commit the timeline, and refresh the current
      frame without reconstructing the pipeline. The current rebuild boundary is
      [preview.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/preview.rs:41),
      and [preview.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/preview.rs:125)
      replaces the pipeline whenever `timeline_needs_rebuild` is set. Cover every
      output-affecting action below:
      - Add a media clip:
        [explorer.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/explorer.rs:767).
      - Add a text clip:
        [timeline_track_menu.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/timeline_track_menu.rs:123).
      - Delete selected clips, including magnet/ripple gap closing:
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:214)
        and [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:316).
      - Cut selected clips:
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:247).
      - Paste clips and any newly imported assets:
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:274).
      - Duplicate selected clips:
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:351).
      - Split or blade clips:
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:197)
        and [mod.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/mod.rs:553).
      - Move one or more clips in time or between compatible tracks:
        [timeline_interactions.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/timeline_interactions.rs:610).
      - Change video position or scale from the properties panel:
        [properties_transform.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/properties_transform.rs:219).
      - Apply one clip's video transform to the other clips on its track:
        [timeline_clip_menu.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/timeline_clip_menu.rs:147).
      - Drag a video's position directly in the preview:
        [preview_timeline.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/preview_timeline.rs:245).
        This is already partially updated in place by
        [timeline_video.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/timeline_video.rs:25);
        extend that path to all transform properties and use it as the pattern
        for other property edits.
      - Change text content, font size, color, or position:
        [properties_text.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/properties_text.rs:222).
      - Add an application track/GES layer:
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:438).
      - Delete a track and its clips:
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:546).
      - Reorder tracks/layers:
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:515).
      - Toggle track visibility:
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:485).
      - Toggle track mute:
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:500).
      - Change the timeline frame rate and retime clips:
        [settings.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/settings.rs:135).
      - Rename a media file or directory and replace affected GES source URIs:
        [explorer_file_menu.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/explorer_file_menu.rs:254).
      - Apply arbitrary model differences produced by undo and redo:
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:645)
        and [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:658).

      The following actions are editor/view state only and must never mutate,
      commit, or rebuild the GES timeline:
      - Copy clips:
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:230).
      - Single, toggle, select-all, and marquee selection:
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:596),
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:609),
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:624),
        and [timeline_interactions.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/timeline_interactions.rs:306).
      - Track locking:
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:473).
      - Snapping and track-magnet toggles:
        [timeline_interactions.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/timeline_interactions.rs:640)
        and [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:735).
      - Timeline zoom and scrolling:
        [timeline_interactions.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/timeline_interactions.rs:180)
        and [timeline_interactions.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/timeline_interactions.rs:651).
      - Playhead scrubbing and frame stepping:
        [timeline_interactions.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/timeline_interactions.rs:713),
        [timeline_interactions.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/timeline_interactions.rs:726),
        and [timeline_interactions.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/timeline_interactions.rs:776).
      - Persisting playhead and scroll state:
        [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:717)
        and [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:726).
      - Context-menu state, preview volume, and playback controls.

      Clip trimming, text-duration resizing, and per-clip audio-property editing
      do not currently have editor actions. Add them to this in-place GES update
      matrix when their UI operations are implemented.

### P1 — visible correctness and resilience

- [ ] Store the editor's application-global settings in the platform
      application-data/config directory instead of the compile-time
      `CARGO_MANIFEST_DIR`. `workspace.rs` writes the last project folder to
      `data/editor-settings.json` and falls back to `CARGO_MANIFEST_DIR` as the
      initial project root. A copied or installed binary therefore reads and
      writes paths from the machine that built it. Keep project-owned timeline
      data in the project folder.

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

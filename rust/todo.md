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
- [ ] Crop and offset cached full-asset waveforms using each clip's `source_in`
      and `source_out`, so split and trimmed clips show the correct source range.
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

- [ ] Stop querying every active GStreamer audio pipeline position every 33 ms.
      Use a master playback clock and throttle drift checks or react to pipeline
      timing messages.
- [ ] Move periodic file-tree scanning off the UI thread or replace it with a
      filesystem watcher plus a background fallback scan for the project root
      and expanded directories.
- [ ] Remove unreachable missing-asset and image branches from
      `load_timeline_position`, or change `visual_clip_at_time` so those cases are
      intentionally returned and handled there.

## Editing modes

Expand the current move, trim, split, duplicate, and delete operations into a
consistent editing toolset.

- [x] Add multi-selection with Command-click and rectangular selection.
- [x] Show collision feedback while moving selected clips instead of silently
      refusing invalid positions. Keep multi-clip moves atomic.
- [ ] Add a snapping toggle and visible guides for playhead and clip-edge
      snap targets.
- [ ] Add tool modes and cursor feedback for selection, blade, trim, and hand/pan.
- [ ] Add copy, cut, and paste for multiple selected clips. Preserve relative
      timing and track placement, and keep paste as one atomic undo operation.
- [ ] Allow project files to be dragged from the asset explorer onto compatible
      tracks at a chosen timeline position, with a visible placement preview and
      collision feedback.
- [ ] Make every compound edit create one atomic undo/redo transaction.
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

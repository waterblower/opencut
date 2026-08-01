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

## Frame-based timeline math

Replace floating-point seconds as the canonical timeline representation with
integer frame/tick values and an explicit rational project frame rate.

- [ ] Add project video settings containing the frame-rate numerator and
      denominator, resolution, and audio sample rate.
- [ ] Introduce a `TimelineTime` or `FrameNumber` newtype backed by an integer.
- [ ] Store `timeline_start`, `source_in`, and `source_out` using the new timeline
      type instead of `f64` seconds.
- [ ] Define conversions between timeline frames, media timestamps, seconds,
      `Duration`, FFmpeg time bases, and GStreamer clock time.
- [ ] Define one rounding policy for seeking, trimming, splitting, snapping, and
      export boundaries.
- [ ] Make the playhead, markers, clip edges, snapping, ruler labels, and frame
      stepping use the same timeline type.
- [ ] Handle source media whose frame rate differs from the project frame rate.
- [ ] Handle variable-frame-rate sources without accumulating timing drift.
- [ ] Keep audio timing sample-accurate while mapping it onto the video timeline.
- [ ] Update project serialization to store exact integer/rational values.
- [ ] Add tests for fractional frame rates such as 23.976, 29.97, and 59.94 fps,
      long timelines, repeated splits, and preview/export boundary agreement.

## Editing modes

Expand the current move, trim, split, duplicate, and delete operations into a
consistent editing toolset.

- [ ] Add multi-selection with Command-click and rectangular selection.
- [ ] Add copy, cut, paste, and duplicate for multiple selected clips.
- [ ] Add linked clip groups so video and its audio move and trim together.
- [ ] Add explicit link and unlink commands.
- [ ] Add ripple delete, closing the resulting timeline gap.
- [ ] Add ripple trim, shifting later clips by the trim delta.
- [ ] Add insert editing, shifting existing clips to make room.
- [ ] Add overwrite editing, replacing content in the destination range.
- [ ] Add roll editing between adjacent clips without changing their combined
      timeline duration.
- [ ] Add slip editing that changes a clip's source range without moving it.
- [ ] Add slide editing that moves a clip while adjusting its neighbours.
- [ ] Allow project files to be dragged from the asset explorer onto compatible
      tracks at a chosen timeline position.
- [ ] Add a snapping toggle and visible guides for playhead, marker, and clip-edge
      snap targets.
- [ ] Add tool modes and cursor feedback for selection, blade, trim, and hand/pan.
- [ ] Make every compound edit create one atomic undo/redo transaction.
- [ ] Add collision, locked-track, track-compatibility, and minimum-duration tests
      for every editing mode.

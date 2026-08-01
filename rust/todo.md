# OpenCut editor TODO

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

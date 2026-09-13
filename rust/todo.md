# OpenCut TODO

- [ ] **P1 — Prevent text edits from restoring stale clip state.**

  The text input callback captures the original clip and submits the entire
  snapshot on every change. Editing text after moving or trimming the clip can
  restore old timing; a resulting overlap can trigger a panic.
  Location: [properties_text.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/properties_text.rs:37).

  **Fix plan:**

  1. Replace the captured full-clip edit with an event carrying the clip ID and
     new text in [properties_text.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/properties_text.rs:37).
  2. Resolve the current clip and update only its text, preserving timing, track,
     length, and other properties. Handle missing clips without panicking and
     preserve history/autosave at the event boundary in
     [editor.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editor.rs:160).
  3. Synchronize the input with undo/redo model changes without emitting another
     edit from [properties_text.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/properties_text.rs:26);
     check restoration through [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:689).
  4. Add regression coverage alongside [editing.test.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/tests/editing.test.rs:217)
     and verify the text input flow: move or trim before typing, undo/redo, and
     a case where restoring the old placement would overlap another clip.

- [ ] **P1 — Update every affected timeline when renaming media.**

  Rename updates asset paths only in the active timeline. Other timelines using
  the renamed file or folder retain obsolete references and cannot load the media.
  Location: [explorer_file_menu.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/explorer_file_menu.rs:106).

  **Fix plan:**

  1. Discover all project timelines, including nested ones, using
     [timeline_document.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/timeline_document.rs:33).
     Prepare affected asset-path updates before renaming; use the active
     in-memory model for the active timeline.
  2. Extend [explorer_file_menu.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/explorer_file_menu.rs:80)
     to remap references in every affected timeline, including descendants of a
     renamed directory and timeline files whose own paths change.
  3. Stage timeline saves and provide rollback on failure around
     [timeline.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/timeline.rs:140)
     and [explorer_file_menu.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/explorer_file_menu.rs:80).
     Propagate failures to the application boundary with file and line context.
  4. Update active undo/redo snapshots in
     [explorer_file_menu.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/explorer_file_menu.rs:123)
     and clipboard paths in [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:6)
     so later edits do not reintroduce obsolete references.
  5. Verify the rename flow in
     [explorer_file_menu.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/explorer_file_menu.rs:80)
     with shared media across two timelines, nested directory renames, reopening
     each timeline, and injected save failures to exercise rollback.

- [ ] **P2 — Preserve text fonts in exports and timeline rebuilds.**

  Full GES timeline construction hardcodes Sans, while incremental text updates
  use the stored font. Exporting or rebuilding can change text appearance.
  Location: [export_gstreamer.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/export_gstreamer.rs:168).

  **Fix plan:**

  1. Build the font description from the stored font family and existing scaled
     font size in [export_gstreamer.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/export_gstreamer.rs:168).
  2. Check consistent font handling across full construction, incremental
     insertion in [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:1510),
     and text updates in [editing.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.rs:1411).
  3. Extend the overlay regression test in
     [export_gstreamer.test.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/tests/export_gstreamer.test.rs:295)
     with a non-Sans font. Verify preview after rebuild and exported output,
     including export at a different resolution.

- [ ] **P2 — Restore the saved playhead when opening a timeline.**

  Startup reads the new playback backend's position instead of the persisted
  saved_playhead_frame, so reopening starts at the beginning.
  Location: [editor.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editor.rs:151).

  **Fix plan:**

  1. Initialize playback from the saved frame after the backend is ready in
     [editor.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editor.rs:148),
     clamping to the valid timeline range and handling empty timelines.
  2. Apply the same restoration behavior when switching timelines in
     [mod.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/mod.rs:281).
  3. Ensure switch and close paths capture the outgoing playhead before saving,
     using [timeline.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/timeline.rs:459)
     and checking the switch flow in [mod.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/mod.rs:281).
  4. Extend view-state coverage in
     [timeline.test.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/tests/timeline.test.rs:36)
     and verify startup/switch restoration at a nonzero frame, fractional frame
     rates, empty timelines, and a saved position beyond shortened content.

- [ ] **Text manipulation in the preview — implement stages in order.**

  Extend existing preview interactions to text, sharing renderer-derived bounds
  between hit detection, selection outlines, and eventual snapping. Preserve the
  serialized text format and existing media interactions.
  Location: [preview_timeline.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/preview_timeline.rs:10).

  1. **Stage 1 — Resolve text geometry (implemented).** Read actual rendered bounds from the
     GStreamer title overlay; convert to preview coordinates with canvas scaling
     and letterboxing. Avoid a separate geometry cache. Exclude hidden, inactive,
     and empty text; wait for rendered dimensions when no frame is ready.
     Verify multiline, Unicode, font-size changes, and preview scaling.
  2. **Stage 2 — Hit detection and selection outline (implemented).** Hit-test text rectangles
     in rendering order, synchronize with timeline selection, and clear selection
     on empty canvas clicks. Draw the accent outline for selected text, including
     selection from the timeline. Locked text is selectable but cannot move;
     unlocked text gets the move cursor. Do not add text resize handles.
     Verify overlaps, hidden/locked tracks, seeking, and letterbox exclusion.
  3. **Stage 3 — Drag and persist text position.** Extend drag data with a typed
     text variant containing starting position and geometry. Select and move in
     one gesture without jumping. Convert movement into normalized renderer
     positions, handling limits and zero movement space safely. Change only the
     current clip's position. Reuse refresh throttling, one undo entry per changed
     drag, and save on release (including outside the preview). Clear unavailable
     drag targets. Complete the stale text-input snapshot fix above so subsequent
     typing cannot restore old positions. Verify movement, undo/redo, save/reopen,
     typing after dragging, and no history entry for an unchanged click.
  4. **Stage 4 — Snapping and regressions.** Reuse the snapping toggle and four
     preview-pixel threshold. Snap edges/centers to canvas and other visible active
     text/media clips; include text as a media snapping target. Snap each axis
     independently and show only guides matching final alignment. Clear guides on
     release. Verify enabled/disabled snapping, threshold boundaries, scaling,
     self/hidden/inactive target exclusion, and existing media movement/resizing.

  **Current implementation scope:** Stages 1–2 only. Stages 3–4 remain deferred.
  Use existing vendored libraries; no FFmpeg build or Python build steps.

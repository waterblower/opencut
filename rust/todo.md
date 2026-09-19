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

- [ ] **Build timeline preview with FFmpeg and GPUI.**

  The current timeline preview is black and silent. Keep the existing document
  format and ruler/track editing behavior. Use renderer-derived text geometry
  for future selection, dragging, resizing, and snapping. Add timeline audio
  playback and export as separate follow-up steps.

- [x] **Restore the saved playhead when opening a timeline.**

  The runtime now uses the saved frame directly and clamps it to the timeline
  duration, including empty timelines.

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
  4. Add regression coverage alongside [editing.test.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/editing.test.rs:217)
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
     [export_gstreamer.test.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/export_gstreamer.test.rs:295)
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
     [timeline.test.rs](/Users/mac/Documents/GitHub/OpenCut/rust/src/editor/timeline.test.rs:36)
     and verify startup/switch restoration at a nonzero frame, fractional frame
     rates, empty timelines, and a saved position beyond shortened content.

# OpenCut Design Spec

This document describes what the OpenCut editor does from a user's point of
view. Each feature is written as a user story followed by acceptance criteria.
The editor is OpenCut's only end-user product; the `player` and `cli` apps are
development tools and are not specified here.

UI mockups are in [pages/](pages/) and are linked from the sections they
illustrate.

Review status (2026-10-16):

- `[x]` (☑ in tables): passed source review and applicable existing tests.
- `[ ]` (☐): a known defect, missing implementation, or blocked existing test,
  explained below.

These checks do not represent full end-to-end acceptance: GUI interactions,
actual MP4 exports, and live MiniMax transcription have not been exercised.

```text
█ passed   ░ open (defect, missing implementation, or blocked test)
Counts: passed / open; each block pair is one criterion.

Projects and media            ████████████████░░░░░░           8 / 3
Timelines                     ████████░░                       4 / 1
Tracks                        ████████░░░░                     4 / 2
Selecting and editing clips   ██████████                       5 / 0
Trimming                      ██████                           3 / 0
Timeline navigation           ████████░░                       4 / 1
Workspace layout              ██████                           3 / 0
Preview                       ████░░░░                         2 / 2
Subtitles and text            ████████████████████            10 / 0
Export                        ██████████                       5 / 0
Keyboard shortcuts            ████████████████████████████░░  14 / 1
Project data and persistence  ████████████░░                   6 / 1
Total                                                         68 / 11
```

## 1. Projects and media

**Story:** As a creator, I want to open a folder as a project and use the files
already in it, so that I don't have to import or copy media.

Mockups: [project picker](pages/01-project-picker.svg),
[editor without a timeline](pages/02-editor-empty.svg),
[media preview](pages/04-editor-media-preview.svg),
[rename dialog](pages/07-dialogs.svg),
[context menus](pages/11-context-menus.svg).

- [x] Any folder can be opened as a project.
- [x] Supported media appears in a live file tree (the Explorer) without an import
  step.
- [x] The Explorer can be filtered.
- [x] Selecting a media file previews it in place.
- [ ] Media can be dragged from the Explorer onto a compatible track.
- [ ] Waveforms are generated in the background and kept in memory only.
- [x] Source media is never rewritten.

Review: the drop handler rejects images, and the planned image tracks do not
exist yet (see [Tracks](#3-tracks)). Background waveform generation and memory
caching are implemented, but the waveform test cannot complete because its media fixture
`rust/data/tests/super long.mp4` is missing.

Supported file extensions:

| Status | Kind | Extensions | Notes |
| --- | --- | --- | --- |
| ☑ | Video | `.mp4`, `.mov`, `.m4v`, `.mkv`, `.webm`, `.avi` | |
| ☐ | Image | `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.bmp`, `.ico` | Added to image tracks as five-second still clips |
| ☑ | Audio | `.aac`, `.flac`, `.m4a`, `.mp3`, `.ogg`, `.wav` | |
| ☑ | Subtitles | `.srt` | Added as text clips |

## 2. Timelines

**Story:** As a creator, I want each edit to be a plain file in my project, so
that I can keep several versions, move them, and back them up like any other
file.

Mockups: [new timeline dialog](pages/07-dialogs.svg),
[timeline settings dialog](pages/07-dialogs.svg),
[context menus](pages/11-context-menus.svg).

- [x] A timeline is a JSON file named `*.timeline` or `*.timeline.json`, anywhere in
  the project.
- [x] Right-clicking a folder and choosing **Create New Timeline** creates a
  `.timeline` file there. Clicking a timeline file in the Explorer opens it.
- [ ] Right-clicking a timeline file and choosing **Settings** changes its frame
  rate. The playhead is rescaled to the new rate.
- [x] A new timeline has no tracks.
- [x] Every committed edit saves the timeline automatically.

Review: decreasing the frame rate can clamp the old playhead before rescaling
it, producing the wrong position.

## 3. Tracks

**Story:** As a creator, I want separate video, image, audio, and text tracks
that I can organize, so that I can layer footage, stills, sound, and subtitles.

Mockups: [editor with a timeline](pages/03-editor-timeline.svg),
[timeline panel](pages/17-timeline-panel.svg).

- [ ] Tracks can be created, deleted, reordered, hidden, muted, and locked.
- [x] Video tracks accept video files only.
- [ ] Image tracks accept still images.
- [x] Audio tracks accept audio files.
- [x] Text tracks accept `.srt` files. Dropping one adds one text clip per subtitle,
  offset by the drop position.
- [x] Locked tracks are excluded from selection shortcuts.

Review: track controls update the document, but visibility, mute, reordering,
and deletion do not refresh the open preview. Image tracks are not implemented;
the drop handler currently rejects image files on every track.

## 4. Selecting and editing clips

**Story:** As a creator, I want to select and rearrange many clips at once, so
that restructuring an edit doesn't take one clip at a time.

Mockups: [editor with a timeline](pages/03-editor-timeline.svg),
[blade tool](pages/06-editor-blade-tool.svg),
[timeline panel](pages/17-timeline-panel.svg),
[context menus](pages/11-context-menus.svg).

- [x] Clips are selected by clicking, Command-clicking to add or remove, or drawing
  a selection rectangle.
- [x] Move, duplicate, copy, cut, paste, and delete work on multiple clips and
  keep their relative timing. Each operation that changes the timeline is a
  single undo step.
- [x] The selection tool moves clips within and between compatible tracks.
- [x] The blade tool splits clips at the playhead.
- [x] Invalid moves are rejected with visible feedback.

## 5. Trimming

**Story:** As a creator, I want to drag a clip's edges to change its duration,
so that I can tighten cuts directly on the timeline.

Mockup: [timeline panel](pages/17-timeline-panel.svg).

- [x] Either edge of a video, audio, image, or text clip can be dragged.
- [x] Trimming respects neighboring clips, the source media's length, and a
  one-frame minimum.
- [x] Each drag is one undo step and saves on release.

Review: image trimming works for existing timeline files that represent images
as video clips. Dedicated image tracks and Explorer image drops remain open in
[Tracks](#3-tracks).

## 6. Timeline navigation

**Story:** As a creator, I want to zoom, scroll, and snap precisely, so that I
can make frame-accurate edits.

Mockups: [editor with a timeline](pages/03-editor-timeline.svg),
[timeline panel](pages/17-timeline-panel.svg).

- [x] The timeline is frame-based.
- [x] The timeline supports horizontal zoom, including trackpad pinch, and
  scrolling. Frame ticks appear at high zoom.
- [x] Clicking the ruler moves the playhead.
- [x] Optional snapping (**Snap on** / **Snap off**) aligns to the playhead and clip
  edges.
- [ ] Playhead, scroll, zoom, and snapping are saved per timeline.

Review: toggling snapping does not save the timeline. Exiting before another
save loses that change. Empty timelines also have a playhead-clamping defect.

## 7. Workspace layout

**Story:** As a creator, I want to size the panels to my task, so that I can
give more room to the timeline or the preview as needed.

Mockups: [editor with a timeline](pages/03-editor-timeline.svg),
[top bar](pages/20-top-bar.svg),
[properties panel](pages/15-properties-panel.svg).

- [x] The media and properties panels beside the preview can be resized.
- [x] Dragging the divider above the timeline resizes the timeline panel.
- [x] Panel sizes are saved per project.

## 8. Preview

**Story:** As a creator, I want to play the timeline as it will be exported, so
that I can judge the edit without exporting.

Mockups: [preview variants](pages/16-preview-panel.svg),
[video player](pages/18-video-player.svg),
[fullscreen preview](pages/12-fullscreen-preview.svg).

The preview design has five states: image, audio, video, error, and none.
Video covers media playback and rendered timelines. A single error state serves
all preview sources: it shows which selected file failed and an error string.
The error string preserves line breaks, wraps long lines, and scrolls when it
exceeds the available space.

- [x] The preview composites video, including each clip's position and scale.
- [ ] Audio is mixed and respects track mute, clip mute, and clip gain.
- [ ] Each saved edit, including undo and redo, refreshes the preview.
- [x] The preview can be shown fullscreen.

Review: mixing handles mute and gain in the player's snapshot, but changing
track mute does not refresh that snapshot. These edits also save without
refreshing the preview: track controls, paste, duplicate, splitting, Explorer
drops, applying a transform to a track, adding a text clip from the track menu,
and renaming media. The video-properties setter also bypasses refresh, but the
current video properties panel displays position and scale as read-only values.
Undo and redo use the refresh path.

## 9. Subtitles and text

**Story:** As a creator of subtitle-driven videos, I want to generate, place,
and style subtitles, and cut my video around them, so that I can turn a raw
recording into a tight, captioned edit.

### Generate subtitles

Mockups: [transcription window](pages/14-transcription-window.svg),
[settings dialog](pages/07-dialogs.svg),
[context menus](pages/11-context-menus.svg).

- [x] Right-clicking an audio or video file and choosing **Generate SRT** opens a
  transcription window.
- [x] **Start** begins transcription. The window shows the current stage
  (preparing, transcribing, saving) and offers **Cancel**.
- [x] The result is written to `<source name>.srt` in the project root.
- [x] Transcription uses MiniMax. The API key is entered under **Settings** in the
  top bar.

Review: the request, stage updates, and cancellation controls are implemented;
a live transcription and cancellation run is still pending.

### Position and style subtitles

Mockups: [text positioning](pages/05-editor-text-positioning.svg),
[context menus](pages/11-context-menus.svg).

- [x] Text clips can be selected and dragged directly in the preview.
- [x] With **Snap on**, dragged text aligns to canvas centers and edges, with
  visible guides.
- [x] Right-clicking a text clip and choosing **Apply style to track** copies its
  font, size, color, and position to every other text clip on the same track,
  preserving their words and timing.

### Keep only the spoken parts

- [x] Right-clicking a timeline file and choosing **Keep Text Sections** removes
  time outside text-covered sections and their padding, across all tracks, and
  closes the gaps.
- [x] About 25 ms of padding, rounded to timeline frames, is kept on each side of
  each text section.
- [x] The operation is one undo step.

## 10. Export

**Story:** As a creator, I want to export my timeline to a video file, so that
I can publish it.

Mockup: [export window](pages/13-export-window.svg).

- [x] The top-right **Export** button opens a separate export window for the active
  timeline.
- [x] The export uses a snapshot of the timeline taken when the export window
  opens.
- [x] The output format is MP4.
- [x] The window shows progress and provides **Stop export**.
- [x] The window cannot be closed while an export is running.

Review: progress and stop handling are implemented, but an actual export and
stop run is still pending.

## 11. Keyboard shortcuts

| Status | Shortcut | Action |
| --- | --- | --- |
| ☑ | `Space` | Play or pause the preview |
| ☑ | `Left` / `Right` | Move the playhead one frame |
| ☑ | `V` / `B` | Selection / blade tool |
| ☑ | `Command-click` | Add or remove a clip from the selection |
| ☑ | `Command-A` | Select all clips on unlocked tracks |
| ☑ | `]` | Select clips intersecting the mouse position and all clips to its right on unlocked tracks (pointer over timeline) |
| ☐ | `Command-B` | Split at the playhead: the selected clips, or every clip crossing the playhead when nothing is selected. Clips on locked tracks are never split. |
| ☑ | `Backspace` / `Delete` | Delete the selected clips |
| ☑ | `Command-D` | Duplicate the selected clips |
| ☑ | `Command-C` / `Command-X` / `Command-V` | Copy / cut / paste clips |
| ☑ | `Command-Z` / `Command-Shift-Z` | Undo / redo |
| ☑ | `F` / `Esc` | Enter / exit fullscreen preview |
| ☑ | `Option-Command-I` | Toggle the GPUI inspector |
| ☑ | `Option-Command-R` | Reveal the selected entry in Finder |
| ☑ | `Control-Shift-Enter` | Open the selected entry in its default app |

Review: Command-B ignores the selection and always splits every clip crossing
the playhead on unlocked tracks. Left and Right move the editing playhead; they do
not seek the preview player.

## 12. Project data and persistence

**Story:** As a creator, I want my projects to be portable and my session
restored, so that I can move work between machines and pick up where I left
off.

- [x] Each timeline file stores its settings, media metadata, tracks, clips, and
  view state.
- [x] Media paths are relative to the directory containing the timeline file. For
  example, `timelines/edit.timeline.json` references `media/clip.mp4` as
  `../media/clip.mp4`. Absolute media paths are also supported.
- [x] Timelines and media can be moved or backed up together as long as their
  relative locations are preserved.
- [x] Waveforms are not written to the project.
- [x] Global settings, including the last opened project and the MiniMax API key,
  are stored in `~/.opencut/editor-settings.json`.
- [x] Each project keeps its active timeline and panel sizes in
  `.opencut/project.json` and its Explorer state in
  `.opencut/file-explorer.json`.
- [ ] On startup, the editor restores the active timeline and its playhead.

Review: normal restoration is implemented and tested, but empty-timeline
playhead clamping can produce frame index -1. The editor test run had 85 passes
and 2 failures (playhead clamping and the missing waveform fixture); the
timeline library's 8 tests passed.

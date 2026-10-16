# OpenCut Design Spec

This document describes what the OpenCut editor does from a user's point of
view. Each feature is written as a user story followed by acceptance criteria.
The editor is OpenCut's only end-user product; the `player` and `cli` apps are
development tools and are not specified here.

## 1. Projects and media

**Story:** As a creator, I want to open a folder as a project and use the files
already in it, so that I don't have to import or copy media.

- Any folder can be opened as a project.
- Supported media appears in a live file tree (the Explorer) without an import
  step.
- The Explorer can be filtered.
- Selecting a media file previews it in place.
- Media can be dragged from the Explorer onto a compatible track.
- Waveforms are generated in the background and kept in memory only.
- Source media is never rewritten.

Supported file extensions:

| Kind | Extensions | Notes |
| --- | --- | --- |
| Video | `.mp4`, `.mov`, `.m4v`, `.mkv`, `.webm`, `.avi` | |
| Image | `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.bmp`, `.ico` | Added as five-second still clips |
| Audio | `.aac`, `.flac`, `.m4a`, `.mp3`, `.ogg`, `.wav` | |
| Subtitles | `.srt` | Added as text clips |

## 2. Timelines

**Story:** As a creator, I want each edit to be a plain file in my project, so
that I can keep several versions, move them, and back them up like any other
file.

- A timeline is a JSON file named `*.timeline` or `*.timeline.json`, anywhere in
  the project.
- Right-clicking a folder and choosing **Create New Timeline** creates a
  `.timeline` file there. Clicking a timeline file in the Explorer opens it.
- Right-clicking a timeline file and choosing **Settings** changes its frame
  rate. The playhead is rescaled to the new rate.
- A new timeline has no tracks.
- Every committed edit saves the timeline automatically.

## 3. Tracks

**Story:** As a creator, I want separate video, audio, and text tracks that I
can organize, so that I can layer footage, sound, and subtitles.

- Tracks can be created, deleted, reordered, hidden, muted, and locked.
- Video tracks accept video and still images.
- Audio tracks accept audio files.
- Text tracks accept `.srt` files. Dropping one adds one text clip per subtitle,
  offset by the drop position.
- Locked tracks are excluded from selection shortcuts.

## 4. Selecting and editing clips

**Story:** As a creator, I want to select and rearrange many clips at once, so
that restructuring an edit doesn't take one clip at a time.

- Clips are selected by clicking, Command-clicking to add or remove, or drawing
  a selection rectangle.
- Move, duplicate, copy, cut, paste, and delete work on multiple clips, keep
  their relative timing, and are each a single undo step.
- The selection tool moves clips within and between compatible tracks.
- The blade tool splits clips at the playhead.
- Invalid moves are rejected with visible feedback.

## 5. Trimming

**Story:** As a creator, I want to drag a clip's edges to change its duration,
so that I can tighten cuts directly on the timeline.

- Either edge of a video, audio, image, or text clip can be dragged.
- Trimming respects neighboring clips, the source media's length, and a
  one-frame minimum.
- Each drag is one undo step and saves on release.

## 6. Timeline navigation

**Story:** As a creator, I want to zoom, scroll, and snap precisely, so that I
can make frame-accurate edits.

- The timeline is frame-based.
- The timeline supports horizontal zoom, including trackpad pinch, and
  scrolling. Frame ticks appear at high zoom.
- Clicking the ruler moves the playhead.
- Optional snapping (**Snap on** / **Snap off**) aligns to the playhead and clip
  edges.
- Playhead, scroll, zoom, and snapping are saved per timeline.

## 7. Workspace layout

**Story:** As a creator, I want to size the panels to my task, so that I can
give more room to the timeline or the preview as needed.

- The media and properties panels beside the preview can be resized.
- Dragging the divider above the timeline resizes the timeline panel.
- Panel sizes are saved per project.

## 8. Preview

**Story:** As a creator, I want to play the timeline as it will be exported, so
that I can judge the edit without exporting.

- The preview composites video, including each clip's position and scale.
- Audio is mixed and respects track mute, clip mute, and clip gain.
- Each saved edit, including undo and redo, refreshes the preview.
- The preview can be shown fullscreen.

## 9. Subtitles and text

**Story:** As a creator of subtitle-driven videos, I want to generate, place,
and style subtitles, and cut my video around them, so that I can turn a raw
recording into a tight, captioned edit.

### Generate subtitles

- Right-clicking an audio or video file and choosing **Generate SRT** opens a
  transcription window.
- **Start** begins transcription. The window shows the current stage
  (preparing, transcribing, saving) and offers **Cancel**.
- The result is written to `<source name>.srt` in the project root.
- Transcription uses MiniMax. The API key is entered under **Settings** in the
  top bar.

### Position and style subtitles

- Text clips can be selected and dragged directly in the preview.
- With **Snap on**, dragged text aligns to canvas centers and edges, with
  visible guides.
- Right-clicking a text clip and choosing **Apply style to track** copies its
  font, size, color, and position to every other text clip on the same track,
  preserving their words and timing.

### Keep only the spoken parts

- Right-clicking a timeline file and choosing **Keep Text Sections** removes
  time not covered by text clips, across all tracks, and closes the gaps.
- The operation is one undo step.

## 10. Export

**Story:** As a creator, I want to export my timeline to a video file, so that
I can publish it.

- The top-right **Export** button opens a separate export window for the active
  timeline.
- The export uses a snapshot of the timeline taken when it starts.
- The output format is MP4.
- The window shows progress and provides **Stop export**.
- The window cannot be closed while an export is running.

## 11. Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| `Space` | Play or pause the preview |
| `Left` / `Right` | Move the playhead one frame |
| `V` / `B` | Selection / blade tool |
| `Command-click` | Add or remove a clip from the selection |
| `Command-A` | Select all clips on unlocked tracks |
| `]` | Select clips intersecting the mouse position and all clips to its right on unlocked tracks (pointer over timeline) |
| `Command-B` | Split the selected clips at the playhead |
| `Backspace` / `Delete` | Delete the selected clips |
| `Command-D` | Duplicate the selected clips |
| `Command-C` / `Command-X` / `Command-V` | Copy / cut / paste clips |
| `Command-Z` / `Command-Shift-Z` | Undo / redo |
| `F` / `Esc` | Enter / exit fullscreen preview |
| `Option-Command-I` | Toggle the GPUI inspector |
| `Option-Command-R` | Reveal the selected entry in Finder |
| `Control-Shift-Enter` | Open the selected entry in its default app |

## 12. Project data and persistence

**Story:** As a creator, I want my projects to be portable and my session
restored, so that I can move work between machines and pick up where I left
off.

- Each timeline file stores its settings, media metadata, tracks, clips, and
  view state.
- Media paths are relative to the directory containing the timeline file. For
  example, `timelines/edit.timeline.json` references `media/clip.mp4` as
  `../media/clip.mp4`. Absolute media paths are also supported.
- Timelines and media can be moved or backed up together as long as their
  relative locations are preserved.
- Waveforms are not written to the project.
- Global settings, including the last opened project and the MiniMax API key,
  are stored in `~/.opencut/editor-settings.json`.
- Each project keeps its active timeline and panel sizes in
  `.opencut/project.json` and its Explorer state in
  `.opencut/file-explorer.json`.
- On startup, the editor restores the active timeline and its playhead.

## Known limitations

- Clips dropped from the Explorer are not reflected in the preview until the
  next edit.

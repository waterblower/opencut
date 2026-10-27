# OpenCut Design Spec

- [x] Implemented
- [ ] Not Implemented

## 1. Projects and media

**Story:** As a creator, I want to open a folder as a project and use the files
already in it, so that I don't have to import or copy media.

Mockups:
[project picker](pages/01-project-picker.html),
[file Explorer](pages/21-file-explorer.html),
[media preview](pages/16-preview-panel.html),
[rename dialog](pages/07-dialogs.html),
[context menus](pages/11-context-menus.html).

- [x] Any folder can be opened as a project.
- [x] Supported media appears in a live file tree (the Explorer) without an import
      step.
- [x] The Explorer can be filtered.
- [x] Selecting a media file previews it in place.
- [x] Video, audio, image, and SRT entries use an open-hand cursor and can start
      a drag. Timeline files, folders, the project root, and unsupported files use
      the standard arrow. Both variants have a full-row hover highlight.
- [x] Opening a file context menu retains the source row's hover highlight until
      dismissal, independently of selection and the active timeline. Opening the
      menu does not select the entry or change the active timeline. After dismissal,
      normal hover, selection, and active-timeline styling apply.
- [ ] Media can be dragged from the Explorer onto a compatible track.
- [ ] Waveforms are generated in the background and kept in memory only.
- [x] Source media is never rewritten.

Supported file extensions:

| Status | Kind      | Extensions                                               | Notes                                                                                                                                    |
| ------ | --------- | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| ☑      | Video     | `.mp4`, `.mov`, `.m4v`, `.mkv`, `.webm`, `.avi`          |                                                                                                                                          |
| ☐      | Image     | `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.bmp`, `.ico` | Preview and drag affordance exist; track placement remains unresolved. The earlier proposal was five-second still clips on image tracks. |
| ☑      | Audio     | `.aac`, `.flac`, `.m4a`, `.mp3`, `.ogg`, `.wav`          |                                                                                                                                          |
| ☑      | Subtitles | `.srt`                                                   | Added as text clips                                                                                                                      |

## 2. Timelines

**Story:** As a creator, I want each edit to be a plain file in my project, so
that I can keep several versions, move them, and back them up like any other
file.

Mockups: [new timeline dialog](pages/07-dialogs.html),
[timeline settings dialog](pages/07-dialogs.html),
[context menus](pages/11-context-menus.html).

- [x] A timeline is a JSON file named `*.timeline` or `*.timeline.json`, anywhere in
      the project.
- [x] Right-clicking a folder and choosing **Create New Timeline** creates a
      `.timeline` file there. Clicking a timeline file in the Explorer opens it.
- [ ] Right-clicking a timeline file and choosing **Settings** changes its frame
      rate. The playhead is rescaled to the new rate.
- [x] A new timeline has no tracks.
- [x] Every committed edit saves the timeline automatically.

## 3. Tracks

**Story:** As a creator, I want separate video, image, audio, and text tracks
that I can organize, so that I can layer footage, stills, sound, and subtitles.

Mockup: [timeline editing area](pages/17-timeline-editing-area.html).

- [ ] Tracks can be created, deleted, reordered, hidden, muted, and locked.
- [x] Video tracks accept video files only.
- [ ] Image tracks accept still images.
- [x] Audio tracks accept audio files.
- [x] Text tracks accept `.srt` files. Dropping one adds one text clip per subtitle,
      offset by the drop position.
- [x] Locked tracks are excluded from selection shortcuts.
- [x] A locked track is covered by neutral diagonal stripes over its header and lane,
      with a stronger stripe overlay on its clips. Clip colors and the amber lock
      toggle are otherwise unchanged. Red hatching stays reserved for rejected
      placements.
- [x] Each track header shows its kind badge and name, reorder arrows, and delete
      action. Long names truncate before the actions. The first track's up arrow
      and the last track's down arrow are disabled.
- [x] Lock, visibility, and mute controls use accent styling when active; hidden
      and muted states use crossed-out icons. Text tracks have no speaker toggle.
      Toggle hover brightens the icon on a gray fill; delete hover turns the icon red.

## 4. Selecting and editing clips

**Story:** As a creator, I want to select and rearrange many clips at once, so
that restructuring an edit doesn't take one clip at a time.

Mockups: [timeline editing area](pages/17-timeline-editing-area.html),
[context menus](pages/11-context-menus.html).

- [x] Clips are selected by clicking, Command-clicking to add or remove, or drawing
      a selection rectangle.
- [x] Move, duplicate, copy, cut, paste, and delete work on multiple clips and
      keep their relative timing. Each operation that changes the timeline is a
      single undo step.
- [x] The selection tool moves clips within and between compatible tracks.
- [x] The blade tool splits clips at the blade guide, the red line that follows the pointer.
- [x] Invalid moves are rejected with visible feedback.

### Clip placement

Moving an existing clip and dropping an Explorer asset use the same destination
preview, as shown in the **Clip Placement** section of the timeline design.

- [x] A valid placement shows an amber translucent preview and a closed-hand
      drag cursor, with the clip name and **Release to place · <kind>**.
- [ ] With Snap enabled, placements can align to clip edges or the playhead and
      show a blue snap guide for both moves and Explorer drops, including subtitles.
- [x] Incompatible tracks, locked tracks, and overlaps show a dark red preview
      with diagonal hatching, a red border, a blocked cursor, and a rejection reason.
      Invalid placements do not show a snap guide; releasing them makes no edit.
- [x] Feedback updates when the destination changes. Moving clips dim at their
      original positions until release; Explorer drops have no original to dim.

## 5. Trimming

**Story:** As a creator, I want to drag a clip's edges to change its duration,
so that I can tighten cuts directly on the timeline.

Mockup: [timeline editing area](pages/17-timeline-editing-area.html).

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

Mockup: [timeline editing area](pages/17-timeline-editing-area.html).

- [x] The timeline is frame-based.
- [x] The timeline supports horizontal zoom, including trackpad pinch, and
      scrolling. Frame ticks appear at high zoom.
- [x] Clicking the ruler moves the playhead.
- [x] The **Snap** toggle indicates enabled/disabled state and aligns supported
      operations to the playhead and clip edges. SRT drop coverage is tracked under
      [Clip placement](#clip-placement).
- [ ] The timeline toolbar, ruler, tracks, and timeline preview agree on one
      playhead, including after keyboard stepping.
- [x] The toolbar provides a **Magnet** toggle. Its current behavior closes gaps
      on affected tracks when deleting clips; the mockup shows the control but does
      not explain that behavior.
- [ ] Playhead, scroll, zoom, and snapping are saved per timeline.

## 7. Workspace layout

**Story:** As a creator, I want to size the panels to my task, so that I can
give more room to the timeline or the preview as needed.

Mockups: [timeline editing area](pages/17-timeline-editing-area.html),
[top bar](pages/20-top-bar.html),
[property panel](pages/15-property-panel.html).

- [x] The media and property panels beside the preview can be resized.
- [x] Dragging the divider above the timeline resizes the timeline editing area.
- [x] Panel sizes are saved per project.

### Top bar

- [x] A fixed 64-point top bar shows the app icon to the left of **OpenCut**,
      followed by the timeline filename (or **No timeline**) and autosave label.
- [x] **Settings**, **Open Folder**, and **Projects** are always available.
      **Export** is enabled only when a timeline is open.
- [ ] Actions match the cyberpunk design: cyan secondary buttons, a neon-yellow
      primary Export button, cut corners, primary hazard slashes, and the specified
      label typography. Disabled Export uses the dimmed variant.

Review: the icon, actions, button shapes, colors, and disabled state are present;
label weight and spacing still differ. The two top-bar cases are **Timeline open**
and **No timeline**. Button interaction variants are defined in the
[design system](pages/00-design-system.html).

## 8. Preview

**Story:** As a creator, I want to play the timeline as it will be exported, so
that I can judge the edit without exporting.

Mockups: [preview variants](pages/16-preview-panel.html),
[video player](pages/18-video-player.html),
[fullscreen preview](pages/12-fullscreen-preview.html).

The preview design has five states: image, audio, video, error, and none.
Video covers media playback and rendered timelines. A single error state serves
all preview sources: it shows which selected file failed and an error string.
The error string preserves line breaks, wraps long lines, and scrolls when it
exceeds the available space.

- [x] The preview composites video, including each clip's position and scale.
- [ ] Audio is mixed and respects track mute, clip mute, and clip gain.
- [ ] Each saved edit, including undo and redo, refreshes the preview.
- [x] The preview can be shown fullscreen.
- [ ] Selecting a file with no preview clears the previous preview and shows the
      None state: illustration, **No preview available**, and guidance to select an
      image, audio, video, or timeline file.
- [ ] Every source type uses the shared error state when opening fails.
- [ ] Audio preview shows a centered music icon, filename and path, with playback
      controls and time below the content.
- [x] Video and timeline playback controls sit below the picture. A 24-point
      circular button shows Play, Pause, or Replay; the filename truncates before
      the time or button shrinks. Before a video frame is decoded, its picture is black.
- [ ] Scrub-bar hover thickens the track, reveals a knob, and previews the seek
      target with a light fill. Dragging updates the picture and time.

## 9. Subtitles and text

**Story:** As a creator of subtitle-driven videos, I want to generate, place,
and style subtitles, and cut my video around them, so that I can turn a raw
recording into a tight, captioned edit.

### Generate subtitles

Mockups: [transcription window](pages/14-transcription-window.html),
[settings dialog](pages/07-dialogs.html),
[context menus](pages/11-context-menus.html).

- [x] Right-clicking an audio or video file and choosing **Generate SRT** opens a
      transcription window.
- [x] Right-clicking a video or audio clip and choosing **Transcribe the Clip**
      opens transcription for that clip's source in/out range. The ready state names
      the file and range rather than only saying **Ready**.
- [x] Before starting, the window shows the audio duration, estimated MiniMax
      cost in CNY, and a **MiniMax pricing** link that opens the provider's price list.
- [x] **Start** begins transcription. The window shows the current stage
      (preparing, transcribing, saving) and elapsed time; Start is disabled while running.
- [ ] The Running design offers **Cancel** across its preparing/transcribing/saving
      states. It explains that cancellation stops the local request and a submitted
      request may continue on the server.
- [x] Whole-file transcription writes `<source stem>.srt` in the project root.
      Clip transcription writes `<source stem>_<clip id>.srt`, with subtitle timestamps
      relative to the beginning of the extracted clip range.
- [x] Completion shows the output path and **Reveal in Finder**. Failure shows
      the request error. Start and Cancel use the cyberpunk button variants.
- [x] Transcription uses MiniMax. The API key is entered under **Settings** in the
      top bar.

### Position and style subtitles

Mockups: [text positioning](pages/05-editor-text-positioning.html),
[context menus](pages/11-context-menus.html).

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

Mockup: [export window](pages/13-export-window.html).

- [x] The top-right **Export** button opens a separate export window for the active
      timeline.
- [x] The export uses a snapshot of the timeline taken when the export window
      opens.
- [x] The output format is MP4.
- [x] The initial video bitrate is **2 Mbps**. Users can enter a value from 0.1
      through 1000 Mbps; an invalid value shows an explanation and disables export.
- [x] The window shows progress and provides **Stop export**.
- [x] The window cannot be closed while an export is running.

## 11. Keyboard shortcuts

| Status | Shortcut                                | Action                                                                                                                                                                       |
| ------ | --------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ☑      | `Space`                                 | Play or pause the preview                                                                                                                                                    |
| ☑      | `Left` / `Right`                        | Move the playhead one frame                                                                                                                                                  |
| ☑      | `V` / `B`                               | Selection / blade tool                                                                                                                                                       |
| ☑      | `Command-click`                         | Add or remove a clip from the selection                                                                                                                                      |
| ☑      | `Command-A`                             | Select all clips on unlocked tracks                                                                                                                                          |
| ☑      | `]`                                     | Select clips intersecting the mouse position and all clips to its right on unlocked tracks (pointer over timeline)                                                           |
| ☐      | `Command-B`                             | Split every clip crossing the cut point: the playhead with the selection tool, the blade guide (mouse position) with the blade tool. Clips on locked tracks are never split. |
| ☑      | `Backspace` / `Delete`                  | Delete the selected clips                                                                                                                                                    |
| ☑      | `Command-D`                             | Duplicate the selected clips                                                                                                                                                 |
| ☑      | `Command-C` / `Command-X` / `Command-V` | Copy / cut / paste clips                                                                                                                                                     |
| ☑      | `Command-Z` / `Command-Shift-Z`         | Undo / redo                                                                                                                                                                  |
| ☑      | `F` / `Esc`                             | Enter / exit fullscreen preview                                                                                                                                              |
| ☑      | `Option-Command-I`                      | Toggle the GPUI inspector                                                                                                                                                    |
| ☑      | `Option-Command-R`                      | Reveal the selected entry in Finder                                                                                                                                          |
| ☑      | `Control-Shift-Enter`                   | Open the selected entry in its default app                                                                                                                                   |

Review: Command-B ignores the selection and splits every clip crossing the cut
point on unlocked tracks. With the selection tool the cut point is the playhead;
with the blade tool it is the blade guide at the mouse position (nothing happens
until the pointer has moved over the timeline). Left and Right move the editing
playhead; they do not seek the preview player.

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
timeline library's 8 tests passed. These are the 2026-10-16 review results.

## 13. Property Panel

**Story:** As a creator, I want properties for the selected object so that I can
inspect media and adjust a clip without leaving the editor.

Mockups: [property panel](pages/15-property-panel.html),
[text clip panel](pages/15-property-panel-text-clip.html).

- [x] The header identifies the selected object. Panels cover text, video, and
      audio clips; timeline, video, audio, image, and subtitle files; and no selection.
- [ ] Text clips expose editable content, duration, font, font size, color, and
      normalized X/Y position. Timeline start remains read-only.
- [ ] Video clips expose editable X/Y position and scale, including a scale
      slider and **Reset** action. Start, duration, source in, and source out are read-only.
- [ ] Audio clips expose editable gain and a **Mute clip** toggle. Timing is read-only.
- [x] Timeline properties show duration, format, clip/track counts, and location.
      Media-file panels show file details and location; subtitle files show parsed cues
      and guidance for adding them to a text track.

# Appendix

1. able to apply all properties to other clips in the same track, not just the transforms as positions.

2. when medias are missing when loading a timeline, the program should not exit. The timeline clips should be properly displayed with missing clips indicated.
   Right click on the missing clips, the user is able to open a missing media dialog like in [./pages/19-missing-media-dialog.tsx].

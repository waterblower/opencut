# Pull-based decoding and application-owned playback

## Objective and boundaries

The application pulls native frames, prepares their presentation, schedules them,
and updates GPUI. Media backends own decoding resources, not playback clocks,
presentation threads, UI notifications, or the currently displayed frame.

Preserve source-video/audio playback, volume/mute, seeking, EOF behavior, timeline
still previews, and frame stepping. Preserve CLI output and existing project-file
formats. This refactor does not add timeline playback/audio mixing, new GPU color
conversion, or new platform-specific hardware implementations. Reuse available
macOS VideoToolbox support and retain software decoding elsewhere.

This replaces the implementation direction in `timeline-decode-acceleration-plan.md`.
Its hardware acceleration and intermediate-frame skipping objectives remain.
No implementation is authorized by creating this plan.

## Interfaces and ownership

### Proposed API pseudocode

The user's revised loop makes audio the master: pull samples into the device
queue, use audio playback time to decide when video is due, and asynchronously
wait for the next useful operation. The application-facing media backend exposes
both `next_samples()` and `next_frame()`; neither owns playback scheduling.

```rust
let t = Instant::now();
while true {
    let samples = video_backend.next_samples();
    push_to_audio_device(samples);
    let is_time_to_push_video = compute_video_frame_time();
    if is_time_to_push_video {
        let frame = video_backend.next_frame(); // Native FFmpeg frame type.
        let rgba = convert_to_image(frame);
        editor.set_current_preview_frame(rgba); // GPUI entity update.
    }
    let wait = compute_time_to_wait();
    await sleep(wait);
}
```

This is conceptual pseudocode, not the final Rust implementation. The detailed
design below retains this application-owned flow while running blocking work off
the UI thread and handling commands, errors, and EOF. The master is the device's
played media position, not samples enqueued or elapsed time since `t`. Video is
prepared ahead of its deadline; a due frame is published without starting a slow
decode at that deadline. Audio refill continues while video work is outstanding,
so this conceptual loop must not become a serial blocking audio/video loop.
Waits are interruptible and driven by output capacity, frame deadlines, commands,
or work completion; duration subtraction saturates at zero.

### Refined concurrent API pseudocode

The agreed refinement uses independent preparation tasks sharing one audio
presentation clock. Retain the original sketch above as motivation; this is the
execution structure to implement. The session controller starts these tasks
concurrently, rather than awaiting the audio loop before starting video.

```rust
// Audio refill task.
while let Some(samples) = audio_decoder.next_samples().await? {
    audio_output.enqueue(samples).await?; // Bounded queue; waits for space.
}

// Concurrent video task.
while let Some(frame) = video_decoder.next_frame().await? {
    let pts = frame.timestamp;
    let image = convert_to_image(frame).await?;

    wait_until_audio_reaches(&audio_output, pts).await?;
    editor.update_preview(image); // GPUI entity update.
}
```

`audio_output.played_position()` exposes the shared media clock. The enqueue
future applies backpressure without blocking the video task or the device callback.
The wait helper belongs to the application scheduling layer and rechecks the clock
after waking; it does not create an independent video clock. The pseudocode omits
late-frame selection and control handling, which remain required below. Decoder
and conversion futures offload blocking work to their owning execution lanes.

### Pull decoding

- A synchronous `VideoDecoder::open(path)` exposes metadata and
  `next_frame(&mut self) -> Result<Option<DecodedVideoFrame>>`. `None` means fully
  drained EOF, never merely that the decoder needs another packet. The method
  internally pumps packets and handles decoder buffering/reordering.
- `DecodedVideoFrame` owns an `ffmpeg_next::frame::Video` plus normalized media
  presentation time and optional duration. Preserve color and rotation metadata;
  normalize timestamps consistently with the existing stream-origin behavior.
- `AudioDecoder::next_samples(&mut self) -> Result<Option<DecodedAudioSamples>>`
  produces an owned PCM block with normalized starting media time, sample rate,
  channel layout, and frame count, resampled to the selected output configuration.
  `None` means drained audio EOF. Audio and video normalize against a common media
  origin, preserving stream offsets. Audio seek flushes decoder/resampler state
  and trims leading samples to the requested position.
- The application-facing media backend offers awaited `next_frame()` and
  `next_samples()` operations through independent video/audio handles. Each wraps
  a synchronous pull decoder on its blocking lane; they can run concurrently and
  contain no autonomous producer loops. Use independent demux contexts, as the
  current playback implementation does, so one stream cannot block the other.
- `seek(&mut self, position) -> Result<DecodedVideoFrame>` locates the nearest
  presentation frame using frames bracketing the target, with the earlier frame
  winning ties, and leaves sequential reading ready to continue after that frame.
  Retain any decoded lookahead needed to avoid skipping the following frame.
  Clamp negative positions to zero and use the last frame when seeking past EOF;
  media with no decodable frame returns an error.
- A separate conversion component prepares a selected frame for display. Maintain
  decoder output format until selection: intermediate frames are not converted to
  RGBA, copied into image buffers, or rotated. Reuse the existing native-surface
  presentation path where supported; keep RGBA/BGRA preparation for timeline and
  software image presentation. Reconfigure conversion resources when the actual
  frame format, dimensions, or color metadata change.
- Timeline decoding exposes `frame_at(document, position) -> Result<TimelineFrame>`.
  The application supplies an immutable document snapshot and exact timeline
  frame. The decoder resolves visible active clips, maps source positions, and
  returns ordered layers. It owns reusable media readers/caches, not the editable
  document, playhead, worker, or displayed frame. Track stacking stays unchanged.

### Blocking execution and application state

- One application-owned session controller coordinates commands and the concurrent
  audio-refill and video-presentation tasks. Both belong to the same session and
  request revision; they do not independently choose playback position or state.
  Share its implementation between editor and debug
  player; keep it outside the decoder modules. GPUI entities contain display/control
  state and a session handle, never live FFmpeg contexts.
- Use one serial blocking execution lane for video and one for audio when present,
  each running on a background worker. Construct/use/drop native decoder and
  conversion resources on their owning lane;
  do not assume every FFmpeg or platform object is `Send`, or add unsafe `Send`
  implementations. Commands carry owned inputs and return owned results through
  awaited replies. Only cross-thread-safe prepared results leave the lane.
- This lane is an execution adapter, not a playback scheduler: no clock, sleeps,
  spontaneous decoding, polling loop, or presentation policy. It waits for work.
  Allow one active video operation and at most one prefetched frame; retain only
  the newest pending seek. Avoid per-frame OS thread creation and unbounded queues.
- Commands cover play, pause, seek/step, volume/mute, document replacement, and stop.
  Application state distinguishes requested position from displayed-frame time.
  Step updates the timeline playhead immediately and requests a paused preview.
  Scrubbing remains paused; source-file seeks preserve their existing resume rules.
- Use a session identity and request revision in work/results. Drop stale results
  after a seek, edit, source switch, or window close. Cancelling an await does not
  cancel a running FFmpeg call; retire the lane after that operation returns and
  dispose resources there, never join a decoding thread from GPUI.
- Business logic propagates errors. The application task logs once with debug
  formatting and updates UI status. Preserve the last good frame during loading
  or failure. Closed entities terminate publication cleanly.

### Timing and audio

- Audio is the master whenever an audio stream is active, including muted playback.
  Compare the next video frame's normalized PTS against the device's played media
  position. Enqueuing PCM does not advance that position; actual device consumption
  and presentation latency determine it. Never gate audio refill on video completion.
- Both streams use the same normalized media origin. Derive played position from
  timestamped PCM blocks, consumed sample frames, and device presentation timing;
  account for latency exactly once. The audio output provides a thread-safe clock
  snapshot plus asynchronous progress/state notification. The application may
  estimate a wake deadline from that snapshot, but must recheck after waking.
  Pause and underrun freeze media advancement; video waits for clock/control changes
  rather than spinning. Ahead-of-clock frames wait; due frames display; obsolete
  frames are discarded to catch up without slowing the audio clock.
- Seek is coordinated by the session controller: advance the request revision,
  suspend presentation/output, invalidate queued old audio and pending video,
  reposition both decoders, and prime new audio plus the target video frame. Reset
  the audio media-time anchor to the target and resume only if playback was active.
  Reject old-revision samples at enqueue/consumption and frames at UI publication.
  A paused seek publishes its target frame without waiting for the stopped clock.
  A newer seek supersedes this preparation; either stream's failure stops the
  coordinated restart and is reported by the controller.
- Silent video uses an application-owned monotonic clock with a media-time anchor.
  Frame deadlines derive from normalized PTS, not repeated `frame_time - elapsed`
  sleeps. Pause/resume and seek reset anchors without accumulating drift.
- Pull/prepare a frame ahead, then await its absolute presentation deadline before
  publishing it through an entity update. Waiting must be interruptible by control
  commands. Use timestamps for variable-rate video; retain existing handling for
  unusable timestamps rather than silently inventing frame cadence.
- If late, advance toward the clock position, discarding obsolete presentation
  candidates before expensive conversion where possible. Still decode reference
  dependencies. Do not reset the clock to hide decode slowness or drop frames during
  paused stepping. Bound catch-up work and check commands between decoder calls.
- Separate pull audio decoding/resampling from device output. The device callback
  only consumes prepared PCM and reports playback progress; it performs no media
  I/O, decoding, blocking waits, or application callbacks.
- With active audio, use the audio output's played media position, adjusted for
  device latency, as presentation authority. Keep PCM buffering bounded (reuse the
  existing output capacity initially), with an independent application refill task
  so video decoding cannot starve audio. Seek invalidates old audio blocks and
  resets sample position; underrun outputs silence without counting that silence
  as consumed media. Pause retains position. EOF completes only after queued media
  audio drains; video-only EOF retains the last frame. If audio ends before video,
  switch to a monotonic clock anchored at the final played audio position once its
  queue drains, preserving continuity. If video ends first, retain its last frame
  and continue audio to completion. Device errors stop playback and are reported
  at the application boundary rather than silently switching clocks.

### Hardware and skipping policy

- Prefer the existing VideoToolbox path for supported 8-bit H.264/HEVC MP4/MOV.
  Unsupported inputs or unavailable hardware initialization use software. A runtime
  hardware failure retries the requested position once through a fresh software
  decoder; software failure propagates. Expose the selected backend for diagnostics.
- Reuse existing target-aware non-reference skipping and output suppression where
  correctness is established. Preserve target/bracketing frames and dependencies.
  Do not globally set `AVDISCARD_NONREF`, which can remove requested frames. The
  software path keeps full decoding unless a safe skip decision can be proven;
  it still avoids conversion of discarded frames. No quality-reducing scrub mode.

## Progress and review checkpoints

0/9 active steps complete. Current step: none; awaiting implementation instruction.
Blockers: none. After each completed step, update this checklist, graph, evidence,
and progress summary, then pause for review unless the user waives checkpoints.
No new tests without an explicit request; migrate and run existing tests.

Direction update: the user's audio-master loop replaces the initial video-only
pseudocode and is refined into concurrent audio/video tasks sharing the device's
played-position clock. P1, P4, and P5 include `next_samples()`, bounded async enqueue,
clock progress reporting, and coordinated seek resets. All implementation steps remain
pending; none were completed or superseded by this clarification.

- [ ] **P1 — Establish contracts and ownership** (pending; no dependencies).
  Introduce the native-frame, metadata, command/result, and application session
  interfaces above. Map existing callers to their replacement interfaces and
  separate timeline document/playhead state from decoder state. Start with the
  high-level control flow; bodies may remain incomplete at this checkpoint.
  Evidence: interface/caller review shows a single owner for each resource and
  no playback timing or UI state in decoder contracts.
- [ ] **P2 — Extract the pull video decoder** (pending; depends on P1).
  Share existing decompression across playback and engine feature configurations.
  Implement native-frame next/seek/EOF, hardware selection/fallback, and safe
  target-aware skipping. Keep old callers working through temporary adapters.
  Evidence: existing applicable decode/seek fixtures pass, covering reordering,
  backward seeks, EOF, and software inputs; editor/CLI/player configurations compile.
- [ ] **P3 — Separate frame conversion and presentation** (pending; depends on P2).
  Move color conversion, rotation, and image preparation behind explicit selected-
  frame operations. Change the video element to accept a prepared frame snapshot
  rather than querying an autonomous backend. Preserve native-surface rendering.
  Evidence: existing image/preview checks pass; review confirms intermediate frames
  do not undergo RGBA conversion and rendering performs no decoding.
- [ ] **P4 — Separate audio decoding and device output** (pending; depends on P1).
  Expose `next_samples()`, bounded async `enqueue()`, `played_position()`, and
  progress/state notifications;
  remove dependence on the old video backend's shared clock/control state. Preserve
  volume, mute, sample rate, channels, seek reset, underrun, and EOF behavior.
  Evidence: existing audio checks pass; callback review confirms no blocking work.
- [ ] **P5 — Implement the application playback task** (pending; depends on P3, P4).
  Implement the session controller with concurrent audio/video tasks, independent
  serial execution lanes, one audio-master clock, coordinated seek resets,
  interruptible waits, and audio refill,
  command processing, bounded lookahead, stale-result rejection, and background
  resource retirement. Instrument decode, conversion, scheduling lateness, and
  publication separately. Evidence: code review covers pause during decode, rapid
  seeks, source replacement, EOF, and errors without UI waits or busy polling.
- [ ] **P6 — Migrate editor source previews** (pending; depends on P5).
  Route video/audio selection and controls through the application session. Entity
  updates publish prepared frames and playback status. Keep cheap control-state
  updates synchronous; render reads state only. Evidence: manually verify source
  switching, play/pause, seek, volume/mute, audio-only files, and close during decode.
- [ ] **P7 — Migrate timeline preview and stepping** (pending; depends on P5).
  Replace TimelineBackend's worker/requested-frame publication with pull frame_at
  operations driven by the application. Keep editable state and playhead in timeline
  runtime state. Carry document snapshots/revisions in requests. Preserve native
  frame layers until presentation preparation; update persistence mappings without
  changing file formats. Evidence: existing timeline/preview/persistence checks pass;
  manually verify arrow stepping, rapid scrubbing, edits while decoding, text layers,
  source-preview-to-timeline transitions, and hidden tracks.
- [ ] **P8 — Migrate remaining callers and remove old scheduling** (pending; depends on P6, P7).
  Migrate debug player to the shared application controller and CLI image reads to
  the synchronous pull decoder/converter. Remove temporary adapters and unused
  presenter threads, polling, duplicated clocks, old backend snapshot APIs, and
  worker-joining UI destructors. Leave unrelated experimental examples alone unless
  required to compile. Evidence: caller search finds no production use of obsolete
  scheduling APIs; affected feature configurations compile. No player tests added.
- [ ] **P9 — Validate behavior and performance** (pending; depends on P8).
  Run existing affected suites with vendored FFmpeg. Compare identical seek sequences
  on the reported media before/after, recording actual selected decode backend,
  conversion counts, stage timings, and memory bounds. Manually verify variable-rate
  video, software fallback, A/V synchronization, long playback without clock drift,
  pause/resume, repeated/backward seeks, EOF, and app closure. Record unverified cases
  explicitly; no claimed speedup without measurements. Evidence: final validation
  record plus reviewed diff, with no FFmpeg build or new test code.

P2 and P4 are independent after P1; P3 can proceed alongside P4. P6 and P7 are
independent after P5. This describes dependency order, not permission to spawn agents.

```mermaid
graph TD
    P1["P1: Contracts — pending"] --> P2["P2: Pull video — pending"]
    P1 --> P4["P4: Audio — pending"]
    P2 --> P3["P3: Conversion — pending"]
    P3 --> P5["P5: Application driver — pending"]
    P4 --> P5
    P5 --> P6["P6: Source previews — pending"]
    P5 --> P7["P7: Timeline previews — pending"]
    P6 --> P8["P8: Remaining callers and cleanup — pending"]
    P7 --> P8
    P8 --> P9["P9: Validation — pending"]
```

---
name: opencut-architecture
description: Architecture rules for OpenCut. Use when designing, implementing, or reviewing state ownership, editor events, panel view models, execution models, API boundaries, or mutations.
---

# OpenCut architecture

Apply these guidelines in the OpenCut repository. Paths are relative to the repository root.

Also apply the repository [general rules](../opencut-general-rules/SKILL.md).

## State design: derive values instead of duplicating state

- If a value can always be derived from existing authoritative data and has no
  independent reason to change, **never store it as a separate state field**.
  Compute it with a pure function, a read-only accessor, or a local expression.
  Reuse across multiple methods is not a reason to store a derived value.
- Duplicate mutable state admits inconsistent combinations. As the code grows,
  the potential state space grows combinatorially, and every update, seek, reset,
  and error path must maintain more synchronization invariants. Missed updates
  create unexpected bugs. Remove the duplicated state rather than adding more
  synchronization code to keep copies consistent.
- Before adding a field, identify the independent information it represents and
  what can legitimately change it. If its only updates mirror another value,
  derive it instead. Keep authoritative state in its actual owner; callers query
  it rather than maintaining their own copies or flags.
- For example, expose `duration()` from existing media metadata; compute a
  displayed frame's end as its PTS plus its duration; query decoder completion
  rather than storing another EOF flag in the player. Completion must include
  decoder and resampler draining, while device playback completion is a separate
  fact. Preserve genuinely distinct information such as a frame's PTS, its
  duration, and the playback clock; do not conflate them merely because their
  types are identical.
- Do not introduce external state or extra stored state when pure computation
  can solve the problem. In existing message-based flows, carry the required
  data in messages rather than adding fields to a broader owner solely so event
  handlers can access it. This is not a reason to introduce message passing.

## Elm-style architecture: user actions as events, panels as view models

The editor is moving toward an Elm architecture. Apply it to new and touched
user actions; migrate the existing code incrementally.

- Express a user action as one `AppEvent` that describes what happened, such as
  `TextClipSelected { clip_id }` or `VideoFileSelected { path, size_bytes }`,
  not an imperative command such as "refresh the panel". The event carries the
  data that is known at the call site and that the handler needs.
- One user action emits one event. Do not split an action into several events
  whose handlers must run in a particular order.
- The event handler is the update step. It owns the consequences of the action:
  modifying `Editor` state, performing the necessary IO (for example, reading
  and parsing an SRT file), and rebuilding the affected view models. Errors are
  propagated or logged there, at the event boundary.
- Model each panel's view model as an enum with one variant per kind of content.
  Each variant wraps a dedicated `XxxPanel` struct that holds everything the UI
  displays, as owned data. For example, `PropertiesPanel::TextClip(TextClipPanel)`.
  The view model is stored on `Editor` and replaced by event handlers.
- Render functions take only their `XxxPanel`, such as
  `fn text_clip_view(panel: &TextClipPanel)`. They must not take `&Editor`,
  derive data from other state, or perform IO.
- A stored view model is a deliberate snapshot for rendering and is the one
  exception to deriving values instead of storing them. Rebuild it from event
  handlers; do not add per-call-site synchronization code to keep it current.
- Events are handled asynchronously, after the UI callback returns. State that
  the callback itself needs immediately (for example, the selection that a
  drag starts from) is still set synchronously in the callback. Never place
  code after an emit that depends on that event's effects.
- Non-user mutations, such as playback updates and periodic file-tree scans,
  are not routed through `AppEvent` for now. Leave playback as it is.

## Execution and configuration boundaries

- Read environment variables only in `main()` or application initialization code.
  Pass the required values explicitly to business logic functions.
- First make synchronous code fast and measure its cost at the call site.
  Synchronous execution exposes performance problems; do not hide them behind
  concurrency. Do not add workers, channels, request/reply protocols, or future
  wrappers unless the user's chosen architecture calls for them.
- Perform quick synchronous work, such as small UI state changes and cheap
  backend commands, directly in UI callbacks without emitting events. User
  actions that change what a panel displays are the exception: follow the
  Elm-style architecture section and emit an event. When
  asynchronous execution is part of the authorized architecture, use event
  handlers to coordinate it and run CPU-heavy work on background workers;
  emitting an event alone does not move work off the UI thread. An explicitly
  synchronous implementation remains synchronous even when its cost blocks the
  UI; optimize that cost before changing the execution model.

## Ownership and API boundaries

- Never pass functions or closures as arguments to simple functions. Pass the
  required values or references directly. If a callback is truly needed for a
  simple function, ask the user first.
- State should live in the narrowest scope that needs it. Prefer a local variable
  over a struct field unless the value actually needs to be shared across methods
  or control flows.
- Localize a feature's control flow and implementation where practical. Keep
  related steps together in the owning event handler or module instead of
  scattering them across files and single-use methods. Extract code when it
  provides meaningful reuse or a clear ownership boundary, not merely to shorten
  a handler. When inlining, preserve cleanup and finalization on early-return paths.
- Do not introduce one-line forwarding wrappers around a helper with only one
  production caller. Put the implementation in the entry-point function, or
  call the implementation directly with the data it needs. An extra function
  name and call layer add unnecessary semantic indirection, even when the
  helper itself has many lines. Test-only callers do not justify the wrapper.
- Do not extract a separate function when it has only one caller and its body is
  a single expression or statement. Inline that logic at the call site. A
  read-only accessor that keeps the underlying owner private without duplicating
  state is appropriate, even if its body is a single expression.
- Functions and methods should accept only the data they use. Prefer passing the
  smallest required values over accepting a broader type such as `&self` when
  the function does not depend on the rest of that type's state.
- Prefer a functional style: helpers and lower-level functions should return
  data or proposed state changes instead of mutating `self` or application state.
  Apply mutations as high in the call stack as possible, ideally at the outermost
  application, event, or UI boundary. Pass helpers only the inputs they need and
  let the caller apply their results. When a stateful resource such as a decoder
  or converter requires mutation, limit mutable access to that resource; do not
  use it as a reason to mutate broader application state inside the helper.

## Errors and atomic mutations

- When a function call returns an error, propagate it through intermediate
  functions instead of logging it there. Log the error only at the highest-level
  application, event, or task boundary.
- Business logic must return errors without deciding how they are reported to
  users or rendered in the UI. Keep user-facing error messages, error status
  updates, and UI notifications in the application or event handling layer.
- A method that can fail must leave `self` unchanged when it returns an error.
  Do all fallible work first, keeping the results in local variables, and
  modify `self` only after every fallible step has succeeded. The commit phase
  should contain no `?`. Order external side effects such as saving files last
  among the fallible steps. Achieve this by ordering the code, not by adding
  rollback or recovery logic. Name the locals that will replace existing state
  with a `new_` prefix so the commit phase is easy to read. For example:

  ```rust
  let new_timeline = TimelineRuntimeState::from_serialize(data, path)?;
  let new_preview_target = timeline_preview_target(&new_timeline, &self.project_root, cx)?;
  save_project_local_settings(&self.project_root, &new_settings)?;

  self.timeline = Some(new_timeline);
  self.preview.target = new_preview_target;
  ```

  instead of assigning `self.timeline` first and then failing while building
  the preview, which leaves the editor showing the new timeline with the old
  preview.
- For timeline updates, keep computation and validation read-only and commit
  mutations together at the end, after all fallible operations have succeeded.
  Do not clear or partially update timeline state before computing its replacement.

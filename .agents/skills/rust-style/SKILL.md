---
name: rust-style
description: Rust style conventions for OpenCut. Use when writing, refactoring, or reviewing Rust code for naming, imports, visibility, source organization, and explicit control flow.
---

# OpenCut Rust style

Apply these guidelines in the OpenCut repository. Paths are relative to the repository root.

Also apply the repository [general rules](../general-rules/SKILL.md).

## Naming and source organization

- Use `action_` for GPUI action handlers that accept an action argument and can
  be registered with `.on_action(cx.listener(Self::action_xxx))`.
- Use `ui_callback_` for functions or methods intended to be called only from
  UI callbacks, such as `ui_callback_set_timeline_frame_rate`. A callback may
  capture option-specific values and pass them to this helper. Do not add an
  `action_` wrapper merely to rename a UI callback helper; reserve that prefix
  for GPUI action handlers. General-purpose business logic uses neither prefix.
- Name new functions and methods that return GPUI elements with a `_view`
  suffix, including those returning concrete element types, `AnyElement`, or
  `impl IntoElement`. Apply this convention to future code; do not rename
  existing functions solely to conform unless the user explicitly requests it.
  Required trait method names are exempt.
- Do not use `super::` paths. Import items explicitly using full paths at the
  beginning of the file, and use unqualified names in the code. Use an import
  alias or qualified name only when needed to resolve a name conflict.
- Import anyhow helpers explicitly and use unqualified names such as `anyhow!`,
  `bail!`, `Result`, and `Error`. Use an import alias when a name conflicts with
  another type. Do not change vendored dependency code to enforce this rule.
- Never use `#[serde(rename_all = "snake_case")]`.
- Always use the smallest visibility that works: private, then `pub(super)`,
  then `pub(crate)`, then `pub`. Use `pub` only for items another crate uses.
  This applies to fields, enum variants, and re-exports as well as functions.
- Place private code at the bottom of each file, after public and
  restricted-public (`pub(...)`) code.
- Define a type's public methods and private methods in separate inherent `impl`
  blocks; restricted-public (`pub(...)`) methods count as public. Mark each block
  with a comment directly above it: `// Public APIs only` or
  `// Private APIs only`. Place the private block after the public one. Trait
  implementations stay in their own `impl Trait for Type` blocks. For example:

  ```rust
  // Public APIs only
  impl VideoPlayer {
      pub fn duration(&self) -> Duration { ... }
  }

  // Private APIs only
  impl VideoPlayer {
      fn seek_video(&mut self, position: Duration) -> Result<()> { ... }
  }
  ```
- Explain easily confused state in concise Chinese comments to the right of the
  relevant code, with clear indentation and aligned comment columns. Distinguish
  sources of truth, derived values, time positions versus durations, and decoder
  EOF versus completed playback. Preserve intentional formatting and existing
  `#[rustfmt::skip]` annotations.
- Do not use the bare word "frame" for a frame index. It is ambiguous: it can mean
  the frame data (a decoded or composited picture) or the position of a frame on
  the timeline. Name an index `frame_index` (or `..._frame_index`, with type
  `TimelineFrameIndex`) and frame data `frame` or `picture`, so a name says which one it is.

## Explicit Rust control flow

- Do not define custom macros. Prefer ordinary functions and explicit control
  flow so the code is easy to read. Standard and dependency-provided macros
  (such as `format!` and derives) are allowed.
- Do not shadow variables in the same scope in our code. Give transformed values
  distinct, descriptive names, such as `rotated_pixels` instead of redeclaring
  `pixels`. This applies even when the new value has the same type.
- Do not use `ensure!`. Write an explicit `if` with an early `return Err(...)` (or
  `bail!`) so the failing condition and its error are visible as ordinary control
  flow. Prefer:

  ```rust
  if data.clip_locked(clip_id) {
      return Err(anyhow!("Clip {clip_id} is on a locked track"));
  }
  ```

  over `ensure!(!data.clip_locked(clip_id), "Clip {clip_id} is on a locked track");`.
  This applies to new and touched code; do not rewrite existing `ensure!` calls
  solely to conform.
- Prefer `let ... else` with an early return when required optional state is
  absent, instead of nesting the remaining control flow inside `if let`.
- Prefer explicit `match` for `Option`/`Result` branching over `map`, `map_or`,
  `map_or_else`, or `map_err` when they express alternative control-flow paths,
  even when the branches are short. Make each case and its result visible.
  This preference does not prohibit ordinary iterator transformations. Prefer:

  ```rust
  match self.start_time_of_system {
      Some(start) => self.start_position_of_video + start.elapsed(),
      None => self.start_position_of_video,
  }
  ```

  over `self.start_position_of_video +
  self.start_time_of_system.map_or(Duration::ZERO, |start| start.elapsed())`.
- When a value depends on a condition, bind it with one `match` expression whose
  arms produce the value. Do not initialize a default with `let mut` and then
  overwrite it inside one arm of a later `match` or `if`; that makes the reader
  track a mutable variable and an empty fallback branch. Prefer:

  ```rust
  let position = match timeline.interaction.pointer_guide {
      Some(pointer_position) => pointer_position,
      None => timeline.playhead(),
  };
  ```

  over:

  ```rust
  let mut position = timeline.playhead();
  match timeline.interaction.pointer_guide {
      Some(pointer_position) => position = pointer_position,
      None => {
          // do nothing
      }
  }
  ```
- Prefer an immediately invoked closure with explicit early returns over
  `Option::map` or `Result::map_err` when transforming a value requires multiple
  steps. For example, prefer:

  ```rust
  let timeline = (|| {
      let Some((path, data)) = active_timeline else {
          return None;
      };
      let ges_timeline = match build_timeline(&data) {
          Ok(timeline) => timeline,
          Err(error) => panic!("could not build timeline: {error}"),
      };
      Some(TimelineRuntimeState::new(path, data, ges_timeline))
  })();
  ```

  over:

  ```rust
  let timeline = active_timeline.map(|(path, data)| {
      let ges_timeline = build_timeline(&data).unwrap();
      TimelineRuntimeState::new(path, data, ges_timeline)
  });
  ```

  For results, prefer explicit error conversion inside the closure:

  ```rust
  let value = (|| {
      let value = match load_value() {
          Ok(value) => value,
          Err(error) => return Err(format!("could not load value: {error}")),
      };
      Ok(transform(value))
  })();
  ```

  instead of chaining `map_err` and `map`.
- Always use debug formatting (`{error:?}`) when logging errors.
- Do not use `_` in a `match` or `if let ... else` pattern when it stands for only one
  remaining case. Name that case explicitly, so adding a new case forces the code to
  be revisited.

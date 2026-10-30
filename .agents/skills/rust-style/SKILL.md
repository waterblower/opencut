---
name: rust-style
description: Rust style conventions for OpenCut. Use when writing, refactoring, or reviewing Rust code for naming, imports, visibility, source organization, control flow, early returns, and error handling.
---

# Rust style

Apply these guidelines in the OpenCut repository. Paths are relative to the repository root.
Also apply the [general rules](../general-rules/SKILL.md).

## Naming and source organization

- Prefix GPUI action handlers (registered with `.on_action(cx.listener(Self::action_xxx))`)
  with `action_`.
- Prefix helpers called only from UI callbacks with `ui_callback_`, e.g.
  `ui_callback_set_timeline_frame_rate`. The callback may capture values and pass them in.
  Don't wrap such a helper in an `action_` function just to rename it. Business logic uses
  neither prefix.
- Suffix new functions that return GPUI elements (`AnyElement`, `impl IntoElement`, or a
  concrete element) with `_view`. Don't rename existing functions unless asked. Required
  trait methods are exempt.
- Don't use `super::` paths. Import items by full path at the top of the file and use
  unqualified names. Use an alias or a qualified name only to resolve a conflict.
- Import anyhow helpers explicitly and use them unqualified: `anyhow!`, `bail!`, `Result`,
  `Error`. Alias on conflict. Don't edit vendored code to enforce this.
- Never use `#[serde(rename_all = "snake_case")]`.
- Preserve Rust enum variant capitalization in JSON. Never add Serde aliases solely
  to accommodate different case conventions (for example, `#[serde(alias = "video")]`
  for `Video`).
- Use the smallest visibility that works: private, then `pub(super)`, `pub(crate)`, `pub`.
  `pub` is only for items another crate uses. This applies to fields, enum variants and
  re-exports too.
- Put private code at the bottom of each file, after public and `pub(...)` code.
- Split a type's public and private methods into separate inherent `impl` blocks
  (`pub(...)` counts as public), each marked with a comment directly above. The private
  block comes second. Trait impls stay in their own `impl Trait for Type` blocks.

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

- Explain easily confused state in short Chinese comments to the right of the code, with
  aligned comment columns: sources of truth vs derived values, time positions vs
  durations, decoder EOF vs finished playback. Keep intentional formatting and existing
  `#[rustfmt::skip]` annotations.
- Never call a frame index just `frame`; it could mean the picture or its position.
  Name indexes `frame_index` (or `…_frame_index`, typed `TimelineFrameIndex`) and frame
  data `frame` or `picture`.

## Explicit control flow

- Don't define custom macros; use functions and plain control flow. Standard and
  dependency macros (`format!`, derives) are fine.
- Don't use `ensure!`. Write an `if` with `return Err(…)` or `bail!`, so the condition
  and error read as ordinary control flow. Applies to new and touched code; don't rewrite
  existing `ensure!` calls just for this.

    ```rust
    if data.clip_locked(clip_id) {
        return Err(anyhow!("Clip {clip_id} is on a locked track"));
    }
    // not: ensure!(!data.clip_locked(clip_id), "Clip {clip_id} is on a locked track");
    ```

- Don't shadow variables in the same scope, even with the same type. Give each
  transformed value its own name: `rotated_pixels`, not a second `pixels`.
- Branch on `Option`/`Result` with `match`, not `map`, `map_or`, `map_or_else` or
  `map_err`, when the cases are different control-flow paths, however short. Iterator
  transformations are fine.

    ```rust
    match self.start_time {
        Some(start) => self.start_position + start.elapsed(),
        None => self.start_position,
    }
    // not: self.start_position + self.start_time.map_or(Duration::ZERO, |start| start.elapsed())
    ```

- Bind a conditional value with one `match` whose arms produce it, not a `let mut`
  default overwritten in one arm later.

    ```rust
    let position = match pointer_guide {
        Some(pointer_position) => pointer_position,
        None => timeline.playhead(),
    };
    // not: let mut position = timeline.playhead(); then `position = …` in a later match
    ```

- Don't use `_` in a `match` or `if let … else` when it stands for a single remaining
  case. Name it, so adding a case forces a revisit.

## Early returns

- Return early wherever possible, in `if` and `match`: handle the case that ends the work
  first, then continue the main path unindented.
- When required optional state is missing, use `let … else` with an early return instead
  of nesting the rest inside `if let`.
- Return from the `match` arm that ends the work. Don't turn it into `None` or a flag and
  check it again afterwards.

    ```rust
    let media_path = match selection.await {
        Ok(Ok(Some(paths))) => {
            /* the chosen path */
        }
        Ok(Ok(None)) => {
            return Ok(()); // the user cancelled
        }
        Err(error) => {
            return Err(anyhow!("file picker failed: {error}"));
        }
    };
    // not: `Ok(Ok(None)) => None`, then `let Some(media_path) = media_path else { return Ok(()) };`
    ```

- When building a value takes several steps, use an immediately invoked closure with
  early returns, and convert errors explicitly inside it.

    ```rust
    let timeline = (|| {
        let Some((path, data)) = active_timeline else {
            return None;
        };
        let state = match build_state(&data) {
            Ok(state) => state,
            Err(error) => panic!("could not build timeline: {error}"),
        };
        Some(TimelineRuntimeState::new(path, state))
    })();
    // not: active_timeline.map(|(path, data)| TimelineRuntimeState::new(path, build_state(&data).unwrap()))
    ```

## Errors and correctness

- Never silently ignore abnormal situations. When an operation needs an object to exist,
  or to have a certain type or state, return an explicit error if it doesn't; e.g. editing
  a clip fails if the clip is missing or the wrong kind. Check with early error returns
  (see Early returns); don't hide failures behind `if let`, `continue`, fallback values or
  `Ok(())`. Absence and no-ops are fine only when they are the intended behaviour, not a
  way to suppress an unexpected failure.
- Don't put source file names or line numbers in error messages by default (including
  `file!()` and `line!()`). Describe the failed operation and its relevant inputs instead.
- Log errors with debug formatting: `{error:?}`.
- Don't call deprecated functions or methods.

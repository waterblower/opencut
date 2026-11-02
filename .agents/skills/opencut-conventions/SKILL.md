---
name: opencut-conventions
description: OpenCut-specific Rust conventions. Use when writing, refactoring, or reviewing Rust code in OpenCut that touches GPUI actions, callbacks or views, timeline frames, vendored crates, or state comments.
---

# OpenCut conventions

Apply these guidelines in the OpenCut repository. Paths are relative to the repository root.
Also apply the general [Rust style](../rust-style/SKILL.md) and the
[general rules](../general-rules/SKILL.md).

## GPUI

- Prefix GPUI action handlers (registered with `.on_action(cx.listener(Self::action_xxx))`)
  with `action_`.
- Prefix helpers called only from UI callbacks with `ui_callback_`, e.g.
  `ui_callback_set_timeline_frame_rate`. The callback may capture values and pass them in.
  Don't wrap such a helper in an `action_` function just to rename it. Business logic uses
  neither prefix.
- Suffix new functions that return GPUI elements (`AnyElement`, `impl IntoElement`, or a
  concrete element) with `_view`. Don't rename existing functions unless asked. Required
  trait methods are exempt.

## Timeline

- Never call a frame index just `frame`; it could mean the picture or its position.
  Name indexes `frame_index` (or `…_frame_index`, typed `TimelineFrameIndex`) and frame
  data `frame` or `picture`.

## Vendored code

- Don't edit vendored code (`rust/vendor/`) to enforce style rules.

## Comments

- Explain easily confused state in short Chinese comments to the right of the code, with
  aligned comment columns: sources of truth vs derived values, time positions vs
  durations, decoder EOF vs finished playback. Keep intentional formatting and existing
  `#[rustfmt::skip]` annotations.

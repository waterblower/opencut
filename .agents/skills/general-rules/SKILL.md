---
name: general-rules
description: General project rules for OpenCut. Use when planning, implementing, reviewing, documenting, building, or validating work in this repository, including workflow and error-handling expectations.
---

# OpenCut general rules

Apply these guidelines in the OpenCut repository. Paths are relative to the repository root.

For Rust changes, also apply [Rust style](../rust-style/SKILL.md). For state, events, execution, or mutation design, also apply [architecture](../architecture/SKILL.md). For new UI or visual changes, also apply [UI design](../ui-design/SKILL.md).

## Workflow and review

- Never call `mcp.cua_repl.js` (also exposed as `mcp__cua_repl.js`).
- Start planning and design at the highest level of abstraction: define the
  intended behavior, public interfaces, and caller control flow before lower-level
  details. When implementing, establish the high-level control flow and data
  structures first. Early increments may contain empty or unimplemented constructs
  and do not have to compile. Gradually fill in the lower-level details until the
  complete implementation works, then run the required validation.
- Do not create a plan file unless the user explicitly asks for one, even in
  Plan mode or for large or complex tasks. Otherwise, keep planning brief and
  in the conversation. When requested, keep the plan proportional to the task;
  do not write a large plan unless the user asks for that level of detail.
- Always make each step as small and focused as practical, with one clear outcome
  that can be reviewed independently. Split large steps before starting them;
  do not bundle unrelated changes merely to reduce the number of checkpoints.
- When a plan file is requested, describe each step: its intended changes, prerequisites, and the
  checks or evidence required to consider it complete. Give steps stable IDs and
  Markdown checkboxes (`[ ]` pending, `[x]` complete). List steps in topological
  order, with prerequisites before dependent steps. Explicitly identify steps
  that have no dependencies on each other and can run in parallel or any order.
- For a requested detailed plan, include a Mermaid dependency graph, using the same step IDs as the
  checklist. Show prerequisite edges and label each step's status: pending,
  in progress, complete, blocked, or superseded. Keep a short progress summary
  with completed/total active steps, the current step, and any blockers.
- When working from a requested plan, after each step update its checkbox, status, completion
  evidence, progress summary, and graph. If the user changes direction, update
  the plan before continuing: revise steps and dependencies, record the changed
  direction, and mark obsolete steps as superseded rather than complete.
- The review checkpoint is the completion of each step, not a fixed line count.
  At each checkpoint, pause implementation, summarize the result with clickable
  code links and, if applicable, a link to the updated plan, and wait for the user's explicit
  instruction before proceeding. Complete the task without these pauses only
  when the user explicitly asks to finish without waiting for review.
- Whenever you mention a code location (a function, type, field, or specific
  lines) in a reply, include a clickable reference with the file path and line
  number so the user can jump there in the editor, for example
  [`preview_player`](rust/apps/editor/src/preview.rs#L33) or a line range such as
  [preview.rs#L33-L76](rust/apps/editor/src/preview.rs#L33-L76). Use paths
  relative to the repository root. Look up the actual line numbers first;
  never guess them.
- When referencing any file to the user, always use a clickable Markdown file
  link. For code locations, include the relevant line number in the link.

## Build and validation

- Do not build FFmpeg ourselves, including through `ffbuild/`. Link against the
  existing vendored libraries in `rust/vendor/ffmpeg-8.1.2/`.
- Never include Python in the build process.
- Do not write new tests unless the user explicitly requests them. Existing tests
  may still be updated to accommodate requested changes and run for validation.
- Do not write tests for `rust/apps/player/`; it is a debug-only demo.

## General correctness

- Never silently ignore abnormal situations. If an operation requires an object
  to exist or have a particular type or state, return an explicit error when that
  requirement is not met. Use explicit `if` or `let ... else` checks with early
  error returns; do not hide failures behind `if let`, `continue`, fallback values,
  or `Ok(())`. For example, editing a clip must fail if the clip is missing or is
  the wrong kind. Optional absence and no-ops are valid only when they are part of
  the operation's intended behavior, not a way to suppress an unexpected failure.
- Do not add source file names or line numbers to error messages by default
  (including `file!()` and `line!()`). Prefer context describing the failed
  operation and relevant inputs.
- Do not call deprecated functions or methods.

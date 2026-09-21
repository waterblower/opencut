# Repository guidelines

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
- Do not build FFmpeg ourselves, including through `ffbuild/`. Link against the
  existing vendored libraries in `rust/vendor/ffmpeg-8.1.2/`.
- Never include Python in the build process.
- Do not write new tests unless the user explicitly requests them. Existing tests
  may still be updated to accommodate requested changes and run for validation.
- Do not write tests for `rust/src/player/`; it is a debug-only demo.
- Do not define custom macros. Prefer ordinary functions and explicit control
  flow so the code is easy to read. Standard and dependency-provided macros
  (such as `format!` and derives) are allowed.
- Read environment variables only in `main()` or application initialization code.
  Pass the required values explicitly to business logic functions.
- Never pass functions or closures as arguments to simple functions. Pass the
  required values or references directly. If a callback is truly needed for a
  simple function, ask the user first.
- Perform quick synchronous work, such as small UI state changes and cheap
  backend commands, directly in UI callbacks without emitting events. Use event
  handlers to coordinate asynchronous or CPU-heavy work. Run CPU-heavy work on
  background workers; emitting an event alone does not move work off the UI
  thread.
- Do not introduce external state or extra stored state that increases the number
  of possible state combinations when a pure function or message passing can
  solve the problem. Prefer carrying the required data in messages over adding
  state to a broader owner solely so event handlers can access it.
- State should live in the narrowest scope that needs it. Prefer a local variable
  over a struct field unless the value actually needs to be shared across methods
  or control flows.
- Do not extract a separate function when it has only one caller and its body is
  a single expression or statement. Inline that logic at the call site.
- Prefer `let ... else` with an early return when required optional state is
  absent, instead of nesting the remaining control flow inside `if let`.
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
- When a function call returns an error, propagate it through intermediate
  functions instead of logging it there. Log the error only at the highest-level
  application, event, or task boundary.
- Business logic must return errors without deciding how they are reported to
  users or rendered in the UI. Keep user-facing error messages, error status
  updates, and UI notifications in the application or event handling layer.
- Always use debug formatting (`{error:?}`) when logging errors.
- Do not use `super::` paths. Import items explicitly using full paths at the
  beginning of the file, and use unqualified names in the code. Use an import
  alias or qualified name only when needed to resolve a name conflict.
- Import anyhow helpers explicitly and use unqualified names such as `anyhow!`,
  `bail!`, `Result`, and `Error`. Use an import alias when a name conflicts with
  another type. Do not change vendored dependency code to enforce this rule.
- Do not add source file names or line numbers to error messages by default
  (including `file!()` and `line!()`). Prefer context describing the failed
  operation and relevant inputs.
- Functions and methods should accept only the data they use. Prefer passing the
  smallest required values over accepting a broader type such as `&self` when
  the function does not depend on the rest of that type's state.
- If a function's first argument is a mutable reference, prefer a method on that
  type using `&mut self` instead of a free function.
- Prefer a functional style: helpers and lower-level functions should return
  data or proposed state changes instead of mutating `self` or application state.
  Apply mutations as high in the call stack as possible, ideally at the outermost
  application, event, or UI boundary. Pass helpers only the inputs they need and
  let the caller apply their results. When a stateful resource such as a decoder
  or converter requires mutation, limit mutable access to that resource; do not
  use it as a reason to mutate broader application state inside the helper.
- Never use `#[serde(rename_all = "snake_case")]`.
- Keep functions, methods, and other items private by default. Expose them only
  when they are used outside their defining module.
- When visibility outside the module is needed, prefer `pub` over `pub(super)`.
- Do not call deprecated functions or methods.
- Place private code at the bottom of each file, after public and
  restricted-public (`pub(...)`) code.
- When referencing any file to the user, always use a clickable Markdown file
  link. For code locations, include the relevant line number in the link.

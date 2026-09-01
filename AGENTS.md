# Repository guidelines

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
- When refactoring or adding error-handling code, always include `file!()` and
  `line!()` information in every newly added error context.
- Functions and methods should accept only the data they use. Prefer passing the
  smallest required values over accepting a broader type such as `&self` when
  the function does not depend on the rest of that type's state.
- Never use `#[serde(rename_all = "snake_case")]`.
- Prefer `pub` over `pub(super)`.
- Do not call deprecated functions or methods.
- Place private code at the bottom of each file, after public and
  restricted-public (`pub(...)`) code.
- When referencing any file to the user, always use a clickable Markdown file
  link. For code locations, include the relevant line number in the link.

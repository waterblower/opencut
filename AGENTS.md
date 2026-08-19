# Repository guidelines

- Do not extract a separate function when it has only one caller and its body is
  a single expression or statement. Inline that logic at the call site.
- Prefer `let ... else` with an early return when required optional state is
  absent, instead of nesting the remaining control flow inside `if let`.
- When a function call returns an error, propagate it through intermediate
  functions instead of logging it there. Log the error only at the highest-level
  application, event, or task boundary.
- Functions and methods should accept only the data they use. Prefer passing the
  smallest required values over accepting a broader type such as `&self` when
  the function does not depend on the rest of that type's state.
- Never use `#[serde(rename_all = "snake_case")]`.

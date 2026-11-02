---
name: rust-style
description: General Rust style conventions. Use when writing, refactoring, or reviewing Rust code for naming, imports, visibility, source organization, control flow, early returns, and error handling.
---

# Rust style

Apply these guidelines in the OpenCut repository. Paths are relative to the repository root.
Also apply the [general rules](../general-rules/SKILL.md) and the project-specific
[OpenCut conventions](../opencut-conventions/SKILL.md) (GPUI, timeline,
vendored code, comments).

## Naming and source organization

- Don't use `super::` paths. Import items by full path at the top of the file and use
  unqualified names. Use an alias or a qualified name only to resolve a conflict.
- Import anyhow helpers explicitly and use them unqualified: `anyhow!`, `bail!`, `Result`,
  `Error`. Alias on conflict.
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
    impl Cache {
        pub fn len(&self) -> usize { ... }
    }

    // Private APIs only
    impl Cache {
        fn evict(&mut self, key: &Key) -> Result<()> { ... }
    }
    ```

## Explicit control flow

- Don't define custom macros; use functions and plain control flow. Standard and
  dependency macros (`format!`, derives) are fine.
- Don't use `ensure!`. Write an `if` with `return Err(…)` or `bail!`, so the condition
  and error read as ordinary control flow. Applies to new and touched code; don't rewrite
  existing `ensure!` calls just for this.

    ```rust
    if store.is_read_only(key) {
        return Err(anyhow!("Key {key} is read-only"));
    }
    // not: ensure!(!store.is_read_only(key), "Key {key} is read-only");
    ```

- Don't shadow variables in the same scope, even with the same type. Give each
  transformed value its own name: `rotated_pixels`, not a second `pixels`.
- Branch on `Option`/`Result` with `match`, not `map`, `map_or`, `map_or_else` or
  `map_err`, when the cases are different control-flow paths, however short. Iterator
  transformations are fine.

    ```rust
    match self.started_at {
        Some(started_at) => self.offset + started_at.elapsed(),
        None => self.offset,
    }
    // not: self.offset + self.started_at.map_or(Duration::ZERO, |started_at| started_at.elapsed())
    ```

- Bind a conditional value with one `match` whose arms produce it, not a `let mut`
  default overwritten in one arm later.

    ```rust
    let limit = match override_limit {
        Some(override_limit) => override_limit,
        None => config.default_limit(),
    };
    // not: let mut limit = config.default_limit(); then `limit = …` in a later match
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
    let input_path = match prompt.await {
        Ok(Ok(Some(paths))) => {
            /* the chosen path */
        }
        Ok(Ok(None)) => {
            return Ok(()); // the user cancelled
        }
        Err(error) => {
            return Err(anyhow!("prompt failed: {error}"));
        }
    };
    // not: `Ok(Ok(None)) => None`, then `let Some(input_path) = input_path else { return Ok(()) };`
    ```

- When building a value takes several steps, use an immediately invoked closure with
  early returns, and convert errors explicitly inside it.

    ```rust
    let session = (|| {
        let Some((path, data)) = saved_session else {
            return None;
        };
        let state = match parse_state(&data) {
            Ok(state) => state,
            Err(error) => panic!("could not parse session: {error}"),
        };
        Some(Session::new(path, state))
    })();
    // not: saved_session.map(|(path, data)| Session::new(path, parse_state(&data).unwrap()))
    ```

## Errors and correctness

- Never silently ignore abnormal situations. When an operation needs an object to exist,
  or to have a certain type or state, return an explicit error if it doesn't; e.g. updating
  a record fails if the record is missing or the wrong kind. Check with early error returns
  (see Early returns); don't hide failures behind `if let`, `continue`, fallback values or
  `Ok(())`. Absence and no-ops are fine only when they are the intended behaviour, not a
  way to suppress an unexpected failure.
- Don't put source file names or line numbers in error messages by default (including
  `file!()` and `line!()`). Describe the failed operation and its relevant inputs instead.
- Log errors with debug formatting: `{error:?}`.
- Don't call deprecated functions or methods.

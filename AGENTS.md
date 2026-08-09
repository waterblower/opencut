# Repository guidelines

- Do not extract a separate function when it has only one caller and its body is
  a single expression or statement. Inline that logic at the call site.

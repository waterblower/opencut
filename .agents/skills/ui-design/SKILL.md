---
name: ui-design
description: UI design practice for OpenCut. Use when designing new UI, changing the look of existing UI, or working in design/system and design/pages, before writing any application code for that UI.
---

# OpenCut UI design

Apply these guidelines in the OpenCut repository. Paths are relative to the repository root.

Also apply the repository [general rules](../general-rules/SKILL.md).

## Design first

- When a UI does not exist in the code yet, always design it first. Do not write
  application code for it until the user has reviewed the design.
- Changes to the look of existing UI also start in the design pages, then move to code.

## Where design work happens

- Write designs in `design/system/pages/<page>.tsx`. Prefer normal HTML JSX with
  Tailwind utilities for layout and styling; this maps more closely to GPUI semantics
  and keeps the source concise. Pages do not have to be SVG. Migrate existing SVG
  layouts gradually as requested; keep SVG where it is useful, such as icon assets.
  Shared parts live in `design/system/components/`, and colours and fonts are defined
  once in `design/system/theme.ts`.
- Build with `deno task build` in `design`. It writes each page to
  `design/pages/<page>.html`. Never edit `design/pages` by hand; it is build output.
- Commit the source only. `design/pages/` is ignored generated output.
- Reuse existing components and theme values. When a part repeats, extract it into a
  component instead of copying it. Use the theme's colours instead of literal values.
- Use a page from `design/system/pages/` as a template for the page structure, title,
  subtitle and notes.

## The design system is static

- The design system is only static visual demos. It is not the final application and
  does not share code with it.
- The built HTML contains no JavaScript; the build fails if it does. JSX runs only at
  build time.
- Show interaction states (hover, selected, disabled, editing) as separate static
  mock-ups side by side, not as behaviour.

## Page content

- `00-design-system` is the source of truth for shared components. Pages and app code
  use its components exactly. Variants within a set keep one shape and differ only in
  colour or fill.
- Put notes and comments in the page margin, outside the drawn UI. Text inside a
  mock-up reads as real UI.
- Show each fact once. Do not add hints, labels or buttons the user did not ask for.
- When proposing options, draw them side by side on one page with a short note each,
  and let the user pick.

## Review

- After building, check the rendered page visually (for example a headless Chrome
  screenshot of the HTML) before reporting.
- Stop after each design step and let the user review. Report the page with a
  clickable link.

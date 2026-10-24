import { raw } from "@design/jsx/jsx-runtime";
import type { Markup } from "@design/jsx/jsx-runtime";
import { FONT_MONO, FONT_SANS } from "../theme.ts";

/// The `.s` (sans) and `.m` (mono) text classes most pages use.
export function FontStyle(): Markup {
  return <style>{raw(`.s{font-family:${FONT_SANS}}.m{font-family:${FONT_MONO}}`)}</style>;
}

import type { Markup } from "@design/jsx/jsx-runtime";
import { Fragment } from "@design/jsx/jsx-runtime";

/// Shapes of the D · Cyberpunk buttons, for <defs>. Pages place them with
/// <use href="#dPrimaryShape">, <use href="#dSecondaryShape"> and <use href="#dSlashes">.
export function CyberpunkShapeDefs(): Markup {
  return (
    <Fragment>
      <polygon id="dPrimaryShape" points="10,0 128,0 128,30 118,40 0,40 0,10" />
      <polygon id="dSecondaryShape" points="10,0 128,0 128,30 118,40 0,40 0,10" />
      <path
        id="dSlashes"
        d="M98 34 L106 6 H109 L101 34Z M105 34 L113 6 H116 L108 34Z M112 34 L120 6 H123 L115 34Z"
      />
    </Fragment>
  );
}

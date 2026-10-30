import { COLOR } from "../../theme.ts";
import type { Markup } from "@design/jsx/jsx-runtime";

/// The OpenCut app icon: a dark rounded tile with a folded-corner "C" in white and gold.
export function AppIcon(props: { size: number }): Markup {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width={props.size} height={props.size} viewBox="0 0 1024 1024">
      <defs>
        <linearGradient id="app-icon-tile" x1="192" y1="64" x2="832" y2="960" gradientUnits="userSpaceOnUse">
          <stop stop-color="#25252B" />
          <stop offset="0.55" stop-color={COLOR.surface} />
          <stop offset="1" stop-color={COLOR.panel} />
        </linearGradient>
        <linearGradient id="app-icon-edge" x1="256" y1="64" x2="768" y2="960" gradientUnits="userSpaceOnUse">
          <stop stop-color="#FFFFFF" stop-opacity="0.16" />
          <stop offset="0.5" stop-color="#FFFFFF" stop-opacity="0.04" />
          <stop offset="1" stop-color="#FFFFFF" stop-opacity="0.08" />
        </linearGradient>
        <linearGradient id="app-icon-gold" x1="456" y1="244" x2="780" y2="568" gradientUnits="userSpaceOnUse">
          <stop stop-color="#FFDA94" />
          <stop offset="0.55" stop-color={COLOR.accent} />
          <stop offset="1" stop-color="#DD963E" />
        </linearGradient>
      </defs>
      <rect x="64" y="72" width="896" height="896" rx="200" fill="#000000" fill-opacity="0.18" />
      <rect x="64" y="64" width="896" height="896" rx="200" fill="url(#app-icon-tile)" />
      <rect x="65" y="65" width="894" height="894" rx="199" stroke="url(#app-icon-edge)" stroke-width="2" />
      <path
        fill={COLOR.text}
        d="M408 244H400C314 244 244 314 244 400V624C244 710 314 780 400 780H624C710 780 780 710 780 624V616L672 508V624C672 650 650 672 624 672H400C374 672 352 650 352 624V400C352 374 374 352 400 352H516L408 244Z"
      />
      <path
        fill="url(#app-icon-gold)"
        d="M456 244H624C710 244 780 314 780 400V568L672 460V400C672 374 650 352 624 352H564L456 244Z"
      />
    </svg>
  );
}

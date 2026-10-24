import { COLOR } from "../theme.ts";
import { FontStyle } from "../components/fonts.tsx";
import { CyberpunkShapeDefs } from "../components/cyberpunk.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

export default function Page20TopBarSvg(): Markup {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      xmlns:xlink="http://www.w3.org/1999/xlink"
      width="1240"
      height="426"
      viewBox="0 0 1240 426"
      role="img"
      aria-labelledby="title description"
    >
      <title id="title">{"Top bar"}</title>
      <desc id="description">
        {"Cyberpunk top bar in two states: timeline open with yellow Export, and no timeline with disabled Export. The OpenCut app icon precedes the app name; secondary actions use cyan cut-corner buttons."}
      </desc>
      <FontStyle />
      <defs>
        <CyberpunkShapeDefs />
        <filter id="shadow" x="-5%" y="-30%" width="110%" height="180%">
          <feDropShadow dx="0" dy="5" stdDeviation="7" flood-color="#000" flood-opacity="0.24" />
        </filter>
        <pattern id="dScanDim" width="4" height="3" patternUnits="userSpaceOnUse">
          <rect width="4" height="1" fill="#ffffff" fill-opacity="0.05" />
        </pattern>
        <filter id="dGlowY" x="-30%" y="-80%" width="160%" height="260%">
          <feDropShadow dx="0" dy="0" stdDeviation="6" flood-color="#fcee0a" flood-opacity="0.45" />
        </filter>
        <filter id="dGlowC" x="-30%" y="-80%" width="160%" height="260%">
          <feDropShadow dx="0" dy="0" stdDeviation="5" flood-color="#00f0ff" flood-opacity="0.35" />
        </filter>
        <linearGradient id="app-tile" x1="192" y1="64" x2="832" y2="960" gradientUnits="userSpaceOnUse">
          <stop stop-color="#25252B" />
          <stop offset="0.55" stop-color={COLOR.surface} />
          <stop offset="1" stop-color={COLOR.panel} />
        </linearGradient>
        <linearGradient id="app-edge" x1="256" y1="64" x2="768" y2="960" gradientUnits="userSpaceOnUse">
          <stop stop-color="#FFFFFF" stop-opacity="0.16" />
          <stop offset="0.5" stop-color="#FFFFFF" stop-opacity="0.04" />
          <stop offset="1" stop-color="#FFFFFF" stop-opacity="0.08" />
        </linearGradient>
        <linearGradient id="app-gold" x1="456" y1="244" x2="780" y2="568" gradientUnits="userSpaceOnUse">
          <stop stop-color="#FFDA94" />
          <stop offset="0.55" stop-color={COLOR.accent} />
          <stop offset="1" stop-color="#DD963E" />
        </linearGradient>
        <g id="app-icon" fill="none">
          <rect x="64" y="72" width="896" height="896" rx="200" fill="#000000" fill-opacity="0.18" />
          <rect x="64" y="64" width="896" height="896" rx="200" fill="url(#app-tile)" />
          <rect x="65" y="65" width="894" height="894" rx="199" stroke="url(#app-edge)" stroke-width="2" />
          <path
            fill={COLOR.text}
            d="M408 244H400C314 244 244 314 244 400V624C244 710 314 780 400 780H624C710 780 780 710 780 624V616L672 508V624C672 650 650 672 624 672H400C374 672 352 650 352 624V400C352 374 374 352 400 352H516L408 244Z"
          />
          <path
            fill="url(#app-gold)"
            d="M456 244H624C710 244 780 314 780 400V568L672 460V400C672 374 650 352 624 352H564L456 244Z"
          />
        </g>
      </defs>
      <rect fill={COLOR.page} x="0" y="0" width="1240" height="426" />
      <text fill={COLOR.text} class="s" x="48" y="61" font-size="24" font-weight="700" text-anchor="start">
        {"Top bar"}
      </text>
      <text fill={COLOR.muted} class="s" x="48" y="89" font-size="14" font-weight="400" text-anchor="start">
        {"Timeline identity and workspace actions. Always available above the editor."}
      </text>
      <text fill={COLOR.text} class="s" x="48" y="136" font-size="14" font-weight="600" text-anchor="start">
        {"Timeline open"}
      </text>
      <text fill={COLOR.muted} class="s" x="48" y="157" font-size="12" font-weight="400" text-anchor="start">
        {"The timeline filename and autosave status sit beside the app name. All actions are available."}
      </text>
      <rect fill={COLOR.panel} x="48" y="170" width="1144" height="64" rx="8" filter="url(#shadow)" />
      <clipPath id="timeline-open">
        <rect x="48" y="170" width="1144" height="64" rx="8" />
      </clipPath>
      <g clip-path="url(#timeline-open)">
        <rect fill={COLOR.panel} x="48" y="170" width="1144" height="64" />
        <rect fill={COLOR.border} x="48" y="233" width="1144" height="1" />
        <text
          fill={COLOR.text}
          class="s"
          x="112.0"
          y="209.2"
          font-size="20"
          font-weight="700"
          text-anchor="start"
        >
          {"OpenCut"}
        </text>
        <text
          fill={COLOR.muted}
          class="m"
          x="207.17"
          y="206.32"
          font-size="12"
          font-weight="400"
          text-anchor="start"
        >
          {"EDITOR · market-vlog.timeline · AUTOSAVED"}
        </text>
        <use transform="translate(64 182) scale(0.0390625)" xlink:href="#app-icon" />
        <g transform="translate(600 182)">
          <use
            xlink:href="#dSecondaryShape"
            fill="#00f0ff"
            fill-opacity=".06"
            stroke="#00f0ff"
            filter="url(#dGlowC)"
          />
          <text
            x="64.0"
            y="24.5"
            font-size="13"
            font-weight="800"
            letter-spacing="2"
            fill="#00f0ff"
            text-anchor="middle"
            class="m"
          >
            {"SETTINGS"}
          </text>
        </g>
        <g transform="translate(740 182)">
          <polygon
            points="10,0 152,0 152,30 142,40 0,40 0,10"
            fill="#00f0ff"
            fill-opacity=".06"
            stroke="#00f0ff"
            filter="url(#dGlowC)"
          />
          <text
            x="76.0"
            y="24.5"
            font-size="13"
            font-weight="800"
            letter-spacing="2"
            fill="#00f0ff"
            text-anchor="middle"
            class="m"
          >
            {"OPEN FOLDER"}
          </text>
        </g>
        <g transform="translate(904 182)">
          <use
            xlink:href="#dSecondaryShape"
            fill="#00f0ff"
            fill-opacity=".06"
            stroke="#00f0ff"
            filter="url(#dGlowC)"
          />
          <text
            x="64.0"
            y="24.5"
            font-size="13"
            font-weight="800"
            letter-spacing="2"
            fill="#00f0ff"
            text-anchor="middle"
            class="m"
          >
            {"PROJECTS"}
          </text>
        </g>
        <g transform="translate(1044 182)">
          <use fill="#fcee0a" xlink:href="#dPrimaryShape" filter="url(#dGlowY)" />
          <use fill="#0a0a0b" xlink:href="#dSlashes" />
          <text
            x="57"
            y="24.5"
            font-size="13"
            font-weight="800"
            letter-spacing="2"
            fill="#0a0a0b"
            text-anchor="middle"
            class="m"
          >
            {"EXPORT"}
          </text>
        </g>
      </g>
      <rect stroke={COLOR.border} x="48.5" y="170.5" width="1143" height="63" fill="none" rx="8" />
      <text fill={COLOR.text} class="s" x="48" y="280" font-size="14" font-weight="600" text-anchor="start">
        {"No timeline"}
      </text>
      <text fill={COLOR.muted} class="s" x="48" y="301" font-size="12" font-weight="400" text-anchor="start">
        {"Export is disabled until a timeline is open. Settings, Open Folder and Projects remain available."}
      </text>
      <rect fill={COLOR.panel} x="48" y="314" width="1144" height="64" rx="8" filter="url(#shadow)" />
      <clipPath id="no-timeline">
        <rect x="48" y="314" width="1144" height="64" rx="8" />
      </clipPath>
      <g clip-path="url(#no-timeline)">
        <rect fill={COLOR.panel} x="48" y="314" width="1144" height="64" />
        <rect fill={COLOR.border} x="48" y="377" width="1144" height="1" />
        <text
          fill={COLOR.text}
          class="s"
          x="112.0"
          y="353.2"
          font-size="20"
          font-weight="700"
          text-anchor="start"
        >
          {"OpenCut"}
        </text>
        <text
          fill={COLOR.muted}
          class="m"
          x="207.17"
          y="350.32"
          font-size="12"
          font-weight="400"
          text-anchor="start"
        >
          {"EDITOR · No timeline · AUTOSAVED"}
        </text>
        <use transform="translate(64 326) scale(0.0390625)" xlink:href="#app-icon" />
        <g transform="translate(600 326)">
          <use
            xlink:href="#dSecondaryShape"
            fill="#00f0ff"
            fill-opacity=".06"
            stroke="#00f0ff"
            filter="url(#dGlowC)"
          />
          <text
            x="64.0"
            y="24.5"
            font-size="13"
            font-weight="800"
            letter-spacing="2"
            fill="#00f0ff"
            text-anchor="middle"
            class="m"
          >
            {"SETTINGS"}
          </text>
        </g>
        <g transform="translate(740 326)">
          <polygon
            points="10,0 152,0 152,30 142,40 0,40 0,10"
            fill="#00f0ff"
            fill-opacity=".06"
            stroke="#00f0ff"
            filter="url(#dGlowC)"
          />
          <text
            x="76.0"
            y="24.5"
            font-size="13"
            font-weight="800"
            letter-spacing="2"
            fill="#00f0ff"
            text-anchor="middle"
            class="m"
          >
            {"OPEN FOLDER"}
          </text>
        </g>
        <g transform="translate(904 326)">
          <use
            xlink:href="#dSecondaryShape"
            fill="#00f0ff"
            fill-opacity=".06"
            stroke="#00f0ff"
            filter="url(#dGlowC)"
          />
          <text
            x="64.0"
            y="24.5"
            font-size="13"
            font-weight="800"
            letter-spacing="2"
            fill="#00f0ff"
            text-anchor="middle"
            class="m"
          >
            {"PROJECTS"}
          </text>
        </g>
        <g transform="translate(1044 326)">
          <use fill="url(#dScanDim)" xlink:href="#dPrimaryShape" stroke="#3a3926" />
          <use fill="#3a3926" xlink:href="#dSlashes" />
          <text
            x="57"
            y="24.5"
            font-size="13"
            font-weight="800"
            letter-spacing="2"
            fill="#57563f"
            text-anchor="middle"
            class="m"
          >
            {"EXPORT"}
          </text>
        </g>
      </g>
      <rect stroke={COLOR.border} x="48.5" y="314.5" width="1143" height="63" fill="none" rx="8" />
    </svg>
  );
}

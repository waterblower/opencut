import { COLOR } from "../theme.ts";
import { FontStyle } from "../components/fonts.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

export default function Page13ExportWindowSvg(): Markup {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width="1240" height="1326" viewBox="0 0 1240 1326">
      <FontStyle />
      <defs>
        <filter id="shadow" x="-20%" y="-20%" width="140%" height="160%">
          <feDropShadow dx="0" dy="8" stdDeviation="10" flood-color="#000" flood-opacity="0.55" />
        </filter>
        <linearGradient id="sky" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#1b2745" />
          <stop offset="0.55" stop-color="#7a4a62" />
          <stop offset="1" stop-color="#e2905c" />
        </linearGradient>
        <radialGradient id="sun">
          <stop offset="0" stop-color="#ffe2a8" />
          <stop offset="0.45" stop-color="#ffc77a" stop-opacity="0.9" />
          <stop offset="1" stop-color="#ffb066" stop-opacity="0" />
        </radialGradient>
        <linearGradient id="street" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#3a4256" />
          <stop offset="0.6" stop-color="#8a5a4c" />
          <stop offset="1" stop-color="#d48a56" />
        </linearGradient>
        <clipPath id="c1">
          <rect x="40" y="80" width="560" height="548" rx="10" />
        </clipPath>
        <clipPath id="c2">
          <rect x="640" y="80" width="560" height="548" rx="10" />
        </clipPath>
        <clipPath id="c3">
          <rect x="40" y="718" width="560" height="548" rx="10" />
        </clipPath>
        <clipPath id="c4">
          <rect x="640" y="718" width="560" height="548" rx="10" />
        </clipPath>
      </defs>
      <rect x="0" y="0" width="1240" height="1326" fill="#1e1e22" />
      <text fill={COLOR.light} class="s" x="40" y="44" font-size="15" font-weight="600">{"Idle"}</text>
      <text class="s" x="40" y="64" font-size="12" fill="#8a8a93">{"Before export"}</text>
      <rect x="40" y="80" width="560" height="548" rx="10" fill="#080809" filter="url(#shadow)" />
      <g clip-path="url(#c1)">
        <rect x="40" y="80" width="560" height="28" fill="#232326" />
        <rect x="40" y="107" width="560" height="1" fill="#000000" />
        <circle cx="54" cy="94" r="6" fill="#ff5f57" />
        <circle cx="74" cy="94" r="6" fill="#febc2e" />
        <circle cx="94" cy="94" r="6" fill="#28c840" />
        <rect x="40" y="108" width="560" height="520" fill="#080809" />
        <text fill={COLOR.text} class="s" x="64" y="151.2" font-size="20">{"Export timeline"}</text>
        <text fill={COLOR.text} class="s" x="64" y="188.76" font-size="16">{"edit.timeline"}</text>
        <text fill={COLOR.muted} class="s" x="64" y="225.04" font-size="14">
          {"1920 × 1080 · 30/1 fps · MP4 / H.264"}
        </text>
        <text fill={COLOR.text} class="s" x="64" y="278.76" font-size="16">{"Video bitrate (Mbps)"}</text>
        <rect
          fill={COLOR.surface}
          stroke={COLOR.border}
          x="227.7"
          y="246.5"
          width="139"
          height="53"
          rx="8"
          stroke-width="1"
        />
        <text fill={COLOR.text} class="m" x="243.2" y="278.76" font-size="16">{"8"}</text>
        <text fill={COLOR.muted} class="s" x="64" y="330" font-size="14">
          {"Exports the timeline as it was when this window opened. Existing files are"}
        </text>
        <text fill={COLOR.muted} class="s" x="64" y="350" font-size="14">{"not replaced."}</text>
        <rect fill={COLOR.accent} x="64" y="372" width="126.72" height="40" rx="6" />
        <text class="s" x="80" y="397.76" font-size="16" fill="#080809">{"Export MP4…"}</text>
      </g>
      <text fill={COLOR.light} class="s" x="640" y="44" font-size="15" font-weight="600">{"Running"}</text>
      <text class="s" x="640" y="64" font-size="12" fill="#8a8a93">{"Window cannot be closed"}</text>
      <rect x="640" y="80" width="560" height="548" rx="10" fill="#080809" filter="url(#shadow)" />
      <g clip-path="url(#c2)">
        <rect x="640" y="80" width="560" height="28" fill="#232326" />
        <rect x="640" y="107" width="560" height="1" fill="#000000" />
        <circle cx="654" cy="94" r="6" fill="#ff5f57" />
        <circle cx="674" cy="94" r="6" fill="#febc2e" />
        <circle cx="694" cy="94" r="6" fill="#28c840" />
        <rect x="640" y="108" width="560" height="520" fill="#080809" />
        <text fill={COLOR.text} class="s" x="664" y="151.2" font-size="20">{"Export timeline"}</text>
        <text fill={COLOR.text} class="s" x="664" y="188.76" font-size="16">{"edit.timeline"}</text>
        <text fill={COLOR.muted} class="s" x="664" y="225.04" font-size="14">
          {"1920 × 1080 · 30/1 fps · MP4 / H.264"}
        </text>
        <text fill={COLOR.text} class="s" x="664" y="278.76" font-size="16">{"Video bitrate (Mbps)"}</text>
        <text fill={COLOR.text} class="s" x="827.2" y="278.76" font-size="16">{"8"}</text>
        <text fill={COLOR.muted} class="s" x="664" y="330" font-size="14">
          {"Exports the timeline as it was when this window opened. Existing files are"}
        </text>
        <text fill={COLOR.muted} class="s" x="664" y="350" font-size="14">{"not replaced."}</text>
        <rect fill={COLOR.muted} x="664" y="372" width="126.72" height="40" rx="6" />
        <text class="s" x="680" y="397.76" font-size="16" fill="#080809">{"Export MP4…"}</text>
        <text fill={COLOR.text} class="s" x="664" y="444.76" font-size="16">
          {"Exporting: 42% · 227 / 540 frames"}
        </text>
        <rect fill={COLOR.muted} x="664" y="458" width="512" height="8" />
        <rect fill={COLOR.accent} x="664" y="458" width="215.04" height="8" />
        <rect fill={COLOR.muted} x="664" y="474" width="115.2" height="40" rx="6" />
        <text fill={COLOR.text} class="s" x="680" y="499.76" font-size="16">{"Stop export"}</text>
      </g>
      <text fill={COLOR.light} class="s" x="40" y="682" font-size="15" font-weight="600">{"Complete"}</text>
      <text class="s" x="40" y="702" font-size="12" fill="#8a8a93">{"After a finished export"}</text>
      <rect x="40" y="718" width="560" height="548" rx="10" fill="#080809" filter="url(#shadow)" />
      <g clip-path="url(#c3)">
        <rect x="40" y="718" width="560" height="28" fill="#232326" />
        <rect x="40" y="745" width="560" height="1" fill="#000000" />
        <circle cx="54" cy="732" r="6" fill="#ff5f57" />
        <circle cx="74" cy="732" r="6" fill="#febc2e" />
        <circle cx="94" cy="732" r="6" fill="#28c840" />
        <rect x="40" y="746" width="560" height="520" fill="#080809" />
        <text fill={COLOR.text} class="s" x="64" y="789.2" font-size="20">{"Export timeline"}</text>
        <text fill={COLOR.text} class="s" x="64" y="826.76" font-size="16">{"edit.timeline"}</text>
        <text fill={COLOR.muted} class="s" x="64" y="863.04" font-size="14">
          {"1920 × 1080 · 30/1 fps · MP4 / H.264"}
        </text>
        <text fill={COLOR.text} class="s" x="64" y="916.76" font-size="16">{"Video bitrate (Mbps)"}</text>
        <rect
          fill={COLOR.surface}
          stroke={COLOR.border}
          x="227.7"
          y="884.5"
          width="139"
          height="53"
          rx="8"
          stroke-width="1"
        />
        <text fill={COLOR.text} class="m" x="243.2" y="916.76" font-size="16">{"8"}</text>
        <text fill={COLOR.muted} class="s" x="64" y="968" font-size="14">
          {"Exports the timeline as it was when this window opened. Existing files are"}
        </text>
        <text fill={COLOR.muted} class="s" x="64" y="988" font-size="14">{"not replaced."}</text>
        <rect fill={COLOR.accent} x="64" y="1010" width="126.72" height="40" rx="6" />
        <text class="s" x="80" y="1035.76" font-size="16" fill="#080809">{"Export MP4…"}</text>
        <text fill={COLOR.text} class="s" x="64" y="1082.76" font-size="16">{"Export complete."}</text>
      </g>
      <text fill={COLOR.light} class="s" x="640" y="682" font-size="15" font-weight="600">
        {"Invalid bitrate"}
      </text>
      <text class="s" x="640" y="702" font-size="12" fill="#8a8a93">{"Export disabled"}</text>
      <rect x="640" y="718" width="560" height="548" rx="10" fill="#080809" filter="url(#shadow)" />
      <g clip-path="url(#c4)">
        <rect x="640" y="718" width="560" height="28" fill="#232326" />
        <rect x="640" y="745" width="560" height="1" fill="#000000" />
        <circle cx="654" cy="732" r="6" fill="#ff5f57" />
        <circle cx="674" cy="732" r="6" fill="#febc2e" />
        <circle cx="694" cy="732" r="6" fill="#28c840" />
        <rect x="640" y="746" width="560" height="520" fill="#080809" />
        <text fill={COLOR.text} class="s" x="664" y="789.2" font-size="20">{"Export timeline"}</text>
        <text fill={COLOR.text} class="s" x="664" y="826.76" font-size="16">{"edit.timeline"}</text>
        <text fill={COLOR.muted} class="s" x="664" y="863.04" font-size="14">
          {"1920 × 1080 · 30/1 fps · MP4 / H.264"}
        </text>
        <text fill={COLOR.text} class="s" x="664" y="916.76" font-size="16">{"Video bitrate (Mbps)"}</text>
        <rect
          fill={COLOR.surface}
          stroke={COLOR.border}
          x="827.7"
          y="884.5"
          width="139"
          height="53"
          rx="8"
          stroke-width="1"
        />
        <text fill={COLOR.text} class="m" x="843.2" y="916.76" font-size="16">{"0"}</text>
        <rect fill={COLOR.accent} x="853.8" y="901" width="1.5" height="20" />
        <text fill={COLOR.error} class="s" x="664" y="969.04" font-size="14">
          {"Enter a bitrate between 0.1 and 1000 Mbps."}
        </text>
        <text fill={COLOR.muted} class="s" x="664" y="1004" font-size="14">
          {"Exports the timeline as it was when this window opened. Existing files are"}
        </text>
        <text fill={COLOR.muted} class="s" x="664" y="1024" font-size="14">{"not replaced."}</text>
        <rect fill={COLOR.muted} x="664" y="1046" width="126.72" height="40" rx="6" />
        <text class="s" x="680" y="1071.76" font-size="16" fill="#080809">{"Export MP4…"}</text>
      </g>
    </svg>
  );
}

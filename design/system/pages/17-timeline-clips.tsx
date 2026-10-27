import { COLOR } from "../theme.ts";
import { Page } from "../components/page.tsx";
import { FontStyle } from "../components/fonts.tsx";
import { WarningIcon } from "../components/warning-icon.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

export default function Page17TimelineClipsSvg(): Markup {
  return (
    <Page
      width={1240}
      height={440}
      title="Timeline Clips"
      subtitle="Clip states in the timeline."
    >
      <FontStyle />
      <OfflineClipSection top={130} />
    </Page>
  );
}

function OfflineClipSection(props: { top: number }): Markup {
  const { top } = props;
  const stripX = 48;
  const stripY = top + 112;
  return (
    <g id="offline-clip">
      <rect fill={COLOR.border} x="48" y={top} width="1144" height="1" />
      <text fill={COLOR.text} class="s" x="48" y={top + 58} font-size="20" font-weight="700">{"Offline Clip"}</text>
      <text fill={COLOR.muted} class="s" x="48" y={top + 84} font-size="14" font-weight="400">
        {"An offline clip keeps its place, length and name."}
      </text>
      <rect x={stripX} y={stripY} width="640" height="80" rx="8" fill={COLOR.panel} />
      <rect x={stripX + 8} y={stripY + 8} width="200" height="64" rx="6" fill="#294d75" />
      <rect x={stripX + 8.5} y={stripY + 8.5} width="199" height="63" rx="6" fill="none" stroke="#395d85" />
      <text fill={COLOR.text} class="s" x={stripX + 16} y={stripY + 60} font-size="12" font-weight="600">
        {"broll-street.mov"}
      </text>
      <rect x={stripX + 210} y={stripY + 8} width="260" height="64" rx="6" fill="#2a1a1c" />
      <rect
        x={stripX + 210.5}
        y={stripY + 8.5}
        width="259"
        height="63"
        rx="6"
        fill="none"
        stroke={COLOR.error}
        stroke-opacity="0.7"
        stroke-dasharray="4 3"
      />
      <WarningIcon x={stripX + 218} y={stripY + 47} />
      <text fill={COLOR.text} class="s" x={stripX + 238} y={stripY + 59} font-size="12" font-weight="600">
        {"2026-08-20 18-36-50.mp4"}
      </text>
      <text fill={COLOR.error} class="s" x={stripX + 218} y={stripY + 30} font-size="11" font-weight="600">
        {"Media offline"}
      </text>
      <rect x={stripX + 472} y={stripY + 8} width="160" height="64" rx="6" fill="#294d75" />
      <rect x={stripX + 472.5} y={stripY + 8.5} width="159" height="63" rx="6" fill="none" stroke="#395d85" />
      <text fill={COLOR.text} class="s" x={stripX + 480} y={stripY + 60} font-size="12" font-weight="600">
        {"interview.mp4"}
      </text>
      <text fill={COLOR.muted} class="s" x="48" y={stripY + 116} font-size="13" font-weight="400">
        {"Dark red fill, dashed error border and a warning icon mark the offline clip."}
      </text>
      <text fill={COLOR.muted} class="s" x="48" y={stripY + 138} font-size="13" font-weight="400">
        {"Selecting it shows Relink… in its property panel."}
      </text>
    </g>
  );
}

import { COLOR } from "../theme.ts";
import { Page } from "../components/page.tsx";
import { FontStyle } from "../components/fonts.tsx";
import { DialogButton, dialogButtonWidth, DialogFrame } from "../components/dialog.tsx";
import { ClipSubtitle, PROPERTY_PANEL_HEADER_HEIGHT, PropertyPanelHeader } from "../components/property-panel-header.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

const OFFLINE_FILL = "#2a1a1c";

export default function MissingMediaPage(): Markup {
  return (
    <Page
      width={1240}
      height={1430}
      title="Missing Media"
      subtitle="A timeline with missing media still opens. Its clips show as offline until the file is relinked."
    >
      <FontStyle />
      <Row y={150} title="Opening a timeline" description="Shown once when a timeline opens with missing files." />
      <MissingFilesDialog x={48} y={210} />
      <Notes
        x={580}
        y={230}
        lines={[
          "Relink… opens a file picker for the first missing file.",
          "Later closes the dialog; the clips stay offline.",
          "Each row is one missing file and how many clips use it.",
        ]}
      />

      <Row y={520} title="Timeline" description="An offline clip keeps its place, length and name." />
      <TimelineStrip x={48} y={580} />
      <Notes
        x={720}
        y={600}
        lines={[
          "Offline: dark red fill, dashed error border, warning icon.",
          "Selecting it shows Relink… in its property panel.",
        ]}
      />

      <Row
        y={720}
        title="Preview and property panel"
        description="With the playhead over an offline clip, and the clip selected."
      />
      <OfflinePreview x={48} y={780} />
      <OfflinePropertyPanel x={600} y={780} />
      <Notes
        x={600}
        y={1010}
        lines={[
          "The preview shows a message, not a stale or black frame.",
          "The rest of the timeline keeps playing normally.",
          "Relink… in the MEDIA section opens the file picker.",
        ]}
      />

      <Row
        y={1140}
        title="After relinking"
        description="Asked when other missing files are found next to the relinked one."
      />
      <RelinkOthersDialog x={48} y={1200} />
      <Notes
        x={580}
        y={1220}
        lines={[
          "Relink updates every clip that uses those files.",
          "Skip relinks only the file that was picked.",
          "Relinking is undoable like any other edit.",
        ]}
      />
    </Page>
  );
}

function Row(props: { y: number; title: string; description: string }): Markup {
  return (
    <g>
      <text fill={COLOR.text} class="s" x="48" y={props.y} font-size="18" font-weight="600">{props.title}</text>
      <text fill={COLOR.muted} class="s" x="48" y={props.y + 24} font-size="12">{props.description}</text>
    </g>
  );
}

/// Notes beside a mock-up, outside the UI.
function Notes(props: { x: number; y: number; lines: string[] }): Markup {
  return (
    <g>
      {props.lines.map((line, index) => (
        <text fill={COLOR.subtle} class="s" x={props.x} y={props.y + index * 22} font-size="13">
          {`· ${line}`}
        </text>
      ))}
    </g>
  );
}

function WarningIcon(props: { x: number; y: number; size?: number }): Markup {
  const scale = (props.size ?? 14) / 24;
  return (
    <g
      transform={`translate(${props.x} ${props.y}) scale(${scale})`}
      fill="none"
      stroke={COLOR.error}
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z" />
      <path d="M12 9v4" />
      <path d="M12 17h.01" />
    </g>
  );
}

function MissingFilesDialog(props: { x: number; y: number }): Markup {
  const { x, y } = props;
  const width = 480;
  const files = [
    { name: "2026-08-20 18-36-50.mp4", clips: "3 clips" },
    { name: "2026-08-20 18-38-56.mp4", clips: "1 clip" },
  ];
  const right = x + width - 20;
  return (
    <DialogFrame x={x} y={y} width={width} height={236} title="2 media files are missing">
      <text fill={COLOR.muted} class="s" x={x + 20} y={y + 72} font-size="14">
        {"Clips that use them stay on the timeline, shown as offline."}
      </text>
      <rect
        x={x + 20.5}
        y={y + 90.5}
        width={width - 41}
        height="71"
        rx="8"
        fill={COLOR.surface}
        stroke={COLOR.border}
      />
      {files.map((file, index) => (
        <g>
          <WarningIcon x={x + 34} y={y + 104 + index * 30} />
          <text fill={COLOR.text} class="m" x={x + 58} y={y + 116 + index * 30} font-size="13">{file.name}</text>
          <text fill={COLOR.muted} class="s" x={right - 14} y={y + 116 + index * 30} font-size="12" text-anchor="end">
            {file.clips}
          </text>
        </g>
      ))}
      <DialogButton right={right} y={y + 178} label="Relink…" primary />
      <DialogButton right={right - dialogButtonWidth("Relink…") - 8} y={y + 178} label="Later" />
    </DialogFrame>
  );
}

function TimelineStrip(props: { x: number; y: number }): Markup {
  const { x, y } = props;
  return (
    <g>
      <rect x={x} y={y} width="640" height="80" rx="8" fill={COLOR.panel} />
      <rect x={x + 8} y={y + 8} width="200" height="64" rx="6" fill="#294d75" />
      <rect x={x + 8.5} y={y + 8.5} width="199" height="63" rx="6" fill="none" stroke="#395d85" />
      <text fill={COLOR.text} class="s" x={x + 16} y={y + 60} font-size="12" font-weight="600">
        {"broll-street.mov"}
      </text>
      <rect x={x + 210} y={y + 8} width="260" height="64" rx="6" fill={OFFLINE_FILL} />
      <rect
        x={x + 210.5}
        y={y + 8.5}
        width="259"
        height="63"
        rx="6"
        fill="none"
        stroke={COLOR.error}
        stroke-opacity="0.7"
        stroke-dasharray="4 3"
      />
      <WarningIcon x={x + 218} y={y + 47} />
      <text fill={COLOR.text} class="s" x={x + 238} y={y + 59} font-size="12" font-weight="600">
        {"2026-08-20 18-36-50.mp4"}
      </text>
      <text fill={COLOR.error} class="s" x={x + 218} y={y + 30} font-size="11" font-weight="600">
        {"Media offline"}
      </text>
      <rect x={x + 472} y={y + 8} width="160" height="64" rx="6" fill="#294d75" />
      <rect x={x + 472.5} y={y + 8.5} width="159" height="63" rx="6" fill="none" stroke="#395d85" />
      <text fill={COLOR.text} class="s" x={x + 480} y={y + 60} font-size="12" font-weight="600">{"interview.mp4"}</text>
    </g>
  );
}

function OfflinePreview(props: { x: number; y: number }): Markup {
  const { x, y } = props;
  const width = 512;
  const height = 288;
  return (
    <g>
      <rect x={x} y={y} width={width} height={height} rx="8" fill="#080808" />
      <WarningIcon x={x + width / 2 - 12} y={y + 104} size={24} />
      <text
        fill={COLOR.text}
        class="s"
        x={x + width / 2}
        y={y + 156}
        font-size="16"
        font-weight="600"
        text-anchor="middle"
      >
        {"Media offline"}
      </text>
      <text fill={COLOR.muted} class="m" x={x + width / 2} y={y + 180} font-size="13" text-anchor="middle">
        {"2026-08-20 18-36-50.mp4"}
      </text>
    </g>
  );
}

function OfflinePropertyPanel(props: { x: number; y: number }): Markup {
  const { x, y } = props;
  const top = y + PROPERTY_PANEL_HEADER_HEIGHT;
  return (
    <g>
      <rect x={x} y={y} width="280" height="200" rx="10" fill={COLOR.panel} filter="url(#shadow)" />
      <clipPath id="offline-panel-clip">
        <rect x={x} y={y} width="280" height="200" rx="10" />
      </clipPath>
      <g clip-path="url(#offline-panel-clip)">
        <PropertyPanelHeader
          x={x}
          y={y}
          icon={{ type: "track", kind: "V" }}
          title="2026-08-20 18-36-50.mp4"
          subtitle={<ClipSubtitle clipType="Video clip" track="Video 1" kind="V" />}
        />
      </g>
      <text fill={COLOR.muted} class="s" x={x + 16} y={top + 24} font-size="11" font-weight="600" letter-spacing="0.6">
        {"MEDIA"}
      </text>
      <text fill={COLOR.accent} class="s" x={x + 264} y={top + 24} font-size="12" text-anchor="end">{"Relink…"}</text>
      <WarningIcon x={x + 16} y={top + 42} />
      <text fill={COLOR.error} class="s" x={x + 36} y={top + 54} font-size="13" font-weight="600">
        {"File not found"}
      </text>
      <text fill={COLOR.soft} class="m" x={x + 16} y={top + 80} font-size="12">{"/Users/me/Movies/OpenCut Dev"}</text>
      <text fill={COLOR.soft} class="m" x={x + 16} y={top + 98} font-size="12">{"Logs/timelines/2026-08-20"}</text>
      <text fill={COLOR.soft} class="m" x={x + 16} y={top + 116} font-size="12">{"18-36-50.mp4"}</text>
      <rect x={x + 0.5} y={y + 0.5} width="279" height="199" rx="10" fill="none" stroke={COLOR.border} />
    </g>
  );
}

function RelinkOthersDialog(props: { x: number; y: number }): Markup {
  const { x, y } = props;
  const width = 480;
  const right = x + width - 20;
  return (
    <DialogFrame x={x} y={y} width={width} height={176} title="Relink 1 more file?">
      <text fill={COLOR.muted} class="s" x={x + 20} y={y + 72} font-size="14">
        {"Another missing file is in the same folder:"}
      </text>
      <text fill={COLOR.text} class="m" x={x + 20} y={y + 98} font-size="13">{"2026-08-20 18-38-56.mp4"}</text>
      <DialogButton right={right} y={y + 118} label="Relink" primary />
      <DialogButton right={right - dialogButtonWidth("Relink") - 8} y={y + 118} label="Skip" />
    </DialogFrame>
  );
}

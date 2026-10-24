import { COLOR } from "../theme.ts";
import { Fragment } from "@design/jsx/jsx-runtime";
import type { Markup } from "@design/jsx/jsx-runtime";

/// Height of the header including its bottom divider: 12px padding around the
/// 32px icon, then the 1px divider.
export const PROPERTY_PANEL_HEADER_HEIGHT = 56;

export type PanelTrackKind = "V" | "A" | "T";

export type PropertyPanelIcon =
  | { type: "track"; kind: PanelTrackKind }
  | { type: "locked" }
  | { type: "file"; label: string; labelColor: string; stroke: string }
  | { type: "timeline" };

const TRACK_ICON: Record<PanelTrackKind, { fill: string; stroke: string; name: string }> = {
  V: { fill: "#294d75", stroke: "#395d85", name: "#91b8df" },
  A: { fill: "#24656b", stroke: "#34757b", name: "#69c5cf" },
  T: { fill: "#7251a3", stroke: "#8261b3", name: "#b69bdc" },
};

const LOCKED_HEADER_BACKGROUND = "#242429";
const LOCKED_BORDER = "#62626e";
const SUBTITLE_LABEL = "#8a8a93";

/// The header of every property panel: icon, title and subtitle above a divider.
/// A locked header has a grey background and a lighter divider.
export function PropertyPanelHeader(props: {
  x: number;
  y: number;
  width?: number;
  icon: PropertyPanelIcon;
  title: string;
  subtitle: Markup;
  locked?: boolean;
  divider?: boolean; // false when tabs follow, since their top edge divides
}): Markup {
  const { x, y } = props;
  const width = props.width ?? 280;
  return (
    <g>
      {props.locked && (
        <rect
          x={x}
          y={y}
          width={width}
          height={PROPERTY_PANEL_HEADER_HEIGHT}
          fill={LOCKED_HEADER_BACKGROUND}
        />
      )}
      <PanelIcon x={x + 12} y={y + 12} icon={props.icon} />
      <text fill={COLOR.text} class="s" x={x + 56} y={y + 25.04} font-size="14" font-weight="600">
        {props.title}
      </text>
      <g transform={`translate(${x + 56} ${y + 41.32})`}>{props.subtitle}</g>
      {props.divider !== false && (
        <rect
          x={x}
          y={y + PROPERTY_PANEL_HEADER_HEIGHT - 1}
          width={width}
          height="1"
          fill={props.locked ? LOCKED_BORDER : COLOR.border}
        />
      )}
    </g>
  );
}

/// "Video clip · Track: Video 1", with the track name in its track colour.
export function ClipSubtitle(props: {
  clipType: string;
  track: string;
  kind: PanelTrackKind;
  locked?: boolean;
}): Markup {
  return (
    <text class="s" font-size="11">
      <tspan fill={SUBTITLE_LABEL}>{`${props.clipType} · Track: `}</tspan>
      <tspan fill={TRACK_ICON[props.kind].name}>{props.track}</tspan>
      {props.locked && (
        <Fragment>
          <tspan fill={SUBTITLE_LABEL}>{" · "}</tspan>
          <tspan fill={COLOR.light}>{"Locked"}</tspan>
        </Fragment>
      )}
    </text>
  );
}

/// A plain subtitle, such as a file type or a path.
export function PlainSubtitle(props: { children: string }): Markup {
  return <text fill={COLOR.muted} class="s" font-size="12">{props.children}</text>;
}

function PanelIcon(props: { x: number; y: number; icon: PropertyPanelIcon }): Markup {
  const { x, y, icon } = props;
  const box = (fill: string, stroke: string) => (
    <rect
      x={x + 0.5}
      y={y + 0.5}
      width="31"
      height="31"
      rx="8"
      fill={fill}
      stroke={stroke}
      stroke-width="1"
    />
  );
  switch (icon.type) {
    case "track":
      return (
        <g>
          {box(TRACK_ICON[icon.kind].fill, TRACK_ICON[icon.kind].stroke)}
          <text
            class="s"
            x={x + 16}
            y={y + 20.32}
            font-size="12"
            fill="#ffffff"
            font-weight="700"
            text-anchor="middle"
          >
            {icon.kind}
          </text>
        </g>
      );
    case "locked":
      return (
        <g>
          {box("#33333b", LOCKED_BORDER)}
          <g
            stroke={COLOR.light}
            transform={`translate(${x + 8} ${y + 7})`}
            fill="none"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <rect x="1" y="7" width="14" height="11" rx="2" />
            <path d="M4 7V4a4 4 0 0 1 8 0v3 M8 11v3" />
          </g>
        </g>
      );
    case "file":
      return (
        <g>
          {box("#131316", icon.stroke)}
          <text
            class="m"
            x={x + 16}
            y={y + 19.24}
            font-size="9"
            fill={icon.labelColor}
            font-weight="600"
            text-anchor="middle"
          >
            {icon.label}
          </text>
        </g>
      );
    case "timeline":
      return (
        <g>
          {box(COLOR.accent, COLOR.accent)}
          <text
            class="s"
            x={x + 16}
            y={y + 19.96}
            font-size="11"
            fill="#17120a"
            font-weight="700"
            text-anchor="middle"
          >
            {"TL"}
          </text>
        </g>
      );
  }
}

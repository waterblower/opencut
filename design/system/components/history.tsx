import { COLOR, TRACK_KIND_COLOR, type TrackKind } from "../theme.ts";
import { Text } from "./text.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

export type HistoryEntryState = "undone" | "current" | "earlier" | "hovered";

export type HistoryEntry = {
  kind: TrackKind;
  target: string; // clip name, or track name for track edits
  action: string;
  change?: string; // value change shown in the current entry's chip
  track: string;
  time: string;
  state: HistoryEntryState;
};

export function TrackBadge(props: { x: number; y: number; kind: TrackKind }): Markup {
  const color = TRACK_KIND_COLOR[props.kind];
  return (
    <g>
      <rect x={props.x} y={props.y} width={16} height={16} rx={4} fill={color.fill} />
      <Text x={props.x + 8} y={props.y + 12} size={10} color={color.text} weight={700} anchor="middle">
        {props.kind}
      </Text>
    </g>
  );
}

export function ValueChip(props: { x: number; y: number; children: string }): Markup {
  const width = props.children.length * 6.7 + 12;
  return (
    <g>
      <rect
        x={props.x}
        y={props.y}
        width={width}
        height={17}
        rx={4}
        fill={COLOR.surface}
        stroke={COLOR.border}
      />
      <Text x={props.x + 6} y={props.y + 12} size={11} color={COLOR.light} mono>
        {props.children}
      </Text>
    </g>
  );
}

function RailDot(props: { x: number; y: number; state: HistoryEntryState }): Markup {
  switch (props.state) {
    case "current":
      return <circle cx={props.x} cy={props.y} r={5} fill={COLOR.accent} />;
    case "earlier":
    case "hovered":
      return <circle cx={props.x} cy={props.y} r={4} fill={COLOR.rail} />;
    case "undone":
      return <circle cx={props.x} cy={props.y} r={4} fill={COLOR.panel} stroke={COLOR.rail} />;
  }
}

const ROW_HEIGHT = 40;
const CURRENT_ROW_HEIGHT = 140;

function rowHeight(entry: HistoryEntry): number {
  return entry.state === "current" ? CURRENT_ROW_HEIGHT : ROW_HEIGHT;
}

/// The history list, newest entry first. The current entry expands with its change,
/// track and the Fork button.
export function HistoryList(props: { x: number; y: number; width: number; entries: HistoryEntry[] }): Markup {
  const { x, width } = props;
  const railX = x + 26;
  const tops = props.entries.reduce<number[]>(
    (
      result,
      entry,
      index,
    ) => [...result, index === 0 ? props.y : result[index - 1] + rowHeight(props.entries[index - 1])],
    [],
  );
  const lastTop = tops[tops.length - 1];
  return (
    <g>
      <rect x={railX - 1} y={props.y + 20} width={2} height={lastTop - props.y} fill={COLOR.border} />
      {props.entries.map((entry, index) => {
        const y = tops[index];
        const isCurrent = entry.state === "current";
        return (
          <g>
            {isCurrent && (
              <rect
                x={x + 8}
                y={y}
                width={width - 16}
                height={CURRENT_ROW_HEIGHT - 6}
                rx={6}
                fill={COLOR.accentBackground}
                stroke={COLOR.accent}
              />
            )}
            {entry.state === "hovered" && (
              <rect x={x + 8} y={y + 2} width={width - 16} height={36} rx={6} fill={COLOR.hover} />
            )}
            <RailDot x={railX} y={y + 20} state={entry.state} />
            <g opacity={entry.state === "undone" ? 0.45 : undefined}>
              <TrackBadge x={x + 44} y={y + 12} kind={entry.kind} />
              <Text x={x + 68} y={y + 25} weight={isCurrent ? 600 : 400}>{entry.target}</Text>
              {!isCurrent && (
                <Text x={x + 68 + entry.target.length * 6.4 + 10} y={y + 25} size={12} color={COLOR.muted}>
                  {entry.action}
                </Text>
              )}
            </g>
            <Text x={x + width - 18} y={y + 25} size={11} color={COLOR.dim} mono anchor="end">
              {entry.time}
            </Text>
            {isCurrent && (
              <g>
                <Text x={x + 44} y={y + 56} size={12} color={COLOR.muted}>{entry.action}</Text>
                {entry.change && (
                  <ValueChip x={x + 44 + entry.action.length * 6.3 + 8} y={y + 44}>{entry.change}</ValueChip>
                )}
                <Text x={x + 44} y={y + 82} size={12} color={COLOR.muted}>Track</Text>
                <Text x={x + width - 24} y={y + 82} size={12} color={COLOR.light} anchor="end">
                  {entry.track}
                </Text>
                <rect
                  x={x + width - 166}
                  y={y + 96}
                  width={142}
                  height={30}
                  rx={6}
                  fill={COLOR.surface}
                  stroke={COLOR.border}
                />
                <Text x={x + width - 95} y={y + 116} size={12} anchor="middle">Fork from here…</Text>
              </g>
            )}
          </g>
        );
      })}
    </g>
  );
}

export const SAMPLE_HISTORY: HistoryEntry[] = [
  { kind: "A", target: "Audio 1", action: "Muted", track: "Audio 1", time: "10:08", state: "undone" },
  { kind: "T", target: "Text 2", action: "Track deleted", track: "Text 2", time: "10:08", state: "undone" },
  {
    kind: "A",
    target: "interview",
    action: "Gain",
    change: "0.00 → −3.00 dB",
    track: "Audio 1",
    time: "10:07",
    state: "current",
  },
  {
    kind: "T",
    target: "Let's get started",
    action: "Font size",
    track: "Text 1",
    time: "10:06",
    state: "earlier",
  },
  {
    kind: "T",
    target: "Let's get started",
    action: "Text",
    track: "Text 1",
    time: "10:06",
    state: "earlier",
  },
  { kind: "V", target: "broll-cafe", action: "Split", track: "Video 1", time: "10:05", state: "hovered" },
  {
    kind: "V",
    target: "broll-cafe",
    action: "Trimmed out",
    track: "Video 1",
    time: "10:03",
    state: "earlier",
  },
  { kind: "V", target: "broll-cafe", action: "Moved", track: "Video 1", time: "10:03", state: "earlier" },
  { kind: "V", target: "broll-cafe", action: "Added", track: "Video 1", time: "10:02", state: "earlier" },
];

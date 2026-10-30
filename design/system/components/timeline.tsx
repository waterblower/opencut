import { COLOR } from "../theme.ts";
import { Icon, type IconName } from "./icons/icons.tsx";
import { type ToggleState, TrackHeader, type TrackKind } from "./track-header.tsx";
import { raw } from "@design/jsx/jsx-runtime";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

const PLAYHEAD_FLAG = raw(Deno.readTextFileSync(new URL("./icons/playhead-flag.svg", import.meta.url)));

export const HEADER_WIDTH = 190;
export const ROW_HEIGHT = 74;
const RULER_HEIGHT = 28; // 27px of ruler and its 1px bottom border
const TOOLBAR_HEIGHT = 46; // 45px of toolbar and its 1px bottom border

// Where a text baseline sits below the top of a `leading-none` box, per font size, so a
// label can be placed by its baseline as the old drawings did.
const SANS_BASELINE = 0.8605;
const MONO_BASELINE = 0.8333;

function textTop(baseline: number, size: number, family: "sans" | "mono"): number {
  return baseline - size * (family === "sans" ? SANS_BASELINE : MONO_BASELINE);
}

export type RulerSpec = {
  labels: string[];
  labelStep: number; // px between labels; the first is 20px into the lane
  fontSize: number;
  baseline: number; // from the top of the ruler
  labelTick: { height: number; color: string };
  minor?: { first: number; step: number; height: number };
  major?: { first: number; step: number; height: number };
};

const TICK_GREY = COLOR.grey;
const TICK_MAJOR = "#5a5a62";

export const MAIN_RULER: RulerSpec = {
  labels: ["00:00.0", "00:05.0", "00:10.0", "00:15.0", "00:20.0", "00:25.0", "00:30.0", "00:35.0"],
  labelStep: 120,
  fontSize: 12,
  baseline: 14,
  labelTick: { height: 27, color: "#333338" },
  minor: { first: 24, step: 4, height: 8 },
  major: { first: 44, step: 24, height: 12 },
};

export const COMPACT_RULER: RulerSpec = {
  labels: ["00:00.0", "00:02.0", "00:04.0", "00:06.0", "00:08.0"],
  labelStep: 80,
  fontSize: 12,
  baseline: 14,
  labelTick: { height: 27, color: "#333338" },
  minor: { first: 24, step: 4, height: 5 },
  major: { first: 60, step: 40, height: 12 },
};

export const PLACEMENT_RULER: RulerSpec = {
  labels: ["00:00", "00:02", "00:04", "00:06", "00:08"],
  labelStep: 76,
  fontSize: 11,
  baseline: 16,
  labelTick: { height: 7, color: TICK_MAJOR },
  minor: { first: 39, step: 19, height: 3 },
};

export type TrackSpec = {
  kind: TrackKind;
  name: string;
  lock?: ToggleState;
  visibility?: ToggleState;
  mute?: ToggleState;
  locked?: boolean; // covers the header and the lane with neutral stripes
  tone?: "alternate" | "panel";
};

/// A timeline mock-up: a card with an optional toolbar, a ruler and track rows. Each row is a
/// header on the left and a lane on the right. `lane` holds absolutely positioned clips and
/// guides in lane coordinates, with row N starting at N × 74px; `overlay` holds things such as
/// cursors, in coordinates from the card's top-left corner.
export function Timeline(props: {
  width: number;
  tracks: TrackSpec[];
  toolbar?: boolean;
  ruler?: RulerSpec | null;
  height?: number;
  playhead?: number; // x in the lane
  lane?: Child;
  overlay?: Child;
  class?: string;
}): Markup {
  const ruler = props.ruler === undefined ? COMPACT_RULER : props.ruler;
  const rulerTop = props.toolbar ? TOOLBAR_HEIGHT : 0;
  const lanesTop = rulerTop + (ruler ? RULER_HEIGHT : 0);
  const laneWidth = props.width - HEADER_WIDTH;
  const lanesHeight = props.tracks.length * ROW_HEIGHT;
  const height = props.height ?? lanesTop + lanesHeight;
  return (
    <div
      class={`relative overflow-hidden rounded-[10px] bg-[#0a0a0c] font-sans ${props.class ?? ""}`}
      style={`width:${props.width}px;height:${height}px`}
    >
      {props.toolbar && <TimelineToolbar />}
      {props.toolbar && <div class="absolute top-[45px] left-0 h-px w-full bg-border" />}
      <div
        class="absolute left-[189px] w-px bg-border"
        style={`top:${rulerTop}px;height:${height - rulerTop}px`}
      />
      {props.tracks.map((track, index) => {
        const top = lanesTop + index * ROW_HEIGHT;
        const dark = track.tone === "panel" || index % 2 === 1;
        return (
          <div>
            <div
              class={`absolute h-[74px] border-b border-border ${dark ? "bg-panel" : "bg-[#101012]"}`}
              style={`left:${HEADER_WIDTH}px;top:${top}px;width:${laneWidth}px`}
            />
            <div class="absolute left-0" style={`top:${top}px`}>
              <TrackHeader
                kind={track.kind}
                name={track.name}
                lock={track.lock}
                visibility={track.visibility}
                mute={track.mute}
              />
            </div>
          </div>
        );
      })}
      {ruler && <TimelineRuler spec={ruler} top={rulerTop} width={props.width} />}
      <div
        class="absolute overflow-hidden"
        style={`left:${HEADER_WIDTH}px;top:${lanesTop}px;width:${laneWidth}px;height:${height - lanesTop}px`}
      >
        {props.lane}
        {props.playhead !== undefined && <PlayheadLine x={props.playhead} />}
      </div>
      {ruler && props.playhead !== undefined && <PlayheadFlag x={HEADER_WIDTH + props.playhead} top={rulerTop} />}
      {props.tracks.map((track, index) =>
        track.locked
          ? (
            <div>
              <div
                class="hatch-lock-row absolute"
                style={`left:${HEADER_WIDTH}px;top:${lanesTop + index * ROW_HEIGHT}px;width:${laneWidth}px;height:73px`}
              />
              <div
                class="hatch-lock-row absolute left-0"
                style={`top:${lanesTop + index * ROW_HEIGHT}px;width:189px;height:73px`}
              />
            </div>
          )
          : null
      )}
      {props.overlay}
      <div class="pointer-events-none absolute inset-0 rounded-[10px] border border-border" />
    </div>
  );
}

function TimelineRuler(props: { spec: RulerSpec; top: number; width: number }): Markup {
  const { spec } = props;
  const laneWidth = props.width - HEADER_WIDTH;
  const ticks = (start: number, step: number, height: number, color: string) => (
    <div
      class="absolute bottom-0"
      style={`left:${start}px;width:${
        laneWidth - start
      }px;height:${height}px;background-image:linear-gradient(to right,${color} 1px,transparent 1px);background-size:${step}px 100%`}
    />
  );
  return (
    <div>
      <div
        class="absolute overflow-hidden"
        style={`left:${HEADER_WIDTH}px;top:${props.top}px;width:${laneWidth}px;height:27px`}
      >
        {spec.minor && ticks(spec.minor.first, spec.minor.step, spec.minor.height, TICK_GREY)}
        {spec.major && ticks(spec.major.first, spec.major.step, spec.major.height, TICK_MAJOR)}
        {spec.labels.map((label, index) => {
          const x = 20 + index * spec.labelStep;
          return (
            <div>
              <div
                class="absolute bottom-0 w-px"
                style={`left:${x}px;height:${spec.labelTick.height}px;background:${spec.labelTick.color}`}
              />
              <span
                class="absolute font-mono leading-none whitespace-nowrap text-muted"
                style={`left:${x + 5}px;top:${
                  textTop(spec.baseline, spec.fontSize, "mono")
                }px;font-size:${spec.fontSize}px`}
              >
                {label}
              </span>
            </div>
          );
        })}
      </div>
      <div class="absolute left-0 h-px w-full bg-border" style={`top:${props.top + 27}px`} />
    </div>
  );
}

/// A one-pixel line at `x` in the lane with a faint three-pixel glow, for the playhead.
function PlayheadLine(props: { x: number }): Markup {
  return (
    <div>
      <div class="absolute top-0 h-full w-[3px] bg-accent/18" style={`left:${props.x - 1}px`} />
      <div class="absolute top-0 h-full w-px bg-accent" style={`left:${props.x}px`} />
    </div>
  );
}

/// The playhead's flag in the ruler, 14px wide, over the ruler's bottom edge.
function PlayheadFlag(props: { x: number; top: number }): Markup {
  return (
    <div>
      <span class="absolute" style={`left:${props.x - 6}px;top:${props.top + 13}px`}>{PLAYHEAD_FLAG}</span>
      <div class="absolute h-0.5 w-px bg-accent" style={`left:${props.x}px;top:${props.top + 26}px`} />
    </div>
  );
}

/// A guide line in the lane, full height, such as the blade guide or the snap guide.
export function LaneLine(props: { x: number; width?: number; color: string; opacity?: number }): Markup {
  return (
    <div
      class="absolute top-0 h-full"
      style={`left:${props.x}px;width:${props.width ?? 1}px;background:${props.color};opacity:${props.opacity ?? 1}`}
    />
  );
}

/// Places a clip in the lane: `row` counts from the first track, `left` is the x in the lane.
export function ClipAt(props: { left: number; row: number; children: Child }): Markup {
  return (
    <div class="absolute" style={`left:${props.left}px;top:${props.row * ROW_HEIGHT + 5}px`}>
      {props.children}
    </div>
  );
}

/// Positions something at (`left`, `top`) from the card's top-left corner.
export function At(props: { left: number; top: number; children: Child }): Markup {
  return (
    <div class="absolute" style={`left:${props.left}px;top:${props.top}px`}>
      {props.children}
    </div>
  );
}

// Toolbar --------------------------------------------------------------------------------

function ToolbarBox(props: { left: number; width: number; children: Child }): Markup {
  return (
    <div
      class="absolute top-[9px] h-7 rounded-[7.5px] border border-border bg-surface"
      style={`left:${props.left}px;width:${props.width}px`}
    >
      {props.children}
    </div>
  );
}

function ToolbarIcon(props: { left: number; name: IconName; color: string }): Markup {
  return <Icon name={props.name} color={props.color} class="absolute top-4" style={`left:${props.left}px`} />;
}

function ToolbarText(
  props: { left: number; color: string; mono?: boolean; size?: number; opacity?: number; children: string },
): Markup {
  const size = props.size ?? 12;
  return (
    <span
      class={`absolute leading-none whitespace-nowrap ${props.mono ? "font-mono" : ""}`}
      style={`left:${props.left}px;top:${
        textTop(27, size, props.mono ? "mono" : "sans")
      }px;font-size:${size}px;color:${props.color};opacity:${props.opacity ?? 1}`}
    >
      {props.children}
    </span>
  );
}

function ToolbarDivider(props: { left: number; top?: number; height?: number }): Markup {
  return (
    <div
      class="absolute w-px bg-border"
      style={`left:${props.left}px;top:${props.top ?? 15}px;height:${props.height ?? 16}px`}
    />
  );
}

/// A highlighted toggle: its 24px pill sits one pixel inside the box.
function ToolbarToggle(props: { left: number; width: number; icon: IconName; label: string }): Markup {
  return (
    <div>
      <ToolbarBox left={props.left} width={props.width}>{null}</ToolbarBox>
      <div
        class="absolute top-[11px] h-6 rounded-[5px] bg-accent/16"
        style={`left:${props.left + 1.5}px;width:${props.width - 3}px`}
      />
      <ToolbarIcon left={props.left + 10} name={props.icon} color={COLOR.accent} />
      <ToolbarText left={props.left + 30} color={COLOR.accent}>{props.label}</ToolbarText>
    </div>
  );
}

function TimelineToolbar(): Markup {
  return (
    <div class="absolute top-0 left-0 h-[45px] w-full">
      <ToolbarBox left={12} width={178.6}>{null}</ToolbarBox>
      <div class="absolute top-[11px] left-[14px] h-6 w-[88.6px] rounded-[5px] bg-accent/16" />
      <ToolbarIcon left={22} name="pointer" color={COLOR.accent} />
      <ToolbarText left={42} color={COLOR.accent}>Select</ToolbarText>
      <ToolbarText left={87.6} color={COLOR.accent} mono size={11} opacity={0.6}>V</ToolbarText>
      <ToolbarIcon left={114.6} name="scissors" color={COLOR.muted} />
      <ToolbarText left={134.6} color={COLOR.muted}>Blade</ToolbarText>
      <ToolbarText left={173.6} color={COLOR.dim} mono size={11}>B</ToolbarText>
      <ToolbarDivider left={202.6} />
      <span
        class="absolute font-mono leading-none whitespace-nowrap text-text"
        style={`left:215.6px;top:${textTop(28, 14, "mono")}px;font-size:14px`}
      >
        0:21<span class="text-muted">{" / 0:34"}</span>
      </span>
      <ToolbarDivider left={320} />
      <ToolbarBox left={333} width={212.4}>{null}</ToolbarBox>
      <ToolbarIcon left={343} name="plus" color={COLOR.muted} />
      <ToolbarText left={363} color={COLOR.text}>Video</ToolbarText>
      <ToolbarDivider left={406} top={16} height={14} />
      <ToolbarIcon left={416} name="plus" color={COLOR.muted} />
      <ToolbarText left={436} color={COLOR.text}>Audio</ToolbarText>
      <ToolbarDivider left={479} top={16} height={14} />
      <ToolbarIcon left={489} name="plus" color={COLOR.muted} />
      <ToolbarText left={509} color={COLOR.text}>Text</ToolbarText>
      <ToolbarDivider left={557.4} />
      <ToolbarToggle left={570.4} width={66.4} icon="snap" label="Snap" />
      <ToolbarToggle left={644.8} width={79.6} icon="magnet" label="Magnet" />
      <span
        class="absolute leading-none font-mono whitespace-nowrap text-muted"
        style={`right:169.6px;top:${textTop(27, 12, "mono")}px;font-size:12px`}
      >
        30.00 fps
      </span>
      <ToolbarBox left={986.4} width={145.6}>{null}</ToolbarBox>
      <ToolbarIcon left={996.4} name="minus" color={COLOR.muted} />
      <ToolbarDivider left={1020.4} top={16} height={14} />
      <ToolbarText left={1030.4} color={COLOR.muted} mono>24.0px/s</ToolbarText>
      <ToolbarDivider left={1098} top={16} height={14} />
      <ToolbarIcon left={1108} name="plus" color={COLOR.muted} />
    </div>
  );
}

// Cursors and drop previews ---------------------------------------------------------------

export type CursorKind = "arrow" | "hand" | "resize" | "marquee" | "blocked";

/// A mouse cursor drawn at (`left`, `top`) from the card's top-left corner. The offset is the
/// cursor's hot spot.
export function Cursor(props: { kind: CursorKind; left: number; top: number }): Markup {
  return (
    <svg
      aria-hidden="true"
      class="absolute overflow-visible"
      style={`left:${props.left}px;top:${props.top}px`}
      width="30"
      height="30"
      viewBox="0 0 30 30"
    >
      {props.kind === "arrow" && (
        <path
          d="M0 0V18L5 14 9 23 12 21 8 12H15Z"
          fill="#fff"
          stroke="#080809"
          stroke-width="1.5"
          stroke-linejoin="round"
        />
      )}
      {props.kind === "hand" && (
        <path
          d="M3 10V6Q3 3 6 3H8V2Q8 0 11 1L13 2Q16 0 18 3Q22 2 23 6L25 11V19Q24 25 19 27H10L2 19Q-1 16 1 14Q3 12 7 16V10Z"
          fill="#fff"
          stroke="#080809"
          stroke-width="1.5"
          stroke-linejoin="round"
        />
      )}
      {props.kind === "marquee" && (
        <path
          d="M0 0 v16 l4 -4 l3 6 l2.5 -1 l-3 -6 h6 Z"
          fill="#fff"
          stroke="#000"
          stroke-width="1"
          stroke-linejoin="round"
        />
      )}
      {props.kind === "resize" && (
        <g>
          <rect x="9.5" y="0" width="1" height="20" fill="#fff" stroke="#000" stroke-width="0.5" />
          <path
            d="M0 10 l5 -5 v3 h10 v-3 l5 5 l-5 5 v-3 h-10 v3 Z"
            fill="#fff"
            stroke="#000"
            stroke-width="1"
            stroke-linejoin="round"
          />
        </g>
      )}
      {props.kind === "blocked" && (
        <g>
          <path
            d="M0 0V18L5 14 9 23 12 21 8 12H15Z"
            fill="#fff"
            stroke="#080809"
            stroke-width="1.5"
            stroke-linejoin="round"
          />
          <circle cx="18" cy="20" r="7" fill="#b73b49" stroke="#fff" />
          <path d="M13 25l10 -10" stroke="#fff" stroke-width="1.5" />
        </g>
      )}
    </svg>
  );
}

/// The clip being placed: amber when it can go there, red and hatched when it cannot. The
/// 1.5px border is an inset shadow, because a CSS border would round down to 1px.
export function GhostClip(props: {
  width: number;
  accepted: boolean;
  name: string;
  caption: string;
}): Markup {
  return (
    <div
      class={`relative h-16 overflow-hidden rounded-md ${
        props.accepted ? "bg-accent/14 text-accent" : "bg-[#321b20]/90 text-error"
      }`}
      style={`width:${props.width}px`}
    >
      {!props.accepted && <div class="hatch-rejected absolute inset-0" />}
      <div
        class={`absolute inset-0 rounded-md ${
          props.accepted
            ? "shadow-[inset_0_0_0_1.5px_var(--color-accent)]"
            : "shadow-[inset_0_0_0_1.5px_var(--color-error)]"
        }`}
      />
      <span
        class="absolute left-2.5 text-xs leading-none font-semibold whitespace-nowrap"
        style={`top:${textTop(20, 12, "sans")}px`}
      >
        {props.name}
      </span>
      <span
        class="absolute left-2.5 text-xs leading-none whitespace-nowrap"
        style={`top:${textTop(46, 12, "sans")}px`}
      >
        {props.caption}
      </span>
    </div>
  );
}

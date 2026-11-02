import { ClipSubtitle, PanelCard, PanelHeader } from "./panel.tsx";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

// The text clip panel is laid out in the coordinates of its original drawing: x and y count from
// the panel's top-left corner. The 1px border means children sit 1px inside those coordinates.
const BORDER = 1;
const PANEL_WIDTH = 280;
/// Distance from the top of a `leading-none` line to its baseline, per pixel of font size.
const ASCENT = 0.86;
// Small caps labels sit a pixel higher than the ratio gives.
function ascent(size: number): number {
  return size === 11 ? 0.95 : ASCENT;
}

/// A line of text whose baseline is at `y`. With `right`, `x` is the right edge instead.
function Txt(props: {
  x: number;
  y: number;
  size: number;
  right?: boolean;
  class?: string;
  children: Child;
}): Markup {
  const top = props.y - props.size * ascent(props.size) - BORDER;
  const horizontal = props.right ? `right:${PANEL_WIDTH - BORDER - props.x}px` : `left:${props.x - BORDER}px`;
  return (
    <span
      class={`absolute leading-none whitespace-nowrap ${props.class ?? ""}`}
      style={`${horizontal};top:${top}px;font-size:${props.size}px`}
    >
      {props.children}
    </span>
  );
}

function Box(props: { x: number; y: number; w: number; h: number; class: string; children?: Child }): Markup {
  return (
    <div
      class={`absolute ${props.class}`}
      style={`left:${props.x - BORDER}px;top:${props.y - BORDER}px;width:${props.w}px;height:${props.h}px`}
    >
      {props.children}
    </div>
  );
}

/// A small inline drawing placed at (`x`, `y`).
function Drawing(props: { x: number; y: number; w: number; h: number; children: Child }): Markup {
  return (
    <svg
      class="absolute"
      width={props.w}
      height={props.h}
      style={`left:${props.x - BORDER}px;top:${props.y - BORDER}px;overflow:visible`}
    >
      {props.children}
    </svg>
  );
}

function Heading(props: { y: number; action?: string; children: Child }): Markup {
  return (
    <>
      <Txt x={16} y={props.y} size={11} class="font-semibold tracking-[0.6px] text-muted">{props.children}</Txt>
      {props.action === undefined ? null : (
        <Txt x={264} y={props.y + 0.36} size={12} right class="text-accent">
          {props.action}
        </Txt>
      )}
    </>
  );
}

function Rule(props: { y: number }): Markup {
  return <Box x={16} y={props.y} w={248} h={1} class="bg-divider" />;
}

/// A bordered input box; locked inputs lose the border and use a flat fill.
function Input(props: { x: number; y: number; w: number; h: number; locked?: boolean; class?: string }): Markup {
  return (
    <Box
      x={props.x}
      y={props.y}
      w={props.w}
      h={props.h}
      class={`rounded-md ${props.locked ? "bg-[#19191d]" : "border border-border bg-surface"} ${props.class ?? ""}`}
    />
  );
}

function ColorSwatch(props: { x: number; y: number; fill: string; accent?: boolean }): Markup {
  return (
    <Box
      x={props.x}
      y={props.y}
      w={16}
      h={16}
      class={`rounded-[4px] border ${props.accent ? "border-accent" : "border-border"}`}
    >
      <div class="size-full rounded-[3px]" style={`background:${props.fill}`} />
    </Box>
  );
}

export type TextClipState = {
  locked?: boolean;
  /// Shifts the content below the header: the full panel is drawn 8px higher than its crops.
  shift: number;
  editingContent?: boolean;
  hexFocused?: boolean;
  fontHover?: boolean;
  font?: string;
  colorHex?: string;
  colorFill?: string;
  colorAccent?: boolean;
  background?: { hex: string; opacity: string };
  overlay?: Child;
};

/// The whole 280×637 text clip panel in one state.
export function TextClipPanel(props: TextClipState): Markup {
  const s = props;
  const locked = s.locked;
  return (
    <PanelCard height={637} locked={locked}>
      <div class="-mt-px -mx-px">
        <PanelHeader
          locked={locked}
          icon={locked ? { type: "locked" } : { type: "track", kind: "T" }}
          title="Let's get started"
          subtitle={<ClipSubtitle clipType="Text clip" track="Text 1" kind="T" locked={locked} />}
          bottomDivider
        />
      </div>
      <div class="absolute inset-0" style={`top:${s.shift}px`}>
        <Heading y={87.96}>CONTENT</Heading>
        <Box
          x={16}
          y={100}
          w={248}
          h={58}
          class={`rounded-md ${
            locked ? "bg-[#19191d]" : `border ${s.editingContent ? "border-accent" : "border-border"} bg-surface`
          }`}
        />
        <Txt x={26} y={121.68} size={13} class="text-text">Let's get started</Txt>
        {s.editingContent ? <Box x={125.02} y={109} w={1.5} h={16} class="bg-accent" /> : null}
        <Rule y={174} />
        <Heading y={197.96}>TIMING</Heading>
        <Txt x={16} y={220.96} size={11} class="text-muted">Start</Txt>
        <Txt x={144} y={220.96} size={11} class="text-muted">Duration</Txt>
        <Box x={16} y={226} w={120} h={30} class={`rounded-md ${locked ? "bg-[#19191d]" : "bg-well"}`} />
        <Txt x={26} y={245.68} size={13} class="font-mono text-text">0:10.03</Txt>
        <Txt x={126} y={244.96} size={11} right class="font-mono text-dim">303 f</Txt>
        <Input x={144} y={226} w={120} h={30} locked={locked} />
        <Txt x={154} y={245.68} size={13} class="font-mono text-text">2.90</Txt>
        <Txt x={254} y={245.32} size={12} right class="text-muted">s</Txt>
        <Rule y={272} />
        <Heading y={295.96}>STYLE</Heading>
        <Txt x={16} y={319} size={12} class="text-soft">Font</Txt>
        <Box
          x={16}
          y={328}
          w={248}
          h={34}
          class={`rounded-md border ${
            locked
              ? "border-[#222226] bg-well"
              : s.fontHover
              ? "border-accent bg-accent-background"
              : "border-border bg-surface"
          }`}
        />
        <Txt x={26} y={350} size={13} class={locked ? "text-dim" : "text-text"}>{s.font ?? "Sans"}</Txt>
        <Drawing x={246} y={343} w={12} h={6}>
          <path
            d="M0 0 l4 4 l4 -4"
            fill="none"
            stroke={locked ? "#46464e" : s.fontHover ? "#f0b75e" : "#777780"}
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </Drawing>
        <Txt x={16} y={383} size={12} class="text-soft">Font size</Txt>
        <Txt x={144} y={383} size={12} class="text-soft">Font color</Txt>
        <Input x={16} y={392} w={120} h={34} locked={locked} />
        <Txt x={26} y={414} size={13} class="font-mono text-text">64</Txt>
        <Txt x={126} y={414} size={12} right class="text-muted">px</Txt>
        <Input
          x={144}
          y={392}
          w={120}
          h={34}
          locked={locked}
          class={s.hexFocused ? "!border-accent" : ""}
        />
        <ColorSwatch x={152} y={401} fill={s.colorFill ?? "#ffffff"} accent={s.colorAccent} />
        <Txt x={176} y={414} size={13} class="font-mono text-muted">#</Txt>
        {s.hexFocused ? <Box x={185} y={399} w={47} h={19} class="rounded-[2px] bg-[#3a5a8c]" /> : null}
        <Txt x={186} y={414} size={13} class="font-mono text-text">{s.colorHex ?? "FFFFFF"}</Txt>
        <Rule y={442} />
        <Heading y={465.96} action={locked ? undefined : s.background ? "Remove" : "Add"}>BACKGROUND</Heading>
        {s.background
          ? (
            <>
              <Input x={16} y={478} w={160} h={30} />
              <ColorSwatch x={24} y={485} fill={`#${s.background.hex}`} />
              <Txt x={48} y={497.68} size={13} class="font-mono text-muted">#</Txt>
              <Txt x={58} y={497.68} size={13} class="font-mono text-text">{s.background.hex}</Txt>
              <Input x={184} y={478} w={80} h={30} />
              <Txt x={194} y={497.68} size={13} class="font-mono text-text">{s.background.opacity}</Txt>
              <Txt x={254} y={497.32} size={12} right class="text-muted">%</Txt>
            </>
          )
          : <Txt x={16} y={490.32} size={12} class="text-dim">No background</Txt>}
        <Rule y={524} />
        <Heading y={547.96}>POSITION</Heading>
        {([["X", 16, "0.50"], ["Y", 144, "0.85"]] as const).map(([axis, x, value]) => (
          <>
            <Box
              x={x}
              y={560}
              w={120}
              h={30}
              class={`rounded-md ${locked ? "bg-[#19191d]" : "border border-border bg-surface"}`}
            />
            <Txt x={x + 10} y={579.32} size={12} class="font-semibold text-muted">{axis}</Txt>
            <Txt x={x + 25.68} y={579.68} size={13} class="font-mono text-text">{value}</Txt>
          </>
        ))}
      </div>
      {s.overlay}
    </PanelCard>
  );
}

const SWATCH_ROWS = [
  ["#FFFFFF", "#CCCCCC", "#999999", "#666666", "#333333", "#000000"],
  ["#ff8b8b", "#f0b75e", "#FFE58A", "#9BDCA4", "#91B8DF", "#B69BDC"],
  ["#E84545", "#E69324", "#E5C638", "#41AC62", "#438FD4", "#9164CE"],
  ["#8E2828", "#915617", "#8A7621", "#256B3A", "#285886", "#58377F"],
];

/// The 24-colour popover under the font colour field. `hover` rings the swatch at row 1, column 4.
export function ColorPalette(props: { hover?: boolean }): Markup {
  return (
    <>
      <Box x={14} y={94} w={252} h={278} class="rounded-[9px] bg-black/35" />
      <Box x={16} y={90} w={248} h={278} class="rounded-lg border border-rail bg-hover" />
      <Txt x={32} y={117} size={13} class="font-semibold text-text">Color families</Txt>
      <Drawing x={240} y={105} w={8} h={8}>
        <path d="M0 0l8 8m0-8l-8 8" stroke="#8a8a93" stroke-width="1.5" />
      </Drawing>
      {SWATCH_ROWS.map((row, rowIndex) =>
        row.map((fill, column) => {
          const selected = rowIndex === 1 && column === 1;
          return (
            <Box
              x={31.5 + 36 * column}
              y={135.5 + 38 * rowIndex}
              w={31}
              h={31}
              class={`rounded-[4px] border ${selected ? "border-text" : "border-rail"}`}
            >
              <div class="size-full rounded-[3px]" style={`background:${fill}`} />
            </Box>
          );
        })
      )}
      <Drawing x={76} y={189} w={14} h={11}>
        <path
          d="M0 0 l5 5 9-11"
          fill="none"
          stroke="#17120a"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </Drawing>
      {props.hover
        ? (
          <>
            <Box x={174} y={172} w={34} h={34} class="rounded-md border-4 border-[#080809]" />
            <Box x={174} y={172} w={34} h={34} class="rounded-md border-2 border-text" />
            <Drawing x={194} y={194} w={16} h={20}>
              <g
                fill="#f2f2f4"
                stroke="#080809"
                stroke-linejoin="round"
                transform="scale(.65)"
              >
                <path d="M5 13V2.5a2 2 0 0 1 4 0V9a2 2 0 0 1 4 0v1a2 2 0 0 1 4 0v1a2 2 0 0 1 4 0v7c0 4-2 7-5 9H8L1 17c-2-3 1-5 3-3l1 1Z" />
              </g>
            </Drawing>
          </>
        )
        : null}
      <Txt x={32} y={313} size={11} class={props.hover ? "text-soft" : "text-[#8a8a93]"}>
        {props.hover ? "Hover highlights · click applies." : "Neutrals, then light, vivid and dark."}
      </Txt>
      <Txt x={32} y={341} size={11} class="text-[#8a8a93]">Custom color? Edit the hex value.</Txt>
      <Drawing x={153} y={366} w={14} h={9}>
        <path d="M0 0H14L7 9Z" fill="#202024" />
        <path d="M1 2l6 7 6-7" stroke="#45454d" fill="none" />
      </Drawing>
    </>
  );
}

const FONTS = [
  { name: "Helvetica Neue", state: "idle" },
  { name: "PingFang SC", state: "selected" },
  { name: "Songti SC", state: "hover" },
] as const;

/// The searchable font list with preview cards, floating over the panel.
export function FontSelector(): Markup {
  return (
    <>
      <Box x={16} y={58} w={248} h={438} class="rounded-lg border border-rail bg-hover" />
      <Txt x={32} y={86} size={13} class="text-text">Font</Txt>
      <Txt x={235} y={87} size={20} class="text-[#8a8a93]">×</Txt>
      <Box x={28} y={100} w={224} h={34} class="rounded-md border border-rail bg-surface" />
      <Txt x={40} y={122} size={13} class="text-muted">Search fonts…</Txt>
      {FONTS.map((font, index) => {
        const top = 146 + 104 * index;
        const selected = font.state === "selected";
        return (
          <>
            <Box
              x={28}
              y={top}
              w={224}
              h={96}
              class={`rounded-[7px] border bg-[#19191d] ${
                selected ? "border-accent" : font.state === "hover" ? "border-[#8a8a93]" : "border-[#34343b]"
              }`}
            />
            <Txt x={40} y={top + 22} size={12} class={selected ? "text-accent" : "text-soft"}>{font.name}</Txt>
            {selected ? <Txt x={227} y={top + 22} size={15} class="text-accent">✓</Txt> : null}
            <Box
              x={29}
              y={top + 33}
              w={222}
              h={1}
              class={selected ? "bg-[#3b3327]" : "bg-[#34343b]"}
            />
            <Txt x={40} y={top + 71} size={17} class="text-text">
              <span style={`font-family:${font.name}`}>你好，世界 · Hello,world</span>
            </Txt>
          </>
        );
      })}
      <Box x={253} y={146} w={3} h={304} class="rounded-[2px] bg-[#34343b]" />
      <Box x={253} y={146} w={3} h={65} class="rounded-[2px] bg-muted" />
      <Txt x={32} y={479} size={12} class="text-[#8a8a93]">Click a font to apply.</Txt>
    </>
  );
}

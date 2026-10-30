import { raw } from "@design/jsx/jsx-runtime";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

const LOCK_ICON = raw(Deno.readTextFileSync(new URL("./icons/lock.svg", import.meta.url)));

export type PanelTrackKind = "V" | "A" | "T";

const TRACK_BADGE: Record<PanelTrackKind, { bg: string; border: string; name: string }> = {
  V: { bg: "bg-[#294d75]", border: "border-[#395d85]", name: "text-[#91b8df]" },
  A: { bg: "bg-[#24656b]", border: "border-[#34757b]", name: "text-[#69c5cf]" },
  T: { bg: "bg-[#7251a3]", border: "border-[#8261b3]", name: "text-[#b69bdc]" },
};

export type PanelIcon =
  | { type: "track"; kind: PanelTrackKind }
  | { type: "locked" }
  | { type: "file"; label: string; labelColor: string; border: string }
  | { type: "timeline" };

/// A 280px property panel with its menu-style shadow. `height` is the outer height.
export function PanelCard(props: { height: number; locked?: boolean; children: Child }): Markup {
  return (
    <div
      class={`relative w-[280px] overflow-hidden rounded-[10px] border bg-panel font-sans text-text shadow-[0_6px_0_rgb(0_0_0/0.35)] ${
        props.locked ? "border-[#62626e]" : "border-border"
      }`}
      style={`height:${props.height}px`}
    >
      {props.children}
      {props.locked
        ? (
          <>
            <div class="hatch-panel-lock absolute top-0 left-0 h-[5px] w-full" />
            <div class="hatch-panel-lock absolute top-0 left-0 h-full w-[5px]" />
          </>
        )
        : null}
    </div>
  );
}

/// The panel header: a 32px icon, the title and a subtitle, 56px tall. `bottomDivider` is off when
/// tabs follow, since the top edge of the tabs divides.
export function PanelHeader(props: {
  icon: PanelIcon;
  title: string;
  subtitle: Child;
  locked?: boolean;
  bottomDivider?: boolean;
}): Markup {
  return (
    <header
      class={`relative h-14 ${props.locked ? "bg-[#242429]" : ""} ${
        props.bottomDivider ? (props.locked ? "border-b border-[#62626e]" : "border-b border-border") : ""
      }`}
    >
      <HeaderIcon icon={props.icon} />
      <p class="absolute top-[12.5px] left-14 text-sm leading-none font-semibold">{props.title}</p>
      <p class="absolute top-[31px] left-14 text-xs leading-none text-muted">{props.subtitle}</p>
    </header>
  );
}

/// The two-tab strip under a header. The active tab joins the panel; the other is darker and boxed.
export function PanelTabs(props: { tabs: [string, string]; active: 0 | 1 }): Markup {
  return (
    <div class="flex h-[30px] w-[280px] text-xs leading-none">
      {props.tabs.map((label, index) => (
        <div
          class={`grid h-[30px] place-items-center border-x border-t border-border ${
            index === 0 ? "w-[140px]" : "-ml-px w-[141px]"
          } ${index === props.active ? "bg-panel font-semibold text-text" : "border-b bg-[#1a1a1e] text-muted"}`}
        >
          {label}
        </div>
      ))}
    </div>
  );
}

/// A 16px badge with the track letter, used in history rows.
export function TrackLetter(props: { kind: PanelTrackKind; class?: string }): Markup {
  return (
    <span
      class={`grid size-4 place-items-center rounded text-[10px] leading-none font-bold text-white ${
        TRACK_BADGE[props.kind].bg
      } ${props.class ?? ""}`}
    >
      {props.kind}
    </span>
  );
}

/// A small track chip such as "V 1", with the track colour as fill and border.
export function TrackChip(props: { kind: PanelTrackKind; label: string }): Markup {
  const style = TRACK_BADGE[props.kind];
  return (
    <span
      class={`grid h-[19px] w-[33px] place-items-center rounded border text-[11px] leading-none font-semibold text-white ${style.bg} ${style.border}`}
    >
      {props.label}
    </span>
  );
}

function HeaderIcon(props: { icon: PanelIcon }): Markup {
  const base = "absolute top-3 left-3 grid size-8 place-items-center rounded-lg border";
  const { icon } = props;
  switch (icon.type) {
    case "track": {
      const style = TRACK_BADGE[icon.kind];
      return (
        <span class={`${base} ${style.bg} ${style.border} text-xs leading-none font-bold text-white`}>{icon.kind}</span>
      );
    }
    case "locked":
      return (
        <span class={`${base} border-[#62626e] bg-[#33333b]`}>
          {LOCK_ICON}
        </span>
      );
    case "file":
      return (
        <span
          class={`${base} bg-[#131316] font-mono text-[9px] leading-none font-semibold`}
          style={`border-color:${icon.border};color:${icon.labelColor}`}
        >
          {icon.label}
        </span>
      );
    case "timeline":
      return <span class={`${base} border-accent bg-accent text-[11px] leading-none font-bold text-[#17120a]`}>TL</span>;
  }
}

/// "Video clip · Track: Video 1", with the track name in its track colour.
export function ClipSubtitle(props: { clipType: string; track: string; kind: PanelTrackKind; locked?: boolean }): Markup {
  return (
    <span class="text-[11px]">
      <span class="text-[#8a8a93]">{`${props.clipType} · Track: `}</span>
      <span class={TRACK_BADGE[props.kind].name}>{props.track}</span>
      {props.locked
        ? (
          <span class="text-[#8a8a93]">
            {" · "}
            <span class="text-light">Locked</span>
          </span>
        )
        : null}
    </span>
  );
}

/// A section label such as "TIMING", with an optional accent action on the right.
export function SectionLabel(props: { class?: string; action?: string; children: Child }): Markup {
  return (
    <div class={`flex items-baseline justify-between text-[11px] leading-none ${props.class ?? ""}`}>
      <span class="font-semibold tracking-[0.6px] text-muted">{props.children}</span>
      {props.action === undefined ? null : <span class="text-xs text-accent">{props.action}</span>}
    </div>
  );
}

export function Divider(props: { class: string }): Markup {
  return <div class={`h-px bg-divider ${props.class}`} />;
}

/// A 28px row with a muted name on the left and a mono value on the right.
export function Pair(props: { name: string; value: string }): Markup {
  return (
    <div class="flex h-7 items-center justify-between text-[13px] leading-none">
      <span class="text-muted">{props.name}</span>
      <span class="font-mono">{props.value}</span>
    </div>
  );
}

/// A read-only timing value: a caption, a mono time and its frame count.
export function TimingCell(props: { name: string; time: string; frames: string; locked?: boolean }): Markup {
  return (
    <div class={`relative h-[50px] rounded-md ${props.locked ? "bg-[#19191d]" : "bg-[#131316]"}`}>
      <span class="absolute top-[9.5px] left-2.5 text-[11px] leading-none text-muted">{props.name}</span>
      <span class="absolute top-[27.9px] left-2.5 font-mono text-[13px] leading-none">{props.time}</span>
      <span class="absolute top-[28.8px] right-2.5 font-mono text-[11px] leading-none text-dim">{props.frames}</span>
    </div>
  );
}

/// A 30px number field with an optional axis prefix and unit. Locked fields lose the border and fill.
export function Field(props: { prefix?: string; value: string; unit?: string; width: number; locked?: boolean }): Markup {
  return (
    <div
      class={`flex h-[30px] items-center rounded-md border px-[9px] leading-none ${
        props.locked ? "border-transparent bg-[#19191d]" : "border-border bg-surface"
      }`}
      style={`width:${props.width}px`}
    >
      {props.prefix === undefined
        ? null
        : <span class="w-[15.7px] shrink-0 text-xs font-semibold text-muted">{props.prefix}</span>}
      <span class="font-mono text-[13px]">{props.value}</span>
      {props.unit === undefined ? null : <span class="ml-auto text-xs text-muted">{props.unit}</span>}
    </div>
  );
}

/// A 104px slider with a tick at the default. `fill` is the filled length in px.
export function Slider(props: { fill: number; locked?: boolean }): Markup {
  const knob = props.locked ? 10 : 14;
  return (
    <div class="relative h-[30px] w-[104px]">
      <div class="absolute top-[13px] h-1 w-full rounded-full bg-border" />
      <div class="absolute top-3.5 left-[51.5px] h-2.5 w-px -translate-y-1 bg-dim" />
      <div
        class={`absolute top-[13px] h-1 rounded-full ${props.locked ? "bg-dim" : "bg-accent"}`}
        style={`width:${props.fill}px`}
      />
      <div
        class={`absolute rounded-full ${props.locked ? "bg-dim" : "border-2 border-panel bg-text"}`}
        style={`left:${props.fill - knob / 2}px;top:${15 - knob / 2}px;width:${knob}px;height:${knob}px`}
      />
    </div>
  );
}

/// The mute switch row: a microphone on the left and a 34×20 toggle on the right.
export function MuteToggle(props: { muted?: boolean; locked?: boolean }): Markup {
  const fade = props.locked ? "opacity-50" : "";
  return (
    <div class="flex h-5 items-center justify-between">
      <MicIcon muted={props.muted} class={fade} />
      <span
        class={`flex h-5 w-[34px] items-center rounded-full px-[3px] ${fade} ${
          props.muted ? "bg-border" : "justify-end bg-accent"
        }`}
      >
        <span class={`size-3.5 rounded-full ${props.muted ? "bg-muted" : "bg-panel"}`} />
      </span>
    </div>
  );
}

/// A microphone, struck through when muted.
function MicIcon(props: { muted?: boolean; class: string }): Markup {
  return (
    <svg
      class={props.class}
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="#c9c9cf"
      stroke-width="2.25"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      {props.muted
        ? (
          <>
            <path d="M2 2l20 20" />
            <path d="M18.89 13.23A7.12 7.12 0 0 0 19 12v-2" />
            <path d="M5 10v2a7 7 0 0 0 12 5" />
            <path d="M15 9.34V5a3 3 0 0 0-5.68-1.33" />
            <path d="M9 9v3a3 3 0 0 0 5.12 2.12" />
            <path d="M12 19v3" />
          </>
        )
        : (
          <>
            <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
            <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
            <path d="M12 19v3" />
          </>
        )}
    </svg>
  );
}

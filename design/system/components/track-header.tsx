import { COLOR } from "../theme.ts";
import { Icon, type IconName } from "./icon.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

export type TrackKind = "V" | "A" | "T";
export type ToggleState = "off" | "active" | "hovered";

const BADGE: Record<TrackKind, { fill: string; border: string }> = {
  V: { fill: "bg-[#294d75]", border: "border-[#395d85]" },
  A: { fill: "bg-[#24656b]", border: "border-[#34757b]" },
  T: { fill: "bg-[#7251a3]", border: "border-[#8261b3]" },
};

// SF Pro's baseline sits 0.8605 × font size below the top of a `leading-none` box.
const baselineTop = (baseline: number, fontSize: number) => baseline - fontSize * 0.8605;

/// A track header at its real size, 190 × 74: kind badge, name, reorder and delete actions,
/// then the lock, visibility and (except for text tracks) mute toggles.
export function TrackHeader(props: {
  kind: TrackKind;
  name: string; // already truncated to fit before the actions
  lock?: ToggleState;
  visibility?: ToggleState;
  mute?: ToggleState;
  moveUp?: "enabled" | "disabled";
  moveDown?: "enabled" | "disabled";
  delete?: "default" | "hovered";
}): Markup {
  const toggles: { state: ToggleState; icon: IconName; activeIcon: IconName }[] = [
    { state: props.lock ?? "off", icon: "lock", activeIcon: "lock" },
    { state: props.visibility ?? "off", icon: "eye", activeIcon: "eye-off" },
    ...(props.kind === "T"
      ? []
      : [{ state: props.mute ?? "off", icon: "unmute" as IconName, activeIcon: "mute" as IconName }]),
  ];
  const badge = BADGE[props.kind];
  return (
    <div class="relative h-[74px] w-[190px] font-sans">
      <div class="absolute top-[73px] left-0 h-px w-[190px] bg-border" />
      <div class={`absolute top-3 left-3 size-[18px] rounded-[5.5px] border ${badge.fill} ${badge.border}`} />
      <span
        class="absolute left-3 w-[18px] text-center text-[10px] leading-none font-bold text-white"
        style={`top:${baselineTop(25, 10)}px`}
      >
        {props.kind}
      </span>
      <span
        class="absolute left-[38px] text-[13px] leading-none font-semibold whitespace-nowrap text-text"
        style={`top:${baselineTop(26, 13)}px`}
      >
        {props.name}
      </span>
      <Icon
        name="arrow-up"
        color={props.moveUp === "disabled" ? COLOR.grey : COLOR.muted}
        class="absolute top-[14px] left-[117px]"
      />
      <Icon
        name="arrow-down"
        color={props.moveDown === "disabled" ? COLOR.grey : COLOR.muted}
        class="absolute top-[14px] left-[139px]"
      />
      {props.delete === "hovered" && <div class="absolute top-[11px] left-[158px] size-5 rounded bg-hover" />}
      <Icon
        name="trash"
        color={props.delete === "hovered" ? COLOR.error : COLOR.muted}
        class="absolute top-[14px] left-[161px]"
      />
      <div
        class="absolute top-[39px] left-3 h-6 rounded-[6.5px] border border-border bg-surface"
        style={`width:${toggles.length * 30 + 2}px`}
      />
      {toggles.map((toggle, index) => (
        <div>
          {toggle.state === "active" && (
            <div class="absolute top-[41px] h-5 w-7 rounded bg-accent/16" style={`left:${14 + index * 30}px`} />
          )}
          {toggle.state === "hovered" && (
            <div class="absolute top-[41px] h-5 w-7 rounded bg-hover" style={`left:${14 + index * 30}px`} />
          )}
          <Icon
            name={toggle.state === "active" ? toggle.activeIcon : toggle.icon}
            color={toggle.state === "active" ? COLOR.accent : toggle.state === "hovered" ? COLOR.text : COLOR.muted}
            class="absolute top-[44px]"
            style={`left:${21 + index * 30}px`}
          />
          {index > 0 && toggle.state === "off" && toggles[index - 1].state === "off" && (
            <div class="absolute top-[45px] h-3 w-px bg-border" style={`left:${13 + index * 30}px`} />
          )}
        </div>
      ))}
    </div>
  );
}

/// A track header shown on its own: scaled 1.4× in a 266px card.
export function TrackHeaderCard(props: { children: Markup }): Markup {
  return (
    <div class="relative h-[103.6px] w-[266px] rounded-[10px] bg-[#0a0a0c]">
      <div class="absolute inset-0 overflow-hidden rounded-[10px]">
        <div class="origin-top-left scale-[1.4]">{props.children}</div>
      </div>
      <div class="absolute inset-0 rounded-[10px] border border-border" />
    </div>
  );
}

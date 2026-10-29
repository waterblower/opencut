import { Scene } from "./scene.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

export type PlayerIcon = "play" | "pause" | "replay";

const HAND_UP =
  "M5 9 V2.6 a1.7 1.7 0 0 1 3.4 0 V8 V6.8 a1.6 1.6 0 0 1 3.2 0 V8.6 V7.6 a1.6 1.6 0 0 1 3.2 0 V9.6 V8.8 a1.6 1.6 0 0 1 3.2 0 V14 c0 4 -2.5 6 -6.5 6 h-1 c-2.6 0 -4 -1.4 -5.2 -3.4 L2.2 13 a1.6 1.6 0 0 1 2.8 -1.6 Z";
const HAND_GRAB =
  "M5 9 V4.5 a1.6 1.6 0 0 1 3.2 0 V8 V3.2 a1.6 1.6 0 0 1 3.2 0 V8 V4 a1.6 1.6 0 0 1 3.2 0 V9 V6.4 a1.6 1.6 0 0 1 3.2 0 V13 c0 4 -2.5 6.5 -6.5 6.5 h-1 c-2.6 0 -4 -1.4 -5.2 -3.4 L2.2 12 a1.6 1.6 0 0 1 2.8 -1.6 Z";

/// The playback button: a 24px circle whose glyph shows what a click does.
export function PlaybackIcon(props: { icon: PlayerIcon; class?: string }): Markup {
  return (
    <svg class={props.class} width="24" height="24" viewBox="-12 -12 24 24">
      <circle r="12" fill="#f2f2f4" />
      {props.icon === "play"
        ? <path d="M-2 -3.5 L3.5 0 L-2 3.5 Z" fill="#0d0d0f" stroke="#0d0d0f" stroke-width="1" stroke-linejoin="round" />
        : props.icon === "pause"
        ? (
          <>
            <rect x="-3" y="-4" width="2.2" height="8" rx="0.8" fill="#0d0d0f" />
            <rect x="0.8" y="-4" width="2.2" height="8" rx="0.8" fill="#0d0d0f" />
          </>
        )
        : (
          <g
            stroke="#0d0d0f"
            transform="translate(-5 -5) scale(0.4167)"
            fill="none"
            stroke-width="3.2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
            <path d="M3 3v5h5" />
          </g>
        )}
    </svg>
  );
}

/// A video player: the picture above a 64px control bar with scrub track, button, time and file name.
/// `progress` is the filled length in px. `seek` is the hover preview length, `knob` is "hover" or
/// "drag", and `hand` places the hand cursor from the panel's top-left.
export function VideoPlayer(props: {
  width: number;
  icon: PlayerIcon;
  progress: number;
  time: string;
  total: string;
  file: string;
  picture?: boolean;
  caption?: string;
  seek?: number;
  knob?: "hover" | "drag";
  hand?: { left: number; top: number; grab?: boolean };
  class?: string;
}): Markup {
  const pictureHeight = props.width * 9 / 16;
  const thick = props.knob !== undefined;
  const trackWidth = props.width - 32;
  const trackTop = thick ? 13 : 14;
  const knobSize = props.knob === "drag" ? 16 : 14;
  return (
    <div
      class={`relative overflow-hidden rounded-[10px] border border-border bg-[#080808] font-sans shadow-[0_6px_0_rgb(0_0_0/0.35)] ${
        props.class ?? ""
      }`}
      style={`width:${props.width}px;height:${pictureHeight + 64}px`}
    >
      {props.picture === false ? null : <Scene width={props.width} class="absolute top-0 left-0" />}
      {props.caption === undefined ? null : (
        <svg
          class="absolute top-0 left-0"
          width={props.width}
          height={pictureHeight}
          viewBox={`0 0 ${props.width} ${pictureHeight}`}
        >
          <text
            class="font-sans"
            x={props.width / 2}
            y="275.44"
            font-size="25"
            font-weight="600"
            fill="#ffffff"
            text-anchor="middle"
            stroke="#000"
            stroke-opacity="0.55"
            stroke-width="3"
            paint-order="stroke"
          >
            {props.caption}
          </text>
        </svg>
      )}
      <div
        class="absolute inset-x-0 h-16 border-t border-divider bg-panel"
        style={`top:${pictureHeight}px`}
      >
        <div
          class="absolute left-4 rounded-full bg-border"
          style={`top:${trackTop - 1}px;width:${trackWidth}px;height:${thick ? 6 : 4}px`}
        />
        {props.seek === undefined ? null : (
          <div
            class="absolute left-4 h-1.5 rounded-full bg-white/[0.14]"
            style={`top:${trackTop - 1}px;width:${props.seek}px`}
          />
        )}
        <div
          class="absolute left-4 rounded-full bg-accent"
          style={`top:${trackTop - 1}px;width:${props.progress}px;height:${thick ? 6 : 4}px`}
        />
        {props.knob === undefined ? null : (
          <>
            {props.knob === "drag"
              ? (
                <div
                  class="absolute size-6 rounded-full bg-accent/[0.18]"
                  style={`left:${16 + props.progress - 12}px;top:3px`}
                />
              )
              : null}
            <div
              class="absolute rounded-full border-2 border-panel bg-text"
              style={`left:${16 + props.progress - knobSize / 2}px;top:${
                15 - knobSize / 2 + 1
              }px;width:${knobSize}px;height:${knobSize}px`}
            />
          </>
        )}
        <PlaybackIcon icon={props.icon} class="absolute top-[31px] left-4" />
        <span class="absolute top-[37px] left-[50px] font-mono text-[13px] leading-none text-text">
          {props.time}
          <span class="text-muted">{" / "}{props.total}</span>
        </span>
        <span class="absolute top-[38px] right-4 max-w-[60%] truncate text-xs leading-none text-muted">
          {props.file}
        </span>
      </div>
      {props.hand === undefined ? null : (
        <svg
          class="absolute"
          style={`left:${props.hand.left}px;top:${props.hand.top}px`}
          width="22"
          height="22"
          viewBox="0 0 22 22"
        >
          <path
            d={props.hand.grab ? HAND_GRAB : HAND_UP}
            fill="#fff"
            stroke="#000"
            stroke-width="1"
            stroke-linejoin="round"
          />
        </svg>
      )}
    </div>
  );
}

import type { Child, Markup } from "@design/jsx/jsx-runtime";

import type { Waveform } from "./waveform-samples.ts";

const CLIP = {
  text: { fill: "bg-[#7251a3]", border: "border-[#8261b3]" },
  video: { fill: "bg-[#294d75]", border: "border-[#395d85]" },
  audio: { fill: "bg-[#24656b]", border: "border-[#34757b]" },
} as const;

/// A 64px clip body with a 1px border. Children are placed from the clip's outer edge;
/// `absolute` offsets in CSS start inside the border, hence the 1px less in each label.
function ClipFrame(props: {
  kind: keyof typeof CLIP;
  width: number;
  selected?: boolean;
  locked?: boolean;
  children?: Child;
}): Markup {
  const colors = CLIP[props.kind];
  return (
    <div
      class={`relative h-16 overflow-hidden rounded-md border ${colors.fill} ${
        props.selected ? "border-accent" : colors.border
      }`}
      style={`width:${props.width}px`}
    >
      {props.children}
      {props.locked && <div class="hatch-lock-clip absolute inset-0" />}
    </div>
  );
}

/// The clip name, 12px semibold, with its baseline 20px (top) or 52px (bottom) below the clip's top.
function ClipName(props: { name: string; bottom?: boolean }): Markup {
  // SF Pro's baseline sits 0.8605 × font size below the top of a `leading-none` box.
  const top = (props.bottom ? 52 : 20) - 12 * 0.8605 - 1;
  return (
    <span
      class="absolute left-[7px] text-xs leading-none font-semibold whitespace-nowrap text-text"
      style={`top:${top}px`}
    >
      {props.name}
    </span>
  );
}

function ClipWaveform(props: { waveform: Waveform; width: number }): Markup {
  const { waveform } = props;
  return (
    <svg
      aria-hidden="true"
      class="absolute -top-px -left-px"
      width={waveform.width}
      height="64"
      viewBox={`0 0 ${waveform.width} 64`}
    >
      <path d={waveform.d} stroke="#69c5cf" stroke-opacity="0.82" stroke-width="2" fill="none" />
    </svg>
  );
}

export function TextClip(props: { width: number; name: string; selected?: boolean }): Markup {
  return (
    <ClipFrame kind="text" width={props.width} selected={props.selected}>
      <ClipName name={props.name} />
    </ClipFrame>
  );
}

/// A video clip: the waveform of its audio, if it has any, under the name at the bottom.
export function VideoClip(
  props: {
    width: number;
    name: string;
    waveform?: Waveform;
    selected?: boolean;
    locked?: boolean;
    labelTop?: boolean; // the name at the top instead of the bottom
  },
): Markup {
  return (
    <ClipFrame kind="video" width={props.width} selected={props.selected} locked={props.locked}>
      {props.waveform && <ClipWaveform waveform={props.waveform} width={props.width} />}
      <ClipName name={props.name} bottom={!props.labelTop} />
    </ClipFrame>
  );
}

/// An audio clip: the waveform, the name at the top and an "Audio" label at the bottom.
export function AudioClip(props: { width: number; name: string; waveform: Waveform; selected?: boolean }): Markup {
  const labelTop = 52 - 12 * 0.8605 - 1;
  return (
    <ClipFrame kind="audio" width={props.width} selected={props.selected}>
      <ClipWaveform waveform={props.waveform} width={props.width} />
      <ClipName name={props.name} />
      <span class="absolute left-[7px] font-mono text-xs leading-none text-[#c8d8e8]" style={`top:${labelTop}px`}>
        Audio
      </span>
    </ClipFrame>
  );
}

/// An existing audio clip seen beside the one being placed, with a plain 11-12px label.
export function NeighborClip(
  props: { width: number; name: string; size: number; left: number; baseline: number },
): Markup {
  return (
    <div
      class="relative h-16 overflow-hidden rounded-md border border-[#34757b] bg-[#24656b]"
      style={`width:${props.width}px`}
    >
      <span
        class="absolute leading-none whitespace-nowrap text-[#c8d8e8]"
        style={`left:${props.left - 1}px;top:${props.baseline - props.size * 0.8605 - 1}px;font-size:${props.size}px`}
      >
        {props.name}
      </span>
    </div>
  );
}

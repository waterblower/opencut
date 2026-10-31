import { raw } from "@design/jsx/jsx-runtime";
import type { Markup } from "@design/jsx/jsx-runtime";

/// The icons in the repository's `icons/` folder, which the editor embeds too.
export type IconName =
  | "alert-triangle"
  | "app-icon"
  | "arrow-down"
  | "arrow-up"
  | "eye"
  | "eye-off"
  | "folder"
  | "lock"
  | "magnet"
  | "mic"
  | "mic-off"
  | "minus"
  | "mute"
  | "picker-glow"
  | "playhead"
  | "plus"
  | "pointer"
  | "scissors"
  | "snap"
  | "spinner"
  | "trash"
  | "unmute"
  | "warning";

/// An icon from `icons/<name>.svg`, scaled to `width` × `height` (14px by default). Single-colour
/// icons draw in `currentColor`, so `color` sets their colour.
export function Icon(props: {
  name: IconName;
  color?: string;
  width?: number;
  height?: number;
  class?: string;
  style?: string;
}): Markup {
  const svg = Deno.readTextFileSync(new URL(`../../../icons/${props.name}.svg`, import.meta.url));
  return (
    <span
      aria-hidden="true"
      class={`block [&>svg]:size-full ${props.class ?? ""}`}
      style={`width:${props.width ?? 14}px;height:${props.height ?? 14}px;${
        props.color === undefined ? "" : `color:${props.color};`
      }${props.style ?? ""}`}
    >
      {raw(svg)}
    </span>
  );
}

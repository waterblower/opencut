import { COLOR, FONT_MONO, FONT_SANS } from "../theme.ts";
import type { Markup } from "@design/jsx/jsx-runtime";

export function Text(props: {
  x: number;
  y: number;
  size?: number;
  color?: string;
  weight?: number;
  mono?: boolean;
  anchor?: "start" | "middle" | "end";
  opacity?: number;
  children: string | number;
}): Markup {
  return (
    <text
      x={props.x}
      y={props.y}
      font-family={props.mono ? FONT_MONO : FONT_SANS}
      font-size={props.size ?? 13}
      fill={props.color ?? COLOR.text}
      font-weight={props.weight ?? 400}
      text-anchor={props.anchor ?? "start"}
      opacity={props.opacity}
    >
      {props.children}
    </text>
  );
}

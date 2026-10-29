import { COLOR } from "../theme.ts";
import type { Markup } from "@design/jsx/jsx-runtime";

export function WarningIcon(props: { x: number; y: number; size?: number }): Markup {
  const scale = (props.size ?? 14) / 24;
  return (
    <g
      transform={`translate(${props.x} ${props.y}) scale(${scale})`}
      fill="none"
      stroke={COLOR.error}
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z" />
      <path d="M12 9v4" />
      <path d="M12 17h.01" />
    </g>
  );
}

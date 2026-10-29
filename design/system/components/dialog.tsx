import { COLOR } from "../theme.ts";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

/// A dialog's panel and title. Content and buttons are placed by the caller.
export function DialogFrame(props: {
  x: number;
  y: number;
  width: number;
  height: number;
  title: string;
  children: Child;
}): Markup {
  return (
    <g>
      <rect
        x={props.x + 0.5}
        y={props.y + 0.5}
        width={props.width - 1}
        height={props.height - 1}
        rx="12"
        fill={COLOR.panel}
        stroke={COLOR.border}
        filter="url(#shadow)"
      />
      <text fill={COLOR.text} class="s" x={props.x + 20} y={props.y + 39} font-size="18" font-weight="600">
        {props.title}
      </text>
      {props.children}
    </g>
  );
}

/// A dialog button, right-aligned at `right`. Returns its width so buttons can be laid out
/// from right to left.
export function dialogButtonWidth(label: string): number {
  return label.length * 8.2 + 32;
}

export function DialogButton(props: {
  right: number;
  y: number;
  label: string;
  primary?: boolean;
}): Markup {
  const width = dialogButtonWidth(props.label);
  const x = props.right - width;
  return (
    <g>
      <rect
        x={x + 0.5}
        y={props.y + 0.5}
        width={width - 1}
        height="39"
        rx="8"
        fill={props.primary ? COLOR.accent : COLOR.surface}
        stroke={props.primary ? COLOR.accent : COLOR.border}
      />
      <text
        fill={props.primary ? "#17120a" : COLOR.text}
        class="s"
        x={x + width / 2}
        y={props.y + 25.26}
        font-size="16"
        text-anchor="middle"
      >
        {props.label}
      </text>
    </g>
  );
}

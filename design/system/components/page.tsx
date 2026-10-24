import { COLOR } from "../theme.ts";
import { Text } from "./text.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

/// A design page: a dark canvas with a title and subtitle in the top-left corner.
export function Page(props: {
  width: number;
  height: number;
  title: string;
  subtitle: string;
  children: Markup | Markup[];
}): Markup {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={props.width}
      height={props.height}
      viewBox={`0 0 ${props.width} ${props.height}`}
    >
      <defs>
        <filter id="shadow" x="-15%" y="-15%" width="130%" height="145%">
          <feDropShadow dx="0" dy="6" stdDeviation="10" flood-color="#000" flood-opacity=".35" />
        </filter>
      </defs>
      <rect width={props.width} height={props.height} fill={COLOR.page} />
      <Text x={48} y={61} size={24} weight={700}>{props.title}</Text>
      <Text x={48} y={89} size={14} color={COLOR.muted}>{props.subtitle}</Text>
      {props.children}
    </svg>
  );
}

/// A note beside a mock-up, outside the UI: a heading followed by lines of text.
export function Notes(props: {
  x: number;
  y: number;
  heading: string;
  lines: string[];
}): Markup {
  return (
    <g>
      <Text x={props.x} y={props.y} size={14} weight={600}>{props.heading}</Text>
      {props.lines.map((line, index) => (
        <Text x={props.x} y={props.y + 28 + index * 22} size={12} color={COLOR.subtle}>
          {line}
        </Text>
      ))}
    </g>
  );
}

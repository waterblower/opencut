import type { Markup } from "@design/jsx/jsx-runtime";

/// Lucide stroke icons used by the timeline, drawn 14px wide in `color`.
export type IconName = "arrow-up" | "arrow-down" | "trash" | "lock" | "eye" | "eye-off" | "volume" | "volume-off";

const PATHS: Record<IconName, Markup> = {
  "arrow-up": (
    <g>
      <path d="m5 12 7-7 7 7" />
      <path d="M12 19V5" />
    </g>
  ),
  "arrow-down": (
    <g>
      <path d="M12 5v14" />
      <path d="m19 12-7 7-7-7" />
    </g>
  ),
  trash: (
    <g>
      <path d="M3 6h18" />
      <path d="M8 6V4h8v2" />
      <path d="M19 6l-1 14H6L5 6" />
      <path d="M10 11v5" />
      <path d="M14 11v5" />
    </g>
  ),
  lock: (
    <g>
      <rect width="16" height="11" x="4" y="11" rx="2" ry="2" />
      <path d="M8 11V7a4 4 0 0 1 8 0v4" />
    </g>
  ),
  eye: (
    <g>
      <path d="M2.1 12a10 10 0 0 1 19.8 0 10 10 0 0 1-19.8 0" />
      <circle cx="12" cy="12" r="3" />
    </g>
  ),
  "eye-off": (
    <g>
      <path d="M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a13.16 13.16 0 0 1-1.67 2.68" />
      <path d="M6.61 6.61A13.53 13.53 0 0 0 2 12s3 7 10 7a9.74 9.74 0 0 0 5.39-1.61" />
      <path d="M9.88 9.88a3 3 0 1 0 4.24 4.24" />
      <path d="m2 2 20 20" />
    </g>
  ),
  volume: (
    <g>
      <path d="M11 5 6 9H2v6h4l5 4V5Z" />
      <path d="M15.54 8.46a5 5 0 0 1 0 7.07" />
      <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
    </g>
  ),
  "volume-off": (
    <g>
      <path d="M11 5 6 9H2v6h4l5 4V5Z" />
      <path d="m22 9-6 6" />
      <path d="m16 9 6 6" />
    </g>
  ),
};

export function Icon(props: { name: IconName; color: string; class?: string; style?: string }): Markup {
  return (
    <svg
      aria-hidden="true"
      class={props.class}
      style={props.style}
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke={props.color}
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      {PATHS[props.name]}
    </svg>
  );
}

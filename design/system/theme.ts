export const COLOR = {
  text: "#f2f2f4",
  muted: "#777780",
  dim: "#55555e",
  subtle: "#a0a0a8",
  soft: "#b4b4bc",
  light: "#c9c9cf",
  border: "#2b2b31",
  divider: "#1f1f24",
  surface: "#17171a",
  panel: "#0d0d0f",
  page: "#151518",
  hover: "#202024",
  accent: "#f0b75e",
  accentBackground: "#2a241b",
  rail: "#45454d",
  accentSoft: "#f3c47e",
  grey: "#3a3a40",
  error: "#ff8b8b",
  cyberCyan: "#00f0ff",
  cyberYellow: "#fcee0a",
  cyberInk: "#0a0a0b",
  cyberDisabledBorder: "#3a3926",
  cyberDisabledText: "#57563f",
} as const;

export type TrackKind = "T" | "V" | "A";

export const TRACK_KIND_COLOR: Record<TrackKind, { fill: string; text: string }> = {
  T: { fill: "#7a5fb0", text: "#e3d4ff" },
  V: { fill: "#3c6a91", text: "#cfe6ff" },
  A: { fill: "#2f7d6d", text: "#c9f2e7" },
};

export const FONT_SANS = '-apple-system,BlinkMacSystemFont,"SF Pro Text","Helvetica Neue",Arial,sans-serif';
export const FONT_MONO = '"SF Mono",Menlo,Monaco,Consolas,monospace';

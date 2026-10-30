import type { Markup } from "@design/jsx/jsx-runtime";

export type CyberVariant = "primary" | "secondary" | "disabled";

const STYLE: Record<CyberVariant, { stroke: string; text: string; glow: string }> = {
  primary: { stroke: "none", text: "text-[#0a0a0b]", glow: "drop-shadow(0 0 6px rgb(252 238 10 / 0.45))" },
  secondary: { stroke: "#00f0ff", text: "text-[#00f0ff]", glow: "drop-shadow(0 0 5px rgb(0 240 255 / 0.35))" },
  disabled: { stroke: "#3a3926", text: "text-[#57563f]", glow: "none" },
};

/// A D · Cyberpunk button, 40px tall with the lower-right and upper-left corners cut by 10px.
/// Primary buttons are yellow with slashes, secondary ones a cyan outline and disabled ones scan lines.
export function CyberButton(props: { width: number; variant: CyberVariant; label: string }): Markup {
  const { width } = props;
  const style = STYLE[props.variant];
  return (
    <div class="relative h-10 font-mono text-[13px] leading-none font-extrabold" style={`width:${width}px`}>
      <CyberShape width={width} variant={props.variant} />
      <span
        class={`absolute inset-y-0 flex items-center ${style.text} ${
          props.variant === "secondary" ? "inset-x-0 justify-center" : "left-6"
        }`}
      >
        {props.label}
      </span>
    </div>
  );
}

/// The cut-corner outline of a button with its fill, glow and slashes.
function CyberShape(props: { width: number; variant: CyberVariant }): Markup {
  const { width } = props;
  const style = STYLE[props.variant];
  const fill = { primary: "#fcee0a", secondary: "rgb(0 240 255 / 0.06)", disabled: "url(#cyber-scan)" }[props.variant];
  const slashColor = props.variant === "primary" ? "#0a0a0b" : "#3a3926";
  return (
    <svg
      class="absolute inset-0 overflow-visible"
      width={width}
      height="40"
      viewBox={`0 0 ${width} 40`}
      style={`filter:${style.glow}`}
    >
      <defs>
        <pattern id="cyber-scan" width="4" height="3" patternUnits="userSpaceOnUse">
          <rect width="4" height="1" fill="#ffffff" fill-opacity="0.05" />
        </pattern>
      </defs>
      <polygon
        points={`10,0 ${width},0 ${width},30 ${width - 10},40 0,40 0,10`}
        fill={fill}
        stroke={style.stroke}
      />
      {props.variant === "secondary" ? null : (
        <path
          transform={`translate(${width - 128} 0)`}
          d="M98 34 L106 6 H109 L101 34Z M105 34 L113 6 H116 L108 34Z M112 34 L120 6 H123 L115 34Z"
          fill={slashColor}
        />
      )}
    </svg>
  );
}

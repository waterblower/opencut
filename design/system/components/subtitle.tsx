import type { Markup } from "@design/jsx/jsx-runtime";

/// A subtitle burned into a picture, centred across its container. `baseline` is the y of the
/// text baseline from the container's top; `outlined` adds a dark outline behind the letters.
export function Subtitle(props: {
  text: string;
  size: number;
  weight: 500 | 600;
  baseline: number;
  outlined?: boolean;
}): Markup {
  return (
    <p
      class={`absolute inset-x-0 text-center leading-none whitespace-nowrap text-white ${
        props.outlined ? "[-webkit-text-stroke:3px_rgb(0_0_0/0.55)] [paint-order:stroke_fill]" : ""
      }`}
      style={`top:${props.baseline - props.size * 0.8605}px;font-size:${props.size}px;font-weight:${props.weight}`}
    >
      {props.text}
    </p>
  );
}

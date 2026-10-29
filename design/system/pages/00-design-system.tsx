import { CyberpunkShapeDefs } from "../components/cyberpunk.tsx";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

type ButtonState = "Default" | "Hover" | "Pressed" | "Disabled";
const buttonStates: ButtonState[] = ["Default", "Hover", "Pressed", "Disabled"];
const inputStates = ["Placeholder", "Default", "Focused", "Selected", "Disabled"] as const;

export default function Page00DesignSystemTailwind(): Markup {
  return (
    <main class="relative h-[1342px] w-[900px] overflow-hidden bg-[#1e1e22] font-sans font-normal text-text">
      {/* Reuse the decorative button assets; all layout and text are HTML/Tailwind. */}
      <svg aria-hidden="true" class="absolute size-0">
        <defs>
          <CyberpunkShapeDefs />
          <pattern id="tw-scan" width="4" height="3" patternUnits="userSpaceOnUse">
            <rect width="4" height="1" fill="#fcee0a" fill-opacity="0.2" />
          </pattern>
          <pattern id="tw-scan-dim" width="4" height="3" patternUnits="userSpaceOnUse">
            <rect width="4" height="1" fill="#ffffff" fill-opacity="0.05" />
          </pattern>
          {[
            { id: "tw-glow-y", color: "#fcee0a", blur: 6, opacity: 0.45 },
            { id: "tw-glow-y-hover", color: "#fcee0a", blur: 12, opacity: 0.75 },
            { id: "tw-glow-c", color: "#00f0ff", blur: 5, opacity: 0.35 },
            { id: "tw-glow-c-hover", color: "#00f0ff", blur: 10, opacity: 0.7 },
          ].map((filter) => (
            <filter id={filter.id} x="-30%" y="-80%" width="160%" height="260%">
              <feDropShadow
                dx="0"
                dy="0"
                stdDeviation={filter.blur}
                flood-color={filter.color}
                flood-opacity={filter.opacity}
              />
            </filter>
          ))}
        </defs>
      </svg>
      <section aria-labelledby="line-buttons" class="absolute inset-x-0 top-0 h-[270px]">
        <h1 id="line-buttons">
          <BaselineText x={40} y={44} size={15} class="font-semibold text-light">C · Line</BaselineText>
        </h1>
        <BaselineText x={40} y={64}>
          A square frame drawn by corner brackets, with mono uppercase labels. Hover opens the brackets; pressing fills the
          button solid. Secondary is underlined text.
        </BaselineText>
        <ButtonMatrix kind="line" />
      </section>
      <section aria-labelledby="cyberpunk-buttons" class="absolute inset-x-0 top-[270px] h-[270px]">
        <h2 id="cyberpunk-buttons">
          <BaselineText x={40} y={44} size={15} class="font-semibold text-light">D · Cyberpunk</BaselineText>
        </h2>
        <BaselineText x={40} y={64}>
          Cut top-left and bottom-right corners, neon yellow and cyan, hazard slashes. Hover brightens the border and glow;
          pressing inverts.
        </BaselineText>
        <ButtonMatrix kind="cyberpunk" />
      </section>
      <section aria-labelledby="dropdown-trigger" class="absolute inset-x-0 top-[580px] h-[320px]">
        <h2 id="dropdown-trigger">
          <BaselineText x={40} y={29} size={22} class="font-bold text-text">Dropdown trigger</BaselineText>
        </h2>
        <BaselineText x={40} y={58}>
          The entire field is clickable. Hover signals that the field can be clicked.
        </BaselineText>
        <h3>
          <BaselineText x={40} y={100} size={15} class="font-semibold text-light">Gold accent</BaselineText>
        </h3>
        <BaselineText x={40} y={122}>Hover uses a warm surface, gold border and gold chevron.</BaselineText>
        <div class="absolute top-[138px] left-10 h-[118px] w-[820px] rounded-[10px] bg-[#0b0b0d]">
          {(["Default", "Hover", "Disabled"] as const).map((state, index) => (
            <div class="absolute top-0 h-full w-[180px]" style={`left:${50 + index * 260}px`}>
              <BaselineText x={0} y={27}>{state}</BaselineText>
              <button
                type="button"
                disabled={state === "Disabled"}
                aria-label={`Font dropdown: ${state}`}
                aria-expanded="false"
                class={`absolute top-[49px] left-0 h-[34px] w-[180px] rounded-md outline-[1px] outline-offset-[-0.5px] ${
                  state === "Hover"
                    ? "bg-accent-background outline-accent"
                    : state === "Disabled"
                    ? "bg-[#131316] outline-[#222226]"
                    : "bg-surface outline-border"
                }`}
              >
                <BaselineText x={10} y={22} size={13} class={state === "Disabled" ? "text-dim" : "text-text"}>
                  Zapfino
                </BaselineText>
                <svg aria-hidden="true" class="absolute top-0 left-0 h-[34px] w-[180px]">
                  <path
                    d="M157 14l5 5 5 -5"
                    fill="none"
                    stroke={state === "Hover" ? "#f0b75e" : state === "Disabled" ? "#46464e" : "#777780"}
                    stroke-width="1.5"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                </svg>
              </button>
            </div>
          ))}
        </div>
        <BaselineText x={40} y={298}>
          The entire field is clickable · no size change on hover · disabled fields do not respond.
        </BaselineText>
      </section>
      <section aria-labelledby="text-inputs" class="absolute inset-x-0 top-[940px] h-[402px]">
        <h2 id="text-inputs">
          <BaselineText x={40} y={26} size={22} class="text-text">Text Input</BaselineText>
        </h2>
        <BaselineText x={40} y={54}>
          Primary uses a bordered field. Secondary uses an underline for a quieter appearance.
        </BaselineText>
        <div class="absolute top-[78px] left-10 h-[222px] w-[820px] rounded-[10px] bg-[#0b0b0d]">
          {inputStates.map((state, index) => (
            <BaselineText x={202 + index * 136} y={30} anchor="middle">{state}</BaselineText>
          ))}
          <BaselineText x={20} y={77} size={14} class="text-light">Primary</BaselineText>
          <BaselineText x={20} y={169} size={14} class="text-light">Secondary</BaselineText>
          {(["Primary", "Secondary"] as const).map((kind) =>
            inputStates.map((state, index) => (
              <div
                class={`absolute ${kind === "Primary" ? "top-[52px]" : "top-[144px]"}`}
                style={`left:${140 + index * 136}px`}
              >
                <InputExample kind={kind} state={state} />
              </div>
            ))
          )}
        </div>
        <BaselineText x={40} y={338}>
          40 px height · 12 px horizontal padding · 13 px monospace text · gold focus accent · blue selection
        </BaselineText>
        <BaselineText x={40} y={362}>Disabled inputs have no editing, focus, caret or hover response.</BaselineText>
      </section>
    </main>
  );
}

// SVG positions text by its baseline. These HTML line boxes use the same font
// metrics (ascent minus descent), so the original baseline coordinates stay exact.
function BaselineText(props: {
  x: number;
  y: number;
  size?: 12 | 13 | 14 | 15 | 22;
  mono?: boolean;
  anchor?: "start" | "middle" | "end";
  class?: string;
  children: Child;
}): Markup {
  const size = props.size ?? 12;
  const ascentMinusDescent = props.mono
    ? { 12: 8, 13: 9, 14: 10, 15: 10, 22: 15 }[size]
    : { 12: 9, 13: 10, 14: 10, 15: 11, 22: 17 }[size];
  return (
    <span
      class={`absolute block whitespace-nowrap text-[${size}px] leading-none ${props.mono ? "font-mono" : "font-sans"} ${
        props.anchor === "middle" ? "-translate-x-1/2" : props.anchor === "end" ? "-translate-x-full" : ""
      } ${props.class ?? "text-[#8a8a93]"}`}
      style={`left:${props.x}px;top:${props.y - (size + ascentMinusDescent) / 2}px`}
    >
      {props.children}
    </span>
  );
}

function ButtonMatrix({ kind }: { kind: "line" | "cyberpunk" }): Markup {
  return (
    <div class="absolute top-20 left-10 h-[190px] w-[820px] rounded-[10px] bg-[#0b0b0d]">
      {buttonStates.map((state, index) => <BaselineText x={240 + index * 160} y={36} anchor="middle">{state}
      </BaselineText>)}
      <BaselineText x={24} y={87} size={14} class="text-light">Primary</BaselineText>
      <BaselineText x={24} y={151} size={14} class="text-light">Secondary</BaselineText>
      {[true, false].map((primary) =>
        buttonStates.map((state, index) => (
          <div class={`absolute ${primary ? "top-[62px]" : "top-[126px]"}`} style={`left:${176 + index * 160}px`}>
            {kind === "line"
              ? <LineButton primary={primary} state={state} />
              : <CyberpunkButton primary={primary} state={state} />}
          </div>
        ))
      )}
    </div>
  );
}

function LineButton({ primary, state }: { primary: boolean; state: ButtonState }): Markup {
  const disabled = state === "Disabled";
  const pressed = state === "Pressed";
  const hover = state === "Hover";
  const color = primary
    ? disabled ? "text-[#4f4f56]" : pressed ? "text-[#0b0b0d]" : hover ? "text-[#d9ff7a]" : "text-[#c8f25a]"
    : disabled
    ? "text-[#46464c]"
    : pressed
    ? "text-white"
    : hover
    ? "text-text"
    : "text-subtle";
  return (
    <button
      type="button"
      disabled={disabled}
      aria-label={`${primary ? "Export" : "Cancel"}: ${state}`}
      class={`relative block h-10 w-32 ${
        primary
          ? pressed ? "bg-[#c8f25a]" : disabled ? "" : hover ? "bg-[#c8f25a]/12" : "bg-[#c8f25a]/5"
          : pressed
          ? "bg-white/7"
          : ""
      }`}
    >
      {primary && !pressed &&
        (["top-left", "top-right", "bottom-left", "bottom-right"] as const).map((corner) => (
          <span
            class={`absolute size-2.5 ${disabled ? "border-grey" : "border-[#c8f25a]"} ${
              corner === "top-left"
                ? "top-0 left-0 border-t-[1.5px] border-l-[1.5px]"
                : corner === "top-right"
                ? "top-0 right-0 border-t-[1.5px] border-r-[1.5px]"
                : corner === "bottom-left"
                ? "bottom-0 left-0 border-b-[1.5px] border-l-[1.5px]"
                : "right-0 bottom-0 border-r-[1.5px] border-b-[1.5px]"
            } ${hover ? (corner.includes("left") ? "-translate-x-[3px]" : "translate-x-[3px]") : ""} ${
              hover ? (corner.includes("top") ? "-translate-y-[3px]" : "translate-y-[3px]") : ""
            }`}
          />
        ))}
      <BaselineText
        x={primary ? 84 : 65}
        y={24.5}
        size={13}
        mono
        anchor={primary ? "end" : "middle"}
        class={`font-bold tracking-[2px] ${color}`}
      >
        {primary ? "EXPORT" : "CANCEL"}
      </BaselineText>
      {primary && (
        <BaselineText x={hover || pressed ? 94 : 90} y={24.5} size={13} mono class={`font-bold ${color}`}>→</BaselineText>
      )}
      {!primary && !disabled && (
        <span
          class={`absolute ${
            pressed
              ? "top-[31.5px] left-4 h-0.5 w-24 bg-white"
              : hover
              ? "top-8 left-4 h-px w-24 bg-text"
              : "top-8 left-[34px] h-px w-[60px] bg-[#4a4a52]"
          }`}
        />
      )}
    </button>
  );
}

function CyberpunkButton({ primary, state }: { primary: boolean; state: ButtonState }): Markup {
  const disabled = state === "Disabled";
  const pressed = state === "Pressed";
  const hover = state === "Hover";
  return (
    <button
      type="button"
      disabled={disabled}
      aria-label={`${primary ? "Export" : "Cancel"}: ${state}`}
      class={`relative block h-10 w-32 ${primary && pressed ? "translate-x-0.5 translate-y-0.5" : ""}`}
    >
      <svg aria-hidden="true" class="absolute inset-0 h-10 w-32 overflow-visible">
        {primary
          ? (
            <>
              <use
                href="#dPrimaryShape"
                fill={disabled ? "url(#tw-scan-dim)" : pressed ? "#0a0a0b" : "#fcee0a"}
                stroke={disabled ? "#3a3926" : hover ? "#fff9b8" : pressed ? "#fcee0a" : undefined}
                stroke-width={disabled ? 1 : 1.5}
                filter={disabled ? undefined : hover || pressed ? "url(#tw-glow-y-hover)" : "url(#tw-glow-y)"}
              />
              {pressed && <use href="#dPrimaryShape" fill="url(#tw-scan)" />}
              <use href="#dSlashes" fill={disabled ? "#3a3926" : pressed ? "#fcee0a" : "#0a0a0b"} />
            </>
          )
          : (
            <>
              {pressed && <use href="#dSecondaryShape" x="3" y="3" fill="#ff2a6d" />}
              <use
                href="#dSecondaryShape"
                fill={disabled ? "url(#tw-scan-dim)" : "#00f0ff"}
                fill-opacity={disabled || pressed ? 1 : hover ? 0.16 : 0.06}
                stroke={disabled ? "#1f3337" : pressed ? undefined : hover ? "#7ffbff" : "#00f0ff"}
                filter={disabled ? undefined : hover || pressed ? "url(#tw-glow-c-hover)" : "url(#tw-glow-c)"}
              />
              {hover && <rect x="0" y="10" width="4" height="30" fill="#00f0ff" />}
            </>
          )}
      </svg>
      <BaselineText
        x={primary ? 57 : 65}
        y={24.5}
        size={13}
        mono
        anchor="middle"
        class={`font-extrabold tracking-[2px] ${
          primary
            ? disabled
              ? "text-cyber-disabled-text"
              : pressed
              ? "text-cyber-yellow [filter:url(#tw-glow-y)]"
              : "text-cyber-ink"
            : disabled
            ? "text-[#34494d]"
            : pressed
            ? "text-[#050506]"
            : hover
            ? "text-[#b5fcff]"
            : "text-cyber-cyan"
        }`}
      >
        {primary ? "EXPORT" : "CANCEL"}
      </BaselineText>
    </button>
  );
}

function InputExample({ kind, state }: { kind: "Primary" | "Secondary"; state: typeof inputStates[number] }): Markup {
  const disabled = state === "Disabled";
  return (
    <div
      role="img"
      aria-label={`${kind} input: ${state}`}
      class={`relative h-10 w-[124px] ${
        kind === "Primary"
          ? `rounded-md outline-[1px] outline-offset-[-0.5px] ${
            disabled
              ? "bg-[#131316] outline-[#222226]"
              : state === "Focused"
              ? "bg-surface outline-accent"
              : "bg-surface outline-border"
          }`
          : ""
      }`}
    >
      {kind === "Secondary" && (
        <span
          class={`absolute top-[39px] left-0 h-px w-full ${
            disabled ? "bg-[#222226]" : state === "Focused" ? "bg-accent" : "bg-border"
          }`}
        />
      )}
      {state === "Selected" && <span class="absolute top-2.5 left-3 h-5 w-[47px] rounded-[2px] bg-[#52779a]/40" />}
      <BaselineText
        x={12}
        y={25}
        size={13}
        mono
        class={disabled ? "text-dim" : state === "Placeholder" ? "text-muted" : "text-text"}
      >
        {state === "Placeholder" ? "Enter…" : "Sample"}
      </BaselineText>
      {state === "Focused" && <span class="absolute top-2.5 left-[59px] h-5 w-px bg-[#d8d8dc]" />}
    </div>
  );
}

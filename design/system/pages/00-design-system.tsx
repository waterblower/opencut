import { COLOR } from "../theme.ts";
import { FontStyle } from "../components/fonts.tsx";
import { CyberpunkShapeDefs } from "../components/cyberpunk.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

export default function Page00DesignSystemSvg(): Markup {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      xmlns:xlink="http://www.w3.org/1999/xlink"
      width="900"
      height="1342"
      viewBox="0 0 900 1342"
    >
      <FontStyle />
      <defs>
        <CyberpunkShapeDefs />
        <pattern id="dScan" width="4" height="3" patternUnits="userSpaceOnUse">
          <rect width="4" height="1" fill="#fcee0a" fill-opacity="0.2" />
        </pattern>
        <pattern id="dScanDim" width="4" height="3" patternUnits="userSpaceOnUse">
          <rect width="4" height="1" fill="#ffffff" fill-opacity="0.05" />
        </pattern>
        <filter id="dGlowY" x="-30%" y="-80%" width="160%" height="260%">
          <feDropShadow dx="0" dy="0" stdDeviation="6" flood-color="#fcee0a" flood-opacity="0.45" />
        </filter>
        <filter id="dGlowYHover" x="-30%" y="-80%" width="160%" height="260%">
          <feDropShadow dx="0" dy="0" stdDeviation="12" flood-color="#fcee0a" flood-opacity="0.75" />
        </filter>
        <filter id="dGlowC" x="-30%" y="-80%" width="160%" height="260%">
          <feDropShadow dx="0" dy="0" stdDeviation="5" flood-color="#00f0ff" flood-opacity="0.35" />
        </filter>
        <filter id="dGlowCHover" x="-30%" y="-80%" width="160%" height="260%">
          <feDropShadow dx="0" dy="0" stdDeviation="10" flood-color="#00f0ff" flood-opacity="0.7" />
        </filter>
      </defs>
      <rect x="0" y="0" width="900" height="1342" fill="#1e1e22" />
      <g transform="translate(0 -540)">
        <text fill={COLOR.light} class="s" x="40" y="584" font-size="15" font-weight="600">{"C · Line"}</text>
        <text class="s" x="40" y="604" font-size="12" fill="#8a8a93">
          {"A square frame drawn by corner brackets, with mono uppercase labels. Hover opens the brackets; pressing fills the button solid. Secondary is underlined text."}
        </text>
        <rect x="40" y="620" width="820" height="190" rx="10" fill="#0b0b0d" />
        <text class="s" x="280" y="656" font-size="12" fill="#8a8a93" text-anchor="middle">{"Default"}</text>
        <text class="s" x="440" y="656" font-size="12" fill="#8a8a93" text-anchor="middle">{"Hover"}</text>
        <text class="s" x="600" y="656" font-size="12" fill="#8a8a93" text-anchor="middle">{"Pressed"}</text>
        <text class="s" x="760" y="656" font-size="12" fill="#8a8a93" text-anchor="middle">{"Disabled"}</text>
        <text fill={COLOR.light} class="s" x="64" y="707" font-size="14">{"Primary"}</text>
        <text fill={COLOR.light} class="s" x="64" y="771" font-size="14">{"Secondary"}</text>
        <rect x="216" y="682" width="128" height="40" fill="#c8f25a" fill-opacity="0.05" />
        <path
          d="M216.75 692 V682.75 H226 M334 682.75 H343.25 V692 M343.25 712 V721.25 H334 M226 721.25 H216.75 V712"
          fill="none"
          stroke="#c8f25a"
          stroke-width="1.5"
        />
        <text
          class="m"
          x="300"
          y="706.5"
          font-size="13"
          font-weight="700"
          letter-spacing="2"
          fill="#c8f25a"
          text-anchor="end"
        >
          {"EXPORT"}
        </text>
        <text class="m" x="306" y="706.5" font-size="13" font-weight="700" fill="#c8f25a">{"→"}</text>
        <rect x="376" y="682" width="128" height="40" fill="#c8f25a" fill-opacity="0.12" />
        <path
          d="M373.75 689 V679.75 H383 M497 679.75 H506.25 V689 M506.25 715 V724.25 H497 M383 724.25 H373.75 V715"
          fill="none"
          stroke="#c8f25a"
          stroke-width="1.5"
        />
        <text
          class="m"
          x="460"
          y="706.5"
          font-size="13"
          font-weight="700"
          letter-spacing="2"
          fill="#d9ff7a"
          text-anchor="end"
        >
          {"EXPORT"}
        </text>
        <text class="m" x="470" y="706.5" font-size="13" font-weight="700" fill="#d9ff7a">{"→"}</text>
        <rect x="536" y="682" width="128" height="40" fill="#c8f25a" />
        <text
          class="m"
          x="620"
          y="706.5"
          font-size="13"
          font-weight="700"
          letter-spacing="2"
          fill="#0b0b0d"
          text-anchor="end"
        >
          {"EXPORT"}
        </text>
        <text class="m" x="630" y="706.5" font-size="13" font-weight="700" fill="#0b0b0d">{"→"}</text>
        <path
          stroke={COLOR.grey}
          d="M696.75 692 V682.75 H706 M814 682.75 H823.25 V692 M823.25 712 V721.25 H814 M706 721.25 H696.75 V712"
          fill="none"
          stroke-width="1.5"
        />
        <text
          class="m"
          x="780"
          y="706.5"
          font-size="13"
          font-weight="700"
          letter-spacing="2"
          fill="#4f4f56"
          text-anchor="end"
        >
          {"EXPORT"}
        </text>
        <text class="m" x="786" y="706.5" font-size="13" font-weight="700" fill="#4f4f56">{"→"}</text>
        <text
          fill={COLOR.subtle}
          class="m"
          x="281"
          y="770.5"
          font-size="13"
          font-weight="700"
          letter-spacing="2"
          text-anchor="middle"
        >
          {"CANCEL"}
        </text>
        <rect x="250" y="778" width="60" height="1" fill="#4a4a52" />
        <text
          fill={COLOR.text}
          class="m"
          x="441"
          y="770.5"
          font-size="13"
          font-weight="700"
          letter-spacing="2"
          text-anchor="middle"
        >
          {"CANCEL"}
        </text>
        <rect fill={COLOR.text} x="392" y="778" width="96" height="1" />
        <rect x="536" y="746" width="128" height="40" fill="#ffffff" fill-opacity="0.07" />
        <text
          class="m"
          x="601"
          y="770.5"
          font-size="13"
          font-weight="700"
          letter-spacing="2"
          fill="#ffffff"
          text-anchor="middle"
        >
          {"CANCEL"}
        </text>
        <rect x="552" y="777.5" width="96" height="2" fill="#ffffff" />
        <text
          class="m"
          x="761"
          y="770.5"
          font-size="13"
          font-weight="700"
          letter-spacing="2"
          fill="#46464c"
          text-anchor="middle"
        >
          {"CANCEL"}
        </text>
        <text fill={COLOR.light} class="s" x="40" y="854" font-size="15" font-weight="600">
          {"D · Cyberpunk"}
        </text>
        <text class="s" x="40" y="874" font-size="12" fill="#8a8a93">
          {"Cut top-left and bottom-right corners, neon yellow and cyan, hazard slashes. Hover brightens the border and glow; pressing inverts."}
        </text>
        <rect x="40" y="890" width="820" height="190" rx="10" fill="#0b0b0d" />
        <text class="s" x="280" y="926" font-size="12" fill="#8a8a93" text-anchor="middle">{"Default"}</text>
        <text class="s" x="440" y="926" font-size="12" fill="#8a8a93" text-anchor="middle">{"Hover"}</text>
        <text class="s" x="600" y="926" font-size="12" fill="#8a8a93" text-anchor="middle">{"Pressed"}</text>
        <text class="s" x="760" y="926" font-size="12" fill="#8a8a93" text-anchor="middle">{"Disabled"}</text>
        <text fill={COLOR.light} class="s" x="64" y="977" font-size="14">{"Primary"}</text>
        <text fill={COLOR.light} class="s" x="64" y="1041" font-size="14">{"Secondary"}</text>
        <use xlink:href="#dPrimaryShape" x="216" y="952" fill="#fcee0a" filter="url(#dGlowY)" />
        <use xlink:href="#dSlashes" x="216" y="952" fill="#0a0a0b" />
        <text
          class="m"
          x="273"
          y="976.5"
          font-size="13"
          font-weight="800"
          letter-spacing="2"
          fill="#0a0a0b"
          text-anchor="middle"
        >
          {"EXPORT"}
        </text>
        <use
          xlink:href="#dPrimaryShape"
          x="376"
          y="952"
          fill="#fcee0a"
          stroke="#fff9b8"
          stroke-width="1.5"
          filter="url(#dGlowYHover)"
        />
        <use xlink:href="#dSlashes" x="376" y="952" fill="#0a0a0b" />
        <text
          class="m"
          x="433"
          y="976.5"
          font-size="13"
          font-weight="800"
          letter-spacing="2"
          fill="#0a0a0b"
          text-anchor="middle"
        >
          {"EXPORT"}
        </text>
        <g transform="translate(2 2)">
          <use
            xlink:href="#dPrimaryShape"
            x="536"
            y="952"
            fill="#0a0a0b"
            stroke="#fcee0a"
            stroke-width="1.5"
            filter="url(#dGlowYHover)"
          />
          <use xlink:href="#dPrimaryShape" x="536" y="952" fill="url(#dScan)" />
          <use xlink:href="#dSlashes" x="536" y="952" fill="#fcee0a" />
          <text
            class="m"
            x="593"
            y="976.5"
            font-size="13"
            font-weight="800"
            letter-spacing="2"
            fill="#fcee0a"
            text-anchor="middle"
            filter="url(#dGlowY)"
          >
            {"EXPORT"}
          </text>
        </g>
        <use xlink:href="#dPrimaryShape" x="696" y="952" fill="url(#dScanDim)" stroke="#3a3926" />
        <use xlink:href="#dSlashes" x="696" y="952" fill="#3a3926" />
        <text
          class="m"
          x="753"
          y="976.5"
          font-size="13"
          font-weight="800"
          letter-spacing="2"
          fill="#57563f"
          text-anchor="middle"
        >
          {"EXPORT"}
        </text>
        <use
          xlink:href="#dSecondaryShape"
          x="216"
          y="1016"
          fill="#00f0ff"
          fill-opacity="0.06"
          stroke="#00f0ff"
          filter="url(#dGlowC)"
        />
        <text
          class="m"
          x="281"
          y="1040.5"
          font-size="13"
          font-weight="800"
          letter-spacing="2"
          fill="#00f0ff"
          text-anchor="middle"
        >
          {"CANCEL"}
        </text>
        <use
          xlink:href="#dSecondaryShape"
          x="376"
          y="1016"
          fill="#00f0ff"
          fill-opacity="0.16"
          stroke="#7ffbff"
          filter="url(#dGlowCHover)"
        />
        <rect x="376" y="1026" width="4" height="30" fill="#00f0ff" />
        <text
          class="m"
          x="441"
          y="1040.5"
          font-size="13"
          font-weight="800"
          letter-spacing="2"
          fill="#b5fcff"
          text-anchor="middle"
        >
          {"CANCEL"}
        </text>
        <use xlink:href="#dSecondaryShape" x="539" y="1019" fill="#ff2a6d" />
        <use xlink:href="#dSecondaryShape" x="536" y="1016" fill="#00f0ff" filter="url(#dGlowCHover)" />
        <text
          class="m"
          x="601"
          y="1040.5"
          font-size="13"
          font-weight="800"
          letter-spacing="2"
          fill="#050506"
          text-anchor="middle"
        >
          {"CANCEL"}
        </text>
        <use xlink:href="#dSecondaryShape" x="696" y="1016" fill="url(#dScanDim)" stroke="#1f3337" />
        <text
          class="m"
          x="761"
          y="1040.5"
          font-size="13"
          font-weight="800"
          letter-spacing="2"
          fill="#34494d"
          text-anchor="middle"
        >
          {"CANCEL"}
        </text>
      </g>
      <g id="dropdown-trigger-proposals">
        <text fill={COLOR.text} class="s" x="40" y="609" font-size="22" font-weight="700">
          {"Dropdown trigger"}
        </text>
        <text class="s" x="40" y="638" font-size="12" fill="#8a8a93" font-weight="400">
          {"The entire field is clickable. Hover signals that the field can be clicked."}
        </text>
        <g transform="translate(0 -192)">
          <text fill={COLOR.light} class="s" x="40" y="872" font-size="15" font-weight="600">
            {"Gold accent"}
          </text>
          <text class="s" x="40" y="894" font-size="12" fill="#8a8a93" font-weight="400">
            {"Hover uses a warm surface, gold border and gold chevron."}
          </text>
          <rect x="40" y="910" width="820" height="118" rx="10" fill="#0b0b0d" stroke="none" />
          <text class="s" x="90" y="937" font-size="12" fill="#8a8a93" font-weight="400">{"Default"}</text>
          <rect fill={COLOR.surface} stroke={COLOR.border} x="90" y="959" width="180" height="34" rx="6" />
          <text fill={COLOR.text} class="s" x="100" y="981" font-size="13" font-weight="400">
            {"Zapfino"}
          </text>
          <path
            stroke={COLOR.muted}
            d="M247 973l5 5 5 -5"
            fill="none"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
          <text class="s" x="350" y="937" font-size="12" fill="#8a8a93" font-weight="400">{"Hover"}</text>
          <rect
            fill={COLOR.accentBackground}
            stroke={COLOR.accent}
            x="350"
            y="959"
            width="180"
            height="34"
            rx="6"
          />
          <text fill={COLOR.text} class="s" x="360" y="981" font-size="13" font-weight="400">
            {"Zapfino"}
          </text>
          <path
            stroke={COLOR.accent}
            d="M507 973l5 5 5 -5"
            fill="none"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
          <text class="s" x="610" y="937" font-size="12" fill="#8a8a93" font-weight="400">{"Disabled"}</text>
          <rect x="610" y="959" width="180" height="34" rx="6" fill="#131316" stroke="#222226" />
          <text fill={COLOR.dim} class="s" x="620" y="981" font-size="13" font-weight="400">{"Zapfino"}</text>
          <path
            d="M767 973l5 5 5 -5"
            fill="none"
            stroke="#46464e"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </g>
        <text class="s" x="40" y="878" font-size="12" fill="#8a8a93">
          {"The entire field is clickable · no size change on hover · disabled fields do not respond."}
        </text>
      </g>
      <g id="text-input-default">
        <title>{"Text input: primary and secondary states"}</title>
        <text fill={COLOR.text} class="s" x="40" y="966" font-size="22">{"Text Input"}</text>
        <text class="s" x="40" y="994" font-size="12" fill="#8a8a93">
          {"Primary uses a bordered field. Secondary uses an underline for a quieter appearance."}
        </text>
        <rect x="40" y="1018" width="820" height="222" rx="10" fill="#0b0b0d" />
        <text class="s" x="242" y="1048" text-anchor="middle" font-size="12" fill="#8a8a93">
          {"Placeholder"}
        </text>
        <text class="s" x="378" y="1048" text-anchor="middle" font-size="12" fill="#8a8a93">{"Default"}</text>
        <text class="s" x="514" y="1048" text-anchor="middle" font-size="12" fill="#8a8a93">{"Focused"}</text>
        <text class="s" x="650" y="1048" text-anchor="middle" font-size="12" fill="#8a8a93">
          {"Selected"}
        </text>
        <text class="s" x="786" y="1048" text-anchor="middle" font-size="12" fill="#8a8a93">
          {"Disabled"}
        </text>
        <text fill={COLOR.light} class="s" x="60" y="1095" font-size="14">{"Primary"}</text>
        <rect fill={COLOR.surface} stroke={COLOR.border} x="180" y="1070" width="124" height="40" rx="6" />
        <text fill={COLOR.muted} class="m" x="192" y="1095" font-size="13">{"Enter…"}</text>
        <rect fill={COLOR.surface} stroke={COLOR.border} x="316" y="1070" width="124" height="40" rx="6" />
        <text fill={COLOR.text} class="m" x="328" y="1095" font-size="13">{"Sample"}</text>
        <rect fill={COLOR.surface} stroke={COLOR.accent} x="452" y="1070" width="124" height="40" rx="6" />
        <text fill={COLOR.text} class="m" x="464" y="1095" font-size="13">{"Sample"}</text>
        <rect x="511" y="1080" width="1" height="20" fill="#d8d8dc" />
        <rect fill={COLOR.surface} stroke={COLOR.border} x="588" y="1070" width="124" height="40" rx="6" />
        <rect x="600" y="1080" width="47" height="20" rx="2" fill="#52779a" fill-opacity="0.4" />
        <text fill={COLOR.text} class="m" x="600" y="1095" font-size="13">{"Sample"}</text>
        <rect x="724" y="1070" width="124" height="40" rx="6" fill="#131316" stroke="#222226" />
        <text fill={COLOR.dim} class="m" x="736" y="1095" font-size="13">{"Sample"}</text>
        <text fill={COLOR.light} class="s" x="60" y="1187" font-size="14">{"Secondary"}</text>
        <rect fill={COLOR.border} x="180" y="1201" width="124" height="1" />
        <text fill={COLOR.muted} class="m" x="192" y="1187" font-size="13">{"Enter…"}</text>
        <rect fill={COLOR.border} x="316" y="1201" width="124" height="1" />
        <text fill={COLOR.text} class="m" x="328" y="1187" font-size="13">{"Sample"}</text>
        <rect fill={COLOR.accent} x="452" y="1201" width="124" height="1" />
        <text fill={COLOR.text} class="m" x="464" y="1187" font-size="13">{"Sample"}</text>
        <rect x="511" y="1172" width="1" height="20" fill="#d8d8dc" />
        <rect fill={COLOR.border} x="588" y="1201" width="124" height="1" />
        <rect x="600" y="1172" width="47" height="20" rx="2" fill="#52779a" fill-opacity="0.4" />
        <text fill={COLOR.text} class="m" x="600" y="1187" font-size="13">{"Sample"}</text>
        <rect x="724" y="1201" width="124" height="1" fill="#222226" />
        <text fill={COLOR.dim} class="m" x="736" y="1187" font-size="13">{"Sample"}</text>
        <text class="s" x="40" y="1278" font-size="12" fill="#8a8a93">
          {"40 px height · 12 px horizontal padding · 13 px monospace text · gold focus accent · blue selection"}
        </text>
        <text class="s" x="40" y="1302" font-size="12" fill="#8a8a93">
          {"Disabled inputs have no editing, focus, caret or hover response."}
        </text>
      </g>
    </svg>
  );
}

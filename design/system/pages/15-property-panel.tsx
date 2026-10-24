import {
  ClipSubtitle,
  PlainSubtitle,
  PROPERTY_PANEL_HEADER_HEIGHT,
  PropertyPanelHeader,
} from "../components/property-panel-header.tsx";
import { COLOR } from "../theme.ts";
import { FontStyle } from "../components/fonts.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

export default function Page15PropertyPanelSvg(): Markup {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width="2296" height="5139" viewBox="0 0 2296 5139">
      <FontStyle />
      <defs>
        <g id="icon-mic" fill="none" stroke-width="2.25" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
          <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
          <path d="M12 19v3" />
        </g>
        <clipPath id="c3">
          <rect x="368" y="198" width="280" height="555" rx="10" />
        </clipPath>
        <clipPath id="c4">
          <rect x="368" y="198" width="280" height="555" rx="0" />
        </clipPath>
        <clipPath id="c5">
          <rect x="688" y="198" width="280" height="555" rx="10" />
        </clipPath>
        <clipPath id="c6">
          <rect x="688" y="198" width="280" height="555" rx="0" />
        </clipPath>
        <clipPath id="c9">
          <rect x="48" y="841" width="280" height="555" rx="10" />
        </clipPath>
        <clipPath id="c10">
          <rect x="48" y="841" width="280" height="555" rx="0" />
        </clipPath>
        <clipPath id="c11">
          <rect x="368" y="841" width="280" height="555" rx="10" />
        </clipPath>
        <clipPath id="c12">
          <rect x="368" y="841" width="280" height="555" rx="0" />
        </clipPath>
        <clipPath id="c13">
          <rect x="688" y="841" width="280" height="555" rx="10" />
        </clipPath>
        <clipPath id="c14">
          <rect x="688" y="841" width="280" height="555" rx="0" />
        </clipPath>
        <clipPath id="c15">
          <rect x="1008" y="841" width="280" height="555" rx="10" />
        </clipPath>
        <clipPath id="c16">
          <rect x="1008" y="841" width="280" height="555" rx="0" />
        </clipPath>
        <pattern id="locked-panel-stripes" width="8" height="8" patternUnits="userSpaceOnUse">
          <path d="M-2 2 L2 -2 M0 8 L8 0 M6 10 L10 6" stroke="#676773" stroke-width="2" />
        </pattern>
      </defs>
      <rect fill={COLOR.page} x="0" y="0" width="2296" height="5139" />
      <text fill={COLOR.text} class="s" x="48" y="60.64" font-size="24" font-weight="700">
        {"Property Panel"}
      </text>
      <text fill={COLOR.muted} class="s" x="48" y="87.04" font-size="14">
        {"One panel type per row. Compare its variants side by side."}
      </text>
      <rect
        fill={COLOR.surface}
        stroke={COLOR.border}
        x="48.5"
        y="104.5"
        width="95"
        height="29"
        rx="6"
        stroke-width="1"
      />
      <text fill={COLOR.muted} class="s" x="58" y="123.32" font-size="12" font-weight="600">{"X"}</text>
      <text fill={COLOR.text} class="m" x="73.68" y="123.68" font-size="13">{"0.50"}</text>
      <text fill={COLOR.soft} class="s" x="154" y="123.68" font-size="13">{"Editable input"}</text>
      <rect x="266.86" y="104" width="96" height="30" rx="6" fill="#131316" />
      <text fill={COLOR.text} class="m" x="276.86" y="123.68" font-size="13">{"0:08.00"}</text>
      <text fill={COLOR.soft} class="s" x="372.86" y="123.68" font-size="13">{"Read-only value"}</text>
      <text fill={COLOR.accent} class="s" x="501.71" y="123.68" font-size="13">{"Change…"}</text>
      <text fill={COLOR.soft} class="s" x="562.15" y="123.68" font-size="13">{"Action"}</text>
      <g id="row-header">
        <text fill={COLOR.text} class="s" x="48" y="180" font-size="18" font-weight="600">{"Header"}</text>
        <text fill={COLOR.muted} class="s" x="48" y="204" font-size="12" font-weight="400">
          {"Shared by every property panel: 12px around the icon, then a divider."}
        </text>
        <HeaderDemo x={48} label="Clip">
          <PropertyPanelHeader
            x={48}
            y={248}
            icon={{ type: "track", kind: "V" }}
            title="broll-street.mov"
            subtitle={<ClipSubtitle clipType="Video clip" track="Video 1" kind="V" />}
          />
        </HeaderDemo>
        <HeaderDemo x={368} label="Clip on a locked track">
          <PropertyPanelHeader
            x={368}
            y={248}
            icon={{ type: "locked" }}
            title="broll-street.mov"
            subtitle={<ClipSubtitle clipType="Video clip" track="Video 1" kind="V" locked />}
            locked
          />
        </HeaderDemo>
        <HeaderDemo x={688} label="File">
          <PropertyPanelHeader
            x={688}
            y={248}
            icon={{ type: "file", label: "MP4", labelColor: "#8fb9dd", stroke: "#355b78" }}
            title="interview.mp4"
            subtitle={<PlainSubtitle>Video file</PlainSubtitle>}
          />
        </HeaderDemo>
        <HeaderDemo x={1008} label="Timeline">
          <PropertyPanelHeader
            x={1008}
            y={248}
            icon={{ type: "timeline" }}
            title="edit.timeline"
            subtitle={<PlainSubtitle>timelines/edit.timeline</PlainSubtitle>}
          />
        </HeaderDemo>
      </g>
      <g transform="translate(0 -500)">
        <g id="row-video-clip">
          <text fill={COLOR.text} class="s" x="48" y="864" font-size="18" font-weight="600">
            {"Video clip"}
          </text>
          <text fill={COLOR.muted} class="s" x="48" y="888" font-size="12" font-weight="400">
            {"Transform, gain and mute editable; timing read-only"}
          </text>
          <text fill={COLOR.soft} class="s" x="48" y="912" font-size="12" font-weight="600">{"Default"}</text>
          <g transform="translate(-320 734)">
            <rect
              x="367.5"
              y="197.5"
              width="281"
              height="556"
              rx="10"
              fill="#000"
              opacity="0.35"
              transform="translate(0 6)"
            />
            <g clip-path="url(#c3)">
              <rect fill={COLOR.panel} x="368" y="198" width="280" height="555" />
              <g clip-path="url(#c4)">
                <PropertyPanelHeader
                  x={368}
                  y={198}
                  icon={{ type: "track", kind: "V" }}
                  title={"broll-street.mov"}
                  subtitle={<ClipSubtitle clipType="Video clip" track="Video 1" kind="V" />}
                />
                <g transform="translate(0 -8)">
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="384"
                    y="285.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"TIMING"}
                  </text>
                  <rect x="384" y="298" width="120" height="50" rx="6" fill="#131316" />
                  <text fill={COLOR.muted} class="s" x="394" y="316.96" font-size="11">{"Start"}</text>
                  <text fill={COLOR.text} class="m" x="394" y="336.68" font-size="13">{"0:08.00"}</text>
                  <text fill={COLOR.dim} class="m" x="494" y="335.96" font-size="11" text-anchor="end">
                    {"240 f"}
                  </text>
                  <rect x="512" y="298" width="120" height="50" rx="6" fill="#131316" />
                  <text fill={COLOR.muted} class="s" x="522" y="316.96" font-size="11">{"Duration"}</text>
                  <text fill={COLOR.text} class="m" x="522" y="336.68" font-size="13">{"0:09.50"}</text>
                  <text fill={COLOR.dim} class="m" x="622" y="335.96" font-size="11" text-anchor="end">
                    {"285 f"}
                  </text>
                  <rect x="384" y="356" width="120" height="50" rx="6" fill="#131316" />
                  <text fill={COLOR.muted} class="s" x="394" y="374.96" font-size="11">{"Source in"}</text>
                  <text fill={COLOR.text} class="m" x="394" y="394.68" font-size="13">{"0:44.00"}</text>
                  <text fill={COLOR.dim} class="m" x="494" y="393.96" font-size="11" text-anchor="end">
                    {"1320 f"}
                  </text>
                  <rect x="512" y="356" width="120" height="50" rx="6" fill="#131316" />
                  <text fill={COLOR.muted} class="s" x="522" y="374.96" font-size="11">{"Source out"}</text>
                  <text fill={COLOR.text} class="m" x="522" y="394.68" font-size="13">{"0:53.50"}</text>
                  <text fill={COLOR.dim} class="m" x="622" y="393.96" font-size="11" text-anchor="end">
                    {"1605 f"}
                  </text>
                  <rect fill={COLOR.divider} x="384" y="422" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="384"
                    y="445.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"TRANSFORM"}
                  </text>
                  <text fill={COLOR.accent} class="s" x="632" y="446.32" font-size="12" text-anchor="end">
                    {"Reset"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="384" y="477.68" font-size="13">{"Position"}</text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="384.5"
                    y="484.5"
                    width="119"
                    height="29"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.muted} class="s" x="394" y="503.32" font-size="12" font-weight="600">
                    {"X"}
                  </text>
                  <text fill={COLOR.text} class="m" x="409.68" y="503.68" font-size="13">{"0"}</text>
                  <text fill={COLOR.muted} class="s" x="494" y="503.32" font-size="12" text-anchor="end">
                    {"px"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="512.5"
                    y="484.5"
                    width="119"
                    height="29"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.muted} class="s" x="522" y="503.32" font-size="12" font-weight="600">
                    {"Y"}
                  </text>
                  <text fill={COLOR.text} class="m" x="537.68" y="503.68" font-size="13">{"0"}</text>
                  <text fill={COLOR.muted} class="s" x="622" y="503.32" font-size="12" text-anchor="end">
                    {"px"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="384" y="549.68" font-size="13">{"Scale"}</text>
                  <rect fill={COLOR.border} x="432" y="543" width="104" height="4" rx="2" />
                  <rect fill={COLOR.dim} x="483.5" y="540" width="1" height="10" />
                  <rect fill={COLOR.accent} x="432" y="543" width="52" height="4" rx="2" />
                  <circle fill={COLOR.text} stroke={COLOR.panel} cx="484" cy="545" r="7" stroke-width="2" />
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="548.5"
                    y="530.5"
                    width="83"
                    height="29"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="m" x="558" y="549.68" font-size="13">{"100"}</text>
                  <text fill={COLOR.muted} class="s" x="622" y="549.32" font-size="12" text-anchor="end">
                    {"%"}
                  </text>
                  <rect fill={COLOR.divider} x="384" y="576" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="384"
                    y="599.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"AUDIO"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="384" y="631.68" font-size="13">{"Gain"}</text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="548.5"
                    y="612.5"
                    width="83"
                    height="29"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="m" x="558" y="631.68" font-size="13">{"0.00"}</text>
                  <text fill={COLOR.muted} class="s" x="622" y="631.32" font-size="12" text-anchor="end">
                    {"dB"}
                  </text>
                  <use stroke={COLOR.light} href="#icon-mic" transform="translate(384 658) scale(0.6667)" />
                  <rect fill={COLOR.accent} x="598" y="656" width="34" height="20" rx="10" />
                  <circle fill={COLOR.panel} cx="622" cy="666" r="7" />
                </g>
              </g>
            </g>
            <rect
              stroke={COLOR.border}
              x="368.5"
              y="198.5"
              width="279"
              height="554"
              rx="10"
              fill="none"
              stroke-width="1"
            />
          </g>
          <text fill={COLOR.soft} class="s" x="368" y="912" font-size="12" font-weight="600">
            {"Locked track · read-only"}
          </text>
          <g transform="translate(0 734)">
            <rect
              x="367.5"
              y="197.5"
              width="281"
              height="556"
              rx="10"
              fill="#000"
              opacity="0.35"
              transform="translate(0 6)"
            />
            <g clip-path="url(#c3)">
              <rect fill={COLOR.panel} x="368" y="198" width="280" height="555" />
              <g clip-path="url(#c4)">
                <PropertyPanelHeader
                  x={368}
                  y={198}
                  icon={{ type: "locked" }}
                  title={"broll-street.mov"}
                  subtitle={<ClipSubtitle clipType="Video clip" track="Video 1" kind="V" locked />}
                  locked
                />
                <rect x="368" y="198" width="280" height="5" fill="url(#locked-panel-stripes)" />
                <rect x="368" y="198" width="5" height="555" fill="url(#locked-panel-stripes)" />
                <g transform="translate(0 -8)">
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="384"
                    y="285.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"TIMING"}
                  </text>
                  <rect x="384" y="298" width="120" height="50" rx="6" fill="#19191d" />
                  <text fill={COLOR.muted} class="s" x="394" y="316.96" font-size="11">{"Start"}</text>
                  <text fill={COLOR.text} class="m" x="394" y="336.68" font-size="13">{"0:08.00"}</text>
                  <text fill={COLOR.dim} class="m" x="494" y="335.96" font-size="11" text-anchor="end">
                    {"240 f"}
                  </text>
                  <rect x="512" y="298" width="120" height="50" rx="6" fill="#19191d" />
                  <text fill={COLOR.muted} class="s" x="522" y="316.96" font-size="11">{"Duration"}</text>
                  <text fill={COLOR.text} class="m" x="522" y="336.68" font-size="13">{"0:09.50"}</text>
                  <text fill={COLOR.dim} class="m" x="622" y="335.96" font-size="11" text-anchor="end">
                    {"285 f"}
                  </text>
                  <rect x="384" y="356" width="120" height="50" rx="6" fill="#19191d" />
                  <text fill={COLOR.muted} class="s" x="394" y="374.96" font-size="11">{"Source in"}</text>
                  <text fill={COLOR.text} class="m" x="394" y="394.68" font-size="13">{"0:44.00"}</text>
                  <text fill={COLOR.dim} class="m" x="494" y="393.96" font-size="11" text-anchor="end">
                    {"1320 f"}
                  </text>
                  <rect x="512" y="356" width="120" height="50" rx="6" fill="#19191d" />
                  <text fill={COLOR.muted} class="s" x="522" y="374.96" font-size="11">{"Source out"}</text>
                  <text fill={COLOR.text} class="m" x="522" y="394.68" font-size="13">{"0:53.50"}</text>
                  <text fill={COLOR.dim} class="m" x="622" y="393.96" font-size="11" text-anchor="end">
                    {"1605 f"}
                  </text>
                  <rect fill={COLOR.divider} x="384" y="422" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="384"
                    y="445.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"TRANSFORM"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="384" y="477.68" font-size="13">{"Position"}</text>
                  <rect x="384" y="484" width="120" height="30" rx="6" fill="#19191d" />
                  <text fill={COLOR.muted} class="s" x="394" y="503.32" font-size="12" font-weight="600">
                    {"X"}
                  </text>
                  <text fill={COLOR.text} class="m" x="409.68" y="503.68" font-size="13">{"0"}</text>
                  <text fill={COLOR.muted} class="s" x="494" y="503.32" font-size="12" text-anchor="end">
                    {"px"}
                  </text>
                  <rect x="512" y="484" width="120" height="30" rx="6" fill="#19191d" />
                  <text fill={COLOR.muted} class="s" x="522" y="503.32" font-size="12" font-weight="600">
                    {"Y"}
                  </text>
                  <text fill={COLOR.text} class="m" x="537.68" y="503.68" font-size="13">{"0"}</text>
                  <text fill={COLOR.muted} class="s" x="622" y="503.32" font-size="12" text-anchor="end">
                    {"px"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="384" y="549.68" font-size="13">{"Scale"}</text>
                  <rect fill={COLOR.border} x="432" y="543" width="104" height="4" rx="2" />
                  <rect fill={COLOR.dim} x="483.5" y="540" width="1" height="10" />
                  <rect fill={COLOR.dim} x="432" y="543" width="52" height="4" rx="2" />
                  <circle fill={COLOR.dim} cx="484" cy="545" r="5" />
                  <rect x="548" y="530" width="84" height="30" rx="6" fill="#19191d" />
                  <text fill={COLOR.text} class="m" x="558" y="549.68" font-size="13">{"100"}</text>
                  <text fill={COLOR.muted} class="s" x="622" y="549.32" font-size="12" text-anchor="end">
                    {"%"}
                  </text>
                  <rect fill={COLOR.divider} x="384" y="576" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="384"
                    y="599.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"AUDIO"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="384" y="631.68" font-size="13">{"Gain"}</text>
                  <rect x="548" y="612" width="84" height="30" rx="6" fill="#19191d" />
                  <text fill={COLOR.text} class="m" x="558" y="631.68" font-size="13">{"0.00"}</text>
                  <text fill={COLOR.muted} class="s" x="622" y="631.32" font-size="12" text-anchor="end">
                    {"dB"}
                  </text>
                  <g
                    stroke={COLOR.light}
                    transform="translate(384 658) scale(0.6667)"
                    fill="none"
                    stroke-width="2.25"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    opacity="0.5"
                  >
                    <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
                    <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
                    <path d="M12 19v3" />
                  </g>
                  <rect fill={COLOR.accent} x="598" y="656" width="34" height="20" rx="10" opacity="0.5" />
                  <circle fill={COLOR.panel} cx="622" cy="666" r="7" opacity="0.5" />
                </g>
              </g>
            </g>
            <rect
              x="368.5"
              y="198.5"
              width="279"
              height="554"
              rx="10"
              fill="none"
              stroke="#62626e"
              stroke-width="1"
            />
          </g>
          <text fill={COLOR.soft} class="s" x="688" y="912" font-size="12" font-weight="600">
            {"Shared AUDIO section · unmuted"}
          </text>
          <g transform="translate(320 734)">
            <rect
              x="367.5"
              y="197.5"
              width="281"
              height="141"
              rx="10"
              fill="#000"
              opacity="0.35"
              transform="translate(0 6)"
            />
            <rect fill={COLOR.panel} x="368" y="198" width="280" height="140" rx="10" />
            <text
              fill={COLOR.muted}
              class="s"
              x="384"
              y="237.96"
              font-size="11"
              font-weight="600"
              letter-spacing="0.6"
            >
              {"AUDIO"}
            </text>
            <text fill={COLOR.muted} class="s" x="384" y="269.68" font-size="13">{"Gain"}</text>
            <rect
              fill={COLOR.surface}
              stroke={COLOR.border}
              x="548.5"
              y="250.5"
              width="83"
              height="29"
              rx="6"
              stroke-width="1"
            />
            <text fill={COLOR.text} class="m" x="558" y="269.68" font-size="13">{"0.00"}</text>
            <text fill={COLOR.muted} class="s" x="622" y="269.32" font-size="12" text-anchor="end">
              {"dB"}
            </text>
            <use stroke={COLOR.light} href="#icon-mic" transform="translate(384 296) scale(0.6667)" />
            <rect fill={COLOR.accent} x="598" y="294" width="34" height="20" rx="10" />
            <circle fill={COLOR.panel} cx="622" cy="304" r="7" />
            <rect
              stroke={COLOR.border}
              x="368.5"
              y="198.5"
              width="279"
              height="139"
              rx="10"
              fill="none"
              stroke-width="1"
            />
            <text fill={COLOR.muted} class="s" x="368" y="364" font-size="12">
              {"One component, used by both panels."}
            </text>
            <text fill={COLOR.muted} class="s" x="368" y="384" font-size="12">
              {"Gain dB field: −96 to +12 dB."}
            </text>
            <text fill={COLOR.muted} class="s" x="368" y="404" font-size="12">
              {"Locked tracks keep the layout, read-only."}
            </text>
          </g>
          <text fill={COLOR.soft} class="s" x="1008" y="912" font-size="12" font-weight="600">
            {"Shared AUDIO section · muted"}
          </text>
          <g transform="translate(640 734)">
            <rect
              x="367.5"
              y="197.5"
              width="281"
              height="141"
              rx="10"
              fill="#000"
              opacity="0.35"
              transform="translate(0 6)"
            />
            <rect fill={COLOR.panel} x="368" y="198" width="280" height="140" rx="10" />
            <text
              fill={COLOR.muted}
              class="s"
              x="384"
              y="237.96"
              font-size="11"
              font-weight="600"
              letter-spacing="0.6"
            >
              {"AUDIO"}
            </text>
            <text fill={COLOR.muted} class="s" x="384" y="269.68" font-size="13">{"Gain"}</text>
            <rect
              fill={COLOR.surface}
              stroke={COLOR.border}
              x="548.5"
              y="250.5"
              width="83"
              height="29"
              rx="6"
              stroke-width="1"
            />
            <text fill={COLOR.text} class="m" x="558" y="269.68" font-size="13">{"0.00"}</text>
            <text fill={COLOR.muted} class="s" x="622" y="269.32" font-size="12" text-anchor="end">
              {"dB"}
            </text>
            <g
              stroke={COLOR.light}
              transform="translate(384 296) scale(0.6667)"
              fill="none"
              stroke-width="2.25"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path d="M2 2l20 20" />
              <path d="M18.89 13.23A7.12 7.12 0 0 0 19 12v-2" />
              <path d="M5 10v2a7 7 0 0 0 12 5" />
              <path d="M15 9.34V5a3 3 0 0 0-5.68-1.33" />
              <path d="M9 9v3a3 3 0 0 0 5.12 2.12" />
              <path d="M12 19v3" />
            </g>
            <rect fill={COLOR.border} x="598" y="294" width="34" height="20" rx="10" />
            <circle fill={COLOR.muted} cx="608" cy="304" r="7" />
            <rect
              stroke={COLOR.border}
              x="368.5"
              y="198.5"
              width="279"
              height="139"
              rx="10"
              fill="none"
              stroke-width="1"
            />
          </g>
        </g>
        <g id="row-audio-clip">
          <text fill={COLOR.text} class="s" x="48" y="1548" font-size="18" font-weight="600">
            {"Audio clip"}
          </text>
          <text fill={COLOR.muted} class="s" x="48" y="1572" font-size="12" font-weight="400">
            {"Gain and mute editable"}
          </text>
          <text fill={COLOR.soft} class="s" x="48" y="1596" font-size="12" font-weight="600">
            {"Default"}
          </text>
          <g transform="translate(-640 1418)">
            <rect
              x="687.5"
              y="197.5"
              width="281"
              height="556"
              rx="10"
              fill="#000"
              opacity="0.35"
              transform="translate(0 6)"
            />
            <g clip-path="url(#c5)">
              <rect fill={COLOR.panel} x="688" y="198" width="280" height="555" />
              <g clip-path="url(#c6)">
                <PropertyPanelHeader
                  x={688}
                  y={198}
                  icon={{ type: "track", kind: "A" }}
                  title={"voice-over.m4a"}
                  subtitle={<ClipSubtitle clipType="Audio clip" track="Audio 1" kind="A" />}
                />
                <g transform="translate(0 -8)">
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="704"
                    y="285.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"TIMING"}
                  </text>
                  <rect x="704" y="298" width="120" height="50" rx="6" fill="#131316" />
                  <text fill={COLOR.muted} class="s" x="714" y="316.96" font-size="11">{"Start"}</text>
                  <text fill={COLOR.text} class="m" x="714" y="336.68" font-size="13">{"0:00.00"}</text>
                  <text fill={COLOR.dim} class="m" x="814" y="335.96" font-size="11" text-anchor="end">
                    {"0 f"}
                  </text>
                  <rect x="832" y="298" width="120" height="50" rx="6" fill="#131316" />
                  <text fill={COLOR.muted} class="s" x="842" y="316.96" font-size="11">{"Duration"}</text>
                  <text fill={COLOR.text} class="m" x="842" y="336.68" font-size="13">{"0:15.00"}</text>
                  <text fill={COLOR.dim} class="m" x="942" y="335.96" font-size="11" text-anchor="end">
                    {"450 f"}
                  </text>
                  <rect x="704" y="356" width="120" height="50" rx="6" fill="#131316" />
                  <text fill={COLOR.muted} class="s" x="714" y="374.96" font-size="11">{"Source in"}</text>
                  <text fill={COLOR.text} class="m" x="714" y="394.68" font-size="13">{"0:00.00"}</text>
                  <text fill={COLOR.dim} class="m" x="814" y="393.96" font-size="11" text-anchor="end">
                    {"0 f"}
                  </text>
                  <rect x="832" y="356" width="120" height="50" rx="6" fill="#131316" />
                  <text fill={COLOR.muted} class="s" x="842" y="374.96" font-size="11">{"Source out"}</text>
                  <text fill={COLOR.text} class="m" x="842" y="394.68" font-size="13">{"0:15.00"}</text>
                  <text fill={COLOR.dim} class="m" x="942" y="393.96" font-size="11" text-anchor="end">
                    {"450 f"}
                  </text>
                  <rect fill={COLOR.divider} x="704" y="422" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="704"
                    y="445.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"AUDIO"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="704" y="477.68" font-size="13">{"Gain"}</text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="868.5"
                    y="458.5"
                    width="83"
                    height="29"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="m" x="878" y="477.68" font-size="13">{"0.00"}</text>
                  <text fill={COLOR.muted} class="s" x="942" y="477.32" font-size="12" text-anchor="end">
                    {"dB"}
                  </text>
                  <use stroke={COLOR.light} href="#icon-mic" transform="translate(704 504) scale(0.6667)" />
                  <rect fill={COLOR.accent} x="918" y="502" width="34" height="20" rx="10" />
                  <circle fill={COLOR.panel} cx="942" cy="512" r="7" />
                </g>
              </g>
            </g>
            <rect
              stroke={COLOR.border}
              x="688.5"
              y="198.5"
              width="279"
              height="554"
              rx="10"
              fill="none"
              stroke-width="1"
            />
          </g>
          <text fill={COLOR.soft} class="s" x="368" y="1596" font-size="12" font-weight="600">
            {"Locked track · read-only"}
          </text>
          <g transform="translate(-320 1418)">
            <rect
              x="687.5"
              y="197.5"
              width="281"
              height="556"
              rx="10"
              fill="#000"
              opacity="0.35"
              transform="translate(0 6)"
            />
            <g clip-path="url(#c5)">
              <rect fill={COLOR.panel} x="688" y="198" width="280" height="555" />
              <g clip-path="url(#c6)">
                <PropertyPanelHeader
                  x={688}
                  y={198}
                  icon={{ type: "locked" }}
                  title={"voice-over.m4a"}
                  subtitle={<ClipSubtitle clipType="Audio clip" track="Audio 1" kind="A" locked />}
                  locked
                />
                <rect x="688" y="198" width="280" height="5" fill="url(#locked-panel-stripes)" />
                <rect x="688" y="198" width="5" height="555" fill="url(#locked-panel-stripes)" />
                <g transform="translate(0 -8)">
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="704"
                    y="285.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"TIMING"}
                  </text>
                  <rect x="704" y="298" width="120" height="50" rx="6" fill="#19191d" />
                  <text fill={COLOR.muted} class="s" x="714" y="316.96" font-size="11">{"Start"}</text>
                  <text fill={COLOR.text} class="m" x="714" y="336.68" font-size="13">{"0:00.00"}</text>
                  <text fill={COLOR.dim} class="m" x="814" y="335.96" font-size="11" text-anchor="end">
                    {"0 f"}
                  </text>
                  <rect x="832" y="298" width="120" height="50" rx="6" fill="#19191d" />
                  <text fill={COLOR.muted} class="s" x="842" y="316.96" font-size="11">{"Duration"}</text>
                  <text fill={COLOR.text} class="m" x="842" y="336.68" font-size="13">{"0:15.00"}</text>
                  <text fill={COLOR.dim} class="m" x="942" y="335.96" font-size="11" text-anchor="end">
                    {"450 f"}
                  </text>
                  <rect x="704" y="356" width="120" height="50" rx="6" fill="#19191d" />
                  <text fill={COLOR.muted} class="s" x="714" y="374.96" font-size="11">{"Source in"}</text>
                  <text fill={COLOR.text} class="m" x="714" y="394.68" font-size="13">{"0:00.00"}</text>
                  <text fill={COLOR.dim} class="m" x="814" y="393.96" font-size="11" text-anchor="end">
                    {"0 f"}
                  </text>
                  <rect x="832" y="356" width="120" height="50" rx="6" fill="#19191d" />
                  <text fill={COLOR.muted} class="s" x="842" y="374.96" font-size="11">{"Source out"}</text>
                  <text fill={COLOR.text} class="m" x="842" y="394.68" font-size="13">{"0:15.00"}</text>
                  <text fill={COLOR.dim} class="m" x="942" y="393.96" font-size="11" text-anchor="end">
                    {"450 f"}
                  </text>
                  <rect fill={COLOR.divider} x="704" y="422" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="704"
                    y="445.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"AUDIO"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="704" y="477.68" font-size="13">{"Gain"}</text>
                  <rect x="868" y="458" width="84" height="30" rx="6" fill="#19191d" />
                  <text fill={COLOR.text} class="m" x="878" y="477.68" font-size="13">{"0.00"}</text>
                  <text fill={COLOR.muted} class="s" x="942" y="477.32" font-size="12" text-anchor="end">
                    {"dB"}
                  </text>
                  <g
                    stroke={COLOR.light}
                    transform="translate(704 504) scale(0.6667)"
                    fill="none"
                    stroke-width="2.25"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    opacity="0.5"
                  >
                    <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
                    <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
                    <path d="M12 19v3" />
                  </g>
                  <rect fill={COLOR.accent} x="918" y="502" width="34" height="20" rx="10" opacity="0.5" />
                  <circle fill={COLOR.panel} cx="942" cy="512" r="7" opacity="0.5" />
                </g>
              </g>
            </g>
            <rect
              x="688.5"
              y="198.5"
              width="279"
              height="554"
              rx="10"
              fill="none"
              stroke="#62626e"
              stroke-width="1"
            />
          </g>
        </g>
        <g transform="translate(0 -684)">
          <g id="row-video-file">
            <text fill={COLOR.text} class="s" x="48" y="2916" font-size="18" font-weight="600">
              {"Video file"}
            </text>
            <text fill={COLOR.muted} class="s" x="48" y="2940" font-size="12" font-weight="400">
              {"Selected in the Explorer"}
            </text>
            <text fill={COLOR.soft} class="s" x="48" y="2964" font-size="12" font-weight="600">
              {"Default"}
            </text>
            <g transform="translate(0 2143)">
              <rect
                x="47.5"
                y="840.5"
                width="281"
                height="556"
                rx="10"
                fill="#000"
                opacity="0.35"
                transform="translate(0 6)"
              />
              <g clip-path="url(#c9)">
                <rect fill={COLOR.panel} x="48" y="841" width="280" height="555" />
                <g clip-path="url(#c10)">
                  <PropertyPanelHeader
                    x={48}
                    y={841}
                    icon={{ type: "file", label: "MP4", labelColor: "#8fb9dd", stroke: "#355b78" }}
                    title={"interview.mp4"}
                    subtitle={<PlainSubtitle>{"Video file"}</PlainSubtitle>}
                  />
                  <g transform="translate(0 -8)">
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="928.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"FILE"}
                    </text>
                    <text fill={COLOR.muted} class="s" x="64" y="959.68" font-size="13">{"Format"}</text>
                    <text fill={COLOR.text} class="m" x="312" y="959.68" font-size="13" text-anchor="end">
                      {"MP4"}
                    </text>
                    <text fill={COLOR.muted} class="s" x="64" y="987.68" font-size="13">{"Size"}</text>
                    <text fill={COLOR.text} class="m" x="312" y="987.68" font-size="13" text-anchor="end">
                      {"1.2 GB"}
                    </text>
                    <rect fill={COLOR.divider} x="64" y="1005" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="1028.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"LOCATION"}
                    </text>
                    <text fill={COLOR.soft} class="m" x="64" y="1051.32" font-size="12">
                      {"/Users/me/market-vlog/"}
                    </text>
                    <text fill={COLOR.soft} class="m" x="64" y="1069.32" font-size="12">
                      {"media/interview.mp4"}
                    </text>
                    <rect fill={COLOR.divider} x="64" y="1091" width="248" height="1" />
                    <text fill={COLOR.muted} class="s" x="64" y="1121.32" font-size="12">
                      {"Drag onto a video track to add it"}
                    </text>
                    <text fill={COLOR.muted} class="s" x="64" y="1138.32" font-size="12">
                      {"to the timeline."}
                    </text>
                  </g>
                </g>
              </g>
              <rect
                stroke={COLOR.border}
                x="48.5"
                y="841.5"
                width="279"
                height="554"
                rx="10"
                fill="none"
                stroke-width="1"
              />
            </g>
          </g>
          <g id="row-audio-file">
            <text fill={COLOR.text} class="s" x="48" y="3600" font-size="18" font-weight="600">
              {"Audio file"}
            </text>
            <text fill={COLOR.muted} class="s" x="48" y="3624" font-size="12" font-weight="400">
              {"Selected in the Explorer"}
            </text>
            <text fill={COLOR.soft} class="s" x="48" y="3648" font-size="12" font-weight="600">
              {"Default"}
            </text>
            <g transform="translate(0 2827)">
              <rect
                x="47.5"
                y="840.5"
                width="281"
                height="556"
                rx="10"
                fill="#000"
                opacity="0.35"
                transform="translate(0 6)"
              />
              <g clip-path="url(#c9)">
                <rect fill={COLOR.panel} x="48" y="841" width="280" height="555" />
                <g clip-path="url(#c10)">
                  <PropertyPanelHeader
                    x={48}
                    y={841}
                    icon={{ type: "file", label: "M4A", labelColor: "#7fd0ae", stroke: "#32725a" }}
                    title={"voice-over.m4a"}
                    subtitle={<PlainSubtitle>{"Audio file"}</PlainSubtitle>}
                  />
                  <g transform="translate(0 -8)">
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="928.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"FILE"}
                    </text>
                    <text fill={COLOR.muted} class="s" x="64" y="959.68" font-size="13">{"Format"}</text>
                    <text fill={COLOR.text} class="m" x="312" y="959.68" font-size="13" text-anchor="end">
                      {"M4A"}
                    </text>
                    <text fill={COLOR.muted} class="s" x="64" y="987.68" font-size="13">{"Size"}</text>
                    <text fill={COLOR.text} class="m" x="312" y="987.68" font-size="13" text-anchor="end">
                      {"12.4 MB"}
                    </text>
                    <rect fill={COLOR.divider} x="64" y="1005" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="1028.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"LOCATION"}
                    </text>
                    <text fill={COLOR.soft} class="m" x="64" y="1051.32" font-size="12">
                      {"/Users/me/market-vlog/"}
                    </text>
                    <text fill={COLOR.soft} class="m" x="64" y="1069.32" font-size="12">
                      {"media/voice-over.m4a"}
                    </text>
                    <rect fill={COLOR.divider} x="64" y="1091" width="248" height="1" />
                    <text fill={COLOR.muted} class="s" x="64" y="1121.32" font-size="12">
                      {"Drag onto an audio track to add it"}
                    </text>
                    <text fill={COLOR.muted} class="s" x="64" y="1138.32" font-size="12">
                      {"to the timeline."}
                    </text>
                  </g>
                </g>
              </g>
              <rect
                stroke={COLOR.border}
                x="48.5"
                y="841.5"
                width="279"
                height="554"
                rx="10"
                fill="none"
                stroke-width="1"
              />
            </g>
          </g>
          <g id="row-image-file">
            <text fill={COLOR.text} class="s" x="48" y="4284" font-size="18" font-weight="600">
              {"Image file"}
            </text>
            <text fill={COLOR.muted} class="s" x="48" y="4308" font-size="12" font-weight="400">
              {"Selected in the Explorer"}
            </text>
            <text fill={COLOR.soft} class="s" x="48" y="4332" font-size="12" font-weight="600">
              {"Default"}
            </text>
            <g transform="translate(-320 3511)">
              <rect
                x="367.5"
                y="840.5"
                width="281"
                height="556"
                rx="10"
                fill="#000"
                opacity="0.35"
                transform="translate(0 6)"
              />
              <g clip-path="url(#c11)">
                <rect fill={COLOR.panel} x="368" y="841" width="280" height="555" />
                <g clip-path="url(#c12)">
                  <PropertyPanelHeader
                    x={368}
                    y={841}
                    icon={{ type: "file", label: "PNG", labelColor: "#c3a9e8", stroke: "#665184" }}
                    title={"logo.png"}
                    subtitle={<PlainSubtitle>{"Image file"}</PlainSubtitle>}
                  />
                  <g transform="translate(0 -8)">
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="384"
                      y="928.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"FILE"}
                    </text>
                    <text fill={COLOR.muted} class="s" x="384" y="959.68" font-size="13">{"Format"}</text>
                    <text fill={COLOR.text} class="m" x="632" y="959.68" font-size="13" text-anchor="end">
                      {"PNG"}
                    </text>
                    <text fill={COLOR.muted} class="s" x="384" y="987.68" font-size="13">{"Size"}</text>
                    <text fill={COLOR.text} class="m" x="632" y="987.68" font-size="13" text-anchor="end">
                      {"84 KB"}
                    </text>
                    <rect fill={COLOR.divider} x="384" y="1005" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="384"
                      y="1028.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"LOCATION"}
                    </text>
                    <text fill={COLOR.soft} class="m" x="384" y="1051.32" font-size="12">
                      {"/Users/me/market-vlog/"}
                    </text>
                    <text fill={COLOR.soft} class="m" x="384" y="1069.32" font-size="12">
                      {"media/logo.png"}
                    </text>
                  </g>
                </g>
              </g>
              <rect
                stroke={COLOR.border}
                x="368.5"
                y="841.5"
                width="279"
                height="554"
                rx="10"
                fill="none"
                stroke-width="1"
              />
            </g>
          </g>
          <g id="row-subtitle-file">
            <text fill={COLOR.text} class="s" x="48" y="4968" font-size="18" font-weight="600">
              {"Subtitle file"}
            </text>
            <text fill={COLOR.muted} class="s" x="48" y="4992" font-size="12" font-weight="400">
              {".srt parsed into cues"}
            </text>
            <text fill={COLOR.soft} class="s" x="48" y="5016" font-size="12" font-weight="600">
              {"Default"}
            </text>
            <g transform="translate(-640 4195)">
              <rect
                x="687.5"
                y="840.5"
                width="281"
                height="556"
                rx="10"
                fill="#000"
                opacity="0.35"
                transform="translate(0 6)"
              />
              <g clip-path="url(#c13)">
                <rect fill={COLOR.panel} x="688" y="841" width="280" height="555" />
                <g clip-path="url(#c14)">
                  <PropertyPanelHeader
                    x={688}
                    y={841}
                    icon={{ type: "file", label: "SRT", labelColor: "#8b8b94", stroke: "#46464e" }}
                    title={"interview.srt"}
                    subtitle={<PlainSubtitle>{"Subtitles · 5 cues · 0:16"}</PlainSubtitle>}
                  />
                  <g transform="translate(0 -8)">
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="704"
                      y="928.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"CUES"}
                    </text>
                    <text fill={COLOR.dim} class="m" x="704" y="956.96" font-size="11">{"01"}</text>
                    <text fill={COLOR.muted} class="m" x="730" y="956.96" font-size="11">
                      {"0:00.5 → 0:03.2"}
                    </text>
                    <text fill={COLOR.text} class="s" x="730" y="976.68" font-size="13">
                      {"Welcome back to the channel"}
                    </text>
                    <rect fill={COLOR.divider} x="730" y="990" width="222" height="1" />
                    <text fill={COLOR.dim} class="m" x="704" y="1014.96" font-size="11">{"02"}</text>
                    <text fill={COLOR.muted} class="m" x="730" y="1014.96" font-size="11">
                      {"0:03.4 → 0:06.8"}
                    </text>
                    <text fill={COLOR.text} class="s" x="730" y="1034.68" font-size="13">
                      {"Today we're walking through"}
                    </text>
                    <rect fill={COLOR.divider} x="730" y="1048" width="222" height="1" />
                    <text fill={COLOR.dim} class="m" x="704" y="1072.96" font-size="11">{"03"}</text>
                    <text fill={COLOR.muted} class="m" x="730" y="1072.96" font-size="11">
                      {"0:07.0 → 0:09.6"}
                    </text>
                    <text fill={COLOR.text} class="s" x="730" y="1092.68" font-size="13">
                      {"the old town market"}
                    </text>
                    <rect fill={COLOR.divider} x="730" y="1106" width="222" height="1" />
                    <text fill={COLOR.dim} class="m" x="704" y="1130.96" font-size="11">{"04"}</text>
                    <text fill={COLOR.muted} class="m" x="730" y="1130.96" font-size="11">
                      {"0:10.1 → 0:13.0"}
                    </text>
                    <text fill={COLOR.text} class="s" x="730" y="1150.68" font-size="13">
                      {"Let's get started"}
                    </text>
                    <rect fill={COLOR.divider} x="730" y="1164" width="222" height="1" />
                    <text fill={COLOR.dim} class="m" x="704" y="1188.96" font-size="11">{"05"}</text>
                    <text fill={COLOR.muted} class="m" x="730" y="1188.96" font-size="11">
                      {"0:13.4 → 0:16.0"}
                    </text>
                    <text fill={COLOR.text} class="s" x="730" y="1208.68" font-size="13">
                      {"First stop: coffee"}
                    </text>
                    <text fill={COLOR.muted} class="s" x="704" y="1247.32" font-size="12">
                      {"Drag onto a text track to add one"}
                    </text>
                    <text fill={COLOR.muted} class="s" x="704" y="1264.32" font-size="12">
                      {"text clip per cue."}
                    </text>
                  </g>
                </g>
              </g>
              <rect
                stroke={COLOR.border}
                x="688.5"
                y="841.5"
                width="279"
                height="554"
                rx="10"
                fill="none"
                stroke-width="1"
              />
            </g>
          </g>
          <g id="row-nothing-selected">
            <text fill={COLOR.text} class="s" x="48" y="5652" font-size="18" font-weight="600">
              {"Nothing selected"}
            </text>
            <text fill={COLOR.muted} class="s" x="48" y="5676" font-size="12" font-weight="400">
              {"No timeline open"}
            </text>
            <text fill={COLOR.soft} class="s" x="48" y="5700" font-size="12" font-weight="600">
              {"Empty state"}
            </text>
            <g transform="translate(-960 4879)">
              <rect
                x="1007.5"
                y="840.5"
                width="281"
                height="556"
                rx="10"
                fill="#000"
                opacity="0.35"
                transform="translate(0 6)"
              />
              <g clip-path="url(#c15)">
                <rect fill={COLOR.panel} x="1008" y="841" width="280" height="555" />
                <g clip-path="url(#c16)">
                  <rect
                    stroke={COLOR.border}
                    x="1124.5"
                    y="1065"
                    width="47"
                    height="47"
                    rx="12"
                    fill="#131316"
                    stroke-width="1"
                  />
                  <path
                    stroke={COLOR.muted}
                    d="M1141.0 1079.5 v17 l4.5 -4 l2.8 6.2 l2.6 -1.2 l-2.8 -6 l6 -0.2Z"
                    fill="none"
                    stroke-width="1.5"
                    stroke-linejoin="round"
                  />
                  <text
                    fill={COLOR.text}
                    class="s"
                    x="1148"
                    y="1139.54"
                    font-size="14"
                    font-weight="600"
                    text-anchor="middle"
                  >
                    {"Nothing selected"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="1148" y="1162.82" font-size="12" text-anchor="middle">
                    {"Select a clip on the timeline or a"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="1148" y="1179.82" font-size="12" text-anchor="middle">
                    {"file in the Explorer."}
                  </text>
                </g>
              </g>
              <rect
                stroke={COLOR.border}
                x="1008.5"
                y="841.5"
                width="279"
                height="554"
                rx="10"
                fill="none"
                stroke-width="1"
              />
            </g>
          </g>
        </g>
      </g>
    </svg>
  );
}

/// One header on its own, in the top part of a panel frame, with a variant label above.
function HeaderDemo(props: { x: number; label: string; children: Markup }): Markup {
  const y = 248;
  return (
    <g>
      <text fill={COLOR.soft} class="s" x={props.x} y={y - 20} font-size="12" font-weight="600">
        {props.label}
      </text>
      <clipPath id={`header-demo-${props.x}`}>
        <rect x={props.x} y={y} width="280" height={PROPERTY_PANEL_HEADER_HEIGHT} rx="10" />
      </clipPath>
      <rect x={props.x} y={y} width="280" height={PROPERTY_PANEL_HEADER_HEIGHT} rx="10" fill={COLOR.panel} />
      <g clip-path={`url(#header-demo-${props.x})`}>{props.children}</g>
      <rect
        x={props.x + 0.5}
        y={y + 0.5}
        width="279"
        height={PROPERTY_PANEL_HEADER_HEIGHT - 1}
        rx="10"
        fill="none"
        stroke={COLOR.border}
      />
    </g>
  );
}

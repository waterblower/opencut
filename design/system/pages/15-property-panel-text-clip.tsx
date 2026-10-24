import { ClipSubtitle, PropertyPanelHeader } from "../components/property-panel-header.tsx";
import { COLOR } from "../theme.ts";
import { FontStyle } from "../components/fonts.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

export default function Page15PropertyPanelTextClipSvg(): Markup {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width="1936" height="2012" viewBox="0 0 1936 2012">
      <FontStyle />
      <defs>
        <clipPath id="c1">
          <rect x="48" y="198" width="280" height="637" rx="10" />
        </clipPath>
        <clipPath id="c2">
          <rect x="48" y="198" width="280" height="637" rx="0" />
        </clipPath>
        <pattern id="locked-panel-stripes" width="8" height="8" patternUnits="userSpaceOnUse">
          <path d="M-2 2 L2 -2 M0 8 L8 0 M6 10 L10 6" stroke="#676773" stroke-width="2" />
        </pattern>
        <clipPath id="section-clip-0">
          <rect x="40" y="262" width="296" height="110" rx="10" />
        </clipPath>
        <clipPath id="section-clip-2">
          <rect x="40" y="470" width="296" height="170" rx="10" />
        </clipPath>
        <clipPath id="section-clip-3">
          <rect x="40" y="287" width="296" height="353" rx="10" />
        </clipPath>
        <clipPath id="section-clip-5">
          <rect x="40" y="250" width="296" height="450" rx="10" />
        </clipPath>
        <clipPath id="section-clip-7">
          <rect x="40" y="640" width="296" height="82" rx="10" />
        </clipPath>
      </defs>
      <rect fill={COLOR.page} x="0" y="0" width="1936" height="2012" />
      <text fill={COLOR.text} class="s" x="48" y="60.64" font-size="24" font-weight="700">
        {"Property Panel · Text clip"}
      </text>
      <text fill={COLOR.muted} class="s" x="48" y="87.04" font-size="14">
        {"Every state of the text clip panel, side by side. Other panels are on the property panel page."}
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
      <g id="text-clip-states">
        <text fill={COLOR.text} class="s" x="48" y="180" font-size="18" font-weight="600">{"Text clip"}</text>
        <text fill={COLOR.muted} class="s" x="48" y="204" font-size="12" font-weight="400">
          {"Clip states change the whole panel. The section rows below show only the part that changes."}
        </text>
        <text fill={COLOR.soft} class="s" x="48" y="228" font-size="12" font-weight="600">{"Default"}</text>
        <g transform="translate(0 50)">
          <rect
            x="47.5"
            y="197.5"
            width="281"
            height="638"
            rx="10"
            fill="#000"
            opacity="0.35"
            transform="translate(0 6)"
          />
          <g clip-path="url(#c1)">
            <rect fill={COLOR.panel} x="48" y="198" width="280" height="637" />
            <g clip-path="url(#c2)">
              <PropertyPanelHeader
                x={48}
                y={198}
                icon={{ type: "track", kind: "T" }}
                title={"Let's get started"}
                subtitle={<ClipSubtitle clipType="Text clip" track="Text 1" kind="T" />}
              />
              <g transform="translate(0 -8)">
                <text
                  fill={COLOR.muted}
                  class="s"
                  x="64"
                  y="285.96"
                  font-size="11"
                  font-weight="600"
                  letter-spacing="0.6"
                >
                  {"CONTENT"}
                </text>
                <rect
                  fill={COLOR.surface}
                  stroke={COLOR.border}
                  x="64.5"
                  y="298.5"
                  width="247"
                  height="57"
                  rx="6"
                  stroke-width="1"
                />
                <text fill={COLOR.text} class="s" x="74" y="319.68" font-size="13">
                  {"Let's get started"}
                </text>
                <rect fill={COLOR.divider} x="64" y="372" width="248" height="1" />
                <text
                  fill={COLOR.muted}
                  class="s"
                  x="64"
                  y="395.96"
                  font-size="11"
                  font-weight="600"
                  letter-spacing="0.6"
                >
                  {"TIMING"}
                </text>
                <text fill={COLOR.muted} class="s" x="64" y="418.96" font-size="11">{"Start"}</text>
                <text fill={COLOR.muted} class="s" x="192" y="418.96" font-size="11">{"Duration"}</text>
                <rect x="64" y="424" width="120" height="30" rx="6" fill="#131316" />
                <text fill={COLOR.text} class="m" x="74" y="443.68" font-size="13">{"0:10.03"}</text>
                <text fill={COLOR.dim} class="m" x="174" y="442.96" font-size="11" text-anchor="end">
                  {"303 f"}
                </text>
                <rect
                  fill={COLOR.surface}
                  stroke={COLOR.border}
                  x="192.5"
                  y="424.5"
                  width="119"
                  height="29"
                  rx="6"
                  stroke-width="1"
                />
                <text fill={COLOR.text} class="m" x="202" y="443.68" font-size="13">{"2.90"}</text>
                <text fill={COLOR.muted} class="s" x="302" y="443.32" font-size="12" text-anchor="end">
                  {"s"}
                </text>
                <rect fill={COLOR.divider} x="64" y="470" width="248" height="1" />
                <g id="text-clip-style">
                  <title>{"Text style: font, font size, and font color"}</title>
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="493.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"STYLE"}
                  </text>
                  <text fill={COLOR.soft} class="s" x="64" y="517" font-size="12">{"Font"}</text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="64.5"
                    y="526.5"
                    width="247"
                    height="33"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="s" x="74" y="548" font-size="13">{"Sans"}</text>
                  <path
                    stroke={COLOR.muted}
                    d="M294 541 l4 4 l4 -4"
                    fill="none"
                    stroke-width="1.5"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                  <text fill={COLOR.soft} class="s" x="64" y="581" font-size="12">{"Font size"}</text>
                  <text fill={COLOR.soft} class="s" x="192" y="581" font-size="12">{"Font color"}</text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="64.5"
                    y="590.5"
                    width="119"
                    height="33"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="m" x="74" y="612" font-size="13">{"64"}</text>
                  <text fill={COLOR.muted} class="s" x="174" y="612" font-size="12" text-anchor="end">
                    {"px"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="192.5"
                    y="590.5"
                    width="119"
                    height="33"
                    rx="6"
                    stroke-width="1"
                  />
                  <rect
                    stroke={COLOR.border}
                    x="200.5"
                    y="599.5"
                    width="15"
                    height="15"
                    rx="4"
                    fill="#ffffff"
                    stroke-width="1"
                  />
                  <text fill={COLOR.muted} class="m" x="224" y="612" font-size="13">{"#"}</text>
                  <text fill={COLOR.text} class="m" x="234" y="612" font-size="13">{"FFFFFF"}</text>
                </g>
                <g id="text-clip-background-border">
                  <title>{"Background: color and opacity, removable."}</title>
                  <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="663.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"BACKGROUND"}
                  </text>
                  <text fill={COLOR.accent} class="s" x="312" y="664.32" font-size="12" text-anchor="end">
                    {"Add"}
                  </text>
                  <text fill={COLOR.dim} class="s" x="64" y="688.32" font-size="12">{"No background"}</text>
                </g>
                <g transform="translate(0 82)">
                  <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="663.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"POSITION"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="64.5"
                    y="676.5"
                    width="119"
                    height="29"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.muted} class="s" x="74" y="695.32" font-size="12" font-weight="600">
                    {"X"}
                  </text>
                  <text fill={COLOR.text} class="m" x="89.68" y="695.68" font-size="13">{"0.50"}</text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="192.5"
                    y="676.5"
                    width="119"
                    height="29"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.muted} class="s" x="202" y="695.32" font-size="12" font-weight="600">
                    {"Y"}
                  </text>
                  <text fill={COLOR.text} class="m" x="217.68" y="695.68" font-size="13">{"0.85"}</text>
                </g>
              </g>
            </g>
          </g>
          <rect
            stroke={COLOR.border}
            x="48.5"
            y="198.5"
            width="279"
            height="636"
            rx="10"
            fill="none"
            stroke-width="1"
          />
        </g>
        <text fill={COLOR.soft} class="s" x="368" y="228" font-size="12" font-weight="600">
          {"Locked track · read-only"}
        </text>
        <g transform="translate(320 50)">
          <rect
            x="47.5"
            y="197.5"
            width="281"
            height="638"
            rx="10"
            fill="#000"
            opacity="0.35"
            transform="translate(0 6)"
          />
          <g clip-path="url(#c1)">
            <rect fill={COLOR.panel} x="48" y="198" width="280" height="637" />
            <g clip-path="url(#c2)">
              <PropertyPanelHeader
                x={48}
                y={198}
                icon={{ type: "locked" }}
                title={"Let's get started"}
                subtitle={<ClipSubtitle clipType="Text clip" track="Text 1" kind="T" locked />}
                locked
              />
              <rect x="48" y="198" width="280" height="5" fill="url(#locked-panel-stripes)" />
              <rect x="48" y="198" width="5" height="637" fill="url(#locked-panel-stripes)" />
              <g transform="translate(0 -8)">
                <text
                  fill={COLOR.muted}
                  class="s"
                  x="64"
                  y="285.96"
                  font-size="11"
                  font-weight="600"
                  letter-spacing="0.6"
                >
                  {"CONTENT"}
                </text>
                <rect x="64" y="298" width="248" height="58" rx="6" fill="#19191d" />
                <text fill={COLOR.text} class="s" x="74" y="319.68" font-size="13">
                  {"Let's get started"}
                </text>
                <rect fill={COLOR.divider} x="64" y="372" width="248" height="1" />
                <text
                  fill={COLOR.muted}
                  class="s"
                  x="64"
                  y="395.96"
                  font-size="11"
                  font-weight="600"
                  letter-spacing="0.6"
                >
                  {"TIMING"}
                </text>
                <text fill={COLOR.muted} class="s" x="64" y="418.96" font-size="11">{"Start"}</text>
                <text fill={COLOR.muted} class="s" x="192" y="418.96" font-size="11">{"Duration"}</text>
                <rect x="64" y="424" width="120" height="30" rx="6" fill="#19191d" />
                <text fill={COLOR.text} class="m" x="74" y="443.68" font-size="13">{"0:10.03"}</text>
                <text fill={COLOR.dim} class="m" x="174" y="442.96" font-size="11" text-anchor="end">
                  {"303 f"}
                </text>
                <rect x="192" y="424" width="120" height="30" rx="6" fill="#19191d" />
                <text fill={COLOR.text} class="m" x="202" y="443.68" font-size="13">{"2.90"}</text>
                <text fill={COLOR.muted} class="s" x="302" y="443.32" font-size="12" text-anchor="end">
                  {"s"}
                </text>
                <rect fill={COLOR.divider} x="64" y="470" width="248" height="1" />
                <g id="text-clip-style-locked">
                  <title>{"Text style: font, font size, and font color"}</title>
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="493.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"STYLE"}
                  </text>
                  <text fill={COLOR.soft} class="s" x="64" y="517" font-size="12">{"Font"}</text>
                  <rect x="64.5" y="526.5" width="247" height="33" rx="6" fill="#131316" stroke="#222226" />
                  <text fill={COLOR.dim} class="s" x="74" y="548" font-size="13">{"Sans"}</text>
                  <path
                    d="M294 541 l4 4 l4 -4"
                    fill="none"
                    stroke="#46464e"
                    stroke-width="1.5"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                  <text fill={COLOR.soft} class="s" x="64" y="581" font-size="12">{"Font size"}</text>
                  <text fill={COLOR.soft} class="s" x="192" y="581" font-size="12">{"Font color"}</text>
                  <rect x="64" y="590" width="120" height="34" rx="6" fill="#19191d" />
                  <text fill={COLOR.text} class="m" x="74" y="612" font-size="13">{"64"}</text>
                  <text fill={COLOR.muted} class="s" x="174" y="612" font-size="12" text-anchor="end">
                    {"px"}
                  </text>
                  <rect x="192" y="590" width="120" height="34" rx="6" fill="#19191d" />
                  <rect
                    stroke={COLOR.border}
                    x="200.5"
                    y="599.5"
                    width="15"
                    height="15"
                    rx="4"
                    fill="#ffffff"
                    stroke-width="1"
                  />
                  <text fill={COLOR.muted} class="m" x="224" y="612" font-size="13">{"#"}</text>
                  <text fill={COLOR.text} class="m" x="234" y="612" font-size="13">{"FFFFFF"}</text>
                </g>
                <g id="text-clip-background-border">
                  <title>{"Background: color and opacity, removable."}</title>
                  <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="663.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"BACKGROUND"}
                  </text>
                  <text fill={COLOR.dim} class="s" x="64" y="688.32" font-size="12">{"No background"}</text>
                </g>
                <g transform="translate(0 82)">
                  <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="663.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"POSITION"}
                  </text>
                  <rect x="64" y="676" width="120" height="30" rx="6" fill="#19191d" />
                  <text fill={COLOR.muted} class="s" x="74" y="695.32" font-size="12" font-weight="600">
                    {"X"}
                  </text>
                  <text fill={COLOR.text} class="m" x="89.68" y="695.68" font-size="13">{"0.50"}</text>
                  <rect x="192" y="676" width="120" height="30" rx="6" fill="#19191d" />
                  <text fill={COLOR.muted} class="s" x="202" y="695.32" font-size="12" font-weight="600">
                    {"Y"}
                  </text>
                  <text fill={COLOR.text} class="m" x="217.68" y="695.68" font-size="13">{"0.85"}</text>
                </g>
              </g>
            </g>
          </g>
          <rect
            x="48.5"
            y="198.5"
            width="279"
            height="636"
            rx="10"
            fill="none"
            stroke="#62626e"
            stroke-width="1"
          />
        </g>
      </g>
      <g transform="translate(0 -70)">
        <g id="text-clip-section-content">
          <text fill={COLOR.text} class="s" x="48" y="1020" font-size="18" font-weight="600">
            {"Content"}
          </text>
          <text fill={COLOR.muted} class="s" x="48" y="1044" font-size="12">
            {"Click to edit; the caret and accent border show focus."}
          </text>
          <text fill={COLOR.soft} class="s" x="48" y="1068" font-size="12" font-weight="600">{"Idle"}</text>
          <g transform="translate(0 826)">
            <g clip-path="url(#section-clip-0)">
              <rect
                x="47.5"
                y="197.5"
                width="281"
                height="638"
                rx="10"
                fill="#000"
                opacity="0.35"
                transform="translate(0 6)"
              />
              <g clip-path="url(#c1)">
                <rect fill={COLOR.panel} x="48" y="198" width="280" height="637" />
                <g clip-path="url(#c2)">
                  <PropertyPanelHeader
                    x={48}
                    y={198}
                    icon={{ type: "track", kind: "T" }}
                    title={"Let's get started"}
                    subtitle={<ClipSubtitle clipType="Text clip" track="Text 1" kind="T" />}
                  />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="285.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"CONTENT"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="64.5"
                    y="298.5"
                    width="247"
                    height="57"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="s" x="74" y="319.68" font-size="13">
                    {"Let's get started"}
                  </text>
                  <rect fill={COLOR.divider} x="64" y="372" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="395.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"TIMING"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="64" y="418.96" font-size="11">{"Start"}</text>
                  <text fill={COLOR.muted} class="s" x="192" y="418.96" font-size="11">{"Duration"}</text>
                  <rect x="64" y="424" width="120" height="30" rx="6" fill="#131316" />
                  <text fill={COLOR.text} class="m" x="74" y="443.68" font-size="13">{"0:10.03"}</text>
                  <text fill={COLOR.dim} class="m" x="174" y="442.96" font-size="11" text-anchor="end">
                    {"303 f"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="192.5"
                    y="424.5"
                    width="119"
                    height="29"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="m" x="202" y="443.68" font-size="13">{"2.90"}</text>
                  <text fill={COLOR.muted} class="s" x="302" y="443.32" font-size="12" text-anchor="end">
                    {"s"}
                  </text>
                  <rect fill={COLOR.divider} x="64" y="470" width="248" height="1" />
                  <g id="text-clip-style">
                    <title>{"Text style: font, font size, and font color"}</title>
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="493.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"STYLE"}
                    </text>
                    <text fill={COLOR.soft} class="s" x="64" y="517" font-size="12">{"Font"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="526.5"
                      width="247"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="s" x="74" y="548" font-size="13">{"Sans"}</text>
                    <path
                      stroke={COLOR.muted}
                      d="M294 541 l4 4 l4 -4"
                      fill="none"
                      stroke-width="1.5"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                    <text fill={COLOR.soft} class="s" x="64" y="581" font-size="12">{"Font size"}</text>
                    <text fill={COLOR.soft} class="s" x="192" y="581" font-size="12">{"Font color"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="74" y="612" font-size="13">{"64"}</text>
                    <text fill={COLOR.muted} class="s" x="174" y="612" font-size="12" text-anchor="end">
                      {"px"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      stroke={COLOR.border}
                      x="200.5"
                      y="599.5"
                      width="15"
                      height="15"
                      rx="4"
                      fill="#ffffff"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="224" y="612" font-size="13">{"#"}</text>
                    <text fill={COLOR.text} class="m" x="234" y="612" font-size="13">{"FFFFFF"}</text>
                  </g>
                  <g id="text-clip-background-border">
                    <title>{"Background: color and opacity, removable."}</title>
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"BACKGROUND"}
                    </text>
                    <text fill={COLOR.accent} class="s" x="312" y="664.32" font-size="12" text-anchor="end">
                      {"Remove"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="159"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      stroke={COLOR.border}
                      x="72.5"
                      y="683.5"
                      width="15"
                      height="15"
                      rx="4"
                      fill="#000000"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="96" y="695.68" font-size="13">{"#"}</text>
                    <text fill={COLOR.text} class="m" x="106" y="695.68" font-size="13">{"000000"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="232.5"
                      y="676.5"
                      width="79"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="242" y="695.68" font-size="13">{"50"}</text>
                    <text fill={COLOR.muted} class="s" x="302" y="695.32" font-size="12" text-anchor="end">
                      {"%"}
                    </text>
                  </g>
                  <g transform="translate(0 82)">
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"POSITION"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="74" y="695.32" font-size="12" font-weight="600">
                      {"X"}
                    </text>
                    <text fill={COLOR.text} class="m" x="89.68" y="695.68" font-size="13">{"0.50"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="202" y="695.32" font-size="12" font-weight="600">
                      {"Y"}
                    </text>
                    <text fill={COLOR.text} class="m" x="217.68" y="695.68" font-size="13">{"0.85"}</text>
                  </g>
                </g>
              </g>
              <rect
                stroke={COLOR.border}
                x="48.5"
                y="198.5"
                width="279"
                height="636"
                rx="10"
                fill="none"
                stroke-width="1"
              />
            </g>
            <rect
              stroke={COLOR.border}
              x="48.5"
              y="262.5"
              width="279"
              height="109"
              rx="10"
              fill="none"
              stroke-width="1"
            />
          </g>
          <text fill={COLOR.soft} class="s" x="368" y="1068" font-size="12" font-weight="600">
            {"Editing"}
          </text>
          <g transform="translate(320 826)">
            <g clip-path="url(#section-clip-0)">
              <rect
                x="47.5"
                y="197.5"
                width="281"
                height="638"
                rx="10"
                fill="#000"
                opacity="0.35"
                transform="translate(0 6)"
              />
              <g clip-path="url(#c1)">
                <rect fill={COLOR.panel} x="48" y="198" width="280" height="637" />
                <g clip-path="url(#c2)">
                  <PropertyPanelHeader
                    x={48}
                    y={198}
                    icon={{ type: "track", kind: "T" }}
                    title={"Let's get started"}
                    subtitle={<ClipSubtitle clipType="Text clip" track="Text 1" kind="T" />}
                  />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="285.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"CONTENT"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.accent}
                    x="64.5"
                    y="298.5"
                    width="247"
                    height="57"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="s" x="74" y="319.68" font-size="13">
                    {"Let's get started"}
                  </text>
                  <rect fill={COLOR.accent} x="173.02" y="307" width="1.5" height="16" />
                  <rect fill={COLOR.divider} x="64" y="372" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="395.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"TIMING"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="64" y="418.96" font-size="11">{"Start"}</text>
                  <text fill={COLOR.muted} class="s" x="192" y="418.96" font-size="11">{"Duration"}</text>
                  <rect x="64" y="424" width="120" height="30" rx="6" fill="#131316" />
                  <text fill={COLOR.text} class="m" x="74" y="443.68" font-size="13">{"0:10.03"}</text>
                  <text fill={COLOR.dim} class="m" x="174" y="442.96" font-size="11" text-anchor="end">
                    {"303 f"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="192.5"
                    y="424.5"
                    width="119"
                    height="29"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="m" x="202" y="443.68" font-size="13">{"2.90"}</text>
                  <text fill={COLOR.muted} class="s" x="302" y="443.32" font-size="12" text-anchor="end">
                    {"s"}
                  </text>
                  <rect fill={COLOR.divider} x="64" y="470" width="248" height="1" />
                  <g id="text-clip-style">
                    <title>{"Text style: font, font size, and font color"}</title>
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="493.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"STYLE"}
                    </text>
                    <text fill={COLOR.soft} class="s" x="64" y="517" font-size="12">{"Font"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="526.5"
                      width="247"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="s" x="74" y="548" font-size="13">{"Sans"}</text>
                    <path
                      stroke={COLOR.muted}
                      d="M294 541 l4 4 l4 -4"
                      fill="none"
                      stroke-width="1.5"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                    <text fill={COLOR.soft} class="s" x="64" y="581" font-size="12">{"Font size"}</text>
                    <text fill={COLOR.soft} class="s" x="192" y="581" font-size="12">{"Font color"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="74" y="612" font-size="13">{"64"}</text>
                    <text fill={COLOR.muted} class="s" x="174" y="612" font-size="12" text-anchor="end">
                      {"px"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      stroke={COLOR.border}
                      x="200.5"
                      y="599.5"
                      width="15"
                      height="15"
                      rx="4"
                      fill="#ffffff"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="224" y="612" font-size="13">{"#"}</text>
                    <text fill={COLOR.text} class="m" x="234" y="612" font-size="13">{"FFFFFF"}</text>
                  </g>
                  <g id="text-clip-background-border">
                    <title>{"Background: color and opacity, removable."}</title>
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"BACKGROUND"}
                    </text>
                    <text fill={COLOR.accent} class="s" x="312" y="664.32" font-size="12" text-anchor="end">
                      {"Remove"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="159"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      stroke={COLOR.border}
                      x="72.5"
                      y="683.5"
                      width="15"
                      height="15"
                      rx="4"
                      fill="#000000"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="96" y="695.68" font-size="13">{"#"}</text>
                    <text fill={COLOR.text} class="m" x="106" y="695.68" font-size="13">{"000000"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="232.5"
                      y="676.5"
                      width="79"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="242" y="695.68" font-size="13">{"50"}</text>
                    <text fill={COLOR.muted} class="s" x="302" y="695.32" font-size="12" text-anchor="end">
                      {"%"}
                    </text>
                  </g>
                  <g transform="translate(0 82)">
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"POSITION"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="74" y="695.32" font-size="12" font-weight="600">
                      {"X"}
                    </text>
                    <text fill={COLOR.text} class="m" x="89.68" y="695.68" font-size="13">{"0.50"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="202" y="695.32" font-size="12" font-weight="600">
                      {"Y"}
                    </text>
                    <text fill={COLOR.text} class="m" x="217.68" y="695.68" font-size="13">{"0.85"}</text>
                  </g>
                </g>
              </g>
              <rect
                stroke={COLOR.border}
                x="48.5"
                y="198.5"
                width="279"
                height="636"
                rx="10"
                fill="none"
                stroke-width="1"
              />
            </g>
            <rect
              stroke={COLOR.border}
              x="48.5"
              y="262.5"
              width="279"
              height="109"
              rx="10"
              fill="none"
              stroke-width="1"
            />
          </g>
        </g>
        <g id="text-clip-section-style">
          <text fill={COLOR.text} class="s" x="48" y="1286" font-size="18" font-weight="600">{"Style"}</text>
          <text fill={COLOR.muted} class="s" x="48" y="1310" font-size="12">
            {"Font, size and font color, with their popovers."}
          </text>
          <text fill={COLOR.soft} class="s" x="48" y="1334" font-size="12" font-weight="600">
            {"Hex input · focused"}
          </text>
          <g transform="translate(0 884)">
            <g clip-path="url(#section-clip-2)">
              <rect
                x="47.5"
                y="197.5"
                width="281"
                height="638"
                rx="10"
                fill="#000"
                opacity="0.35"
                transform="translate(0 6)"
              />
              <g clip-path="url(#c1)">
                <rect fill={COLOR.panel} x="48" y="198" width="280" height="637" />
                <g clip-path="url(#c2)">
                  <PropertyPanelHeader
                    x={48}
                    y={198}
                    icon={{ type: "track", kind: "T" }}
                    title={"Let's get started"}
                    subtitle={<ClipSubtitle clipType="Text clip" track="Text 1" kind="T" />}
                  />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="285.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"CONTENT"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="64.5"
                    y="298.5"
                    width="247"
                    height="57"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="s" x="74" y="319.68" font-size="13">
                    {"Let's get started"}
                  </text>
                  <rect fill={COLOR.divider} x="64" y="372" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="395.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"TIMING"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="64" y="418.96" font-size="11">{"Start"}</text>
                  <text fill={COLOR.muted} class="s" x="192" y="418.96" font-size="11">{"Duration"}</text>
                  <rect x="64" y="424" width="120" height="30" rx="6" fill="#131316" />
                  <text fill={COLOR.text} class="m" x="74" y="443.68" font-size="13">{"0:10.03"}</text>
                  <text fill={COLOR.dim} class="m" x="174" y="442.96" font-size="11" text-anchor="end">
                    {"303 f"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="192.5"
                    y="424.5"
                    width="119"
                    height="29"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="m" x="202" y="443.68" font-size="13">{"2.90"}</text>
                  <text fill={COLOR.muted} class="s" x="302" y="443.32" font-size="12" text-anchor="end">
                    {"s"}
                  </text>
                  <rect fill={COLOR.divider} x="64" y="470" width="248" height="1" />
                  <g id="text-clip-style-hex">
                    <title>{"Text style: font, font size, and font color"}</title>
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="493.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"STYLE"}
                    </text>
                    <text fill={COLOR.soft} class="s" x="64" y="517" font-size="12">{"Font"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="526.5"
                      width="247"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="s" x="74" y="548" font-size="13">{"Sans"}</text>
                    <path
                      stroke={COLOR.muted}
                      d="M294 541 l4 4 l4 -4"
                      fill="none"
                      stroke-width="1.5"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                    <text fill={COLOR.soft} class="s" x="64" y="581" font-size="12">{"Font size"}</text>
                    <text fill={COLOR.soft} class="s" x="192" y="581" font-size="12">{"Font color"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="74" y="612" font-size="13">{"64"}</text>
                    <text fill={COLOR.muted} class="s" x="174" y="612" font-size="12" text-anchor="end">
                      {"px"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.accent}
                      x="192.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      stroke={COLOR.border}
                      x="200.5"
                      y="599.5"
                      width="15"
                      height="15"
                      rx="4"
                      fill="#ffffff"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="224" y="612" font-size="13">{"#"}</text>
                    <rect x="233" y="597" width="47" height="19" rx="2" fill="#3a5a8c" />
                    <text fill={COLOR.text} class="m" x="234" y="612" font-size="13">{"FFFFFF"}</text>
                  </g>
                  <g id="text-clip-background-border">
                    <title>{"Background: color and opacity, removable."}</title>
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"BACKGROUND"}
                    </text>
                    <text fill={COLOR.accent} class="s" x="312" y="664.32" font-size="12" text-anchor="end">
                      {"Remove"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="159"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      stroke={COLOR.border}
                      x="72.5"
                      y="683.5"
                      width="15"
                      height="15"
                      rx="4"
                      fill="#000000"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="96" y="695.68" font-size="13">{"#"}</text>
                    <text fill={COLOR.text} class="m" x="106" y="695.68" font-size="13">{"000000"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="232.5"
                      y="676.5"
                      width="79"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="242" y="695.68" font-size="13">{"50"}</text>
                    <text fill={COLOR.muted} class="s" x="302" y="695.32" font-size="12" text-anchor="end">
                      {"%"}
                    </text>
                  </g>
                  <g transform="translate(0 82)">
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"POSITION"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="74" y="695.32" font-size="12" font-weight="600">
                      {"X"}
                    </text>
                    <text fill={COLOR.text} class="m" x="89.68" y="695.68" font-size="13">{"0.50"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="202" y="695.32" font-size="12" font-weight="600">
                      {"Y"}
                    </text>
                    <text fill={COLOR.text} class="m" x="217.68" y="695.68" font-size="13">{"0.85"}</text>
                  </g>
                </g>
              </g>
              <rect
                stroke={COLOR.border}
                x="48.5"
                y="198.5"
                width="279"
                height="636"
                rx="10"
                fill="none"
                stroke-width="1"
              />
            </g>
            <rect
              stroke={COLOR.border}
              x="48.5"
              y="470.5"
              width="279"
              height="169"
              rx="10"
              fill="none"
              stroke-width="1"
            />
          </g>
          <text class="s" x="48" y="1548" font-size="12" fill="#8a8a93">
            {"Fixed # prefix · edit six hex digits only."}
          </text>
          <text fill={COLOR.soft} class="s" x="368" y="1334" font-size="12" font-weight="600">
            {"Color palette · open"}
          </text>
          <g transform="translate(320 1067)">
            <g clip-path="url(#section-clip-3)">
              <rect
                x="47.5"
                y="197.5"
                width="281"
                height="638"
                rx="10"
                fill="#000"
                opacity="0.35"
                transform="translate(0 6)"
              />
              <g clip-path="url(#c1)">
                <rect fill={COLOR.panel} x="48" y="198" width="280" height="637" />
                <g clip-path="url(#c2)">
                  <PropertyPanelHeader
                    x={48}
                    y={198}
                    icon={{ type: "track", kind: "T" }}
                    title={"Let's get started"}
                    subtitle={<ClipSubtitle clipType="Text clip" track="Text 1" kind="T" />}
                  />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="285.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"CONTENT"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="64.5"
                    y="298.5"
                    width="247"
                    height="57"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="s" x="74" y="319.68" font-size="13">
                    {"Let's get started"}
                  </text>
                  <rect fill={COLOR.divider} x="64" y="372" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="395.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"TIMING"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="64" y="418.96" font-size="11">{"Start"}</text>
                  <text fill={COLOR.muted} class="s" x="192" y="418.96" font-size="11">{"Duration"}</text>
                  <rect x="64" y="424" width="120" height="30" rx="6" fill="#131316" />
                  <text fill={COLOR.text} class="m" x="74" y="443.68" font-size="13">{"0:10.03"}</text>
                  <text fill={COLOR.dim} class="m" x="174" y="442.96" font-size="11" text-anchor="end">
                    {"303 f"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="192.5"
                    y="424.5"
                    width="119"
                    height="29"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="m" x="202" y="443.68" font-size="13">{"2.90"}</text>
                  <text fill={COLOR.muted} class="s" x="302" y="443.32" font-size="12" text-anchor="end">
                    {"s"}
                  </text>
                  <rect fill={COLOR.divider} x="64" y="470" width="248" height="1" />
                  <g id="text-clip-style-palette">
                    <title>{"Text style: font, font size, and font color"}</title>
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="493.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"STYLE"}
                    </text>
                    <text fill={COLOR.soft} class="s" x="64" y="517" font-size="12">{"Font"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="526.5"
                      width="247"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="s" x="74" y="548" font-size="13">{"Sans"}</text>
                    <path
                      stroke={COLOR.muted}
                      d="M294 541 l4 4 l4 -4"
                      fill="none"
                      stroke-width="1.5"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                    <text fill={COLOR.soft} class="s" x="64" y="581" font-size="12">{"Font size"}</text>
                    <text fill={COLOR.soft} class="s" x="192" y="581" font-size="12">{"Font color"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="74" y="612" font-size="13">{"64"}</text>
                    <text fill={COLOR.muted} class="s" x="174" y="612" font-size="12" text-anchor="end">
                      {"px"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      fill={COLOR.accent}
                      stroke={COLOR.accent}
                      x="200.5"
                      y="599.5"
                      width="15"
                      height="15"
                      rx="4"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="224" y="612" font-size="13">{"#"}</text>
                    <text fill={COLOR.text} class="m" x="234" y="612" font-size="13">{"F0B75E"}</text>
                  </g>
                  <g id="text-clip-background-border">
                    <title>{"Background: color and opacity, removable."}</title>
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"BACKGROUND"}
                    </text>
                    <text fill={COLOR.accent} class="s" x="312" y="664.32" font-size="12" text-anchor="end">
                      {"Remove"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="159"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      stroke={COLOR.border}
                      x="72.5"
                      y="683.5"
                      width="15"
                      height="15"
                      rx="4"
                      fill="#000000"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="96" y="695.68" font-size="13">{"#"}</text>
                    <text fill={COLOR.text} class="m" x="106" y="695.68" font-size="13">{"000000"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="232.5"
                      y="676.5"
                      width="79"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="242" y="695.68" font-size="13">{"50"}</text>
                    <text fill={COLOR.muted} class="s" x="302" y="695.32" font-size="12" text-anchor="end">
                      {"%"}
                    </text>
                  </g>
                  <g transform="translate(0 82)">
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"POSITION"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="74" y="695.32" font-size="12" font-weight="600">
                      {"X"}
                    </text>
                    <text fill={COLOR.text} class="m" x="89.68" y="695.68" font-size="13">{"0.50"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="202" y="695.32" font-size="12" font-weight="600">
                      {"Y"}
                    </text>
                    <text fill={COLOR.text} class="m" x="217.68" y="695.68" font-size="13">{"0.85"}</text>
                  </g>
                </g>
              </g>
              <rect
                stroke={COLOR.border}
                x="48.5"
                y="198.5"
                width="279"
                height="636"
                rx="10"
                fill="none"
                stroke-width="1"
              />
              <g id="font-color-palette">
                <title>
                  {"B2: 24 preset colors. Choose a swatch to apply and keep exploring. Close only with × or an outside click. Custom hex values remain available in the field."}
                </title>
                <rect x="62" y="292" width="252" height="278" rx="9" fill="#000" opacity=".35" />
                <rect
                  fill={COLOR.hover}
                  stroke={COLOR.rail}
                  x="64.5"
                  y="288.5"
                  width="247"
                  height="277"
                  rx="8"
                />
                <text fill={COLOR.text} class="s" x="80" y="315" font-size="13" font-weight="600">
                  {"Color families"}
                </text>
                <path d="M288 303l8 8m0-8l-8 8" stroke="#8a8a93" stroke-width="1.5" />
                <rect stroke={COLOR.rail} x="80" y="334" width="30" height="30" rx="4" fill="#FFFFFF" />
                <rect stroke={COLOR.rail} x="116" y="334" width="30" height="30" rx="4" fill="#CCCCCC" />
                <rect stroke={COLOR.rail} x="152" y="334" width="30" height="30" rx="4" fill="#999999" />
                <rect stroke={COLOR.rail} x="188" y="334" width="30" height="30" rx="4" fill="#666666" />
                <rect stroke={COLOR.rail} x="224" y="334" width="30" height="30" rx="4" fill="#333333" />
                <rect stroke={COLOR.rail} x="260" y="334" width="30" height="30" rx="4" fill="#000000" />
                <rect fill={COLOR.error} stroke={COLOR.rail} x="80" y="372" width="30" height="30" rx="4" />
                <rect fill={COLOR.accent} stroke={COLOR.text} x="116" y="372" width="30" height="30" rx="4" />
                <path
                  d="M124 387 l5 5 9-11"
                  fill="none"
                  stroke="#17120a"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
                <rect stroke={COLOR.rail} x="152" y="372" width="30" height="30" rx="4" fill="#FFE58A" />
                <rect stroke={COLOR.rail} x="188" y="372" width="30" height="30" rx="4" fill="#9BDCA4" />
                <rect stroke={COLOR.rail} x="224" y="372" width="30" height="30" rx="4" fill="#91B8DF" />
                <rect stroke={COLOR.rail} x="260" y="372" width="30" height="30" rx="4" fill="#B69BDC" />
                <rect stroke={COLOR.rail} x="80" y="410" width="30" height="30" rx="4" fill="#E84545" />
                <rect stroke={COLOR.rail} x="116" y="410" width="30" height="30" rx="4" fill="#E69324" />
                <rect stroke={COLOR.rail} x="152" y="410" width="30" height="30" rx="4" fill="#E5C638" />
                <rect stroke={COLOR.rail} x="188" y="410" width="30" height="30" rx="4" fill="#41AC62" />
                <rect stroke={COLOR.rail} x="224" y="410" width="30" height="30" rx="4" fill="#438FD4" />
                <rect stroke={COLOR.rail} x="260" y="410" width="30" height="30" rx="4" fill="#9164CE" />
                <rect stroke={COLOR.rail} x="80" y="448" width="30" height="30" rx="4" fill="#8E2828" />
                <rect stroke={COLOR.rail} x="116" y="448" width="30" height="30" rx="4" fill="#915617" />
                <rect stroke={COLOR.rail} x="152" y="448" width="30" height="30" rx="4" fill="#8A7621" />
                <rect stroke={COLOR.rail} x="188" y="448" width="30" height="30" rx="4" fill="#256B3A" />
                <rect stroke={COLOR.rail} x="224" y="448" width="30" height="30" rx="4" fill="#285886" />
                <rect stroke={COLOR.rail} x="260" y="448" width="30" height="30" rx="4" fill="#58377F" />
                <text class="s" x="80" y="511" font-size="11" fill="#8a8a93">
                  {"Neutrals, then light, vivid and dark."}
                </text>
                <text class="s" x="80" y="539" font-size="11" fill="#8a8a93">
                  {"Custom color? Edit the hex value."}
                </text>
                <path fill={COLOR.hover} d="M201 564H215L208 573Z" />
                <path stroke={COLOR.rail} d="M202 566l6 7 6-7" fill="none" />
              </g>
            </g>
            <rect
              stroke={COLOR.border}
              x="48.5"
              y="287.5"
              width="279"
              height="352"
              rx="10"
              fill="none"
              stroke-width="1"
            />
          </g>
          <text class="s" x="368" y="1731" font-size="12" fill="#8a8a93">
            {"Select to explore · × / outside closes."}
          </text>
          <text fill={COLOR.soft} class="s" x="688" y="1334" font-size="12" font-weight="600">
            {"Color palette · swatch hover"}
          </text>
          <g transform="translate(640 1067)">
            <g clip-path="url(#section-clip-3)">
              <rect
                x="47.5"
                y="197.5"
                width="281"
                height="638"
                rx="10"
                fill="#000"
                opacity="0.35"
                transform="translate(0 6)"
              />
              <g clip-path="url(#c1)">
                <rect fill={COLOR.panel} x="48" y="198" width="280" height="637" />
                <g clip-path="url(#c2)">
                  <PropertyPanelHeader
                    x={48}
                    y={198}
                    icon={{ type: "track", kind: "T" }}
                    title={"Let's get started"}
                    subtitle={<ClipSubtitle clipType="Text clip" track="Text 1" kind="T" />}
                  />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="285.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"CONTENT"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="64.5"
                    y="298.5"
                    width="247"
                    height="57"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="s" x="74" y="319.68" font-size="13">
                    {"Let's get started"}
                  </text>
                  <rect fill={COLOR.divider} x="64" y="372" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="395.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"TIMING"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="64" y="418.96" font-size="11">{"Start"}</text>
                  <text fill={COLOR.muted} class="s" x="192" y="418.96" font-size="11">{"Duration"}</text>
                  <rect x="64" y="424" width="120" height="30" rx="6" fill="#131316" />
                  <text fill={COLOR.text} class="m" x="74" y="443.68" font-size="13">{"0:10.03"}</text>
                  <text fill={COLOR.dim} class="m" x="174" y="442.96" font-size="11" text-anchor="end">
                    {"303 f"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="192.5"
                    y="424.5"
                    width="119"
                    height="29"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="m" x="202" y="443.68" font-size="13">{"2.90"}</text>
                  <text fill={COLOR.muted} class="s" x="302" y="443.32" font-size="12" text-anchor="end">
                    {"s"}
                  </text>
                  <rect fill={COLOR.divider} x="64" y="470" width="248" height="1" />
                  <g id="text-clip-style-palette-hover">
                    <title>{"Text style: font, font size, and font color"}</title>
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="493.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"STYLE"}
                    </text>
                    <text fill={COLOR.soft} class="s" x="64" y="517" font-size="12">{"Font"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="526.5"
                      width="247"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="s" x="74" y="548" font-size="13">{"Sans"}</text>
                    <path
                      stroke={COLOR.muted}
                      d="M294 541 l4 4 l4 -4"
                      fill="none"
                      stroke-width="1.5"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                    <text fill={COLOR.soft} class="s" x="64" y="581" font-size="12">{"Font size"}</text>
                    <text fill={COLOR.soft} class="s" x="192" y="581" font-size="12">{"Font color"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="74" y="612" font-size="13">{"64"}</text>
                    <text fill={COLOR.muted} class="s" x="174" y="612" font-size="12" text-anchor="end">
                      {"px"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      fill={COLOR.accent}
                      stroke={COLOR.accent}
                      x="200.5"
                      y="599.5"
                      width="15"
                      height="15"
                      rx="4"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="224" y="612" font-size="13">{"#"}</text>
                    <text fill={COLOR.text} class="m" x="234" y="612" font-size="13">{"F0B75E"}</text>
                  </g>
                  <g id="text-clip-background-border">
                    <title>{"Background: color and opacity, removable."}</title>
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"BACKGROUND"}
                    </text>
                    <text fill={COLOR.accent} class="s" x="312" y="664.32" font-size="12" text-anchor="end">
                      {"Remove"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="159"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      stroke={COLOR.border}
                      x="72.5"
                      y="683.5"
                      width="15"
                      height="15"
                      rx="4"
                      fill="#000000"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="96" y="695.68" font-size="13">{"#"}</text>
                    <text fill={COLOR.text} class="m" x="106" y="695.68" font-size="13">{"000000"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="232.5"
                      y="676.5"
                      width="79"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="242" y="695.68" font-size="13">{"50"}</text>
                    <text fill={COLOR.muted} class="s" x="302" y="695.32" font-size="12" text-anchor="end">
                      {"%"}
                    </text>
                  </g>
                  <g transform="translate(0 82)">
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"POSITION"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="74" y="695.32" font-size="12" font-weight="600">
                      {"X"}
                    </text>
                    <text fill={COLOR.text} class="m" x="89.68" y="695.68" font-size="13">{"0.50"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="202" y="695.32" font-size="12" font-weight="600">
                      {"Y"}
                    </text>
                    <text fill={COLOR.text} class="m" x="217.68" y="695.68" font-size="13">{"0.85"}</text>
                  </g>
                </g>
              </g>
              <rect
                stroke={COLOR.border}
                x="48.5"
                y="198.5"
                width="279"
                height="636"
                rx="10"
                fill="none"
                stroke-width="1"
              />
              <g id="font-color-palette-hover">
                <title>
                  {"B2: 24 preset colors. Choose a swatch to apply and keep exploring. Close only with × or an outside click. Custom hex values remain available in the field."}
                </title>
                <rect x="62" y="292" width="252" height="278" rx="9" fill="#000" opacity=".35" />
                <rect
                  fill={COLOR.hover}
                  stroke={COLOR.rail}
                  x="64.5"
                  y="288.5"
                  width="247"
                  height="277"
                  rx="8"
                />
                <text fill={COLOR.text} class="s" x="80" y="315" font-size="13" font-weight="600">
                  {"Color families"}
                </text>
                <path d="M288 303l8 8m0-8l-8 8" stroke="#8a8a93" stroke-width="1.5" />
                <rect stroke={COLOR.rail} x="80" y="334" width="30" height="30" rx="4" fill="#FFFFFF" />
                <rect stroke={COLOR.rail} x="116" y="334" width="30" height="30" rx="4" fill="#CCCCCC" />
                <rect stroke={COLOR.rail} x="152" y="334" width="30" height="30" rx="4" fill="#999999" />
                <rect stroke={COLOR.rail} x="188" y="334" width="30" height="30" rx="4" fill="#666666" />
                <rect stroke={COLOR.rail} x="224" y="334" width="30" height="30" rx="4" fill="#333333" />
                <rect stroke={COLOR.rail} x="260" y="334" width="30" height="30" rx="4" fill="#000000" />
                <rect fill={COLOR.error} stroke={COLOR.rail} x="80" y="372" width="30" height="30" rx="4" />
                <rect fill={COLOR.accent} stroke={COLOR.text} x="116" y="372" width="30" height="30" rx="4" />
                <path
                  d="M124 387 l5 5 9-11"
                  fill="none"
                  stroke="#17120a"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
                <rect stroke={COLOR.rail} x="152" y="372" width="30" height="30" rx="4" fill="#FFE58A" />
                <rect stroke={COLOR.rail} x="188" y="372" width="30" height="30" rx="4" fill="#9BDCA4" />
                <rect stroke={COLOR.rail} x="224" y="372" width="30" height="30" rx="4" fill="#91B8DF" />
                <rect
                  x="222"
                  y="370"
                  width="34"
                  height="34"
                  rx="6"
                  fill="none"
                  stroke="#080809"
                  stroke-width="4"
                />
                <rect
                  stroke={COLOR.text}
                  x="222"
                  y="370"
                  width="34"
                  height="34"
                  rx="6"
                  fill="none"
                  stroke-width="2"
                />
                <rect stroke={COLOR.rail} x="260" y="372" width="30" height="30" rx="4" fill="#B69BDC" />
                <rect stroke={COLOR.rail} x="80" y="410" width="30" height="30" rx="4" fill="#E84545" />
                <rect stroke={COLOR.rail} x="116" y="410" width="30" height="30" rx="4" fill="#E69324" />
                <rect stroke={COLOR.rail} x="152" y="410" width="30" height="30" rx="4" fill="#E5C638" />
                <rect stroke={COLOR.rail} x="188" y="410" width="30" height="30" rx="4" fill="#41AC62" />
                <rect stroke={COLOR.rail} x="224" y="410" width="30" height="30" rx="4" fill="#438FD4" />
                <rect stroke={COLOR.rail} x="260" y="410" width="30" height="30" rx="4" fill="#9164CE" />
                <rect stroke={COLOR.rail} x="80" y="448" width="30" height="30" rx="4" fill="#8E2828" />
                <rect stroke={COLOR.rail} x="116" y="448" width="30" height="30" rx="4" fill="#915617" />
                <rect stroke={COLOR.rail} x="152" y="448" width="30" height="30" rx="4" fill="#8A7621" />
                <rect stroke={COLOR.rail} x="188" y="448" width="30" height="30" rx="4" fill="#256B3A" />
                <rect stroke={COLOR.rail} x="224" y="448" width="30" height="30" rx="4" fill="#285886" />
                <rect stroke={COLOR.rail} x="260" y="448" width="30" height="30" rx="4" fill="#58377F" />
                <text fill={COLOR.soft} class="s" x="80" y="511" font-size="11">
                  {"Hover highlights · click applies."}
                </text>
                <text class="s" x="80" y="539" font-size="11" fill="#8a8a93">
                  {"Custom color? Edit the hex value."}
                </text>
                <g
                  fill={COLOR.text}
                  transform="translate(242 392) scale(.65)"
                  stroke="#080809"
                  stroke-width="1.5"
                  stroke-linejoin="round"
                >
                  <path d="M5 13V2.5a2 2 0 0 1 4 0V9a2 2 0 0 1 4 0v1a2 2 0 0 1 4 0v1a2 2 0 0 1 4 0v7c0 4-2 7-5 9H8L1 17c-2-3 1-5 3-3l1 1Z" />
                </g>
                <path fill={COLOR.hover} d="M201 564H215L208 573Z" />
                <path stroke={COLOR.rail} d="M202 566l6 7 6-7" fill="none" />
              </g>
            </g>
            <rect
              stroke={COLOR.border}
              x="48.5"
              y="287.5"
              width="279"
              height="352"
              rx="10"
              fill="none"
              stroke-width="1"
            />
          </g>
          <text class="s" x="688" y="1731" font-size="12" fill="#8a8a93">
            {"2 px hover ring · checkmark stays on selection."}
          </text>
          <text fill={COLOR.soft} class="s" x="1008" y="1334" font-size="12" font-weight="600">
            {"Font selector · preview cards"}
          </text>
          <g transform="translate(960 1104)">
            <g clip-path="url(#section-clip-5)">
              <rect
                x="47.5"
                y="197.5"
                width="281"
                height="638"
                rx="10"
                fill="#000"
                opacity="0.35"
                transform="translate(0 6)"
              />
              <g clip-path="url(#c1)">
                <rect fill={COLOR.panel} x="48" y="198" width="280" height="637" />
                <g clip-path="url(#c2)">
                  <PropertyPanelHeader
                    x={48}
                    y={198}
                    icon={{ type: "track", kind: "T" }}
                    title={"Let's get started"}
                    subtitle={<ClipSubtitle clipType="Text clip" track="Text 1" kind="T" />}
                  />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="285.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"CONTENT"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="64.5"
                    y="298.5"
                    width="247"
                    height="57"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="s" x="74" y="319.68" font-size="13">
                    {"Let's get started"}
                  </text>
                  <rect fill={COLOR.divider} x="64" y="372" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="395.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"TIMING"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="64" y="418.96" font-size="11">{"Start"}</text>
                  <text fill={COLOR.muted} class="s" x="192" y="418.96" font-size="11">{"Duration"}</text>
                  <rect x="64" y="424" width="120" height="30" rx="6" fill="#131316" />
                  <text fill={COLOR.text} class="m" x="74" y="443.68" font-size="13">{"0:10.03"}</text>
                  <text fill={COLOR.dim} class="m" x="174" y="442.96" font-size="11" text-anchor="end">
                    {"303 f"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="192.5"
                    y="424.5"
                    width="119"
                    height="29"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="m" x="202" y="443.68" font-size="13">{"2.90"}</text>
                  <text fill={COLOR.muted} class="s" x="302" y="443.32" font-size="12" text-anchor="end">
                    {"s"}
                  </text>
                  <rect fill={COLOR.divider} x="64" y="470" width="248" height="1" />
                  <g id="text-clip-style-font">
                    <title>{"Text style: font, font size, and font color"}</title>
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="493.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"STYLE"}
                    </text>
                    <text fill={COLOR.soft} class="s" x="64" y="517" font-size="12">{"Font"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="526.5"
                      width="247"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="s" x="74" y="548" font-size="13">{"PingFang SC"}</text>
                    <path
                      stroke={COLOR.muted}
                      d="M294 541 l4 4 l4 -4"
                      fill="none"
                      stroke-width="1.5"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                    <text fill={COLOR.soft} class="s" x="64" y="581" font-size="12">{"Font size"}</text>
                    <text fill={COLOR.soft} class="s" x="192" y="581" font-size="12">{"Font color"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="74" y="612" font-size="13">{"64"}</text>
                    <text fill={COLOR.muted} class="s" x="174" y="612" font-size="12" text-anchor="end">
                      {"px"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      stroke={COLOR.border}
                      x="200.5"
                      y="599.5"
                      width="15"
                      height="15"
                      rx="4"
                      fill="#ffffff"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="224" y="612" font-size="13">{"#"}</text>
                    <text fill={COLOR.text} class="m" x="234" y="612" font-size="13">{"FFFFFF"}</text>
                  </g>
                  <g id="text-clip-background-border">
                    <title>{"Background: color and opacity, removable."}</title>
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"BACKGROUND"}
                    </text>
                    <text fill={COLOR.accent} class="s" x="312" y="664.32" font-size="12" text-anchor="end">
                      {"Remove"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="159"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      stroke={COLOR.border}
                      x="72.5"
                      y="683.5"
                      width="15"
                      height="15"
                      rx="4"
                      fill="#000000"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="96" y="695.68" font-size="13">{"#"}</text>
                    <text fill={COLOR.text} class="m" x="106" y="695.68" font-size="13">{"000000"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="232.5"
                      y="676.5"
                      width="79"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="242" y="695.68" font-size="13">{"50"}</text>
                    <text fill={COLOR.muted} class="s" x="302" y="695.32" font-size="12" text-anchor="end">
                      {"%"}
                    </text>
                  </g>
                  <g transform="translate(0 82)">
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"POSITION"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="74" y="695.32" font-size="12" font-weight="600">
                      {"X"}
                    </text>
                    <text fill={COLOR.text} class="m" x="89.68" y="695.68" font-size="13">{"0.50"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="202" y="695.32" font-size="12" font-weight="600">
                      {"Y"}
                    </text>
                    <text fill={COLOR.text} class="m" x="217.68" y="695.68" font-size="13">{"0.85"}</text>
                  </g>
                </g>
              </g>
              <rect
                stroke={COLOR.border}
                x="48.5"
                y="198.5"
                width="279"
                height="636"
                rx="10"
                fill="none"
                stroke-width="1"
              />
              <g transform="translate(-1600 -50)">
                <g id="font-selector-b3" transform="translate(-280 0)">
                  <title>
                    {"Font selector: searchable preview cards. Click applies and keeps the picker open. Hover highlights the card without changing the selected font."}
                  </title>
                  <rect
                    fill={COLOR.hover}
                    stroke={COLOR.rail}
                    x="1944"
                    y="306"
                    width="248"
                    height="438"
                    rx="8"
                  />
                  <text fill={COLOR.text} class="s" x="1960" y="334" font-size="13">{"Font"}</text>
                  <text class="s" x="2163" y="335" font-size="20" fill="#8a8a93">{"×"}</text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.rail}
                    x="1956"
                    y="348"
                    width="224"
                    height="34"
                    rx="6"
                  />
                  <text fill={COLOR.muted} class="s" x="1968" y="370" font-size="13">{"Search fonts…"}</text>
                  <rect x="1956" y="394" width="224" height="96" rx="7" fill="#19191d" stroke="#34343b" />
                  <text fill={COLOR.soft} class="s" x="1968" y="416" font-size="12">{"Helvetica Neue"}</text>
                  <rect x="1957" y="427" width="222" height="1" rx="0" fill="#34343b" stroke="none" />
                  <text
                    fill={COLOR.text}
                    class="s"
                    x="1968"
                    y="464"
                    font-size="17"
                    style="font-family:Helvetica Neue"
                  >
                    {"你好，世界 · Hello,world"}
                  </text>
                  <rect
                    stroke={COLOR.accent}
                    x="1956"
                    y="498"
                    width="224"
                    height="96"
                    rx="7"
                    fill="#19191d"
                  />
                  <text fill={COLOR.accent} class="s" x="1968" y="520" font-size="12">{"PingFang SC"}</text>
                  <text fill={COLOR.accent} class="s" x="2155" y="520" font-size="15">{"✓"}</text>
                  <rect x="1957" y="531" width="222" height="1" rx="0" fill="#3b3327" stroke="none" />
                  <text
                    fill={COLOR.text}
                    class="s"
                    x="1968"
                    y="568"
                    font-size="17"
                    style="font-family:PingFang SC"
                  >
                    {"你好，世界 · Hello,world"}
                  </text>
                  <rect x="1956" y="602" width="224" height="96" rx="7" fill="#19191d" stroke="#8a8a93" />
                  <text fill={COLOR.soft} class="s" x="1968" y="624" font-size="12">{"Songti SC"}</text>
                  <rect x="1957" y="635" width="222" height="1" rx="0" fill="#34343b" stroke="none" />
                  <text
                    fill={COLOR.text}
                    class="s"
                    x="1968"
                    y="672"
                    font-size="17"
                    style="font-family:Songti SC"
                  >
                    {"你好，世界 · Hello,world"}
                  </text>
                  <rect x="2181" y="394" width="3" height="304" rx="2" fill="#34343b" stroke="none" />
                  <rect fill={COLOR.muted} x="2181" y="394" width="3" height="65" rx="2" stroke="none" />
                  <text class="s" x="1960" y="727" font-size="12" fill="#8a8a93">
                    {"Click a font to apply."}
                  </text>
                </g>
              </g>
            </g>
            <rect
              stroke={COLOR.border}
              x="48.5"
              y="250.5"
              width="279"
              height="449"
              rx="10"
              fill="none"
              stroke-width="1"
            />
          </g>
          <text class="s" x="1008" y="1828" font-size="12" fill="#8a8a93">
            {"Floating overlay · click applies and stays open."}
          </text>
          <text fill={COLOR.soft} class="s" x="1328" y="1334" font-size="12" font-weight="600">
            {"Font dropdown · hover"}
          </text>
          <g transform="translate(1280 884)">
            <g clip-path="url(#section-clip-2)">
              <rect
                x="47.5"
                y="197.5"
                width="281"
                height="638"
                rx="10"
                fill="#000"
                opacity="0.35"
                transform="translate(0 6)"
              />
              <g clip-path="url(#c1)">
                <rect fill={COLOR.panel} x="48" y="198" width="280" height="637" />
                <g clip-path="url(#c2)">
                  <PropertyPanelHeader
                    x={48}
                    y={198}
                    icon={{ type: "track", kind: "T" }}
                    title={"Let's get started"}
                    subtitle={<ClipSubtitle clipType="Text clip" track="Text 1" kind="T" />}
                  />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="285.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"CONTENT"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="64.5"
                    y="298.5"
                    width="247"
                    height="57"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="s" x="74" y="319.68" font-size="13">
                    {"Let's get started"}
                  </text>
                  <rect fill={COLOR.divider} x="64" y="372" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="395.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"TIMING"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="64" y="418.96" font-size="11">{"Start"}</text>
                  <text fill={COLOR.muted} class="s" x="192" y="418.96" font-size="11">{"Duration"}</text>
                  <rect x="64" y="424" width="120" height="30" rx="6" fill="#131316" />
                  <text fill={COLOR.text} class="m" x="74" y="443.68" font-size="13">{"0:10.03"}</text>
                  <text fill={COLOR.dim} class="m" x="174" y="442.96" font-size="11" text-anchor="end">
                    {"303 f"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="192.5"
                    y="424.5"
                    width="119"
                    height="29"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="m" x="202" y="443.68" font-size="13">{"2.90"}</text>
                  <text fill={COLOR.muted} class="s" x="302" y="443.32" font-size="12" text-anchor="end">
                    {"s"}
                  </text>
                  <rect fill={COLOR.divider} x="64" y="470" width="248" height="1" />
                  <g id="text-clip-style-font-hover">
                    <title>{"Text style: font, font size, and font color"}</title>
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="493.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"STYLE"}
                    </text>
                    <text fill={COLOR.soft} class="s" x="64" y="517" font-size="12">{"Font"}</text>
                    <rect
                      fill={COLOR.accentBackground}
                      stroke={COLOR.accent}
                      x="64.5"
                      y="526.5"
                      width="247"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="s" x="74" y="548" font-size="13">{"Sans"}</text>
                    <path
                      stroke={COLOR.accent}
                      d="M294 541 l4 4 l4 -4"
                      fill="none"
                      stroke-width="1.5"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                    <text fill={COLOR.soft} class="s" x="64" y="581" font-size="12">{"Font size"}</text>
                    <text fill={COLOR.soft} class="s" x="192" y="581" font-size="12">{"Font color"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="74" y="612" font-size="13">{"64"}</text>
                    <text fill={COLOR.muted} class="s" x="174" y="612" font-size="12" text-anchor="end">
                      {"px"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      stroke={COLOR.border}
                      x="200.5"
                      y="599.5"
                      width="15"
                      height="15"
                      rx="4"
                      fill="#ffffff"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="224" y="612" font-size="13">{"#"}</text>
                    <text fill={COLOR.text} class="m" x="234" y="612" font-size="13">{"FFFFFF"}</text>
                  </g>
                  <g id="text-clip-background-border">
                    <title>{"Background: color and opacity, removable."}</title>
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"BACKGROUND"}
                    </text>
                    <text fill={COLOR.accent} class="s" x="312" y="664.32" font-size="12" text-anchor="end">
                      {"Remove"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="159"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      stroke={COLOR.border}
                      x="72.5"
                      y="683.5"
                      width="15"
                      height="15"
                      rx="4"
                      fill="#000000"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="96" y="695.68" font-size="13">{"#"}</text>
                    <text fill={COLOR.text} class="m" x="106" y="695.68" font-size="13">{"000000"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="232.5"
                      y="676.5"
                      width="79"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="242" y="695.68" font-size="13">{"50"}</text>
                    <text fill={COLOR.muted} class="s" x="302" y="695.32" font-size="12" text-anchor="end">
                      {"%"}
                    </text>
                  </g>
                  <g transform="translate(0 82)">
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"POSITION"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="74" y="695.32" font-size="12" font-weight="600">
                      {"X"}
                    </text>
                    <text fill={COLOR.text} class="m" x="89.68" y="695.68" font-size="13">{"0.50"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="202" y="695.32" font-size="12" font-weight="600">
                      {"Y"}
                    </text>
                    <text fill={COLOR.text} class="m" x="217.68" y="695.68" font-size="13">{"0.85"}</text>
                  </g>
                </g>
              </g>
              <rect
                stroke={COLOR.border}
                x="48.5"
                y="198.5"
                width="279"
                height="636"
                rx="10"
                fill="none"
                stroke-width="1"
              />
            </g>
            <rect
              stroke={COLOR.border}
              x="48.5"
              y="470.5"
              width="279"
              height="169"
              rx="10"
              fill="none"
              stroke-width="1"
            />
          </g>
          <text class="s" x="1328" y="1548" font-size="12" fill="#8a8a93">
            {"Gold accent · warm fill, gold border and chevron."}
          </text>
        </g>
        <g id="text-clip-section-background">
          <text fill={COLOR.text} class="s" x="48" y="1892" font-size="18" font-weight="600">
            {"Background"}
          </text>
          <text fill={COLOR.muted} class="s" x="48" y="1916" font-size="12">
            {"None by default. Add sets a color and opacity; Remove clears it."}
          </text>
          <text fill={COLOR.soft} class="s" x="48" y="1940" font-size="12" font-weight="600">
            {"No background"}
          </text>
          <g transform="translate(0 1320)">
            <g clip-path="url(#section-clip-7)">
              <rect
                x="47.5"
                y="197.5"
                width="281"
                height="638"
                rx="10"
                fill="#000"
                opacity="0.35"
                transform="translate(0 6)"
              />
              <g clip-path="url(#c1)">
                <rect fill={COLOR.panel} x="48" y="198" width="280" height="637" />
                <g clip-path="url(#c2)">
                  <PropertyPanelHeader
                    x={48}
                    y={198}
                    icon={{ type: "track", kind: "T" }}
                    title={"Let's get started"}
                    subtitle={<ClipSubtitle clipType="Text clip" track="Text 1" kind="T" />}
                  />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="285.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"CONTENT"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.accent}
                    x="64.5"
                    y="298.5"
                    width="247"
                    height="57"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="s" x="74" y="319.68" font-size="13">
                    {"Let's get started"}
                  </text>
                  <rect fill={COLOR.accent} x="173.02" y="307" width="1.5" height="16" />
                  <rect fill={COLOR.divider} x="64" y="372" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="395.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"TIMING"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="64" y="418.96" font-size="11">{"Start"}</text>
                  <text fill={COLOR.muted} class="s" x="192" y="418.96" font-size="11">{"Duration"}</text>
                  <rect x="64" y="424" width="120" height="30" rx="6" fill="#131316" />
                  <text fill={COLOR.text} class="m" x="74" y="443.68" font-size="13">{"0:10.03"}</text>
                  <text fill={COLOR.dim} class="m" x="174" y="442.96" font-size="11" text-anchor="end">
                    {"303 f"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="192.5"
                    y="424.5"
                    width="119"
                    height="29"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="m" x="202" y="443.68" font-size="13">{"2.90"}</text>
                  <text fill={COLOR.muted} class="s" x="302" y="443.32" font-size="12" text-anchor="end">
                    {"s"}
                  </text>
                  <rect fill={COLOR.divider} x="64" y="470" width="248" height="1" />
                  <g id="text-clip-style">
                    <title>{"Text style: font, font size, and font color"}</title>
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="493.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"STYLE"}
                    </text>
                    <text fill={COLOR.soft} class="s" x="64" y="517" font-size="12">{"Font"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="526.5"
                      width="247"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="s" x="74" y="548" font-size="13">{"Sans"}</text>
                    <path
                      stroke={COLOR.muted}
                      d="M294 541 l4 4 l4 -4"
                      fill="none"
                      stroke-width="1.5"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                    <text fill={COLOR.soft} class="s" x="64" y="581" font-size="12">{"Font size"}</text>
                    <text fill={COLOR.soft} class="s" x="192" y="581" font-size="12">{"Font color"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="74" y="612" font-size="13">{"64"}</text>
                    <text fill={COLOR.muted} class="s" x="174" y="612" font-size="12" text-anchor="end">
                      {"px"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      stroke={COLOR.border}
                      x="200.5"
                      y="599.5"
                      width="15"
                      height="15"
                      rx="4"
                      fill="#ffffff"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="224" y="612" font-size="13">{"#"}</text>
                    <text fill={COLOR.text} class="m" x="234" y="612" font-size="13">{"FFFFFF"}</text>
                  </g>
                  <g id="text-clip-background-border">
                    <title>{"Background: color and opacity, removable."}</title>
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"BACKGROUND"}
                    </text>
                    <text fill={COLOR.accent} class="s" x="312" y="664.32" font-size="12" text-anchor="end">
                      {"Add"}
                    </text>
                    <text fill={COLOR.dim} class="s" x="64" y="688.32" font-size="12">
                      {"No background"}
                    </text>
                  </g>
                  <g transform="translate(0 82)">
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"POSITION"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="74" y="695.32" font-size="12" font-weight="600">
                      {"X"}
                    </text>
                    <text fill={COLOR.text} class="m" x="89.68" y="695.68" font-size="13">{"0.50"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="202" y="695.32" font-size="12" font-weight="600">
                      {"Y"}
                    </text>
                    <text fill={COLOR.text} class="m" x="217.68" y="695.68" font-size="13">{"0.85"}</text>
                  </g>
                </g>
              </g>
              <rect
                stroke={COLOR.border}
                x="48.5"
                y="198.5"
                width="279"
                height="636"
                rx="10"
                fill="none"
                stroke-width="1"
              />
            </g>
            <rect
              stroke={COLOR.border}
              x="48.5"
              y="640.5"
              width="279"
              height="81"
              rx="10"
              fill="none"
              stroke-width="1"
            />
          </g>
          <text fill={COLOR.soft} class="s" x="368" y="1940" font-size="12" font-weight="600">{"Set"}</text>
          <g transform="translate(320 1320)">
            <g clip-path="url(#section-clip-7)">
              <rect
                x="47.5"
                y="197.5"
                width="281"
                height="638"
                rx="10"
                fill="#000"
                opacity="0.35"
                transform="translate(0 6)"
              />
              <g clip-path="url(#c1)">
                <rect fill={COLOR.panel} x="48" y="198" width="280" height="637" />
                <g clip-path="url(#c2)">
                  <PropertyPanelHeader
                    x={48}
                    y={198}
                    icon={{ type: "track", kind: "T" }}
                    title={"Let's get started"}
                    subtitle={<ClipSubtitle clipType="Text clip" track="Text 1" kind="T" />}
                  />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="285.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"CONTENT"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.accent}
                    x="64.5"
                    y="298.5"
                    width="247"
                    height="57"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="s" x="74" y="319.68" font-size="13">
                    {"Let's get started"}
                  </text>
                  <rect fill={COLOR.accent} x="173.02" y="307" width="1.5" height="16" />
                  <rect fill={COLOR.divider} x="64" y="372" width="248" height="1" />
                  <text
                    fill={COLOR.muted}
                    class="s"
                    x="64"
                    y="395.96"
                    font-size="11"
                    font-weight="600"
                    letter-spacing="0.6"
                  >
                    {"TIMING"}
                  </text>
                  <text fill={COLOR.muted} class="s" x="64" y="418.96" font-size="11">{"Start"}</text>
                  <text fill={COLOR.muted} class="s" x="192" y="418.96" font-size="11">{"Duration"}</text>
                  <rect x="64" y="424" width="120" height="30" rx="6" fill="#131316" />
                  <text fill={COLOR.text} class="m" x="74" y="443.68" font-size="13">{"0:10.03"}</text>
                  <text fill={COLOR.dim} class="m" x="174" y="442.96" font-size="11" text-anchor="end">
                    {"303 f"}
                  </text>
                  <rect
                    fill={COLOR.surface}
                    stroke={COLOR.border}
                    x="192.5"
                    y="424.5"
                    width="119"
                    height="29"
                    rx="6"
                    stroke-width="1"
                  />
                  <text fill={COLOR.text} class="m" x="202" y="443.68" font-size="13">{"2.90"}</text>
                  <text fill={COLOR.muted} class="s" x="302" y="443.32" font-size="12" text-anchor="end">
                    {"s"}
                  </text>
                  <rect fill={COLOR.divider} x="64" y="470" width="248" height="1" />
                  <g id="text-clip-style">
                    <title>{"Text style: font, font size, and font color"}</title>
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="493.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"STYLE"}
                    </text>
                    <text fill={COLOR.soft} class="s" x="64" y="517" font-size="12">{"Font"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="526.5"
                      width="247"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="s" x="74" y="548" font-size="13">{"Sans"}</text>
                    <path
                      stroke={COLOR.muted}
                      d="M294 541 l4 4 l4 -4"
                      fill="none"
                      stroke-width="1.5"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                    <text fill={COLOR.soft} class="s" x="64" y="581" font-size="12">{"Font size"}</text>
                    <text fill={COLOR.soft} class="s" x="192" y="581" font-size="12">{"Font color"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="74" y="612" font-size="13">{"64"}</text>
                    <text fill={COLOR.muted} class="s" x="174" y="612" font-size="12" text-anchor="end">
                      {"px"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="590.5"
                      width="119"
                      height="33"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      stroke={COLOR.border}
                      x="200.5"
                      y="599.5"
                      width="15"
                      height="15"
                      rx="4"
                      fill="#ffffff"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="224" y="612" font-size="13">{"#"}</text>
                    <text fill={COLOR.text} class="m" x="234" y="612" font-size="13">{"FFFFFF"}</text>
                  </g>
                  <g id="text-clip-background-border">
                    <title>{"Background: color and opacity, removable."}</title>
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"BACKGROUND"}
                    </text>
                    <text fill={COLOR.accent} class="s" x="312" y="664.32" font-size="12" text-anchor="end">
                      {"Remove"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="159"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <rect
                      stroke={COLOR.border}
                      x="72.5"
                      y="683.5"
                      width="15"
                      height="15"
                      rx="4"
                      fill="#000000"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="m" x="96" y="695.68" font-size="13">{"#"}</text>
                    <text fill={COLOR.text} class="m" x="106" y="695.68" font-size="13">{"000000"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="232.5"
                      y="676.5"
                      width="79"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.text} class="m" x="242" y="695.68" font-size="13">{"50"}</text>
                    <text fill={COLOR.muted} class="s" x="302" y="695.32" font-size="12" text-anchor="end">
                      {"%"}
                    </text>
                  </g>
                  <g transform="translate(0 82)">
                    <rect fill={COLOR.divider} x="64" y="640" width="248" height="1" />
                    <text
                      fill={COLOR.muted}
                      class="s"
                      x="64"
                      y="663.96"
                      font-size="11"
                      font-weight="600"
                      letter-spacing="0.6"
                    >
                      {"POSITION"}
                    </text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="64.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="74" y="695.32" font-size="12" font-weight="600">
                      {"X"}
                    </text>
                    <text fill={COLOR.text} class="m" x="89.68" y="695.68" font-size="13">{"0.50"}</text>
                    <rect
                      fill={COLOR.surface}
                      stroke={COLOR.border}
                      x="192.5"
                      y="676.5"
                      width="119"
                      height="29"
                      rx="6"
                      stroke-width="1"
                    />
                    <text fill={COLOR.muted} class="s" x="202" y="695.32" font-size="12" font-weight="600">
                      {"Y"}
                    </text>
                    <text fill={COLOR.text} class="m" x="217.68" y="695.68" font-size="13">{"0.85"}</text>
                  </g>
                </g>
              </g>
              <rect
                stroke={COLOR.border}
                x="48.5"
                y="198.5"
                width="279"
                height="636"
                rx="10"
                fill="none"
                stroke-width="1"
              />
            </g>
            <rect
              stroke={COLOR.border}
              x="48.5"
              y="640.5"
              width="279"
              height="81"
              rx="10"
              fill="none"
              stroke-width="1"
            />
          </g>
        </g>
      </g>
    </svg>
  );
}

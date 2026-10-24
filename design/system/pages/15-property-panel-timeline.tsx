import { COLOR } from "../theme.ts";
import { FontStyle } from "../components/fonts.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

export default function Page15PropertyPanelTimelineSvg(): Markup {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width="688" height="830" viewBox="0 0 688 830">
      <FontStyle />
      <rect fill={COLOR.page} x="0" y="0" width="688" height="830" />
      <text fill={COLOR.text} class="s" x="48" y="60.64" font-size="24" font-weight="700" text-anchor="start">
        {"Property Panel · Timeline"}
      </text>
      <text
        fill={COLOR.muted}
        class="s"
        x="48"
        y="87.04"
        font-size="14"
        font-weight="400"
        text-anchor="start"
      >
        {"The timeline panel and its two tabs. Other panels are on the property panel page."}
      </text>
      <text fill={COLOR.text} class="s" x="48" y="150" font-size="18" font-weight="600" text-anchor="start">
        {"Timeline file"}
      </text>
      <text fill={COLOR.muted} class="s" x="48" y="174" font-size="12" font-weight="400" text-anchor="start">
        {"Active timeline, nothing selected"}
      </text>
      <text fill={COLOR.soft} class="s" x="48" y="214" font-size="12" font-weight="600" text-anchor="start">
        {"Properties tab"}
      </text>
      <rect
        x="47.5"
        y="229.5"
        width="281"
        height="556"
        rx="10"
        fill="#000"
        opacity="0.35"
        transform="translate(0 6)"
      />
      <rect fill={COLOR.panel} x="48" y="230" width="280" height="555" rx="10" />
      <rect fill={COLOR.accent} stroke={COLOR.accent} x="60.5" y="242.5" width="31" height="31" rx="8" />
      <text class="s" x="76" y="261.96" font-size="11" fill="#17120a" font-weight="700" text-anchor="middle">
        {"TL"}
      </text>
      <text fill={COLOR.text} class="s" x="104" y="255" font-size="14" font-weight="600" text-anchor="start">
        {"edit.timeline"}
      </text>
      <text
        fill={COLOR.muted}
        class="s"
        x="104"
        y="271.3"
        font-size="12"
        font-weight="400"
        text-anchor="start"
      >
        {"timelines/edit.timeline"}
      </text>
      <rect x="187" y="286" width="141" height="30" fill="#1a1a1e" />
      <rect fill={COLOR.border} x="187" y="286" width="141" height="1" />
      <rect fill={COLOR.border} x="187" y="286" width="1" height="30" />
      <rect fill={COLOR.border} x="327" y="286" width="1" height="30" />
      <rect fill={COLOR.border} x="187" y="315" width="141" height="1" />
      <text
        fill={COLOR.muted}
        class="s"
        x="257.5"
        y="306"
        font-size="12"
        font-weight="400"
        text-anchor="middle"
      >
        {"History"}
      </text>
      <rect fill={COLOR.panel} x="48" y="286" width="140" height="30" />
      <rect fill={COLOR.border} x="48" y="286" width="140" height="1" />
      <rect fill={COLOR.border} x="48" y="286" width="1" height="30" />
      <rect fill={COLOR.border} x="187" y="286" width="1" height="30" />
      <text
        fill={COLOR.text}
        class="s"
        x="118.0"
        y="306"
        font-size="12"
        font-weight="600"
        text-anchor="middle"
      >
        {"Properties"}
      </text>
      <g transform="translate(-960 55)">
        <text
          fill={COLOR.muted}
          class="s"
          x="1024"
          y="285.96"
          font-size="11"
          font-weight="600"
          letter-spacing="0.6"
        >
          {"DURATION"}
        </text>
        <text fill={COLOR.text} class="m" x="1024" y="318.64" font-size="24" font-weight="600">{"0:18"}</text>
        <g transform="translate(0 -24)">
          <rect fill={COLOR.divider} x="1024" y="358" width="248" height="1" />
          <text
            fill={COLOR.muted}
            class="s"
            x="1024"
            y="381.96"
            font-size="11"
            font-weight="600"
            letter-spacing="0.6"
          >
            {"FORMAT"}
          </text>
          <text fill={COLOR.accent} class="s" x="1272" y="382.32" font-size="12" text-anchor="end">
            {"Change…"}
          </text>
          <text fill={COLOR.muted} class="s" x="1024" y="412.68" font-size="13">{"Frame rate"}</text>
          <text fill={COLOR.text} class="m" x="1272" y="412.68" font-size="13" text-anchor="end">
            {"30 fps"}
          </text>
          <text fill={COLOR.muted} class="s" x="1024" y="440.68" font-size="13">{"Resolution"}</text>
          <text fill={COLOR.text} class="m" x="1272" y="440.68" font-size="13" text-anchor="end">
            {"1920 × 1080"}
          </text>
          <text fill={COLOR.muted} class="s" x="1024" y="468.68" font-size="13">{"Audio rate"}</text>
          <text fill={COLOR.text} class="m" x="1272" y="468.68" font-size="13" text-anchor="end">
            {"48 kHz"}
          </text>
          <rect fill={COLOR.divider} x="1024" y="486" width="248" height="1" />
          <text
            fill={COLOR.muted}
            class="s"
            x="1024"
            y="509.96"
            font-size="11"
            font-weight="600"
            letter-spacing="0.6"
          >
            {"CONTENTS"}
          </text>
          <text fill={COLOR.muted} class="s" x="1024" y="540.68" font-size="13">{"Clips"}</text>
          <text fill={COLOR.text} class="m" x="1272" y="540.68" font-size="13" text-anchor="end">{"9"}</text>
          <text fill={COLOR.muted} class="s" x="1024" y="568.68" font-size="13">{"Tracks"}</text>
          <rect
            x="1238.5"
            y="554.5"
            width="33"
            height="19"
            rx="4"
            fill="#24656b"
            stroke="#34757b"
            stroke-width="1"
          />
          <text
            class="s"
            x="1255"
            y="567.96"
            font-size="11"
            fill="#ffffff"
            font-weight="600"
            text-anchor="middle"
          >
            {"A 1"}
          </text>
          <rect
            x="1198.5"
            y="554.5"
            width="33"
            height="19"
            rx="4"
            fill="#294d75"
            stroke="#395d85"
            stroke-width="1"
          />
          <text
            class="s"
            x="1215"
            y="567.96"
            font-size="11"
            fill="#ffffff"
            font-weight="600"
            text-anchor="middle"
          >
            {"V 1"}
          </text>
          <rect
            x="1158.5"
            y="554.5"
            width="33"
            height="19"
            rx="4"
            fill="#7251a3"
            stroke="#8261b3"
            stroke-width="1"
          />
          <text
            class="s"
            x="1175"
            y="567.96"
            font-size="11"
            fill="#ffffff"
            font-weight="600"
            text-anchor="middle"
          >
            {"T 1"}
          </text>
          <rect fill={COLOR.divider} x="1024" y="586" width="248" height="1" />
          <text
            fill={COLOR.muted}
            class="s"
            x="1024"
            y="609.96"
            font-size="11"
            font-weight="600"
            letter-spacing="0.6"
          >
            {"LOCATION"}
          </text>
          <text fill={COLOR.soft} class="m" x="1024" y="632.32" font-size="12">
            {"/Users/me/market-vlog/"}
          </text>
          <text fill={COLOR.soft} class="m" x="1024" y="650.32" font-size="12">
            {"timelines/edit.timeline"}
          </text>
        </g>
      </g>
      <rect stroke={COLOR.border} x="48.5" y="230.5" width="279" height="554" rx="10" fill="none" />
      <text fill={COLOR.soft} class="s" x="368" y="214" font-size="12" font-weight="600" text-anchor="start">
        {"History tab"}
      </text>
      <rect
        x="367.5"
        y="229.5"
        width="281"
        height="556"
        rx="10"
        fill="#000"
        opacity="0.35"
        transform="translate(0 6)"
      />
      <rect fill={COLOR.panel} x="368" y="230" width="280" height="555" rx="10" />
      <rect fill={COLOR.accent} stroke={COLOR.accent} x="380.5" y="242.5" width="31" height="31" rx="8" />
      <text class="s" x="396" y="261.96" font-size="11" fill="#17120a" font-weight="700" text-anchor="middle">
        {"TL"}
      </text>
      <text fill={COLOR.text} class="s" x="424" y="255" font-size="14" font-weight="600" text-anchor="start">
        {"edit.timeline"}
      </text>
      <text
        fill={COLOR.muted}
        class="s"
        x="424"
        y="271.3"
        font-size="12"
        font-weight="400"
        text-anchor="start"
      >
        {"timelines/edit.timeline"}
      </text>
      <rect x="368" y="286" width="141" height="30" fill="#1a1a1e" />
      <rect fill={COLOR.border} x="368" y="286" width="141" height="1" />
      <rect fill={COLOR.border} x="368" y="286" width="1" height="30" />
      <rect fill={COLOR.border} x="508" y="286" width="1" height="30" />
      <rect fill={COLOR.border} x="368" y="315" width="141" height="1" />
      <text
        fill={COLOR.muted}
        class="s"
        x="438.5"
        y="306"
        font-size="12"
        font-weight="400"
        text-anchor="middle"
      >
        {"Properties"}
      </text>
      <rect fill={COLOR.panel} x="508" y="286" width="140" height="30" />
      <rect fill={COLOR.border} x="508" y="286" width="140" height="1" />
      <rect fill={COLOR.border} x="508" y="286" width="1" height="30" />
      <rect fill={COLOR.border} x="647" y="286" width="1" height="30" />
      <text
        fill={COLOR.text}
        class="s"
        x="578.0"
        y="306"
        font-size="12"
        font-weight="600"
        text-anchor="middle"
      >
        {"History"}
      </text>
      <rect fill={COLOR.border} x="387" y="341" width="2" height="356" />
      <circle fill={COLOR.panel} stroke={COLOR.rail} cx="388" cy="341" r="4" />
      <g opacity="0.45">
        <rect x="402" y="333" width="16" height="16" rx="4" fill="#24656b" />
        <text class="s" x="410" y="345" font-size="10" fill="#ffffff" font-weight="700" text-anchor="middle">
          {"A"}
        </text>
        <text
          fill={COLOR.text}
          class="s"
          x="424"
          y="345"
          font-size="12"
          font-weight="400"
          text-anchor="start"
        >
          {"Audio 1"}
        </text>
        <text
          fill={COLOR.muted}
          class="s"
          x="474.0"
          y="345"
          font-size="11"
          font-weight="400"
          text-anchor="start"
        >
          {"Muted"}
        </text>
      </g>
      <text fill={COLOR.dim} class="m" x="634" y="345" font-size="10" font-weight="400" text-anchor="end">
        {"10:08"}
      </text>
      <circle fill={COLOR.panel} stroke={COLOR.rail} cx="388" cy="375" r="4" />
      <g opacity="0.45">
        <rect x="402" y="367" width="16" height="16" rx="4" fill="#7251a3" />
        <text class="s" x="410" y="379" font-size="10" fill="#ffffff" font-weight="700" text-anchor="middle">
          {"T"}
        </text>
        <text
          fill={COLOR.text}
          class="s"
          x="424"
          y="379"
          font-size="12"
          font-weight="400"
          text-anchor="start"
        >
          {"Text 2"}
        </text>
        <text
          fill={COLOR.muted}
          class="s"
          x="468.0"
          y="379"
          font-size="11"
          font-weight="400"
          text-anchor="start"
        >
          {"Track deleted"}
        </text>
      </g>
      <text fill={COLOR.dim} class="m" x="634" y="379" font-size="10" font-weight="400" text-anchor="end">
        {"10:08"}
      </text>
      <rect
        fill={COLOR.accentBackground}
        stroke={COLOR.accent}
        x="374"
        y="392"
        width="268"
        height="112"
        rx="6"
      />
      <circle fill={COLOR.accent} cx="388" cy="409" r="5" />
      <g>
        <rect x="402" y="401" width="16" height="16" rx="4" fill="#24656b" />
        <text class="s" x="410" y="413" font-size="10" fill="#ffffff" font-weight="700" text-anchor="middle">
          {"A"}
        </text>
        <text
          fill={COLOR.text}
          class="s"
          x="424"
          y="413"
          font-size="12"
          font-weight="600"
          text-anchor="start"
        >
          {"interview"}
        </text>
      </g>
      <text fill={COLOR.dim} class="m" x="634" y="413" font-size="10" font-weight="400" text-anchor="end">
        {"10:07"}
      </text>
      <text fill={COLOR.muted} class="s" x="402" y="442" font-size="11" font-weight="400" text-anchor="start">
        {"Gain"}
      </text>
      <rect fill={COLOR.surface} stroke={COLOR.border} x="434" y="430" width="105.0" height="17" rx="4" />
      <text fill={COLOR.light} class="m" x="440" y="442" font-size="10" font-weight="400" text-anchor="start">
        {"0.00 → −3.00 dB"}
      </text>
      <text fill={COLOR.muted} class="s" x="402" y="468" font-size="11" font-weight="400" text-anchor="start">
        {"Track"}
      </text>
      <text fill={COLOR.light} class="s" x="628" y="468" font-size="11" font-weight="400" text-anchor="end">
        {"Audio 1"}
      </text>
      <rect fill={COLOR.surface} stroke={COLOR.border} x="498" y="476" width="130" height="26" rx="6" />
      <text fill={COLOR.text} class="s" x="563" y="493" font-size="12" font-weight="400" text-anchor="middle">
        {"Fork from here…"}
      </text>
      <circle fill={COLOR.rail} cx="388" cy="527" r="4" />
      <g>
        <rect x="402" y="519" width="16" height="16" rx="4" fill="#7251a3" />
        <text class="s" x="410" y="531" font-size="10" fill="#ffffff" font-weight="700" text-anchor="middle">
          {"T"}
        </text>
        <text
          fill={COLOR.text}
          class="s"
          x="424"
          y="531"
          font-size="12"
          font-weight="400"
          text-anchor="start"
        >
          {"Let's get started"}
        </text>
        <text
          fill={COLOR.muted}
          class="s"
          x="534.0"
          y="531"
          font-size="11"
          font-weight="400"
          text-anchor="start"
        >
          {"Font size"}
        </text>
      </g>
      <text fill={COLOR.dim} class="m" x="634" y="531" font-size="10" font-weight="400" text-anchor="end">
        {"10:06"}
      </text>
      <circle fill={COLOR.rail} cx="388" cy="561" r="4" />
      <g>
        <rect x="402" y="553" width="16" height="16" rx="4" fill="#7251a3" />
        <text class="s" x="410" y="565" font-size="10" fill="#ffffff" font-weight="700" text-anchor="middle">
          {"T"}
        </text>
        <text
          fill={COLOR.text}
          class="s"
          x="424"
          y="565"
          font-size="12"
          font-weight="400"
          text-anchor="start"
        >
          {"Let's get started"}
        </text>
        <text
          fill={COLOR.muted}
          class="s"
          x="534.0"
          y="565"
          font-size="11"
          font-weight="400"
          text-anchor="start"
        >
          {"Text"}
        </text>
      </g>
      <text fill={COLOR.dim} class="m" x="634" y="565" font-size="10" font-weight="400" text-anchor="end">
        {"10:06"}
      </text>
      <rect fill={COLOR.hover} x="374" y="579" width="268" height="32" rx="6" />
      <circle fill={COLOR.rail} cx="388" cy="595" r="4" />
      <g>
        <rect x="402" y="587" width="16" height="16" rx="4" fill="#294d75" />
        <text class="s" x="410" y="599" font-size="10" fill="#ffffff" font-weight="700" text-anchor="middle">
          {"V"}
        </text>
        <text
          fill={COLOR.text}
          class="s"
          x="424"
          y="599"
          font-size="12"
          font-weight="400"
          text-anchor="start"
        >
          {"broll-cafe"}
        </text>
        <text
          fill={COLOR.muted}
          class="s"
          x="492.0"
          y="599"
          font-size="11"
          font-weight="400"
          text-anchor="start"
        >
          {"Split"}
        </text>
      </g>
      <text fill={COLOR.dim} class="m" x="634" y="599" font-size="10" font-weight="400" text-anchor="end">
        {"10:05"}
      </text>
      <circle fill={COLOR.rail} cx="388" cy="629" r="4" />
      <g>
        <rect x="402" y="621" width="16" height="16" rx="4" fill="#294d75" />
        <text class="s" x="410" y="633" font-size="10" fill="#ffffff" font-weight="700" text-anchor="middle">
          {"V"}
        </text>
        <text
          fill={COLOR.text}
          class="s"
          x="424"
          y="633"
          font-size="12"
          font-weight="400"
          text-anchor="start"
        >
          {"broll-cafe"}
        </text>
        <text
          fill={COLOR.muted}
          class="s"
          x="492.0"
          y="633"
          font-size="11"
          font-weight="400"
          text-anchor="start"
        >
          {"Trimmed out"}
        </text>
      </g>
      <text fill={COLOR.dim} class="m" x="634" y="633" font-size="10" font-weight="400" text-anchor="end">
        {"10:03"}
      </text>
      <circle fill={COLOR.rail} cx="388" cy="663" r="4" />
      <g>
        <rect x="402" y="655" width="16" height="16" rx="4" fill="#294d75" />
        <text class="s" x="410" y="667" font-size="10" fill="#ffffff" font-weight="700" text-anchor="middle">
          {"V"}
        </text>
        <text
          fill={COLOR.text}
          class="s"
          x="424"
          y="667"
          font-size="12"
          font-weight="400"
          text-anchor="start"
        >
          {"broll-cafe"}
        </text>
        <text
          fill={COLOR.muted}
          class="s"
          x="492.0"
          y="667"
          font-size="11"
          font-weight="400"
          text-anchor="start"
        >
          {"Moved"}
        </text>
      </g>
      <text fill={COLOR.dim} class="m" x="634" y="667" font-size="10" font-weight="400" text-anchor="end">
        {"10:03"}
      </text>
      <circle fill={COLOR.rail} cx="388" cy="697" r="4" />
      <g>
        <rect x="402" y="689" width="16" height="16" rx="4" fill="#294d75" />
        <text class="s" x="410" y="701" font-size="10" fill="#ffffff" font-weight="700" text-anchor="middle">
          {"V"}
        </text>
        <text
          fill={COLOR.text}
          class="s"
          x="424"
          y="701"
          font-size="12"
          font-weight="400"
          text-anchor="start"
        >
          {"broll-cafe"}
        </text>
        <text
          fill={COLOR.muted}
          class="s"
          x="492.0"
          y="701"
          font-size="11"
          font-weight="400"
          text-anchor="start"
        >
          {"Added"}
        </text>
      </g>
      <text fill={COLOR.dim} class="m" x="634" y="701" font-size="10" font-weight="400" text-anchor="end">
        {"10:02"}
      </text>
      <rect stroke={COLOR.border} x="368.5" y="230.5" width="279" height="554" rx="10" fill="none" />
    </svg>
  );
}

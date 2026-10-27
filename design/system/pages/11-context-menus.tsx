import { COLOR } from "../theme.ts";
import { FontStyle } from "../components/fonts.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

export default function Page11ContextMenusSvg(): Markup {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width="1260" height="1040" viewBox="0 0 1260 1040">
      <FontStyle />
      <defs>
        <filter id="shadow" x="-20%" y="-20%" width="140%" height="160%">
          <feDropShadow dx="0" dy="8" stdDeviation="10" flood-color="#000" flood-opacity="0.55" />
        </filter>
      </defs>
      <rect x="0" y="0" width="1260" height="1040" fill="#1e1e22" />
      <text fill={COLOR.light} class="s" x="40" y="44" font-size="15" font-weight="600">
        {"Video or audio file"}
      </text>
      <rect
        stroke={COLOR.border}
        x="40.5"
        y="80.5"
        width="266"
        height="247"
        rx="8"
        fill="#1b1b1e"
        stroke-width="1"
        filter="url(#shadow)"
      />
      <text fill={COLOR.text} class="s" x="56" y="109.04" font-size="14">{"Generate SRT"}</text>
      <text fill={COLOR.text} class="s" x="56" y="149.04" font-size="14">{"Reveal in Finder"}</text>
      <text fill={COLOR.muted} class="m" x="291" y="149.04" font-size="14" text-anchor="end">{"⌥⌘R"}</text>
      <text fill={COLOR.text} class="s" x="56" y="189.04" font-size="14">{"Open in Default App"}</text>
      <text fill={COLOR.muted} class="m" x="291" y="189.04" font-size="14" text-anchor="end">{"⌃⇧↵"}</text>
      <text fill={COLOR.text} class="s" x="56" y="229.04" font-size="14">{"Copy Path"}</text>
      <text fill={COLOR.text} class="s" x="56" y="269.04" font-size="14">{"Rename"}</text>
      <text fill={COLOR.error} class="s" x="56" y="309.04" font-size="14">{"Move to Trash"}</text>
      <text fill={COLOR.light} class="s" x="347" y="44" font-size="15" font-weight="600">
        {"Timeline file"}
      </text>
      <text class="s" x="347" y="64" font-size="12" fill="#8a8a93">
        {"The open timeline asks for confirmation first"}
      </text>
      <rect
        stroke={COLOR.border}
        x="347.5"
        y="80.5"
        width="266"
        height="287"
        rx="8"
        fill="#1b1b1e"
        stroke-width="1"
        filter="url(#shadow)"
      />
      <text fill={COLOR.text} class="s" x="363" y="109.04" font-size="14">{"Settings"}</text>
      <text fill={COLOR.text} class="s" x="363" y="149.04" font-size="14">{"Keep Text Sections"}</text>
      <text fill={COLOR.text} class="s" x="363" y="189.04" font-size="14">{"Reveal in Finder"}</text>
      <text fill={COLOR.muted} class="m" x="598" y="189.04" font-size="14" text-anchor="end">{"⌥⌘R"}</text>
      <text fill={COLOR.text} class="s" x="363" y="229.04" font-size="14">{"Open in Default App"}</text>
      <text fill={COLOR.muted} class="m" x="598" y="229.04" font-size="14" text-anchor="end">{"⌃⇧↵"}</text>
      <text fill={COLOR.text} class="s" x="363" y="269.04" font-size="14">{"Copy Path"}</text>
      <text fill={COLOR.text} class="s" x="363" y="309.04" font-size="14">{"Rename"}</text>
      <text fill={COLOR.error} class="s" x="363" y="349.04" font-size="14">{"Move to Trash"}</text>
      <text fill={COLOR.light} class="s" x="654" y="44" font-size="15" font-weight="600">{"Other file"}</text>
      <text class="s" x="654" y="64" font-size="12" fill="#8a8a93">
        {"Images, subtitles and other files"}
      </text>
      <rect
        stroke={COLOR.border}
        x="654.5"
        y="80.5"
        width="266"
        height="207"
        rx="8"
        fill="#1b1b1e"
        stroke-width="1"
        filter="url(#shadow)"
      />
      <text fill={COLOR.text} class="s" x="670" y="109.04" font-size="14">{"Reveal in Finder"}</text>
      <text fill={COLOR.muted} class="m" x="905" y="109.04" font-size="14" text-anchor="end">{"⌥⌘R"}</text>
      <text fill={COLOR.text} class="s" x="670" y="149.04" font-size="14">{"Open in Default App"}</text>
      <text fill={COLOR.muted} class="m" x="905" y="149.04" font-size="14" text-anchor="end">{"⌃⇧↵"}</text>
      <text fill={COLOR.text} class="s" x="670" y="189.04" font-size="14">{"Copy Path"}</text>
      <text fill={COLOR.text} class="s" x="670" y="229.04" font-size="14">{"Rename"}</text>
      <text fill={COLOR.error} class="s" x="670" y="269.04" font-size="14">{"Move to Trash"}</text>
      <text fill={COLOR.light} class="s" x="40" y="411" font-size="15" font-weight="600">{"Folder"}</text>
      <text class="s" x="40" y="431" font-size="12" fill="#8a8a93">
        {"No Move to Trash if it holds the open timeline"}
      </text>
      <rect
        stroke={COLOR.border}
        x="40.5"
        y="447.5"
        width="266"
        height="247"
        rx="8"
        fill="#1b1b1e"
        stroke-width="1"
        filter="url(#shadow)"
      />
      <text fill={COLOR.text} class="s" x="56" y="476.04" font-size="14">{"Create New Timeline"}</text>
      <text fill={COLOR.text} class="s" x="56" y="516.04" font-size="14">{"Reveal in Finder"}</text>
      <text fill={COLOR.muted} class="m" x="291" y="516.04" font-size="14" text-anchor="end">{"⌥⌘R"}</text>
      <text fill={COLOR.text} class="s" x="56" y="556.04" font-size="14">{"Open in Default App"}</text>
      <text fill={COLOR.muted} class="m" x="291" y="556.04" font-size="14" text-anchor="end">{"⌃⇧↵"}</text>
      <text fill={COLOR.text} class="s" x="56" y="596.04" font-size="14">{"Copy Path"}</text>
      <text fill={COLOR.text} class="s" x="56" y="636.04" font-size="14">{"Rename"}</text>
      <text fill={COLOR.error} class="s" x="56" y="676.04" font-size="14">{"Move to Trash"}</text>
      <text fill={COLOR.light} class="s" x="347" y="411" font-size="15" font-weight="600">
        {"Project root"}
      </text>
      <text class="s" x="347" y="431" font-size="12" fill="#8a8a93">
        {"Right-click the project name in the explorer"}
      </text>
      <rect
        stroke={COLOR.border}
        x="347.5"
        y="447.5"
        width="266"
        height="167"
        rx="8"
        fill="#1b1b1e"
        stroke-width="1"
        filter="url(#shadow)"
      />
      <text fill={COLOR.text} class="s" x="363" y="476.04" font-size="14">{"Create New Timeline"}</text>
      <text fill={COLOR.text} class="s" x="363" y="516.04" font-size="14">{"Reveal in Finder"}</text>
      <text fill={COLOR.muted} class="m" x="598" y="516.04" font-size="14" text-anchor="end">{"⌥⌘R"}</text>
      <text fill={COLOR.text} class="s" x="363" y="556.04" font-size="14">{"Open in Default App"}</text>
      <text fill={COLOR.muted} class="m" x="598" y="556.04" font-size="14" text-anchor="end">{"⌃⇧↵"}</text>
      <text fill={COLOR.text} class="s" x="363" y="596.04" font-size="14">{"Copy Path"}</text>
      <text fill={COLOR.light} class="s" x="40" y="738" font-size="15" font-weight="600">{"Video clip"}</text>
      <text class="s" x="40" y="758" font-size="12" fill="#8a8a93">
        {"Copies transform, gain and mute; disabled with nothing to apply to"}
      </text>
      <rect
        stroke={COLOR.border}
        x="40.5"
        y="774.5"
        width="290"
        height="87"
        rx="8"
        fill="#1b1b1e"
        stroke-width="1"
        filter="url(#shadow)"
      />
      <text fill={COLOR.text} class="s" x="56" y="803.04" font-size="14">{"Transcribe the Clip"}</text>
      <text fill={COLOR.text} class="s" x="56" y="843.04" font-size="14">
        {"Apply Properties to All Other Clips"}
      </text>
      <text fill={COLOR.light} class="s" x="371" y="738" font-size="15" font-weight="600">
        {"Audio clip"}
      </text>
      <text class="s" x="371" y="758" font-size="12" fill="#8a8a93">
        {"Apply Properties is always disabled"}
      </text>
      <rect
        stroke={COLOR.border}
        x="371.5"
        y="774.5"
        width="290"
        height="87"
        rx="8"
        fill="#1b1b1e"
        stroke-width="1"
        filter="url(#shadow)"
      />
      <text fill={COLOR.text} class="s" x="387" y="803.04" font-size="14">{"Transcribe the Clip"}</text>
      <text fill={COLOR.muted} class="s" x="387" y="843.04" font-size="14">
        {"Apply Properties to All Other Clips"}
      </text>
      <text fill={COLOR.light} class="s" x="702" y="738" font-size="15" font-weight="600">{"Text clip"}</text>
      <text class="s" x="702" y="758" font-size="12" fill="#8a8a93">
        {"Apply style needs other text clips on the track"}
      </text>
      <rect
        stroke={COLOR.border}
        x="702.5"
        y="774.5"
        width="238"
        height="47"
        rx="8"
        fill="#1b1b1e"
        stroke-width="1"
        filter="url(#shadow)"
      />
      <text fill={COLOR.text} class="s" x="718" y="803.04" font-size="14">{"Apply style to track"}</text>
      <text fill={COLOR.light} class="s" x="981" y="738" font-size="15" font-weight="600">
        {"Text track"}
      </text>
      <text class="s" x="981" y="758" font-size="12" fill="#8a8a93">
        {"Add text inserts at the right-clicked position"}
      </text>
      <rect
        stroke={COLOR.border}
        x="981.5"
        y="774.5"
        width="238"
        height="47"
        rx="8"
        fill="#1b1b1e"
        stroke-width="1"
        filter="url(#shadow)"
      />
      <text fill={COLOR.text} class="s" x="997" y="803.04" font-size="14">{"Add text"}</text>
      <text fill={COLOR.light} class="s" x="40" y="916" font-size="15" font-weight="600">
        {"Default item"}
      </text>
      <rect
        stroke={COLOR.border}
        x="40.5"
        y="952.5"
        width="199"
        height="47"
        rx="8"
        fill="#1b1b1e"
        stroke-width="1"
        filter="url(#shadow)"
      />
      <text fill={COLOR.text} class="s" x="56" y="981.04" font-size="14">{"Reveal in Finder"}</text>
      <text fill={COLOR.muted} class="m" x="224" y="981.04" font-size="14" text-anchor="end">{"⌥⌘R"}</text>
      <text fill={COLOR.light} class="s" x="280" y="916" font-size="15" font-weight="600">
        {"Hovered item"}
      </text>
      <rect
        stroke={COLOR.border}
        x="280.5"
        y="952.5"
        width="199"
        height="47"
        rx="8"
        fill="#1b1b1e"
        stroke-width="1"
        filter="url(#shadow)"
      />
      <rect x="284" y="956" width="192" height="40" rx="6" fill="#34343a" />
      <text fill={COLOR.text} class="s" x="296" y="981.04" font-size="14">{"Reveal in Finder"}</text>
      <text fill={COLOR.muted} class="m" x="464" y="981.04" font-size="14" text-anchor="end">{"⌥⌘R"}</text>
      <text fill={COLOR.light} class="s" x="520" y="916" font-size="15" font-weight="600">
        {"Disabled item"}
      </text>
      <rect
        stroke={COLOR.border}
        x="520.5"
        y="952.5"
        width="199"
        height="47"
        rx="8"
        fill="#1b1b1e"
        stroke-width="1"
        filter="url(#shadow)"
      />
      <text fill={COLOR.muted} class="s" x="536" y="981.04" font-size="14">{"Reveal in Finder"}</text>
      <text fill={COLOR.muted} class="m" x="704" y="981.04" font-size="14" text-anchor="end">{"⌥⌘R"}</text>
      <text fill={COLOR.light} class="s" x="760" y="916" font-size="15" font-weight="600">
        {"Destructive item"}
      </text>
      <rect
        stroke={COLOR.border}
        x="760.5"
        y="952.5"
        width="199"
        height="47"
        rx="8"
        fill="#1b1b1e"
        stroke-width="1"
        filter="url(#shadow)"
      />
      <text fill={COLOR.error} class="s" x="776" y="981.04" font-size="14">{"Move to Trash"}</text>
      <text fill={COLOR.light} class="s" x="1000" y="916" font-size="15" font-weight="600">
        {"Hovered destructive item"}
      </text>
      <rect
        stroke={COLOR.border}
        x="1000.5"
        y="952.5"
        width="199"
        height="47"
        rx="8"
        fill="#1b1b1e"
        stroke-width="1"
        filter="url(#shadow)"
      />
      <rect x="1004" y="956" width="192" height="40" rx="6" fill="#34343a" />
      <text fill={COLOR.error} class="s" x="1016" y="981.04" font-size="14">{"Move to Trash"}</text>
    </svg>
  );
}

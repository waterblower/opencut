import { COLOR } from "../theme.ts";
import { Notes, Page } from "../components/page.tsx";
import { Text } from "../components/text.tsx";
import { HistoryList, SAMPLE_HISTORY } from "../components/history.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

const PANEL_X = 48;
const PANEL_Y = 130;
const PANEL_WIDTH = 356;

export default function EditHistoryPage(): Markup {
  return (
    <Page
      width={900}
      height={860}
      title="Edit History"
      subtitle="Newest edit on top. Rows lead with the edited target; the current edit expands with its details."
    >
      <rect
        x={PANEL_X}
        y={PANEL_Y}
        width={PANEL_WIDTH}
        height={670}
        rx={10}
        fill={COLOR.panel}
        filter="url(#shadow)"
      />
      <Text x={PANEL_X + 16} y={PANEL_Y + 28} size={12} color={COLOR.muted} weight={600}>HISTORY</Text>
      <rect
        x={PANEL_X + 16.5}
        y={PANEL_Y + 40.5}
        width={37}
        height={17}
        rx={3}
        fill="none"
        stroke="#8a652d"
      />
      <Text x={PANEL_X + 35} y={PANEL_Y + 53.5} size={11} color={COLOR.accent} weight={600} anchor="middle">
        TL
      </Text>
      <Text x={PANEL_X + 62} y={PANEL_Y + 54} mono>devlog 10.timeline</Text>
      <rect x={PANEL_X} y={PANEL_Y + 70} width={PANEL_WIDTH} height={1} fill={COLOR.border} />
      <HistoryList x={PANEL_X} y={PANEL_Y + 82} width={PANEL_WIDTH} entries={SAMPLE_HISTORY} />
      <Notes
        x={460}
        y={140}
        heading="States"
        lines={[
          "Current: accent dot and card, expanded with change, track and Fork.",
          "Earlier: filled grey dot. Click to jump: it becomes current and expands.",
          "Hovered: grey row.",
          "Undone: hollow dot, dimmed. Click to jump forward, like redo.",
        ]}
      />
      <Notes
        x={460}
        y={290}
        heading="Row text"
        lines={[
          "Collapsed: badge, target name, then the action in muted text.",
          "Expanded: the action with its value chip on its own line.",
          "The target is the clip, or the track for track edits.",
        ]}
      />
    </Page>
  );
}

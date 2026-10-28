import { ContextMenu, type ContextMenuItem } from "../components/context-menu.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

const REVEAL: ContextMenuItem = { label: "Reveal in Finder", shortcut: "⌥⌘R" };
const OPEN: ContextMenuItem = { label: "Open in Default App", shortcut: "⌃⇧↵" };
const COPY_PATH: ContextMenuItem = { label: "Copy Path" };
const RENAME: ContextMenuItem = { label: "Rename" };
const TRASH: ContextMenuItem = { label: "Move to Trash", destructive: true };

export default function ContextMenusPage(): Markup {
  return (
    <main class="relative h-[1040px] w-[1260px] bg-[#1e1e22] font-sans">
      <Group x={40} y={44} title="Video or audio file" />
      <Menu x={40} y={80} width={267} items={[{ label: "Generate SRT" }, REVEAL, OPEN, COPY_PATH, RENAME, TRASH]} />

      <Group x={347} y={44} title="Timeline file" caption="The open timeline asks for confirmation first" />
      <Menu
        x={347}
        y={80}
        width={267}
        items={[{ label: "Settings" }, { label: "Keep Text Sections" }, REVEAL, OPEN, COPY_PATH, RENAME, TRASH]}
      />

      <Group x={654} y={44} title="Other file" caption="Images, subtitles and other files" />
      <Menu x={654} y={80} width={267} items={[REVEAL, OPEN, COPY_PATH, RENAME, TRASH]} />

      <Group x={40} y={411} title="Folder" caption="No Move to Trash if it holds the open timeline" />
      <Menu
        x={40}
        y={447}
        width={267}
        items={[{ label: "Create New Timeline" }, REVEAL, OPEN, COPY_PATH, RENAME, TRASH]}
      />

      <Group x={347} y={411} title="Project root" caption="Right-click the project name in the explorer" />
      <Menu x={347} y={447} width={267} items={[{ label: "Create New Timeline" }, REVEAL, OPEN, COPY_PATH]} />

      <Group
        x={40}
        y={738}
        title="Video clip"
        caption="Copies transform, gain and mute; disabled with nothing to apply to"
      />
      <Menu
        x={40}
        y={774}
        width={291}
        items={[{ label: "Transcribe the Clip" }, { label: "Apply Properties to All Other Clips" }]}
      />

      <Group x={371} y={738} title="Audio clip" caption="Apply Properties is always disabled" />
      <Menu
        x={371}
        y={774}
        width={291}
        items={[{ label: "Transcribe the Clip" }, { label: "Apply Properties to All Other Clips", state: "disabled" }]}
      />

      <Group x={702} y={738} title="Text clip" caption="Apply style needs other text clips on the track" />
      <Menu x={702} y={774} width={239} items={[{ label: "Apply style to track" }]} />

      <Group x={981} y={738} title="Text track" caption="Add text inserts at the right-clicked position" />
      <Menu x={981} y={774} width={239} items={[{ label: "Add text" }]} />

      <Group x={40} y={916} title="Default item" />
      <Menu x={40} y={952} width={200} items={[REVEAL]} />
      <Group x={280} y={916} title="Hovered item" />
      <Menu x={280} y={952} width={200} items={[{ ...REVEAL, state: "hovered" }]} />
      <Group x={520} y={916} title="Disabled item" />
      <Menu x={520} y={952} width={200} items={[{ ...REVEAL, state: "disabled" }]} />
      <Group x={760} y={916} title="Destructive item" />
      <Menu x={760} y={952} width={200} items={[TRASH]} />
      <Group x={1000} y={916} title="Hovered destructive item" />
      <Menu x={1000} y={952} width={200} items={[{ ...TRASH, state: "hovered" }]} />
    </main>
  );
}

/// With `leading-none`, the baseline of SF Pro text sits 0.8605 × font size below the box top.
function topForBaseline(baseline: number, fontSize: number): number {
  return baseline - fontSize * 0.8605;
}

/// A group title at baseline `y`, with an optional caption 20px below it.
function Group(props: { x: number; y: number; title: string; caption?: string }): Markup {
  return (
    <div>
      <p
        class="absolute text-[15px] leading-none font-semibold whitespace-nowrap text-light"
        style={`left:${props.x}px;top:${topForBaseline(props.y, 15)}px`}
      >
        {props.title}
      </p>
      {props.caption && (
        <p
          class="absolute text-xs leading-none whitespace-nowrap text-[#8a8a93]"
          style={`left:${props.x}px;top:${topForBaseline(props.y + 20, 12)}px`}
        >
          {props.caption}
        </p>
      )}
    </div>
  );
}

function Menu(props: { x: number; y: number; width: number; items: ContextMenuItem[] }): Markup {
  return (
    <div class="absolute" style={`left:${props.x}px;top:${props.y}px`}>
      <ContextMenu width={props.width} items={props.items} />
    </div>
  );
}

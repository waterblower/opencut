import { ContextMenu, type ContextMenuItem } from "../components/context-menu.tsx";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

const REVEAL: ContextMenuItem = { label: "Reveal in Finder", shortcut: "⌥⌘R" };
const OPEN: ContextMenuItem = { label: "Open in Default App", shortcut: "⌃⇧↵" };
const COPY_PATH: ContextMenuItem = { label: "Copy Path" };
const RENAME: ContextMenuItem = { label: "Rename" };
const TRASH: ContextMenuItem = { label: "Move to Trash", destructive: true };
const RELINK: ContextMenuItem = { label: "Relink Media…" };

export default function ContextMenusPage(): Markup {
  return (
    <main class="w-[1260px] px-10 pt-10 pb-16 font-sans text-[#1b1b1e]">
      <h1 class="text-2xl leading-7 font-bold">Context Menus</h1>
      <p class="mt-2 text-sm leading-5 text-[#55555e]">Right-click menus, grouped by where they open.</p>

      <Section title="File Explorer" description="Right-click a file, folder or the project name.">
        <Row>
          <MenuGroup
            width={267}
            title="Video or audio file"
            items={[{ label: "Generate SRT" }, REVEAL, OPEN, COPY_PATH, RENAME, TRASH]}
          />
          <MenuGroup
            width={267}
            title="Timeline file"
            caption="The open timeline asks for confirmation first"
            items={[{ label: "Settings" }, { label: "Keep Text Sections" }, REVEAL, OPEN, COPY_PATH, RENAME, TRASH]}
          />
          <MenuGroup
            width={267}
            title="Other file"
            caption="Images, subtitles and other files"
            items={[REVEAL, OPEN, COPY_PATH, RENAME, TRASH]}
          />
        </Row>
        <Row>
          <MenuGroup
            width={267}
            title="Folder"
            caption="No Move to Trash if it holds the open timeline"
            items={[{ label: "Create New Timeline" }, REVEAL, OPEN, COPY_PATH, RENAME, TRASH]}
          />
          <MenuGroup
            width={267}
            title="Project root"
            caption="Right-click the project name in the explorer"
            items={[{ label: "Create New Timeline" }, REVEAL, OPEN, COPY_PATH]}
          />
        </Row>
      </Section>

      <Section title="Timeline" description="Right-click a clip or an empty spot on a track.">
        <Row>
          <MenuGroup
            width={291}
            title="Video clip"
            caption="Copies transform, gain and mute; disabled with nothing to apply to"
            items={[{ label: "Transcribe the Clip" }, { label: "Apply Properties to All Other Clips" }]}
          />
          <MenuGroup
            width={291}
            title="Audio clip"
            caption="Apply Properties is always disabled"
            items={[{ label: "Transcribe the Clip" }, {
              label: "Apply Properties to All Other Clips",
              state: "disabled",
            }]}
          />
          <MenuGroup
            width={239}
            title="Text clip"
            caption="Apply style needs other text clips on the track"
            items={[{ label: "Apply style to track" }]}
          />
          <MenuGroup
            width={239}
            title="Text track"
            caption="Add text inserts at the right-clicked position"
            items={[{ label: "Add text" }]}
          />
        </Row>
        <Row>
          <MenuGroup
            width={291}
            title="Offline video clip"
            caption="Only Relink Media… works until the media is found"
            items={[
              RELINK,
              { label: "Transcribe the Clip", state: "disabled" },
              { label: "Apply Properties to All Other Clips", state: "disabled" },
            ]}
          />
          <MenuGroup
            width={291}
            title="Offline audio clip"
            caption="Only Relink Media… works until the media is found"
            items={[
              RELINK,
              { label: "Transcribe the Clip", state: "disabled" },
              { label: "Apply Properties to All Other Clips", state: "disabled" },
            ]}
          />
          <MenuGroup
            width={291}
            title="Offline image clip"
            caption="Only Relink Media… works until the media is found"
            items={[RELINK, { label: "Apply Properties to All Other Clips", state: "disabled" }]}
          />
        </Row>
      </Section>

      <Section title="Menu Items" description="States every menu item can be in.">
        <Row>
          <MenuGroup width={200} title="Default item" items={[REVEAL]} />
          <MenuGroup width={200} title="Hovered item" items={[{ ...REVEAL, state: "hovered" }]} />
          <MenuGroup width={200} title="Disabled item" items={[{ ...REVEAL, state: "disabled" }]} />
          <MenuGroup width={200} title="Destructive item" items={[TRASH]} />
          <MenuGroup width={200} title="Hovered destructive item" items={[{ ...TRASH, state: "hovered" }]} />
        </Row>
      </Section>
    </main>
  );
}

/// A logical group of menus under one heading, separated from the previous section by a rule.
function Section(props: { title: string; description: string; children: Child }): Markup {
  return (
    <section class="mt-12 border-t border-[#d9d9de] pt-8">
      <h2 class="text-xl leading-7 font-bold">{props.title}</h2>
      <p class="mt-1 text-sm leading-5 text-[#55555e]">{props.description}</p>
      <div class="mt-6 space-y-12">{props.children}</div>
    </section>
  );
}

/// Menus side by side. The row is a three-track grid (title, caption, menu) shared by every
/// group in it, so titles, captions and menus each line up across the row.
function Row(props: { children: Child }): Markup {
  return <div class="grid auto-cols-max grid-flow-col grid-rows-[auto_auto_auto] gap-x-10">{props.children}</div>;
}

/// A menu with its title and optional caption, as wide as the menu. A long caption wraps.
/// Its three parts sit in the row's shared tracks.
function MenuGroup(props: { width: number; title: string; caption?: string; items: ContextMenuItem[] }): Markup {
  return (
    <div class="row-span-3 grid grid-rows-subgrid" style={`width:${props.width}px`}>
      <h3 class="text-[15px] leading-5 font-semibold">{props.title}</h3>
      <p class="mt-1 text-xs leading-4 text-[#55555e]">{props.caption ?? ""}</p>
      <div class="mt-3">
        <ContextMenu width={props.width} items={props.items} />
      </div>
    </div>
  );
}

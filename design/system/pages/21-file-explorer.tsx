import type { Markup } from "@design/jsx/jsx-runtime";

type ExplorerState = "media-hover" | "timeline-hover" | "context-menu";
type CursorKind = "grab" | "default";

const videos = [
  { name: "broll-cafe.mp4", extension: "MP4", size: "31.9 MB" },
  { name: "broll-street.mov", extension: "MOV", size: "87.6 MB" },
  { name: "broll-station.mp4", extension: "MP4", size: "42.1 MB" },
];

const menuItems = [
  { label: "Generate SRT" },
  { label: "Reveal in Finder", shortcut: "⌥⌘R", hovered: true },
  { label: "Open in Default App", shortcut: "⌃⇧↵" },
  { label: "Copy Path" },
  { label: "Rename" },
  { label: "Move to Trash", destructive: true },
];

export default function Page21FileExplorer(): Markup {
  return (
    <main class="w-[1240px] bg-page px-12 pt-10 pb-12 font-sans text-text">
      <h1 class="text-2xl leading-7 font-bold">File Explorer</h1>
      <p class="mt-2 text-sm leading-5 text-muted">
        File hover reflects drag support. Context menus keep their source row visible.
      </p>
      <div class="mt-6 grid grid-cols-2 gap-8">
        <section aria-labelledby="draggable-hover">
          <h2 id="draggable-hover" class="text-sm leading-5 font-semibold">Draggable · hover</h2>
          <p class="mt-1 text-xs leading-[18px] text-muted">
            Video, audio, images and SRT files use the open hand.
          </p>
          <div class="mt-2">
            <Explorer state="media-hover" />
          </div>
        </section>
        <section aria-labelledby="not-draggable-hover">
          <h2 id="not-draggable-hover" class="text-sm leading-5 font-semibold">Not draggable · hover</h2>
          <p class="mt-1 text-xs leading-[18px] text-muted">
            Timeline files, folders and unsupported files use the arrow.
          </p>
          <div class="mt-2">
            <Explorer state="timeline-hover" />
          </div>
        </section>
      </div>
      <hr class="mt-9 border-border" />
      <section class="mt-6" aria-labelledby="context-menu-open">
        <h2 id="context-menu-open" class="text-sm leading-5 font-semibold">Context menu open</h2>
        <p class="mt-1 text-xs leading-[18px] text-muted">
          Keep the source row highlighted, even after the pointer moves into the menu.
        </p>
        <div class="mt-4 grid grid-cols-2 gap-8">
          <Explorer state="context-menu" />
          <aside class="pt-7">
            <h3 class="text-base leading-6 font-semibold">The menu belongs to broll-street.mov</h3>
            <p class="mt-3 text-[13px] leading-5 text-subtle">
              The same hover background stays on that row while<br />
              the menu is open. Moving through menu items does<br />
              not transfer the highlight to another file.
            </p>
            <div class="mt-24 ml-[212px] space-y-8">
              <div>
                <h3 class="text-sm leading-5 font-semibold">On dismissal</h3>
                <p class="mt-3 text-xs leading-5 text-muted">
                  Clear the temporary highlight.<br />
                  Keep it only if the pointer is still<br />
                  over the source row.
                </p>
              </div>
              <div>
                <h3 class="text-sm leading-5 font-semibold">Selection stays separate</h3>
                <p class="mt-3 text-xs leading-5 text-muted">
                  Opening a menu does not select a file<br />
                  or change the active timeline.
                </p>
              </div>
            </div>
          </aside>
        </div>
      </section>
    </main>
  );
}

function Explorer({ state }: { state: ExplorerState }): Markup {
  const menuOpen = state === "context-menu";
  return (
    <div class="relative">
      <div
        class={`overflow-hidden rounded-[10px] bg-panel shadow-popover outline-1 -outline-offset-1 outline-border ${
          menuOpen ? "h-[428px]" : "h-[324px]"
        }`}
      >
        <div class="flex h-[38px] cursor-default items-center border-b border-border bg-surface px-4 font-mono text-sm text-muted">
          Filter files…
        </div>
        <ul aria-label="Project files">
          <FolderRow name="market-vlog" expanded root />
          <FolderRow name="media" expanded />
          {videos.map((video) => (
            <FileRow
              {...video}
              highlighted={video.name === "broll-street.mov" && state !== "timeline-hover"}
              cursor={video.name === "broll-street.mov" && state === "media-hover" ? "grab" : undefined}
            />
          ))}
          <FolderRow name="audio" />
          <FileRow
            name="timeline-1.timeline"
            extension="TL"
            active
            highlighted={state === "timeline-hover"}
            cursor={state === "timeline-hover" ? "default" : undefined}
          />
        </ul>
      </div>
      {menuOpen && <FileContextMenu />}
    </div>
  );
}

function FolderRow({ name, expanded = false, root = false }: {
  name: string;
  expanded?: boolean;
  root?: boolean;
}): Markup {
  return (
    <li class={`flex h-[38px] cursor-default items-center gap-2 pr-3 ${root ? "pl-3" : "pl-7"}`}>
      <span aria-hidden="true" class="w-3.5 text-[13px] text-muted">{expanded ? "▾" : "▸"}</span>
      <span class={`font-mono text-sm ${root ? "font-semibold" : "text-subtle"}`}>{name}</span>
    </li>
  );
}

function FileRow({ name, extension, size, active = false, highlighted = false, cursor }: {
  name: string;
  extension: string;
  size?: string;
  active?: boolean;
  highlighted?: boolean;
  cursor?: CursorKind;
}): Markup {
  const background = highlighted ? "bg-hover" : active ? "bg-active-timeline" : "";
  return (
    <li
      class={`relative flex h-[38px] items-center gap-2 pr-3 ${
        active ? "cursor-default pl-[26px]" : "cursor-grab pl-[42px]"
      } ${background}`}
    >
      <span
        class={`flex h-[18px] w-[38px] shrink-0 items-center justify-center rounded-[3px] border text-[11px] font-semibold ${
          active ? "border-accent-border text-accent" : "border-video-badge-border text-video-badge-text"
        }`}
      >
        {extension}
      </span>
      <span class={`min-w-0 flex-1 truncate font-mono text-sm ${active ? "font-semibold" : ""}`}>{name}</span>
      {active
        ? (
          <span class="flex h-5 w-[60px] items-center justify-center rounded-[3px] border border-accent-border bg-accent-background font-mono text-[11px] font-semibold text-accent">
            ACTIVE
          </span>
        )
        : <span class="font-mono text-xs text-dim">{size}</span>}
      {cursor && <Cursor kind={cursor} class="absolute top-1 left-[72%]" />}
    </li>
  );
}

function FileContextMenu(): Markup {
  return (
    <ul
      aria-label="File actions for broll-street.mov"
      class="absolute top-[190px] left-[calc(100%_-_104px)] z-10 w-[300px] rounded-lg border border-border bg-menu p-[3px] shadow-popover"
    >
      {menuItems.map(({ label, shortcut, hovered, destructive }) => (
        <li>
          <button
            type="button"
            class={`relative flex h-10 w-full cursor-default items-center justify-between rounded-md px-3 text-sm ${
              hovered ? "bg-menu-hover" : ""
            } ${destructive ? "text-error" : "text-text"}`}
          >
            <span>{label}</span>
            {shortcut && <kbd class="font-mono text-sm text-muted">{shortcut}</kbd>}
            {hovered && <Cursor kind="default" class="absolute top-5 left-[184px]" />}
          </button>
        </li>
      ))}
    </ul>
  );
}

// Cursor illustrations are icons; the panels, rows and menu use HTML layout.
function Cursor({ kind, class: className }: { kind: CursorKind; class: string }): Markup {
  return (
    <svg
      aria-hidden="true"
      viewBox="-2 -2 34 34"
      class={`pointer-events-none size-8 fill-text stroke-cursor-outline ${className}`}
      stroke-width="1.5"
      stroke-linejoin="round"
    >
      <path
        d={kind === "grab"
          ? "M7 15 3 11C1 9-1 12 1 15L7 24Q9 28 13 28H19Q24 27 25 22L28 9Q29 6 26 6Q24 6 24 9L22 16 23 4Q23 1 21 1Q18 1 18 4L17 14V2Q17-1 14-1Q12-1 12 2V14L10 5Q9 2 7 3Q5 4 6 7Z"
          : "M0 0V20L5.5 15.5 10 25 14 23 9.5 13.5 17 13Z"}
      />
    </svg>
  );
}

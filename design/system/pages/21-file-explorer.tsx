import type { Child, Markup } from "@design/jsx/jsx-runtime";

type ExplorerState = "media-hover" | "timeline-hover";
type CursorKind = "grab" | "default";
type BadgeKind = "video" | "audio" | "image" | "srt" | "other";

const BADGE_CLASS: Record<BadgeKind, string> = {
  video: "border-video-badge-border text-video-badge-text",
  audio: "border-audio-badge-border text-audio-badge-text",
  image: "border-image-badge-border text-image-badge-text",
  srt: "border-srt-badge-border text-srt-badge-text",
  other: "border-other-badge-border text-other-badge-text",
};

const videos = [
  { name: "broll-cafe.mp4", extension: "MP4", size: "31.9 MB" },
  { name: "broll-street.mov", extension: "MOV", size: "87.6 MB" },
  { name: "broll-station.mp4", extension: "MP4", size: "42.1 MB" },
];

export default function Page21FileExplorer(): Markup {
  return (
    <main class="w-[1240px] bg-page px-12 pt-10 pb-12 font-sans text-text">
      <h1 class="text-2xl leading-7 font-bold">File Explorer</h1>
      <p class="mt-2 text-sm leading-5 text-muted">File hover reflects drag support.</p>
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
      <section class="mt-6" aria-labelledby="file-entries">
        <h2 id="file-entries" class="text-sm leading-5 font-semibold">File entries</h2>
        <p class="mt-1 text-xs leading-[18px] text-muted">
          Each kind of entry on its own. Gray is only for files OpenCut does not support.
        </p>
        <div class="mt-4 grid grid-cols-3 gap-x-8 gap-y-6">
          <Entry title="Video" caption="Open hand · drag to a track">
            <FileRow name="broll-cafe.mp4" extension="MP4" size="31.9 MB" standalone />
          </Entry>
          <Entry title="Audio" caption="Open hand · drag to a track">
            <FileRow name="voice-over.m4a" extension="M4A" size="4.2 MB" badge="audio" standalone />
          </Entry>
          <Entry title="Image" caption="Open hand · drag to a track">
            <FileRow name="title-card.png" extension="PNG" size="860 KB" badge="image" standalone />
          </Entry>
          <Entry title="Subtitles" caption="Open hand · drag to a track">
            <FileRow name="interview.srt" extension="SRT" size="2 KB" badge="srt" standalone />
          </Entry>
          <Entry title="Timeline · active" caption="Arrow · the open timeline is tinted and tagged">
            <FileRow name="timeline-1.timeline" extension="TL" active standalone />
          </Entry>
          <Entry title="Unsupported file" caption="Arrow · cannot be dragged">
            <FileRow name="notes.txt" extension="TXT" size="1 KB" badge="other" standalone />
          </Entry>
          <Entry title="Folder · expanded" caption="Arrow">
            <FolderRow name="media" expanded standalone />
          </Entry>
          <Entry title="Folder · collapsed" caption="Arrow">
            <FolderRow name="audio" standalone />
          </Entry>
        </div>
      </section>
    </main>
  );
}

function Explorer({ state }: { state: ExplorerState }): Markup {
  return (
    <div class="h-[400px] overflow-hidden rounded-[10px] bg-panel shadow-popover outline-1 -outline-offset-1 outline-border">
      <div class="flex h-[38px] cursor-default items-center border-b border-border bg-surface px-4 font-mono text-sm text-muted">
        Filter files…
      </div>
      <ul aria-label="Project files">
        <FolderRow name="market-vlog" expanded root />
        <FolderRow name="media" expanded />
        {videos.map((video) => (
          <FileRow
            {...video}
            highlighted={video.name === "broll-street.mov" && state === "media-hover"}
            cursor={video.name === "broll-street.mov" && state === "media-hover" ? "grab" : undefined}
          />
        ))}
        <FileRow name="interview.srt" extension="SRT" size="2 KB" badge="srt" />
        <FileRow name="notes.txt" extension="TXT" size="1 KB" badge="other" />
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
  );
}

/// One entry on its own: a title, a caption and the row in a small panel.
function Entry(props: { title: string; caption: string; children: Child }): Markup {
  return (
    <figure>
      <h3 class="text-sm leading-5 font-semibold">{props.title}</h3>
      <p class="mt-0.5 text-xs leading-4 text-muted">{props.caption}</p>
      <ul class="mt-2 overflow-hidden rounded-[10px] bg-panel outline-1 -outline-offset-1 outline-border">
        {props.children}
      </ul>
    </figure>
  );
}

function FolderRow({ name, expanded = false, root = false, standalone = false }: {
  name: string;
  expanded?: boolean;
  root?: boolean;
  standalone?: boolean;
}): Markup {
  return (
    <li class={`flex h-[38px] cursor-default items-center gap-2 pr-3 ${root || standalone ? "pl-3" : "pl-7"}`}>
      <span aria-hidden="true" class="w-3.5 text-[13px] text-muted">{expanded ? "▾" : "▸"}</span>
      <span class={`font-mono text-sm ${root ? "font-semibold" : "text-subtle"}`}>{name}</span>
    </li>
  );
}

function FileRow({
  name,
  extension,
  size,
  badge = "video",
  active = false,
  highlighted = false,
  standalone = false,
  cursor,
}: {
  name: string;
  extension: string;
  badge?: BadgeKind;
  size?: string;
  active?: boolean;
  highlighted?: boolean;
  /// Shown on its own rather than indented in the tree.
  standalone?: boolean;
  cursor?: CursorKind;
}): Markup {
  const background = highlighted ? "bg-hover" : active ? "bg-active-timeline" : "";
  const indent = standalone ? "pl-3" : active ? "pl-[26px]" : "pl-[42px]";
  const pointer = active || badge === "other" ? "cursor-default" : "cursor-grab";
  return (
    <li class={`relative flex h-[38px] items-center gap-2 pr-3 ${indent} ${pointer} ${background}`}>
      <span
        class={`flex h-[18px] w-[38px] shrink-0 items-center justify-center rounded-[3px] border text-[11px] font-semibold ${
          active ? "border-accent-border text-accent" : BADGE_CLASS[badge]
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

// Cursor illustrations are icons; the panels and rows use HTML layout.
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

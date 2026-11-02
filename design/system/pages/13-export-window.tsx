import { Checkbox } from "../components/panel.tsx";
import { Figure } from "../components/figure.tsx";
import { Window } from "../components/window.tsx";
import { TRACK_KIND_COLOR } from "../theme.ts";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

const TEXT_TRACKS = [
  { name: "Subtitles EN", checked: true },
  { name: "Subtitles ZH", checked: true },
  { name: "Titles", checked: false },
];
const UNCHECKED_TRACKS = TEXT_TRACKS.map((track) => ({ ...track, checked: false }));

export default function ExportWindowPage(): Markup {
  return (
    <main class="grid w-[1240px] grid-cols-[560px_560px] gap-x-10 gap-y-[38px] bg-sheet px-10 pt-7 pb-[60px] font-sans">
      <Figure title="Idle" caption="Before export; no text track is checked, so no .srt file is saved">
        <ExportBody>
          <BitrateRow>
            <ValueField value="8" />
          </BitrateRow>
          <ErrorSlot />
          <TrackList tracks={UNCHECKED_TRACKS} />
          <Help class="mt-[15px]" />
          <ExportButton />
        </ExportBody>
      </Figure>
      <Figure title="Running" caption="Window cannot be closed; the text track list is locked">
        <ExportBody>
          <BitrateRow>
            <span>8</span>
          </BitrateRow>
          <ErrorSlot />
          <TrackList tracks={TEXT_TRACKS} locked />
          <Help class="mt-[15px]" />
          <ExportButton disabled />
          <p class="mt-[19px] text-base leading-none">Exporting: 42% · 227 / 540 frames</p>
          <div class="mt-[11px] h-2 w-[512px] bg-muted">
            <div class="h-2 w-[215.04px] bg-accent" />
          </div>
          <button type="button" class="mt-2 h-10 w-[115.2px] rounded-md bg-muted text-base leading-none">
            Stop export
          </button>
        </ExportBody>
      </Figure>
      <Figure title="Complete" caption="After a finished export">
        <ExportBody>
          <BitrateRow>
            <ValueField value="8" />
          </BitrateRow>
          <ErrorSlot />
          <TrackList tracks={UNCHECKED_TRACKS} />
          <Help class="mt-[15px]" />
          <ExportButton />
          <p class="mt-[19px] text-base leading-none">Export complete.</p>
        </ExportBody>
      </Figure>
      <Figure title="Invalid bitrate" caption="Export disabled">
        <ExportBody>
          <BitrateRow>
            <ValueField value="0" caret />
          </BitrateRow>
          <ErrorSlot message="Enter a bitrate between 0.1 and 1000 Mbps." />
          <TrackList tracks={UNCHECKED_TRACKS} />
          <Help class="mt-[15px]" />
          <ExportButton disabled />
        </ExportBody>
      </Figure>
      <Figure title="Text tracks checked" caption="Each checked text track is saved as its own .srt file next to the MP4">
        <ExportBody>
          <BitrateRow>
            <ValueField value="8" />
          </BitrateRow>
          <ErrorSlot />
          <TrackList tracks={TEXT_TRACKS} />
          <Help class="mt-[15px]" />
          <ExportButton />
        </ExportBody>
      </Figure>
      <Figure title="No text tracks" caption="The list is empty, so no .srt file is saved">
        <ExportBody>
          <BitrateRow>
            <ValueField value="8" />
          </BitrateRow>
          <ErrorSlot />
          <TrackList tracks={[]} />
          <Help class="mt-[15px]" />
          <ExportButton />
        </ExportBody>
      </Figure>
    </main>
  );
}

/// The shared top of the export window: title, file name and format summary.
function ExportBody(props: { children: Child }): Markup {
  return (
    <Window width={560} height={784}>
      <div class="px-6 pt-[26px]">
        <h3 class="text-xl leading-none">Export timeline</h3>
        <p class="mt-[21px] text-base leading-none">edit.timeline</p>
        <p class="mt-[22px] text-sm leading-none text-muted">1920 × 1080 · 30/1 fps · MP4 / H.264</p>
        {props.children}
      </div>
    </Window>
  );
}

function BitrateRow(props: { children: Child }): Markup {
  return (
    <div class="mt-[19px] flex h-[54px] items-center text-base leading-none">
      <span class="w-[163.7px] shrink-0">Video bitrate (Mbps)</span>
      {props.children}
    </div>
  );
}

/// The text tracks of the timeline, one checkbox row each.
/// The line under the bitrate field, always taking its space so a message does not move what follows.
function ErrorSlot(props: { message?: string }): Markup {
  return <p class="mt-4 mb-2 h-5 text-sm leading-5 text-error">{props.message}</p>;
}

function TrackList(props: { tracks: { name: string; checked: boolean }[]; locked?: boolean }): Markup {
  const color = TRACK_KIND_COLOR.T;
  return (
    <div class={`mt-1 flex flex-col ${props.locked ? "opacity-50" : ""}`}>
      <p class="mb-1 text-base leading-none">Export subtitles</p>
      {props.tracks.length === 0
        ? <p class="h-9 text-sm leading-9 text-muted">No text tracks in this timeline.</p>
        : props.tracks.map((track) => (
          <div class="flex h-9 items-center gap-3 text-base leading-none">
            <Checkbox checked={track.checked} />
            <span
              class="flex size-4 items-center justify-center rounded-[4px] text-[10px] leading-none font-bold"
              style={`background:${color.fill};color:${color.text}`}
            >
              T
            </span>
            <span>{track.name}</span>
          </div>
        ))}
    </div>
  );
}

function ValueField(props: { value: string; caret?: boolean }): Markup {
  return (
    <div class="flex h-[54px] w-[140px] items-center rounded-lg border border-border bg-surface px-[15px] font-mono text-base leading-none">
      <span>{props.value}</span>
      {props.caret ? <span class="ml-[0.7px] h-5 w-[1.5px] bg-accent" /> : null}
    </div>
  );
}

function Help(props: { class: string }): Markup {
  return (
    <p class={`text-sm leading-5 text-muted ${props.class}`}>
      Exports the timeline as it was when this window opened. Existing files are<br />not replaced.
    </p>
  );
}

function ExportButton(props: { disabled?: boolean }): Markup {
  return (
    <button
      type="button"
      class={`mt-[17px] h-10 rounded-md px-4 text-base leading-none text-[#080809] ${
        props.disabled ? "bg-muted" : "bg-accent"
      }`}
    >
      Export MP4…
    </button>
  );
}

import { Window } from "../components/window.tsx";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

export default function ExportWindowPage(): Markup {
  return (
    <main class="grid w-[1240px] grid-cols-[560px_560px] gap-x-10 gap-y-[38px] bg-[#f4f4f6] px-10 pt-7 pb-[60px] font-sans">
      <Figure title="Idle" caption="Before export">
        <ExportBody>
          <BitrateRow>
            <ValueField value="8" />
          </BitrateRow>
          <Help class="mt-[15px]" />
          <ExportButton />
        </ExportBody>
      </Figure>
      <Figure title="Running" caption="Window cannot be closed">
        <ExportBody>
          <BitrateRow>
            <span>8</span>
          </BitrateRow>
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
          <p class="mt-4 text-sm leading-5 text-error">Enter a bitrate between 0.1 and 1000 Mbps.</p>
          <Help class="mt-[15px]" />
          <ExportButton disabled />
        </ExportBody>
      </Figure>
    </main>
  );
}

function Figure(props: { title: string; caption: string; children: Child }): Markup {
  return (
    <figure>
      <h2 class="text-[15px] leading-5 font-semibold text-[#1b1b1e]">{props.title}</h2>
      <p class="mt-[3px] text-xs leading-4 text-[#55555e]">{props.caption}</p>
      <div class="mt-3">{props.children}</div>
    </figure>
  );
}

/// The shared top of the export window: title, file name and format summary.
function ExportBody(props: { children: Child }): Markup {
  return (
    <Window width={560} height={548}>
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

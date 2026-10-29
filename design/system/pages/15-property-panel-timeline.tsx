import { PanelCard, PanelHeader, PanelTabs, type PanelTrackKind, TrackChip, TrackLetter } from "../components/panel.tsx";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

export default function PropertyPanelTimelinePage(): Markup {
  return (
    <main class="w-[688px] bg-[#f4f4f6] px-12 pt-[38px] pb-[60px] font-sans text-[#1b1b1e]">
      <h1 class="text-2xl leading-8 font-bold">Property Panel · Timeline</h1>
      <p class="mt-2 text-sm leading-5 text-[#55555e]">
        The timeline panel and its two tabs. Other panels are on the property panel page.
      </p>
      <h2 class="mt-[43px] text-lg leading-6 font-semibold">Timeline file</h2>
      <p class="mt-1 text-xs leading-4 text-[#55555e]">Active timeline, nothing selected</p>
      <div class="mt-5 grid grid-cols-[280px_280px] gap-x-10">
        <figure>
          <figcaption class="text-xs leading-4 font-semibold text-[#55555e]">Properties tab</figcaption>
          <div class="mt-2.5">
            <PropertiesTab />
          </div>
        </figure>
        <figure>
          <figcaption class="text-xs leading-4 font-semibold text-[#55555e]">History tab</figcaption>
          <div class="mt-2.5">
            <HistoryTab />
          </div>
        </figure>
      </div>
    </main>
  );
}

function PanelTop(props: { active: 0 | 1 }): Markup {
  return (
    <>
      <PanelHeader icon={{ type: "timeline" }} title="edit.timeline" subtitle="timelines/edit.timeline" />
      <PanelTabs tabs={["Properties", "History"]} active={props.active} />
    </>
  );
}

function PropertiesTab(): Markup {
  return (
    <PanelCard height={555}>
      <PanelTop active={0} />
      <div class="px-4 pt-[15.5px]">
        <Label>DURATION</Label>
        <p class="mt-[11px] font-mono text-2xl leading-none font-semibold">0:18</p>
        <Divider class="mt-[11.4px]" />
        <Label class="mt-[13.5px]" action="Change…">FORMAT</Label>
        <Rows>
          <Pair name="Frame rate" value="30 fps" />
          <Pair name="Resolution" value="1920 × 1080" />
          <Pair name="Audio rate" value="48 kHz" />
        </Rows>
        <Divider class="mt-2" />
        <Label class="mt-[13.5px]">CONTENTS</Label>
        <Rows>
          <Pair name="Clips" value="9" />
          <div class="flex h-7 items-center justify-between text-[13px] leading-none">
            <span class="text-muted">Tracks</span>
            <span class="flex flex-row-reverse gap-[7px]">
              <TrackChip kind="A" label="A 1" />
              <TrackChip kind="V" label="V 1" />
              <TrackChip kind="T" label="T 1" />
            </span>
          </div>
        </Rows>
        <Divider class="mt-2" />
        <Label class="mt-[13.5px]">LOCATION</Label>
        <p class="mt-[7.8px] font-mono text-xs leading-[18px] text-soft">
          /Users/me/market-vlog/<br />timelines/edit.timeline
        </p>
      </div>
    </PanelCard>
  );
}

function Label(props: { class?: string; action?: string; children: Child }): Markup {
  return (
    <div class={`flex items-baseline justify-between text-[11px] leading-none ${props.class ?? ""}`}>
      <span class="font-semibold tracking-[0.6px] text-muted">{props.children}</span>
      {props.action === undefined ? null : <span class="text-xs text-accent">{props.action}</span>}
    </div>
  );
}

function Divider(props: { class: string }): Markup {
  return <div class={`h-px bg-divider ${props.class}`} />;
}

function Rows(props: { children: Child }): Markup {
  return <div class="mt-[10.5px]">{props.children}</div>;
}

function Pair(props: { name: string; value: string }): Markup {
  return (
    <div class="flex h-7 items-center justify-between text-[13px] leading-none">
      <span class="text-muted">{props.name}</span>
      <span class="font-mono">{props.value}</span>
    </div>
  );
}

function HistoryTab(): Markup {
  return (
    <PanelCard height={555}>
      <PanelTop active={1} />
      <div class="absolute top-[111px] left-[19px] h-[356px] w-0.5 bg-border" />
      <Entry cy={111} kind="A" title="Audio 1" detail="Muted" time="10:08" dim hollow />
      <Entry cy={145} kind="T" title="Text 2" detail="Track deleted" time="10:08" dim hollow />
      <div class="absolute top-[162px] left-1.5 h-28 w-[268px] rounded-md border border-accent bg-accent-background" />
      <Entry cy={179} kind="A" title="interview" time="10:07" current />
      <span class="absolute top-[202.5px] left-[34px] text-[11px] leading-none text-muted">Gain</span>
      <span class="absolute top-[200px] left-[66px] flex h-[17px] w-[105px] items-center rounded border border-border bg-surface px-1.5 font-mono text-[10px] leading-none text-light">
        0.00 → −3.00 dB
      </span>
      <span class="absolute top-[228.5px] left-[34px] text-[11px] leading-none text-muted">Track</span>
      <span class="absolute top-[228.5px] right-5 text-[11px] leading-none text-light">Audio 1</span>
      <span class="absolute top-[246px] left-[130px] grid h-[26px] w-[130px] place-items-center rounded-md border border-border bg-surface text-xs leading-none">
        Fork from here…
      </span>
      <Entry cy={297} kind="T" title="Let's get started" detail="Font size" time="10:06" />
      <Entry cy={331} kind="T" title="Let's get started" detail="Text" time="10:06" />
      <Entry cy={365} kind="V" title="broll-cafe" detail="Split" time="10:05" hover />
      <Entry cy={399} kind="V" title="broll-cafe" detail="Trimmed out" time="10:03" />
      <Entry cy={433} kind="V" title="broll-cafe" detail="Moved" time="10:03" />
      <Entry cy={467} kind="V" title="broll-cafe" detail="Added" time="10:02" />
    </PanelCard>
  );
}

/// A history row centred on `cy` from the panel's top. `hollow` marks undone entries, `current`
/// the one the document is at.
function Entry(props: {
  cy: number;
  kind: PanelTrackKind;
  title: string;
  detail?: string;
  time: string;
  dim?: boolean;
  hollow?: boolean;
  current?: boolean;
  hover?: boolean;
}): Markup {
  return (
    <div
      class={`absolute left-1.5 flex h-8 w-[268px] items-center rounded-md ${props.hover ? "bg-hover" : ""} ${
        props.dim ? "opacity-45" : ""
      }`}
      style={`top:${props.cy - 16}px`}
    >
      {props.current ? <span class="absolute left-[9px] size-2.5 rounded-full bg-accent" /> : (
        <span
          class={`absolute left-2.5 size-2 rounded-full ${props.hollow ? "border border-rail bg-panel" : "bg-rail"}`}
        />
      )}
      <TrackLetter kind={props.kind} class="ml-[22px]" />
      <span class={`ml-2 text-xs leading-none ${props.current ? "font-semibold" : ""}`}>{props.title}</span>
      {props.detail === undefined ? null : <span class="ml-2 text-[11px] leading-none text-muted">{props.detail}</span>}
      <span class="absolute right-2 font-mono text-[10px] leading-none text-dim">{props.time}</span>
    </div>
  );
}

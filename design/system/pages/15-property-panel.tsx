import {
  ClipSubtitle,
  Divider,
  Field,
  MuteToggle,
  Pair,
  PanelCard,
  PanelHeader,
  SectionLabel,
  Slider,
  TimingCell,
} from "../components/panel.tsx";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

const COLUMN = "grid grid-cols-[280px_280px_280px_280px] gap-x-10";

export default function PropertyPanelPage(): Markup {
  return (
    <main class="w-[1336px] bg-[#f4f4f6] px-12 pt-9 pb-[60px] font-sans text-[#1b1b1e]">
      <h1 class="text-2xl leading-8 font-bold">Property Panel</h1>
      <p class="mt-2 text-sm leading-5 text-[#55555e]">One panel type per row. Compare its variants side by side.</p>
      <Legend />
      <Section title="Header" caption="Shared by every property panel: 12px around the icon, then a divider." first>
        <Variant label="Clip">
          <PanelCard height={56}>
            <PanelHeader
              bottomDivider={false}
              icon={{ type: "track", kind: "V" }}
              title="broll-street.mov"
              subtitle={<ClipSubtitle clipType="Video clip" track="Video 1" kind="V" />}
            />
          </PanelCard>
        </Variant>
        <Variant label="Clip on a locked track">
          <PanelCard height={56}>
            <PanelHeader
              locked
              bottomDivider={false}
              icon={{ type: "locked" }}
              title="broll-street.mov"
              subtitle={<ClipSubtitle clipType="Video clip" track="Video 1" kind="V" locked />}
            />
          </PanelCard>
        </Variant>
        <Variant label="File">
          <PanelCard height={56}>
            <PanelHeader
              bottomDivider={false}
              icon={{ type: "file", label: "MP4", labelColor: "#8fb9dd", border: "#355b78" }}
              title="interview.mp4"
              subtitle="Video file"
            />
          </PanelCard>
        </Variant>
        <Variant label="Timeline">
          <PanelCard height={56}>
            <PanelHeader
              bottomDivider={false}
              icon={{ type: "timeline" }}
              title="edit.timeline"
              subtitle="timelines/edit.timeline"
            />
          </PanelCard>
        </Variant>
      </Section>

      <Section title="Video clip" caption="Transform, gain and mute editable; timing read-only">
        <Variant label="Default">
          <VideoClipPanel />
        </Variant>
        <Variant label="Locked track · read-only">
          <VideoClipPanel locked />
        </Variant>
        <Variant label="Shared AUDIO section · unmuted">
          <AudioCard />
          <Notes />
        </Variant>
        <Variant label="Shared AUDIO section · muted">
          <AudioCard muted />
        </Variant>
      </Section>

      <Section title="Audio clip" caption="Gain and mute editable">
        <Variant label="Default">
          <AudioClipPanel />
        </Variant>
        <Variant label="Locked track · read-only">
          <AudioClipPanel locked />
        </Variant>
      </Section>

      <Section title="Video file" caption="Selected in the Explorer">
        <Variant label="Default">
          <FilePanel
            icon={{ type: "file", label: "MP4", labelColor: "#8fb9dd", border: "#355b78" }}
            title="interview.mp4"
            subtitle="Video file"
            format="MP4"
            size="1.2 GB"
            path="media/interview.mp4"
            hint={["Drag onto a video track to add it", "to the timeline."]}
          />
        </Variant>
      </Section>

      <Section title="Audio file" caption="Selected in the Explorer">
        <Variant label="Default">
          <FilePanel
            icon={{ type: "file", label: "M4A", labelColor: "#7fd0ae", border: "#32725a" }}
            title="voice-over.m4a"
            subtitle="Audio file"
            format="M4A"
            size="12.4 MB"
            path="media/voice-over.m4a"
            hint={["Drag onto an audio track to add it", "to the timeline."]}
          />
        </Variant>
      </Section>

      <Section title="Image file" caption="Selected in the Explorer">
        <Variant label="Default">
          <FilePanel
            icon={{ type: "file", label: "PNG", labelColor: "#c3a9e8", border: "#665184" }}
            title="logo.png"
            subtitle="Image file"
            format="PNG"
            size="84 KB"
            path="media/logo.png"
          />
        </Variant>
      </Section>

      <Section title="Subtitle file" caption=".srt parsed into cues">
        <Variant label="Default">
          <SubtitlePanel />
        </Variant>
      </Section>

      <Section title="Nothing selected" caption="No timeline open">
        <Variant label="Empty state">
          <PanelCard height={555}>
            <div class="absolute top-[223.5px] left-[116px] grid size-12 place-items-center rounded-xl border border-border bg-[#131316]">
              <svg
                width="20"
                height="22"
                viewBox="0 0 20 22"
                fill="none"
                stroke="#777780"
                stroke-width="1.5"
                stroke-linejoin="round"
              >
                <path d="M1.5 1.5 v17 l4.5 -4 l2.8 6.2 l2.6 -1.2 l-2.8 -6 l6 -0.2Z" />
              </svg>
            </div>
            <p class="absolute inset-x-0 top-[286.5px] text-center text-sm leading-none font-semibold">
              Nothing selected
            </p>
            <p class="absolute inset-x-0 top-[306.5px] text-center text-xs leading-[17px] text-muted">
              Select a clip on the timeline or a<br />file in the Explorer.
            </p>
          </PanelCard>
        </Variant>
      </Section>
    </main>
  );
}

/// The three kinds of cell used in panels: editable input, read-only value and action.
function Legend(): Markup {
  return (
    <div class="mt-3 flex items-center gap-x-6 text-[13px] leading-none text-[#55555e]">
      <div class="flex items-center gap-2.5">
        <span class="flex h-[30px] w-[96px] items-center rounded-md border border-border bg-surface px-[9px] text-text">
          <span class="w-[15.7px] text-xs font-semibold text-muted">X</span>
          <span class="font-mono">0.50</span>
        </span>
        Editable input
      </div>
      <div class="flex items-center gap-2.5">
        <span class="flex h-[30px] w-24 items-center rounded-md bg-[#131316] px-2.5 font-mono text-text">0:08.00</span>
        Read-only value
      </div>
      <div class="flex items-center gap-2.5">
        <span class="text-accent">Change…</span>Action
      </div>
    </div>
  );
}

/// A titled row of up to four panel variants. Rows are 684px apart.
function Section(props: { title: string; caption: string; first?: boolean; children: Child }): Markup {
  return (
    <section class={props.first ? "mt-[27px]" : "mt-[42px]"}>
      <h2 class="text-lg leading-6 font-semibold">{props.title}</h2>
      <p class="mt-2 text-xs leading-4 text-[#55555e]">{props.caption}</p>
      <div class={`mt-[6px] ${COLUMN}`}>{props.children}</div>
    </section>
  );
}

function Variant(props: { label: string; children: Child }): Markup {
  return (
    <div>
      <p class="text-xs leading-4 font-semibold text-[#55555e]">{props.label}</p>
      <div class="mt-4">{props.children}</div>
    </div>
  );
}

function Notes(): Markup {
  return (
    <div class="mt-6 text-xs leading-5 text-[#55555e]">
      <p>One component, used by both panels.</p>
      <p>Gain dB field: −96 to +12 dB.</p>
      <p>Locked tracks keep the layout, read-only.</p>
    </div>
  );
}

function ClipTop(props: { audio?: boolean; locked?: boolean; title: string }): Markup {
  return (
    <PanelHeader
      locked={props.locked}
      icon={props.locked ? { type: "locked" } : { type: "track", kind: props.audio ? "A" : "V" }}
      title={props.title}
      subtitle={
        <ClipSubtitle
          clipType={props.audio ? "Audio clip" : "Video clip"}
          track={props.audio ? "Audio 1" : "Video 1"}
          kind={props.audio ? "A" : "V"}
          locked={props.locked}
        />
      }
      bottomDivider
    />
  );
}

function Timing(props: { values: [string, string][]; locked?: boolean }): Markup {
  const names = ["Start", "Duration", "Source in", "Source out"];
  return (
    <div class="mt-[10.5px] grid grid-cols-2 gap-2">
      {names.map((name, index) => (
        <TimingCell name={name} time={props.values[index][0]} frames={props.values[index][1]} locked={props.locked} />
      ))}
    </div>
  );
}

function VideoClipPanel(props: { locked?: boolean }): Markup {
  const { locked } = props;
  return (
    <PanelCard height={555} locked={locked}>
      <ClipTop title="broll-street.mov" locked={locked} />
      <div class="px-4 pt-[14.5px]">
        <SectionLabel>TIMING</SectionLabel>
        <Timing
          locked={locked}
          values={[["0:08.00", "240 f"], ["0:09.50", "285 f"], ["0:44.00", "1320 f"], ["0:53.50", "1605 f"]]}
        />
        <Divider class="mt-4" />
        <SectionLabel class="mt-[13.5px]" action={locked ? undefined : "Reset"}>TRANSFORM</SectionLabel>
        <p class="mt-[19px] text-[13px] leading-none text-muted">Position</p>
        <div class="mt-[4.5px] flex gap-2">
          <Field prefix="X" value="0" unit="px" width={120} locked={locked} />
          <Field prefix="Y" value="0" unit="px" width={120} locked={locked} />
        </div>
        <div class="mt-4 flex items-center text-[13px] leading-none text-muted">
          <span class="w-12">Scale</span>
          <Slider fill={52} locked={locked} />
          <span class="ml-3">
            <Field value="100" unit="%" width={84} locked={locked} />
          </span>
        </div>
        <Divider class="mt-4" />
        <SectionLabel class="mt-[13.5px]">AUDIO</SectionLabel>
        <GainRow locked={locked} class="mt-[10.5px]" />
        <div class="mt-3.5">
          <MuteToggle locked={locked} />
        </div>
      </div>
    </PanelCard>
  );
}

function GainRow(props: { locked?: boolean; class: string }): Markup {
  return (
    <div class={`flex h-[30px] items-center justify-between text-[13px] leading-none text-muted ${props.class}`}>
      Gain
      <Field value="0.00" unit="dB" width={84} locked={props.locked} />
    </div>
  );
}

/// The AUDIO section on its own, as shared by the video and audio clip panels.
function AudioCard(props: { muted?: boolean }): Markup {
  return (
    <PanelCard height={140}>
      <div class="px-4 pt-[30.5px]">
        <SectionLabel>AUDIO</SectionLabel>
        <GainRow class="mt-[10.5px]" />
        <div class="mt-3.5">
          <MuteToggle muted={props.muted} />
        </div>
      </div>
    </PanelCard>
  );
}

function AudioClipPanel(props: { locked?: boolean }): Markup {
  const { locked } = props;
  return (
    <PanelCard height={555} locked={locked}>
      <ClipTop audio title="voice-over.m4a" locked={locked} />
      <div class="px-4 pt-[14.5px]">
        <SectionLabel>TIMING</SectionLabel>
        <Timing
          locked={locked}
          values={[["0:00.00", "0 f"], ["0:15.00", "450 f"], ["0:00.00", "0 f"], ["0:15.00", "450 f"]]}
        />
        <Divider class="mt-4" />
        <SectionLabel class="mt-[13.5px]">AUDIO</SectionLabel>
        <GainRow locked={locked} class="mt-[10.5px]" />
        <div class="mt-3.5">
          <MuteToggle locked={locked} />
        </div>
      </div>
    </PanelCard>
  );
}

function FilePanel(props: {
  icon: { type: "file"; label: string; labelColor: string; border: string };
  title: string;
  subtitle: string;
  format: string;
  size: string;
  path: string;
  hint?: [string, string];
}): Markup {
  return (
    <PanelCard height={555}>
      <PanelHeader icon={props.icon} title={props.title} subtitle={props.subtitle} bottomDivider />
      <div class="px-4 pt-[14.5px]">
        <SectionLabel>FILE</SectionLabel>
        <div class="mt-[10.5px]">
          <Pair name="Format" value={props.format} />
          <Pair name="Size" value={props.size} />
        </div>
        <Divider class="mt-2" />
        <SectionLabel class="mt-[13.5px]">LOCATION</SectionLabel>
        <p class="mt-[7.8px] font-mono text-xs leading-[18px] text-soft">
          /Users/me/market-vlog/<br />
          {props.path}
        </p>
        {props.hint === undefined ? null : (
          <>
            <Divider class="mt-[16.7px]" />
            <p class="mt-[16.5px] text-xs leading-[17px] text-muted">
              {props.hint[0]}
              <br />
              {props.hint[1]}
            </p>
          </>
        )}
      </div>
    </PanelCard>
  );
}

const CUES: [string, string, string][] = [
  ["01", "0:00.5 → 0:03.2", "Welcome back to the channel"],
  ["02", "0:03.4 → 0:06.8", "Today we're walking through"],
  ["03", "0:07.0 → 0:09.6", "the old town market"],
  ["04", "0:10.1 → 0:13.0", "Let's get started"],
  ["05", "0:13.4 → 0:16.0", "First stop: coffee"],
];

function SubtitlePanel(): Markup {
  return (
    <PanelCard height={555}>
      <PanelHeader
        icon={{ type: "file", label: "SRT", labelColor: "#8b8b94", border: "#46464e" }}
        title="interview.srt"
        subtitle="Subtitle file"
        bottomDivider
      />
      <div class="px-4 pt-[14.5px]">
        <SectionLabel>CUES</SectionLabel>
        <ol class="mt-2.5">
          {CUES.map(([number, time, text], index) => (
            <li class="relative pt-[14.8px] pb-[11.5px] pl-[26px]">
              <span class="absolute top-[14.8px] left-0 font-mono text-[11px] leading-none text-dim">{number}</span>
              <span class="block font-mono text-[11px] leading-none text-muted">{time}</span>
              <span class="mt-[6.7px] block text-[13px] leading-none">{text}</span>
              {index === CUES.length - 1 ? null : <span class="absolute right-0 bottom-0 left-[26px] h-px bg-divider" />}
            </li>
          ))}
        </ol>
        <p class="mt-px text-xs leading-[17px] text-muted">
          Drag onto a text track to add one<br />text clip per cue.
        </p>
      </div>
    </PanelCard>
  );
}

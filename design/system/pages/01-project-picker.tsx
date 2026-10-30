import { AppIcon } from "../components/icons/app-icon.tsx";
import { Window } from "../components/window.tsx";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

export default function ProjectPickerPage(): Markup {
  return (
    <main class="w-[1256px] bg-[#f4f4f6] px-12 pt-10 pb-12 font-sans text-[#1b1b1e]">
      <h1 class="text-2xl leading-8 font-bold">Project picker</h1>
      <p class="mt-1 text-sm leading-5 text-[#55555e]">
        Shown at launch when there is no project to restore. Window 560 × 380.
      </p>
      <div class="mt-[38px] grid grid-cols-[560px_560px] gap-x-10 gap-y-[34px]">
        <Figure title="Ready" caption="First launch, no project opened before">
          <Picker>
            <OpenButton />
          </Picker>
        </Figure>
        <Figure title="Choosing" caption="Folder dialog open; button disabled">
          <Picker>
            <ChoosingButton />
          </Picker>
        </Figure>
        <Figure title="Last project missing" caption="Saved project folder no longer exists">
          <Picker raised>
            <Notice tone="warning" title="Last project folder not found" detail="~/Movies/old-vlog" />
            <OpenButton class="mt-5" />
          </Picker>
        </Figure>
        <Figure title="Error" caption="The chosen folder could not be opened">
          <Picker raised>
            <Notice tone="error" title="Could not open project folder" detail="Permission denied (os error 13)" />
            <OpenButton class="mt-5" />
          </Picker>
        </Figure>
      </div>
    </main>
  );
}

function Figure(props: { title: string; caption: string; children: Child }): Markup {
  return (
    <figure>
      <h2 class="text-sm leading-5 font-semibold">{props.title}</h2>
      <p class="mt-0.5 text-xs leading-4 text-[#55555e]">{props.caption}</p>
      <div class="mt-3">{props.children}</div>
    </figure>
  );
}

/// The launch window. `raised` moves the content up to make room for a notice.
function Picker(props: { raised?: boolean; children: Child }): Markup {
  return (
    <Window width={560} height={408}>
      <div class="relative h-[380px]">
        <div class="absolute top-[-32px] left-[30px] h-[300px] w-[500px] rounded-[50%] bg-[radial-gradient(closest-side,rgb(240_183_94/0.10),transparent)]" />
        <div class={`relative flex flex-col items-center ${props.raised ? "pt-[42.4px]" : "pt-[52.4px]"}`}>
          <AppIcon size={73.14} />
          <h3 class="mt-[17.5px] text-[26px] leading-none font-bold">OpenCut</h3>
          <p class="mt-[13px] text-sm leading-none text-muted">
            Open any folder as a project. Your media stays where it is.
          </p>
          {props.children}
        </div>
      </div>
    </Window>
  );
}

const FOLDER_PATH =
  "M0.5 1.5 a1.5 1.5 0 0 1 1.5 -1.5 h4 l1.8 2 h6.2 a1.5 1.5 0 0 1 1.5 1.5 v7 a1.5 1.5 0 0 1 -1.5 1.5 h-12 a1.5 1.5 0 0 1 -1.5 -1.5 z";

function OpenButton(props: { class?: string }): Markup {
  return (
    <button
      type="button"
      class={`relative mt-[31px] h-10 w-[147.24px] rounded-lg bg-accent text-[15px] leading-none font-semibold text-[#17120a] ${
        props.class ?? ""
      }`}
    >
      <svg class="absolute top-[14px] left-[13px]" width="16" height="14" viewBox="-1 -1 16 14">
        <path
          d={FOLDER_PATH}
          transform="translate(1 0)"
          fill="none"
          stroke="#17120a"
          stroke-width="1.6"
          stroke-linejoin="round"
        />
      </svg>
      <span class="absolute top-[13px] left-10">Open Folder…</span>
    </button>
  );
}

function ChoosingButton(): Markup {
  return (
    <button
      type="button"
      class="relative mt-[31px] h-10 w-[171.36px] rounded-lg border border-border bg-surface text-[15px] leading-none font-semibold text-muted"
    >
      <svg class="absolute top-[12px] left-[13.5px]" width="14" height="14" viewBox="-7 -7 14 14">
        <circle r="6" fill="none" stroke="#777780" stroke-opacity="0.3" stroke-width="2" />
        <path d="M0 -6 a6 6 0 0 1 6 6" fill="none" stroke="#777780" stroke-width="2" stroke-linecap="round" />
      </svg>
      <span class="absolute top-[12px] left-[38.5px]">Choosing folder…</span>
    </button>
  );
}

function Notice(props: { tone: "warning" | "error"; title: string; detail: string }): Markup {
  const warning = props.tone === "warning";
  const color = warning ? "#f0b75e" : "#ff8b8b";
  return (
    <div
      class={`relative mt-[31px] h-14 w-[432px] rounded-lg border ${
        warning ? "border-[#4a3a20] bg-[#1c170f]" : "border-[#5a2a2c] bg-[#1f1213]"
      }`}
    >
      <svg class="absolute top-[10px] left-[10px]" width="16" height="16" viewBox="-8 -8 16 16">
        <circle r="7" fill="none" stroke={color} stroke-width="1.6" />
        <path d="M0 -3.5 v4" stroke={color} stroke-width="1.6" stroke-linecap="round" />
        <circle cy="3.6" r="0.9" fill={color} />
      </svg>
      <p class="absolute top-[11.5px] left-[33px] text-[13px] leading-none font-semibold" style={`color:${color}`}>
        {props.title}
      </p>
      <p class="absolute top-[31.3px] left-[33px] font-mono text-xs leading-none text-soft">{props.detail}</p>
    </div>
  );
}

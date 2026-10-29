import { Scene } from "../components/scene.tsx";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

export default function PreviewPanelPage(): Markup {
  return (
    <main class="w-[1240px] bg-[#f4f4f6] px-12 pt-[38px] pb-12 font-sans text-[#1b1b1e]">
      <h1 class="text-2xl leading-8 font-bold">Preview panel</h1>
      <p class="mt-2 text-sm leading-5 text-[#55555e]">
        One preview at a time. Content and controls follow the selected source.
      </p>
      <p class="mt-2 font-mono text-[11px] leading-4 text-[#55555e]">IMAGE / AUDIO / VIDEO / ERROR / NONE</p>
      <div class="mt-[43px] grid grid-cols-[556px_556px] gap-x-8 gap-y-8">
        <Figure title="Image" caption="Still image · fit within the panel">
          <Scene width={556} class="absolute top-[23.6px] left-0" />
        </Figure>
        <Figure title="Video" caption="Media file or rendered timeline · playback controls">
          <Scene width={462.2} class="absolute top-0 left-[46.9px]" />
          <Controls progress={124.21} time="0:21 / 1:34" source="media/broll-street.mov" />
        </Figure>
        <Figure title="Audio" caption="Audio file · playback controls">
          <div class="absolute top-12 left-[250px] grid size-14 place-items-center rounded-xl border border-[#29473d] bg-[#142320]">
            <svg width="56" height="56" viewBox="250 48 56 56" class="col-start-1 row-start-1">
              <path
                d="M273 85 V64 L289 60 V81 M273 69 L289 65"
                fill="none"
                stroke="#7fd0ae"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
              <ellipse cx="269" cy="85" rx="4" ry="3" fill="#7fd0ae" />
              <ellipse cx="285" cy="81" rx="4" ry="3" fill="#7fd0ae" />
            </svg>
          </div>
          <p class="absolute inset-x-0 top-[124.5px] text-center text-lg leading-none font-semibold text-text">
            voice-over.m4a
          </p>
          <p class="absolute inset-x-0 top-[154.7px] text-center font-mono text-xs leading-none text-muted">
            media/voice-over.m4a
          </p>
          <Controls progress={222.4} time="0:06 / 0:15" source="voice-over.m4a" />
        </Figure>
        <Figure title="None" caption="Nothing selected, or the selected file has no preview">
          <svg class="absolute top-[109px] left-[256px]" width="44" height="34" viewBox="0 0 44 34">
            <rect x="0.5" y="0.5" width="43" height="33" rx="5" fill="none" stroke="#55555e" />
            <path
              d="M6 27 L17 16 L24 23 L30 17 L38 27"
              fill="none"
              stroke="#55555e"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
            <circle cx="30" cy="10" r="3" fill="#55555e" />
          </svg>
          <p class="absolute inset-x-0 top-[170px] text-center text-base leading-none text-soft">
            No preview available
          </p>
          <p class="absolute inset-x-0 top-[198.8px] text-center text-[13px] leading-none text-muted">
            Select an image, audio, video, or timeline file.
          </p>
        </Figure>
        <Figure title="Error" caption="Selected file and error string · shared by all preview sources">
          <div class="absolute top-8 left-8 grid size-10 place-items-center rounded-[10px] bg-[#2b171a]">
            <svg width="40" height="40" viewBox="32 32 40 40">
              <path
                d="M52 42 L63 61 H41 Z M52 49 V54"
                fill="none"
                stroke="#ed8585"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
              <circle cx="52" cy="58" r="1" fill="#ed8585" />
            </svg>
          </div>
          <p class="absolute top-[33.5px] left-[88px] text-lg leading-none font-semibold text-text">
            Unable to preview file
          </p>
          <p class="absolute top-[61px] left-[88px] font-mono text-xs leading-none text-soft">
            timelines/devlog 7.timeline.json
          </p>
          <div class="absolute top-[104px] left-8 h-48 w-[492px] rounded-md border border-[#463033] bg-[#151113] px-[15px] pt-[12.7px] font-mono text-[13px] leading-6 text-[#e6b3b3]">
            <p>Failed to load timeline.</p>
            <p>Could not open media file:</p>
            <p>timelines/2026-09-01 12-36-13.mp4</p>
            <p>No such file or directory.</p>
          </div>
        </Figure>
      </div>
    </main>
  );
}

/// A 556×360 preview panel under a heading. Children are positioned from the panel's top-left.
function Figure(props: { title: string; caption: string; children: Child }): Markup {
  return (
    <figure>
      <h2 class="text-sm leading-5 font-semibold">{props.title}</h2>
      <p class="mt-[3px] text-xs leading-4 text-[#55555e]">{props.caption}</p>
      <div class="relative mt-2.5 h-[360px] w-[556px] overflow-hidden rounded-[10px] border border-border bg-[#080808] shadow-[0_6px_0_#000] text-text">
        {props.children}
      </div>
    </figure>
  );
}

/// The playback bar and the row under it: label, elapsed time and source.
function Controls(props: { progress: number; time: string; source: string }): Markup {
  return (
    <>
      <div class="absolute top-[260px] h-5 w-full bg-[#303030]">
        <div class="h-5 bg-[#dba34b]" style={`width:${props.progress}px`} />
      </div>
      <span class="absolute top-[312.2px] left-6 text-base leading-none text-[#eeeeee]">Play</span>
      <span class="absolute top-[312.2px] left-20 text-base leading-none text-[#eeeeee]">{props.time}</span>
      <span class="absolute top-[314px] left-[206px] text-sm leading-none text-soft">{props.source}</span>
    </>
  );
}

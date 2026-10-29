import { PlaybackIcon, VideoPlayer } from "../components/video-player.tsx";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

export default function VideoPlayerPage(): Markup {
  return (
    <main class="w-[1240px] bg-[#f4f4f6] px-12 pt-[38px] pb-[60px] font-sans text-[#1b1b1e]">
      <h1 class="text-2xl leading-8 font-bold">Video player</h1>
      <p class="mt-2 text-sm leading-5 text-[#55555e]">
        Plays a media file or a rendered timeline. Controls sit below the picture, never over it.
      </p>
      <div class="mt-[40px] grid grid-cols-[556px_556px] gap-x-8 gap-y-[51px]">
        <Figure title="Paused" caption="Primary button plays; time and file name follow">
          <VideoPlayer width={556} icon="play" progress={117.1} time="0:21" total="1:34" file="broll-street.mov" />
        </Figure>
        <Figure title="Playing" caption="Button switches to pause">
          <VideoPlayer width={556} icon="pause" progress={267.6} time="0:48" total="1:34" file="broll-street.mov" />
        </Figure>
        <Figure
          title="Hover on scrub bar"
          caption="Track thickens, knob appears; light fill previews the seek target"
        >
          <VideoPlayer
            width={556}
            icon="play"
            progress={117.1}
            seek={272.5}
            knob="hover"
            hand={{ left: 282.5, top: 336 }}
            time="0:21"
            total="1:34"
            file="broll-street.mov"
          />
        </Figure>
        <Figure title="Scrubbing" caption="Picture and time follow the knob while dragging">
          <VideoPlayer
            width={556}
            icon="play"
            progress={351.2}
            knob="drag"
            hand={{ left: 366.2, top: 333, grab: true }}
            time="1:03"
            total="1:34"
            file="broll-street.mov"
          />
        </Figure>
        <Figure title="Ended" caption="Bar full; button replays from the start">
          <VideoPlayer width={556} icon="replay" progress={524} time="1:34" total="1:34" file="broll-street.mov" />
        </Figure>
        <Figure title="Before the first frame" caption="Picture stays black until a frame is decoded">
          <VideoPlayer
            width={556}
            icon="play"
            progress={0}
            picture={false}
            time="0:00"
            total="1:34"
            file="broll-street.mov"
          />
        </Figure>
        <Figure title="Timeline" caption="Rendered timeline with subtitles; same controls">
          <VideoPlayer
            width={556}
            icon="pause"
            progress={349.3}
            caption="Let's get started"
            time="0:12"
            total="0:18"
            file="devlog 7.timeline"
          />
        </Figure>
        <Figure title="Narrow panel" caption="File name truncates first; time and button keep their size">
          <VideoPlayer
            width={360}
            icon="play"
            progress={73.3}
            time="0:21"
            total="1:34"
            file="interview-camera-a-take-03…"
          />
        </Figure>
      </div>
      <section class="mt-[100px] border-t border-border pt-[37px]">
        <h2 class="text-xl leading-7 font-bold">Playback button</h2>
        <p class="mt-[3px] text-sm leading-5 text-[#55555e]">24px circle. The icon shows what a click does.</p>
        <table class="mt-[27px] w-[480px] rounded-[10px] border border-border bg-panel text-[13px] leading-none text-text shadow-[0_6px_0_rgb(0_0_0/0.35)]">
          <thead>
            <tr class="h-9 border-b border-divider text-[11px] font-semibold tracking-[0.6px] text-muted">
              <th class="w-20 pl-4 text-left font-semibold">ICON</th>
              <th class="w-[104px] text-left font-semibold">STATE</th>
              <th class="text-left font-semibold">WHEN</th>
            </tr>
          </thead>
          <tbody>
            <Row icon="play" state="Play" when="Paused, or before the first frame" />
            <Row icon="pause" state="Pause" when="Playing" />
            <Row icon="replay" state="Replay" when="Ended" last />
          </tbody>
        </table>
      </section>
    </main>
  );
}

function Figure(props: { title: string; caption: string; children: Child }): Markup {
  return (
    <figure>
      <h2 class="text-sm leading-5 font-semibold">{props.title}</h2>
      <p class="mt-[5px] text-xs leading-4 text-[#55555e]">{props.caption}</p>
      <div class="mt-2">{props.children}</div>
    </figure>
  );
}

function Row(props: { icon: "play" | "pause" | "replay"; state: string; when: string; last?: boolean }): Markup {
  return (
    <tr class={`h-14 ${props.last ? "" : "border-b border-divider"}`}>
      <td class="pl-4">
        <PlaybackIcon icon={props.icon} class="size-9" />
      </td>
      <td class="font-semibold">{props.state}</td>
      <td class="text-muted">{props.when}</td>
    </tr>
  );
}

import { WarningIcon } from "../components/warning-icon.tsx";
import { AudioClip, TextClip, VideoClip } from "../components/timeline-clip.tsx";
import { AUDIO_WAVEFORM, VIDEO_WAVEFORM } from "../components/waveform-samples.ts";
import type { Markup } from "@design/jsx/jsx-runtime";

export default function Page17TimelineClips(): Markup {
  return (
    <main class="w-[1240px] px-12 pt-10 pb-12 font-sans text-[#1b1b1e]">
      <h1 class="text-2xl leading-7 font-bold">Timeline Clips</h1>
      <p class="mt-2 text-sm leading-5 text-[#55555e]">Clip states in the timeline.</p>
      <section id="clips" aria-labelledby="clips-title" class="mt-[34px] border-t border-[#d9d9de] pt-9">
        <h2 id="clips-title" class="text-xl leading-7 font-bold">Clips</h2>
        <p class="mt-1 text-sm leading-5 text-[#55555e]">
          Each track kind has its own clip colour; selection turns the border amber.
        </p>
        <div class="mt-7 space-y-8">
          <div class="flex items-start gap-10">
            <ClipVariant title="Text clip">
              <TextClip width={160} name="Let's get started" />
            </ClipVariant>
            <ClipVariant title="Video clip">
              <VideoClip width={348} name="interview-a.mov" waveform={VIDEO_WAVEFORM} />
            </ClipVariant>
            <ClipVariant title="Selected">
              <VideoClip width={348} name="interview-a.mov" waveform={VIDEO_WAVEFORM} selected />
            </ClipVariant>
          </div>
          <div class="flex items-start gap-10">
            <ClipVariant title="Video clip without audio">
              <VideoClip width={168} name="broll-street.mov" />
            </ClipVariant>
            <ClipVariant title="Audio clip">
              <AudioClip width={348} name="music-bed.m4a" waveform={AUDIO_WAVEFORM} />
            </ClipVariant>
          </div>
        </div>
      </section>
      <section id="offline-clip" aria-labelledby="offline-clip-title" class="mt-12 border-t border-[#d9d9de] pt-9">
        <h2 id="offline-clip-title" class="text-xl leading-7 font-bold">Offline Clip</h2>
        <p class="mt-1 text-sm leading-5 text-[#55555e]">An offline clip keeps its place, length and name.</p>
        <div class="mt-7 grid grid-cols-2 gap-8">
          <div>
            <h3 class="text-sm leading-5 font-semibold">Default</h3>
            <OfflineClipStrip />
          </div>
        </div>
        <div class="mt-7 space-y-1 text-[13px] leading-[18px] text-[#55555e]">
          <p>Dark red fill, dashed error border and a warning icon mark the offline clip.</p>
          <p>Selecting it shows Relink… in its property panel.</p>
        </div>
      </section>
    </main>
  );
}

function OfflineClipStrip({ contextMenu = false }: { contextMenu?: boolean }): Markup {
  return (
    <div class="mt-3 flex h-20 w-full gap-0.5 rounded-lg bg-panel p-2 text-xs font-semibold text-text">
      <div class="flex min-w-0 flex-1 items-end rounded-md border border-[#395d85] bg-[#294d75] px-[7px] pb-[7px]">
        <span class="truncate">broll-street.mov</span>
      </div>
      <div class="relative flex w-[260px] shrink-0 flex-col justify-between rounded-md border border-dashed border-error/70 bg-[#2a1a1c] p-[7px]">
        <span class="text-[11px] leading-4 text-error">Media offline</span>
        <div class="flex items-center gap-1.5 leading-4">
          <svg aria-hidden="true" class="size-3.5 shrink-0" viewBox="0 0 14 14">
            <WarningIcon x={0} y={0} />
          </svg>
          <span>2026-08-20 18-36-50.mp4</span>
        </div>
        {contextMenu && (
          <div
            role="menu"
            aria-label="Offline clip context menu"
            class="absolute top-5 left-7 z-10 w-48 rounded-lg border border-border bg-menu p-1 shadow-popover"
          >
            <div role="menuitem" class="rounded px-3 py-2 text-sm leading-5 font-normal text-text">
              Relink media
            </div>
          </div>
        )}
      </div>
      <div class="flex min-w-0 flex-1 items-end rounded-md border border-[#395d85] bg-[#294d75] px-[7px] pb-[7px]">
        <span class="truncate">interview.mp4</span>
      </div>
    </div>
  );
}

function ClipVariant(props: { title: string; children: Markup }): Markup {
  return (
    <div>
      <h3 class="text-sm leading-5 font-semibold">{props.title}</h3>
      <div class="mt-3">{props.children}</div>
    </div>
  );
}

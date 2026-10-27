import { WarningIcon } from "../components/warning-icon.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

export default function Page17TimelineClips(): Markup {
  return (
    <main class="min-h-[440px] w-[1240px] bg-page px-12 pt-10 pb-12 font-sans text-text">
      <h1 class="text-2xl leading-7 font-bold">Timeline Clips</h1>
      <p class="mt-2 text-sm leading-5 text-muted">Clip states in the timeline.</p>
      <section id="offline-clip" aria-labelledby="offline-clip-title" class="mt-[34px] border-t border-border pt-9">
        <h2 id="offline-clip-title" class="text-xl leading-7 font-bold">Offline Clip</h2>
        <p class="mt-1 text-sm leading-5 text-muted">An offline clip keeps its place, length and name.</p>
        <div class="mt-7 flex h-20 w-[640px] gap-0.5 rounded-lg bg-panel p-2 text-xs font-semibold">
          <div class="flex w-[200px] shrink-0 items-end rounded-md border border-[#395d85] bg-[#294d75] px-[7px] pb-[7px]">
            broll-street.mov
          </div>
          <div class="flex w-[260px] shrink-0 flex-col justify-between rounded-md border border-dashed border-error/70 bg-[#2a1a1c] p-[7px]">
            <span class="text-[11px] leading-4 text-error">Media offline</span>
            <div class="flex items-center gap-1.5 leading-4">
              <svg aria-hidden="true" class="size-3.5 shrink-0" viewBox="0 0 14 14">
                <WarningIcon x={0} y={0} />
              </svg>
              <span>2026-08-20 18-36-50.mp4</span>
            </div>
          </div>
          <div class="flex w-40 shrink-0 items-end rounded-md border border-[#395d85] bg-[#294d75] px-[7px] pb-[7px]">
            interview.mp4
          </div>
        </div>
        <div class="mt-7 space-y-1 text-[13px] leading-[18px] text-muted">
          <p>Dark red fill, dashed error border and a warning icon mark the offline clip.</p>
          <p>Selecting it shows Relink… in its property panel.</p>
        </div>
      </section>
    </main>
  );
}

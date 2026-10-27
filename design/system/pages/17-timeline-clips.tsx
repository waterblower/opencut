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
        <div class="mt-7 grid grid-cols-2 gap-8">
          <div>
            <h3 class="text-sm leading-5 font-semibold">Default</h3>
            <OfflineClipStrip />
          </div>
          <div>
            <h3 class="text-sm leading-5 font-semibold">Right-click</h3>
            <OfflineClipStrip contextMenu />
          </div>
        </div>
        <div class="mt-7 space-y-1 text-[13px] leading-[18px] text-muted">
          <p>Dark red fill, dashed error border and a warning icon mark the offline clip.</p>
          <p>Right-click the offline clip and choose Relink media.</p>
          <p>Selecting it shows Relink… in its property panel.</p>
        </div>
      </section>
    </main>
  );
}

function OfflineClipStrip({ contextMenu = false }: { contextMenu?: boolean }): Markup {
  return (
    <div class="mt-3 flex h-20 w-full gap-0.5 rounded-lg bg-panel p-2 text-xs font-semibold">
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

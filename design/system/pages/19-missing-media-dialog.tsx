import { Icon } from "../components/icon.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

export default function MissingMediaDialogPage(): Markup {
  const files = [
    { name: "2026-08-20 18-36-50.mp4", clips: "3 clips" },
    { name: "2026-08-20 18-38-56.mp4", clips: "1 clip" },
  ];

  return (
    <main class="min-h-[480px] w-[1240px] bg-page px-12 pt-10 pb-12 font-sans text-text">
      <h1 class="text-2xl leading-7 font-bold">Missing Media Dialog</h1>
      <p class="mt-2 text-sm leading-5 text-muted">Shown when opening a timeline with missing media files.</p>
      <div class="mt-10 flex items-start gap-12">
        <section
          role="dialog"
          aria-labelledby="missing-media-title"
          aria-describedby="missing-media-description"
          class="w-[560px] shrink-0 rounded-xl border border-border bg-panel p-5 shadow-popover"
        >
          <h2 id="missing-media-title" class="text-lg leading-7 font-semibold">2 media files are missing</h2>
          <p id="missing-media-description" class="mt-3 text-sm leading-5 text-muted">
            Clips stay on the timeline, shown as offline.
          </p>
          <ul class="mt-5 divide-y divide-border rounded-lg border border-border bg-surface px-3">
            {files.map((file) => (
              <li class="flex items-center gap-3 py-4">
                <Icon name="alert-triangle" color="#ff8b8b" class="shrink-0" />
                <div class="min-w-0 flex-1">
                  <p class="font-mono text-xs leading-5 break-all">{file.name}</p>
                  <p class="mt-1 text-xs text-muted">{file.clips}</p>
                </div>
                <button
                  type="button"
                  class="h-9 shrink-0 rounded-lg border border-accent bg-accent px-3 text-sm whitespace-nowrap text-[#17120a]"
                >
                  Relink media
                </button>
              </li>
            ))}
          </ul>
          <div class="mt-5 flex justify-end border-t border-border pt-4">
            <button type="button" class="h-10 rounded-lg border border-border bg-surface px-4 text-sm whitespace-nowrap">
              Continue with offline media
            </button>
          </div>
        </section>
        <aside class="space-y-2 pt-4 text-[13px] leading-5 text-subtle">
          <p>Each file is relinked separately.</p>
          <p>Relink media opens a file picker for that file.</p>
          <p>Relinking updates every clip using that file.</p>
          <p>Continue with offline media leaves unresolved files offline.</p>
        </aside>
      </div>
    </main>
  );
}

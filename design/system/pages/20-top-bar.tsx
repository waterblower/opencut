import type { Markup } from "@design/jsx/jsx-runtime";

const appIcon = "data:image/svg+xml," + encodeURIComponent(
  Deno.readTextFileSync(new URL("../../../icons/app-icon.svg", import.meta.url)),
);

export default function Page20TopBar(): Markup {
  return (
    <main class="w-[1240px] bg-page px-12 pt-10 pb-12 font-sans text-text">
      <h1 class="text-2xl leading-7 font-bold">Top bar</h1>
      <p class="mt-2 text-sm leading-5 text-muted">
        Timeline identity and workspace actions. Always available above the editor.
      </p>
      <section class="mt-6" aria-labelledby="timeline-open">
        <h2 id="timeline-open" class="text-sm leading-5 font-semibold">Timeline open</h2>
        <p class="mt-1 text-xs leading-[18px] text-muted">
          The timeline filename and autosave status sit beside the app name. All actions are available.
        </p>
        <TopBar timelineName="market-vlog.timeline" />
      </section>
      <section class="mt-8" aria-labelledby="no-timeline">
        <h2 id="no-timeline" class="text-sm leading-5 font-semibold">No timeline</h2>
        <p class="mt-1 text-xs leading-[18px] text-muted">
          Export is disabled until a timeline is open. Settings, Open Folder and Projects remain available.
        </p>
        <TopBar />
      </section>
    </main>
  );
}

function TopBar({ timelineName }: { timelineName?: string }): Markup {
  return (
    <header class="mt-2 flex h-16 items-center gap-4 rounded-lg border border-border bg-panel pr-5 pl-4 shadow-panel">
      <div class="flex shrink-0 items-center gap-2">
        <img src={appIcon} alt="" class="size-10" />
        <span class="text-xl font-bold">OpenCut</span>
      </div>
      <p class="min-w-0 flex-1 truncate font-mono text-xs text-muted">
        EDITOR · {timelineName ?? "No timeline"} · AUTOSAVED
      </p>
      <nav class="flex shrink-0 items-center gap-3" aria-label="Workspace actions">
        <ActionButton label="Settings" />
        <ActionButton label="Open Folder" wide />
        <ActionButton label="Projects" />
        <ActionButton label="Export" primary disabled={!timelineName} />
      </nav>
    </header>
  );
}

function ActionButton({ label, wide = false, primary = false, disabled = false }: {
  label: string;
  wide?: boolean;
  primary?: boolean;
  disabled?: boolean;
}): Markup {
  const glow = disabled ? "" : primary ? "drop-shadow-cyber-yellow" : "drop-shadow-cyber-cyan";
  const colors = disabled
    ? "bg-cyber-disabled-border text-cyber-disabled-text"
    : primary
    ? "bg-cyber-yellow text-cyber-ink"
    : "bg-cyber-cyan text-cyber-cyan";
  return (
    <div class={glow}>
      <button
        type="button"
        disabled={disabled}
        class={`cut-corners relative flex h-10 items-center justify-center font-mono text-[13px] font-extrabold tracking-[2px] uppercase ${
          wide ? "w-[152px]" : "w-32"
        } ${colors}`}
      >
        {(!primary || disabled) && (
          <span aria-hidden="true" class="cut-corners-inset absolute inset-px bg-panel">
            <span class={`absolute inset-0 ${disabled ? "scanlines" : "bg-cyber-cyan/6"}`} />
          </span>
        )}
        <span class={`relative ${primary ? "pr-3.5" : ""}`}>{label}</span>
        {primary && (
          <span aria-hidden="true" class="absolute top-1.5 right-2 flex -skew-x-[16deg] gap-1">
            {[0, 1, 2].map(() => <span class={`h-7 w-[3px] ${disabled ? "bg-cyber-disabled-border" : "bg-cyber-ink"}`} />)}
          </span>
        )}
      </button>
    </div>
  );
}

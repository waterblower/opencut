import { TrackHeader, TrackHeaderCard } from "../components/track-header.tsx";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

export default function TimelineTrackPage(): Markup {
  return (
    <main class="w-[1240px] px-12 pt-10 pb-12 font-sans text-[#1b1b1e]">
      <h1 class="text-2xl leading-7 font-bold">Timeline Track</h1>
      <p class="mt-2 text-sm leading-5 text-[#55555e]">Parts of a timeline track.</p>

      <section class="mt-[34px] border-t border-[#d9d9de] pt-9">
        <h2 class="text-xl leading-7 font-bold">Track header</h2>
        <p class="mt-1 text-sm leading-5 text-[#55555e]">
          Kind badge and name, then reorder and delete. State toggles below.
        </p>
        <div class="mt-7 space-y-8">
          <Row>
            <Variant title="Video track" caption="Default state">
              <TrackHeader kind="V" name="Video 1" />
            </Variant>
            <Variant title="Audio track" caption="Default state">
              <TrackHeader kind="A" name="Audio 1" />
            </Variant>
            <Variant title="Text track" caption="No speaker toggle">
              <TrackHeader kind="T" name="Text 1" />
            </Variant>
            <Variant title="Long name" caption="Truncated before the actions">
              <TrackHeader kind="V" name="Intervie…" />
            </Variant>
          </Row>
          <Row>
            <Variant title="Locked" caption="Lock turns accent">
              <TrackHeader kind="V" name="Video 1" lock="active" />
            </Variant>
            <Variant title="Hidden" caption="Eye crossed out, accent">
              <TrackHeader kind="V" name="Video 1" visibility="active" />
            </Variant>
            <Variant title="Muted" caption="Speaker crossed out, accent">
              <TrackHeader kind="A" name="Audio 1" mute="active" />
            </Variant>
            <Variant title="Locked, hidden and muted" caption="All three toggles active">
              <TrackHeader kind="V" name="Video 1" lock="active" visibility="active" mute="active" />
            </Variant>
          </Row>
          <Row>
            <Variant title="Hover: toggle" caption="Gray fill; icon brightens">
              <TrackHeader kind="V" name="Video 1" visibility="hovered" />
            </Variant>
            <Variant title="Hover: delete" caption="Icon turns red">
              <TrackHeader kind="V" name="Video 1" delete="hovered" />
            </Variant>
            <Variant title="First track" caption="Move up is disabled">
              <TrackHeader kind="T" name="Text 1" moveUp="disabled" />
            </Variant>
            <Variant title="Last track" caption="Move down is disabled">
              <TrackHeader kind="A" name="Audio 1" moveDown="disabled" />
            </Variant>
          </Row>
        </div>
      </section>
    </main>
  );
}

/// Four cards per row, 266px wide, sharing title, caption and card tracks.
function Row(props: { children: Child }): Markup {
  return (
    <div class="grid auto-cols-[266px] grid-flow-col grid-rows-[auto_auto_auto] gap-x-[26.6667px]">
      {props.children}
    </div>
  );
}

function Variant(props: { title: string; caption: string; children: Markup }): Markup {
  return (
    <div class="row-span-3 grid grid-rows-subgrid">
      <h3 class="text-sm leading-5 font-semibold">{props.title}</h3>
      <p class="mt-0.5 text-xs leading-4 text-[#55555e]">{props.caption}</p>
      <div class="mt-3">
        <TrackHeaderCard>{props.children}</TrackHeaderCard>
      </div>
    </div>
  );
}

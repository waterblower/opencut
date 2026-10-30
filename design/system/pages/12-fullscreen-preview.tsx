import { Scene } from "../components/scene.tsx";
import { Subtitle } from "../components/subtitle.tsx";
import { Window } from "../components/window.tsx";
import type { Markup } from "@design/jsx/jsx-runtime";

export default function FullscreenPreviewPage(): Markup {
  return (
    <main class="h-[928px] w-[1440px]">
      <Window width={1440} height={928}>
        <div class="relative h-[900px] w-[1440px] bg-[#080808]">
          <div class="absolute top-0 left-[8.89px] h-[800px] w-[1422px]">
            <Scene width={1422.2} />
            <Subtitle text="Let's get started" size={47.4} weight={500} baseline={696.9} />
          </div>
          <div class="absolute top-[800px] h-5 w-full bg-[#303030]">
            <div class="h-5 w-[960px] bg-[#dba34b]" />
          </div>
          <span class="absolute top-[852px] left-6 text-base leading-none text-[#eeeeee]">Pause</span>
          <span class="absolute top-[852px] left-[92.8px] text-base leading-none text-[#eeeeee]">0:12 / 0:18</span>
        </div>
      </Window>
    </main>
  );
}

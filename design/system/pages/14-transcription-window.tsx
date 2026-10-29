import { CyberButton } from "../components/cyber-button.tsx";
import { Window } from "../components/window.tsx";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

export default function TranscriptionWindowPage(): Markup {
  return (
    <main class="grid w-[1320px] grid-cols-[600px_600px] gap-x-10 gap-y-9 bg-[#f4f4f6] px-10 pt-7 pb-12 font-sans">
      <Figure
        title="Ready"
        caption="Opened from Generate SRT. “MiniMax pricing” opens https://platform.minimax.cn/docs/guides/pricing in the browser"
      >
        <Body status="Generate the SRT transcription for interview.mp4" line={<Cost cost="¥0.52" audio="12:34" />}>
          <StartCancel />
        </Body>
      </Figure>
      <Figure title="Running" caption="Preparing / transcribing / saving">
        <Body status="Uploading and transcribing.." line={<p class="mt-[22px]">Elapsed: 0:47</p>}>
          <p class="mt-[18px] max-w-[470px] text-sm leading-5 text-muted">
            Cancel stops this local request. A request already sent may continue processing on the server.
          </p>
          <StartCancel disabled class="mt-[17px]" />
        </Body>
      </Figure>
      <Figure title="Complete" caption="Written to <source name>.srt in the project root">
        <Body status="Subtitles saved" line={<p class="mt-[22px]">/Users/me/market-vlog/interview.srt</p>}>
          <div class="mt-[15px]">
            <CyberButton width={193} variant="primary" label="REVEAL IN FINDER" />
          </div>
          <StartCancel class="mt-4" />
        </Body>
      </Figure>
      <Figure title="Failed" caption="Error message from the request">
        <Body status="Transcription failed">
          <p class="mt-[22px] text-sm leading-none text-error">MiniMax API key is not set. Add it under Settings.</p>
          <StartCancel class="mt-[19px]" />
        </Body>
        <p class="mt-3 max-w-[540px] text-xs leading-[18px] text-[#55555e]">
          Comment: the error is shown as the request returns it, so a missing key currently reads “missing_api_key: an API
          key is required for transcription”. The text in the window is a sample.
        </p>
      </Figure>
      <Figure
        title="Clip"
        caption="Opened from Transcribe the Clip. Writes <source stem>_<clip id>.srt in the project root; subtitle times start at the clip"
      >
        <Body
          status="Generate the SRT transcription for 0:12–1:05 of interview.mp4"
          line={<Cost cost="¥0.04" audio="0:53" />}
        >
          <StartCancel />
        </Body>
      </Figure>
    </main>
  );
}

function Figure(props: { title: string; caption: string; children: Child }): Markup {
  return (
    <figure>
      <h2 class="text-[15px] leading-5 font-semibold text-[#1b1b1e]">{props.title}</h2>
      <p class="mt-[3px] max-w-[600px] text-xs leading-4 text-[#55555e]">{props.caption}</p>
      <div class="mt-3">{props.children}</div>
    </figure>
  );
}

/// The shared top of the window: title, file name and the accent status line, then `line` and buttons.
function Body(props: { status: string; line?: Child; children: Child }): Markup {
  return (
    <Window width={600} height={428}>
      <div class="px-6 pt-[26px] text-base leading-none">
        <h3 class="text-xl leading-none">Generate SRT</h3>
        <p class="mt-[21px]">interview.mp4</p>
        <p class="mt-[22px] text-accent">{props.status}</p>
        {props.line}
        {props.children}
      </div>
    </Window>
  );
}

function Cost(props: { cost: string; audio: string }): Markup {
  return (
    <p class="mt-[22px]">
      Estimated cost: {props.cost} for {props.audio} of audio ·{" "}
      <span class="text-[#00f0ff] underline">MiniMax pricing</span>
    </p>
  );
}

/// The START and CANCEL buttons side by side. START is disabled while running.
function StartCancel(props: { disabled?: boolean; class?: string }): Markup {
  return (
    <div class={`flex gap-3 ${props.class ?? "mt-[19px]"}`}>
      <CyberButton width={107} variant={props.disabled ? "disabled" : "primary"} label="START" />
      <CyberButton width={111} variant="secondary" label="CANCEL" />
    </div>
  );
}

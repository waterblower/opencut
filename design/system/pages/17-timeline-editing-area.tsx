import { COLOR } from "../theme.ts";
import { AudioClip, NeighborClip, TextClip, VideoClip } from "../components/timeline-clip.tsx";
import {
  At,
  ClipAt,
  Cursor,
  GhostClip,
  LaneLine,
  MAIN_RULER,
  PLACEMENT_RULER,
  Timeline,
  type TrackSpec,
} from "../components/timeline.tsx";
import { INTERVIEW_A, INTERVIEW_A_WIDE, INTERVIEW_B, MUSIC_BED, VOICE_OVER } from "../components/waveform-samples.ts";
import type { Child, Markup } from "@design/jsx/jsx-runtime";

const TRACKS: TrackSpec[] = [
  { kind: "T", name: "Text 1" },
  { kind: "V", name: "Video 1" },
  { kind: "A", name: "Audio 1" },
];
const PAIR: TrackSpec[] = [
  { kind: "V", name: "Video 1" },
  { kind: "A", name: "Audio 1" },
];

export default function TimelineEditingAreaPage(): Markup {
  return (
    <main class="w-[1240px] px-12 pt-10 pb-12 font-sans text-[#1b1b1e]">
      <h1 class="text-2xl leading-7 font-bold">Timeline editing area</h1>
      <p class="mt-2 text-sm leading-5 text-[#55555e]">
        One open timeline. Toolbar, ruler and tracks share one playhead.
      </p>

      <Section title="Timeline" description="The toolbar, ruler and tracks, and the states a timeline can be in.">
        <Figure title="Default" caption="Selection tool · one selected clip">
          <MainTimeline />
        </Figure>
        <Row>
          <Figure title="No timeline" caption="Until a timeline file is opened">
            <div class="grid h-[250px] w-[556px] place-items-center rounded-[10px] border border-border bg-[#0a0a0c] text-sm leading-none text-muted">
              Create or select a timeline to begin editing
            </div>
          </Figure>
          <Figure title="Track states" caption="Accent marks a track that is locked, hidden or muted">
            <Timeline
              width={556}
              tracks={[
                { ...TRACKS[0], lock: "active" },
                { ...TRACKS[1], visibility: "active" },
                { ...TRACKS[2], mute: "active" },
              ]}
              playhead={140}
              lane={<SmallClips />}
            />
          </Figure>
        </Row>
        <Row>
          <Figure
            title="Blade"
            caption="Blade tool active. The red blade guide follows the pointer; a click splits clips at the blade guide."
          >
            <Timeline
              width={556}
              tracks={TRACKS}
              playhead={140}
              lane={
                <>
                  <SmallClips />
                  <LaneLine x={264} width={2} color={COLOR.error} />
                </>
              }
              overlay={<Cursor kind="arrow" left={454} top={130} />}
            />
          </Figure>
          <Figure title="Trim" caption="Hovering a clip edge highlights its trim handle">
            <Timeline
              width={556}
              tracks={TRACKS}
              playhead={140}
              lane={
                <>
                  <SmallClips videoSelected brollLeft={244} brollWidth={136} />
                  <At left={222} top={107}>
                    <div class="h-16 w-1.5 bg-accent" />
                  </At>
                </>
              }
              overlay={<Cursor kind="resize" left={405.2} top={129} />}
            />
          </Figure>
        </Row>
        <Row>
          <Figure title="Marquee selection" caption="Drag on empty space to select clips inside the box">
            <Timeline
              width={556}
              tracks={TRACKS}
              playhead={60}
              lane={<SmallClips selected={[1, 2]} videoSelected />}
              overlay={
                <>
                  <At left={338} top={30}>
                    <div class="h-[104px] w-[72px] border border-accent bg-accent/14" />
                  </At>
                  <Cursor kind="marquee" left={410} top={134} />
                </>
              }
            />
          </Figure>
        </Row>
      </Section>

      <Section
        title="Clip Placement"
        description="One placement preview for dragging an asset or moving an existing clip. The destination determines its state."
      >
        <Row>
          <Figure title="Valid" caption="Compatible, unlocked track with enough room for the clip.">
            <Timeline
              width={556}
              ruler={PLACEMENT_RULER}
              tracks={PAIR}
              lane={
                <>
                  <ClipAt left={28} row={1}>
                    <GhostClip width={280} accepted name="ambience-park.wav" caption="Release to place · Audio" />
                  </ClipAt>
                  <LaneLine x={28} color={COLOR.accent} opacity={0.7} />
                </>
              }
              overlay={<Cursor kind="hand" left={466} top={144} />}
            />
          </Figure>
          <Figure title="Snapped" caption="Valid placement aligned to a clip edge. Blue guide marks the snap.">
            <Timeline
              width={556}
              ruler={PLACEMENT_RULER}
              tracks={PAIR}
              lane={
                <>
                  <ClipAt left={0} row={1}>
                    <NeighborClip width={88} name="voice-over…" size={11} left={8} baseline={20} />
                  </ClipAt>
                  <ClipAt left={88} row={1}>
                    <GhostClip width={220} accepted name="ambience-park.wav" caption="Release to place · Audio" />
                  </ClipAt>
                  <LaneLine x={88} width={2} color="#7bb6ff" />
                </>
              }
              overlay={<Cursor kind="hand" left={466} top={144} />}
            />
          </Figure>
        </Row>
        <Row>
          <Figure title="Incompatible track" caption="The clip type does not match the destination track.">
            <Timeline
              width={556}
              ruler={PLACEMENT_RULER}
              tracks={PAIR}
              lane={
                <>
                  <ClipAt left={28} row={0}>
                    <GhostClip width={280} accepted={false} name="ambience-park.wav" caption="Incompatible track" />
                  </ClipAt>
                  <LaneLine x={28} color={COLOR.error} opacity={0.7} />
                </>
              }
              overlay={<Cursor kind="blocked" left={466} top={72} />}
            />
          </Figure>
          <Figure title="Locked track" caption="Matching media type, but the destination track is locked.">
            <Timeline
              width={556}
              ruler={PLACEMENT_RULER}
              tracks={[PAIR[0], { ...PAIR[1], lock: "active" }]}
              lane={
                <>
                  <ClipAt left={28} row={1}>
                    <GhostClip width={280} accepted={false} name="ambience-park.wav" caption="Track is locked" />
                  </ClipAt>
                  <LaneLine x={28} color={COLOR.error} opacity={0.7} />
                </>
              }
              overlay={<Cursor kind="blocked" left={466} top={146} />}
            />
          </Figure>
        </Row>
        <Row>
          <Figure title="Overlap" caption="The proposed position collides with an existing clip.">
            <Timeline
              width={556}
              ruler={PLACEMENT_RULER}
              tracks={PAIR}
              lane={
                <>
                  <ClipAt left={120} row={1}>
                    <NeighborClip width={235} name="voice-over.m4a" size={12} left={12} baseline={53} />
                  </ClipAt>
                  <ClipAt left={28} row={1}>
                    <GhostClip width={280} accepted={false} name="ambience-park.wav" caption="Clip overlapped" />
                  </ClipAt>
                  <LaneLine x={28} color={COLOR.error} opacity={0.7} />
                </>
              }
              overlay={<Cursor kind="blocked" left={466} top={146} />}
            />
          </Figure>
          <Notes title="Shared placement behavior">
            <p>Valid: amber ghost, drag cursor, release to place.</p>
            <p>Rejected: red hatching, blocked cursor and a reason. Releasing a rejected placement makes no change.</p>
            <p>Update immediately when the destination changes.</p>
            <p>When moving, dim the original until release. Cancel restores it. Asset drops have no original to dim.</p>
          </Notes>
        </Row>
      </Section>

      <Section
        title="Locked track"
        description="Locking a track covers its header and lane with neutral diagonal stripes. Clips keep their color."
      >
        <Row>
          <Figure title="Default" caption="Unlocked: the track looks and behaves normally">
            <Timeline
              width={556}
              ruler={null}
              height={73}
              tracks={[{ kind: "V", name: "Video 1", tone: "panel" }]}
              lane={<LockedTrackClips />}
            />
          </Figure>
          <Figure title="Locked" caption="Stripes over the lane, header and clips; lock toggle turns amber">
            <Timeline
              width={556}
              ruler={null}
              height={73}
              tracks={[{ kind: "V", name: "Video 1", tone: "panel", lock: "active", locked: true }]}
              lane={<LockedTrackClips locked />}
            />
          </Figure>
        </Row>
        <Notes title="Locked track behavior">
          <p>Stripes are neutral white. Red hatching stays reserved for rejected placements.</p>
          <p>Clip colors, borders and labels are unchanged; only the stripes and the lock toggle change.</p>
          <p>Selection shortcuts skip a locked track, and the blade never splits its clips.</p>
          <p>Dropping or moving a clip onto a locked track is rejected with “Track is locked”.</p>
        </Notes>
      </Section>
    </main>
  );
}

/// The main timeline: toolbar, ruler, a text track, a video track and an audio track.
function MainTimeline(): Markup {
  const firstTexts: [number, string][] = [[68, "Let'…"], [130.4, "Toda…"], [192.8, "the …"], [255.2, "down…"], [
    317.6,
    "best…",
  ]];
  const secondTexts: [number, string][] = [
    [428, "down…"],
    [485.6, "best…"],
    [543.2, "Firs…"],
    [600.8, "the …"],
    [658.4, "fold…"],
    [716, "Let'…"],
  ];
  return (
    <Timeline
      width={1144}
      toolbar
      ruler={MAIN_RULER}
      tracks={TRACKS}
      playhead={524}
      lane={
        <>
          {firstTexts.map(([left, name]) => (
            <ClipAt left={left} row={0}>
              <TextClip width={57.6} name={name} />
            </ClipAt>
          ))}
          {secondTexts.map(([left, name]) => (
            <ClipAt left={left} row={0}>
              <TextClip width={52.8} name={name} />
            </ClipAt>
          ))}
          <ClipAt left={20} row={1}>
            <VideoClip width={348} name="interview-a.mov" waveform={INTERVIEW_A_WIDE} selected />
          </ClipAt>
          <ClipAt left={368} row={1}>
            <VideoClip width={168} name="broll-street.mov" />
          </ClipAt>
          <ClipAt left={536} row={1}>
            <VideoClip width={300} name="interview-b.mov" waveform={INTERVIEW_B} />
          </ClipAt>
          <ClipAt left={20} row={2}>
            <AudioClip width={720} name="music-bed.m4a" waveform={MUSIC_BED} />
          </ClipAt>
        </>
      }
    />
  );
}

const SMALL_TEXTS: [number, string][] = [[36, "Let's g…"], [118, "Today w…"], [200, "the int…"], [282, "down to…"]];

/// The clips shared by the small timeline cards. `selected` lists text clips by index.
function SmallClips(
  props: { selected?: number[]; videoSelected?: boolean; brollLeft?: number; brollWidth?: number },
): Markup {
  return (
    <>
      {SMALL_TEXTS.map(([left, name], index) => (
        <ClipAt left={left} row={0}>
          <TextClip width={76} name={name} selected={props.selected?.includes(index)} />
        </ClipAt>
      ))}
      <ClipAt left={20} row={1}>
        <VideoClip width={208} name="interview-a.mov" waveform={INTERVIEW_A} selected={props.videoSelected} />
      </ClipAt>
      <ClipAt left={props.brollLeft ?? 228} row={1}>
        <VideoClip width={props.brollWidth ?? 152} name="broll-street.mov" />
      </ClipAt>
      <ClipAt left={44} row={2}>
        <AudioClip width={272} name="voice-over.m4a" waveform={VOICE_OVER} />
      </ClipAt>
    </>
  );
}

function LockedTrackClips(props: { locked?: boolean }): Markup {
  return (
    <>
      <ClipAt left={20} row={0}>
        <VideoClip width={150} name="interview_a.mp4" labelTop locked={props.locked} />
      </ClipAt>
      <ClipAt left={176} row={0}>
        <VideoClip width={170} name="broll_city.mp4" labelTop locked={props.locked} />
      </ClipAt>
    </>
  );
}

// Page layout ---------------------------------------------------------------------------

function Section(props: { title: string; description: string; children: Child }): Markup {
  return (
    <section class="mt-12 border-t border-[#d9d9de] pt-8">
      <h2 class="text-xl leading-7 font-bold">{props.title}</h2>
      <p class="mt-1 text-sm leading-5 text-[#55555e]">{props.description}</p>
      <div class="mt-6 space-y-10">{props.children}</div>
    </section>
  );
}

/// Two figures side by side: 556px each with 32px between.
function Row(props: { children: Child }): Markup {
  return <div class="grid grid-cols-[556px_556px] items-start gap-x-8">{props.children}</div>;
}

function Figure(props: { title: string; caption: string; children: Child }): Markup {
  return (
    <figure>
      <figcaption>
        <h3 class="text-sm leading-5 font-semibold">{props.title}</h3>
        <p class="mt-0.5 text-xs leading-4 text-[#55555e]">{props.caption}</p>
      </figcaption>
      <div class="mt-3">{props.children}</div>
    </figure>
  );
}

function Notes(props: { title: string; children: Child }): Markup {
  return (
    <div>
      <h3 class="text-sm leading-5 font-semibold">{props.title}</h3>
      <div class="mt-3 max-w-[556px] space-y-2 text-[13px] leading-[22px] text-[#55555e]">{props.children}</div>
    </div>
  );
}

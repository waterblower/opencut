use super::{
    ACCENT, TIMELINE_PADDING, TRACK_HEIGHT,
    model::deserialize_ulid,
    timeline::{FrameRate, TimelineTime},
};
use gpui::{
    InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div, px,
    rgb,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use ulid::Ulid;

/// Static visual adjustments for one timeline clip.
///
/// Position is an offset in timeline pixels from the clip's centered placement. Scale is a
/// normalized multiplier, so `1.0` means 100%.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub(super) struct VideoClipProperties {
    pub position_x: f64,
    pub position_y: f64,
    pub scale: f64,
}

impl Default for VideoClipProperties {
    fn default() -> Self {
        Self {
            position_x: 0.0,
            position_y: 0.0,
            scale: 1.0,
        }
    }
}

/// Static audio adjustments for one timeline clip.
///
/// `0 dB` is unity gain and pan ranges from `-1.0` (left) to `1.0` (right).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub(super) struct AudioClipProperties {
    pub gain_db: f64,
    pub muted: bool,
}

impl Default for AudioClipProperties {
    fn default() -> Self {
        Self {
            gain_db: 0.0,
            muted: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub(super) struct TextClipProperties {
    pub text: String,
    pub font: String,
    pub font_size: f64,
    pub color: u32,
    pub position_x: f64,
    pub position_y: f64,
}

impl Default for TextClipProperties {
    fn default() -> Self {
        Self {
            text: "Text".to_string(),
            font: "Sans".to_string(),
            font_size: 64.0,
            color: 0xffffffff,
            position_x: 0.5,
            position_y: 0.5,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
// https://serde.rs/enum-representations.html#adjacently-tagged
#[serde(tag = "kind", content = "data")]
pub(super) enum Clip {
    Video(VideoClip),
    Audio(AudioClip),
    Text(TextClip),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct MediaClipData {
    #[serde(deserialize_with = "deserialize_ulid")]
    pub id: Ulid,
    #[serde(deserialize_with = "deserialize_ulid")]
    pub track_id: Ulid,
    #[serde(default = "Ulid::nil", deserialize_with = "deserialize_ulid")]
    pub asset_id: Ulid,
    pub timeline_start: TimelineTime,
    pub source_in: TimelineTime,
    pub source_out: TimelineTime,
    #[serde(default)]
    pub video_properties: VideoClipProperties,
    #[serde(default)]
    pub audio_properties: AudioClipProperties,
}

pub(super) type VideoClip = MediaClipData;
pub(super) type AudioClip = MediaClipData;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct TextClip {
    #[serde(deserialize_with = "deserialize_ulid")]
    pub id: Ulid,
    #[serde(deserialize_with = "deserialize_ulid")]
    pub track_id: Ulid,
    pub timeline_start: TimelineTime,
    pub length: Duration,
    pub properties: TextClipProperties,
}

impl TextClip {
    pub fn frame_length(&self, frame_rate: FrameRate) -> TimelineTime {
        frame_rate.frames_from_duration_nearest(self.length)
    }
}

impl Clip {
    pub fn id(&self) -> Ulid {
        match self {
            Self::Video(clip) | Self::Audio(clip) => clip.id,
            Self::Text(clip) => clip.id,
        }
    }

    pub fn set_id(&mut self, id: Ulid) {
        match self {
            Self::Video(clip) | Self::Audio(clip) => clip.id = id,
            Self::Text(clip) => clip.id = id,
        }
    }

    pub fn track_id(&self) -> Ulid {
        match self {
            Self::Video(clip) | Self::Audio(clip) => clip.track_id,
            Self::Text(clip) => clip.track_id,
        }
    }

    pub fn set_track_id(&mut self, track_id: Ulid) {
        match self {
            Self::Video(clip) | Self::Audio(clip) => clip.track_id = track_id,
            Self::Text(clip) => clip.track_id = track_id,
        }
    }

    pub fn timeline_start(&self) -> TimelineTime {
        match self {
            Self::Video(clip) | Self::Audio(clip) => clip.timeline_start,
            Self::Text(clip) => clip.timeline_start,
        }
    }

    pub fn set_timeline_start(&mut self, timeline_start: TimelineTime) {
        match self {
            Self::Video(clip) | Self::Audio(clip) => clip.timeline_start = timeline_start,
            Self::Text(clip) => clip.timeline_start = timeline_start,
        }
    }

    pub fn media(&self) -> Option<&MediaClipData> {
        match self {
            Self::Video(clip) | Self::Audio(clip) => Some(clip),
            Self::Text(_) => None,
        }
    }

    pub fn media_mut(&mut self) -> Option<&mut MediaClipData> {
        match self {
            Self::Video(clip) | Self::Audio(clip) => Some(clip),
            Self::Text(_) => None,
        }
    }

    pub fn text(&self) -> Option<&TextClip> {
        let Self::Text(clip) = self else {
            return None;
        };
        Some(clip)
    }

    pub fn frame_length(&self, frame_rate: FrameRate) -> TimelineTime {
        match self {
            Self::Video(clip) | Self::Audio(clip) => {
                (clip.source_out - clip.source_in).max(TimelineTime::ZERO)
            }
            Self::Text(clip) => clip.frame_length(frame_rate).max(TimelineTime::ZERO),
        }
    }

    pub fn timeline_end(&self, frame_rate: FrameRate) -> TimelineTime {
        self.timeline_start() + self.frame_length(frame_rate)
    }

    pub fn source_time_at(&self, timeline_position: TimelineTime) -> Option<TimelineTime> {
        let clip = self.media()?;
        let local = (timeline_position - clip.timeline_start)
            .clamp(TimelineTime::ZERO, clip.source_out - clip.source_in);
        Some((clip.source_in + local).min(clip.source_out))
    }

    pub fn split_at(
        &self,
        timeline_position: TimelineTime,
        frame_rate: FrameRate,
    ) -> Option<(Self, Self)> {
        let local = timeline_position - self.timeline_start();
        if local < TimelineTime::ONE_FRAME
            || local > self.frame_length(frame_rate) - TimelineTime::ONE_FRAME
        {
            return None;
        }

        let mut left = self.clone();
        let mut right = self.clone();
        right.set_id(Ulid::generate());
        right.set_timeline_start(timeline_position);
        match (&mut left, &mut right) {
            (Self::Video(left), Self::Video(right)) | (Self::Audio(left), Self::Audio(right)) => {
                let source_split = left.source_in + local;
                left.source_out = source_split;
                right.source_in = source_split;
            }
            (Self::Text(left), Self::Text(right)) => {
                left.length = frame_rate.duration(local);
                right.length = right.length.saturating_sub(left.length);
            }
            _ => unreachable!("a cloned clip must retain its variant"),
        }
        Some((left, right))
    }
}

pub(super) fn text_clip_component(
    clip: TextClip,
    frame_rate: FrameRate,
    pixels_per_second: f32,
    selected: bool,
    moving: bool,
) -> impl StatefulInteractiveElement + IntoElement {
    let clip_id = clip.id;
    let left =
        TIMELINE_PADDING + frame_rate.seconds(clip.timeline_start) as f32 * pixels_per_second;
    let width =
        (frame_rate.seconds(clip.frame_length(frame_rate)) as f32 * pixels_per_second).max(4.0);

    div()
        .id(gpui::SharedString::from(format!("timeline-clip-{clip_id}")))
        .absolute()
        .left(px(left))
        .top(px(5.0))
        .w(px(width))
        .h(px(TRACK_HEIGHT - 10.0))
        .overflow_hidden()
        .rounded_md()
        .border_1()
        .border_color(rgb(if selected { ACCENT } else { 0x8261b3 }))
        .bg(rgb(0x7251a3))
        .opacity(if moving { 0.3 } else { 1.0 })
        .child(
            div()
                .absolute()
                .inset_0()
                .p_2()
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_ellipsis()
                .child(clip.properties.text),
        )
}

use super::{ACCENT, FrameRate, TIMELINE_PADDING, TRACK_HEIGHT, TimelineTime};
use gpui::{
    InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div, px,
    rgb,
};
pub use opencut_player::timeline::{
    AudioClip, AudioClipProperties, Clip, TextClip, TextClipProperties, VideoClip,
    VideoClipProperties,
};
use ulid::Ulid;
pub trait ClipEditingExt {
    fn split_at(
        &self,
        timeline_position: TimelineTime,
        frame_rate: FrameRate,
    ) -> Option<(Self, Self)>
    where
        Self: Sized;
}
impl ClipEditingExt for Clip {
    fn split_at(
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

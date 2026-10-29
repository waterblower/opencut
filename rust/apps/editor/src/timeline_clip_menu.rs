use crate::context_menu::ContextMenu;
use crate::edit_action::EditAction;
use crate::editor::Editor;
use crate::event_bus::AppEvent;
use crate::model::MediaKind;
use crate::timeline_clip::{AudioClipProperties, VideoClipProperties};
use crate::track::TrackKind;
use ::timeline::TimelineEditingState;
use gpui::prelude::*;
use ulid::Ulid;

/// The source video clip's properties and the indices of the other clips on its track
/// whose properties differ from them.
pub(super) fn clip_property_targets(
    timeline: &TimelineEditingState,
    source_clip_id: Ulid,
) -> Option<(VideoClipProperties, AudioClipProperties, Vec<usize>)> {
    let source = timeline.clip(source_clip_id)?;
    let source_media = source.video()?;
    let track = timeline.track(source.track_id())?;
    let source_asset = timeline.asset(source_media.asset_id)?;
    if track.locked || track.kind != TrackKind::Video || source_asset.kind == MediaKind::Audio {
        return None;
    }
    let video_properties = source_media.video_properties;
    let audio_properties = source_media.audio_properties;
    let targets = timeline
        .clips
        .iter()
        .enumerate()
        .filter(|(_, clip)| clip.id() != source.id() && clip.track_id() == source.track_id())
        .filter(|(_, clip)| {
            clip.video()
                .and_then(|clip| timeline.asset(clip.asset_id))
                .is_some_and(|asset| asset.kind != MediaKind::Audio)
        })
        .filter(|(_, clip)| {
            clip.video().is_some_and(|clip| {
                clip.video_properties != video_properties
                    || clip.audio_properties != audio_properties
            })
        })
        .map(|(index, _)| index)
        .collect();
    Some((video_properties, audio_properties, targets))
}

impl Editor {
    pub(super) fn apply_properties_to_track_clips(&mut self, cx: &mut Context<Self>) {
        let ContextMenu::TimelineClip(menu) =
            std::mem::replace(&mut self.context_menu, ContextMenu::None)
        else {
            return;
        };
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let Some((video_properties, audio_properties, targets)) =
            clip_property_targets(&timeline.editing_state, menu.clip_id)
        else {
            return;
        };
        let clip_ids = targets
            .into_iter()
            .map(|index| timeline.editing_state.clips[index].id())
            .collect();
        self.emit_event(
            cx,
            AppEvent::Edit(EditAction::SetVideoClipProperties {
                clip_ids,
                video_properties,
                audio_properties,
            }),
        );
        self.property_panel_state.transform_input_clip_id = None;
    }
}

#[cfg(test)]
#[path = "tests/timeline_clip_menu.test.rs"]
mod tests;

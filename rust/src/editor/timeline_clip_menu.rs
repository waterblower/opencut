use super::*;

pub(super) fn transform_targets(
    timeline: &TimelineSerialization,
    source_clip_id: Ulid,
) -> Option<(VideoClipProperties, Vec<usize>)> {
    let source = timeline.clip(source_clip_id)?;
    let source_media = source.media()?;
    let track = timeline.track(source.track_id())?;
    let source_asset = timeline.asset(source_media.asset_id)?;
    if track.locked || track.kind != TrackKind::Video || source_asset.kind == MediaKind::Audio {
        return None;
    }
    let properties = source_media.video_properties;
    let targets = timeline
        .clips
        .iter()
        .enumerate()
        .filter(|(_, clip)| clip.id() != source.id() && clip.track_id() == source.track_id())
        .filter(|(_, clip)| {
            clip.media()
                .and_then(|clip| timeline.asset(clip.asset_id))
                .is_some_and(|asset| asset.kind != MediaKind::Audio)
        })
        .filter(|(_, clip)| {
            clip.media()
                .is_some_and(|clip| clip.video_properties != properties)
        })
        .map(|(index, _)| index)
        .collect();
    Some((properties, targets))
}

impl Editor {
    pub(super) fn apply_transform_to_track_clips(&mut self) {
        let ContextMenu::TimelineClip(menu) = self.take_context_menu() else {
            return;
        };
        let source_clip_id = menu.clip_id;
        let Some((properties, targets)) = self
            .timeline
            .as_ref()
            .and_then(|timeline| transform_targets(&timeline.data, source_clip_id))
        else {
            return;
        };
        if targets.is_empty() {
            return;
        }
        let changed = targets.len();
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.record_editing_history();
        let clip_ids = targets
            .into_iter()
            .map(|index| timeline.data.clips[index].id())
            .collect();
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
            timeline,
            EditAction::SetVideoProperties {
                clip_ids,
                properties,
            },
        )
        .expect("setting video properties cannot be rejected");
        self.properties.transform_input_clip_id = None;

        timeline.save(&self.global_settings.project_root);

        self.status = Some(format!(
            "Applied transforms to {changed} other clip{}.",
            if changed == 1 { "" } else { "s" }
        ));
    }
}

#[cfg(test)]
#[path = "timeline_clip_menu.test.rs"]
mod tests;

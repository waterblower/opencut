use super::*;

impl Editor {
    pub(super) fn add_text(
        &mut self,
        track_id: Ulid,
        position: TimelineTime,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_context_menu();
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        let clip = match text_clip_at(&timeline.data, track_id, position) {
            Ok(clip) => clip,
            Err(error) => {
                self.status = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        timeline.record_editing_history();
        let clip_id = clip.id();
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
            timeline,
            EditAction::AddClips {
                clips: vec![clip],
                assets: Vec::new(),
            },
        )
        .expect("text clip placement was validated before recording history");
        timeline.interaction.selected_clip_id = Some(clip_id);
        timeline.interaction.selected_clip_ids.clear();
        timeline.interaction.selected_clip_ids.insert(clip_id);
        timeline.save(&self.global_settings.project_root);
        self.status = Some("Added text clip.".to_string());
        cx.notify();
    }
}

fn text_clip_at(
    timeline: &TimelineSerialization,
    track_id: Ulid,
    position: TimelineTime,
) -> Result<Clip, &'static str> {
    let Some(track) = timeline.track(track_id) else {
        return Err("The text track is unavailable.");
    };
    if track.kind != TrackKind::Text {
        return Err("Text can only be added to a text track.");
    }
    if track.locked {
        return Err("Unlock the text track before adding text.");
    }
    if timeline.clips_on_track(track_id).any(|clip| {
        clip.timeline_start() <= position
            && position < clip.timeline_end(timeline.settings.frame_rate)
    }) {
        return Err("A text clip already exists at this position.");
    }

    let default_duration = timeline.ceil_time(5.0).max(TimelineTime::ONE_FRAME);
    let duration = timeline
        .clips_on_track(track_id)
        .filter(|clip| clip.timeline_start() > position)
        .map(|clip| clip.timeline_start() - position)
        .min()
        .map_or(default_duration, |available| {
            available.min(default_duration)
        });
    Ok(Clip::Text(TextClip {
        id: Ulid::generate(),
        track_id,
        timeline_start: position,
        length: timeline.duration(duration),
        properties: TextClipProperties::default(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_text_when_the_position_is_inside_an_existing_clip() {
        let track_id = Ulid::generate();
        let mut timeline = TimelineSerialization::default();
        timeline.tracks.push(Track {
            id: track_id,
            name: "Text 1".to_string(),
            kind: TrackKind::Text,
            locked: false,
            muted: false,
            visible: true,
        });
        let clip = text_clip_at(&timeline, track_id, TimelineTime::ZERO).unwrap();
        let clip_end = clip.timeline_end(timeline.settings.frame_rate);
        timeline.clips.push(clip);

        assert_eq!(
            text_clip_at(&timeline, track_id, TimelineTime::ONE_FRAME).unwrap_err(),
            "A text clip already exists at this position."
        );
        assert!(text_clip_at(&timeline, track_id, clip_end).is_ok());
    }
}

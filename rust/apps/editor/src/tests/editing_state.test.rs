use crate::edit_action::{EditAction, edit_timeline};
use crate::timeline::TimelineRuntimeState;
use ::timeline::{
    Clip, FrameRate, TextClip, TextClipProperties, TimelineEditingState, TimelineFrameIndex,
    TimelineSettings, Track, TrackKind,
};
use anyhow::Result;
use gpui::{point, px};
use std::time::Duration;
use ulid::Ulid;

#[test]
fn rejected_edits_preserve_content_playhead_and_history() -> Result<()> {
    let mut timeline = runtime()?;
    timeline.set_playhead(TimelineFrameIndex::from(10));
    for action in [
        EditAction::MoveClips {
            placements: vec![(
                Ulid::from(2_u128),
                Ulid::from(1_u128),
                TimelineFrameIndex::from(-1),
            )],
        },
        EditAction::SetTextProperties {
            clip_id: Ulid::from(2_u128),
            properties: TextClipProperties {
                font_size: 0.0,
                ..Default::default()
            },
        },
    ] {
        assert!(edit_timeline(&mut timeline, action).is_err());
        assert_eq!(
            timeline.editing_state.clips[0].timeline_start(),
            TimelineFrameIndex::ZERO
        );
        let Clip::Text(clip) = &timeline.editing_state.clips[0] else {
            panic!("expected the original text clip");
        };
        assert_eq!(clip.properties.font_size, 64.0);
        assert_eq!(timeline.playhead(), TimelineFrameIndex::from(10));
        assert!(timeline.undo_stack.is_empty() && timeline.redo_stack.is_empty());
    }
    Ok(())
}

#[test]
fn history_replacement_preserves_live_view_preferences_and_playhead() -> Result<()> {
    let mut timeline = runtime()?;
    edit_timeline(
        &mut timeline,
        EditAction::MoveClips {
            placements: vec![(
                Ulid::from(2_u128),
                Ulid::from(1_u128),
                TimelineFrameIndex::from(10),
            )],
        },
    )?;
    let redo = timeline.editing_state.clone();
    let undo = timeline.undo_stack.pop().unwrap();

    timeline.pixels_per_second = 180.0;
    timeline.snapping_enabled = false;
    timeline.track_magnet_enabled = false;
    timeline.h_scroll.set_offset(point(px(-40.0), px(0.0)));
    timeline.v_scroll.set_offset(point(px(0.0), px(-20.0)));
    let horizontal = timeline.h_scroll.offset();
    let vertical = timeline.v_scroll.offset();
    timeline.set_playhead(TimelineFrameIndex::from(15));

    for (snapshot, start) in [(undo, 0), (redo, 10)] {
        edit_timeline(
            &mut timeline,
            EditAction::ReplaceTimeline { timeline: snapshot },
        )?;
        assert_eq!(
            i64::from(timeline.editing_state.clips[0].timeline_start()),
            start
        );
        assert_eq!(timeline.playhead(), TimelineFrameIndex::from(15));
        assert_eq!(timeline.pixels_per_second, 180.0);
        assert!(!timeline.snapping_enabled && !timeline.track_magnet_enabled);
        assert_eq!(timeline.h_scroll.offset(), horizontal);
        assert_eq!(timeline.v_scroll.offset(), vertical);
    }
    Ok(())
}

#[test]
fn persistence_captures_selected_state() -> Result<()> {
    let mut timeline = runtime()?;
    timeline.set_playhead(TimelineFrameIndex::from(10));
    timeline.pixels_per_second = 180.0;
    timeline.snapping_enabled = false;
    timeline.track_magnet_enabled = false;
    timeline.h_scroll.set_offset(point(px(-40.0), px(0.0)));
    timeline.v_scroll.set_offset(point(px(0.0), px(-20.0)));
    timeline.record_editing_history();
    timeline.redo_stack.push(timeline.editing_state.clone());
    timeline.interaction.selected_clip_id = None;
    timeline.interaction.selected_clip_ids.clear();

    let document = timeline.to_serialize();
    let json = serde_json::to_value(&document)?;
    assert_eq!(json.as_object().unwrap().len(), 2);
    assert!(json.get("editing_state").is_some());
    assert_eq!(
        json["view_state"],
        serde_json::json!({
            "saved_playhead_frame": 10, "horizontal_scroll": 40.0,
            "vertical_scroll": 20.0, "pixels_per_second": 180.0,
            "snapping_enabled": false, "track_magnet_enabled": false
        })
    );

    let restored = TimelineRuntimeState::from_serialize(document, timeline.path.clone())?;
    assert_eq!(serde_json::to_value(restored.to_serialize())?, json);
    assert!(restored.undo_stack.is_empty() && restored.redo_stack.is_empty());
    assert_eq!(
        restored.interaction.selected_clip_id,
        Some(Ulid::from(2_u128))
    );
    assert!(restored.preview_drop_asset.is_none());
    assert!(
        TimelineRuntimeState::from_serialize(timeline.to_serialize(), "relative.json".into())
            .is_err()
    );
    assert!(TimelineRuntimeState::load("relative.json".into()).is_err());
    timeline.path = "relative.json".into();
    assert!(timeline.save().is_err());
    Ok(())
}

fn runtime() -> Result<TimelineRuntimeState> {
    let track_id = Ulid::from(1_u128);
    let data = TimelineEditingState {
        settings: TimelineSettings {
            frame_rate: FrameRate::new(10, 1),
            width: 16,
            height: 12,
            ..Default::default()
        },
        tracks: vec![Track {
            id: track_id,
            name: "Text".into(),
            kind: TrackKind::Text,
            locked: false,
            muted: false,
            visible: true,
        }],
        clips: vec![Clip::Text(TextClip {
            id: Ulid::from(2_u128),
            track_id,
            timeline_start: TimelineFrameIndex::ZERO,
            duration: Duration::from_secs(3),
            properties: TextClipProperties::default(),
        })],
        ..Default::default()
    };
    let root = std::env::temp_dir();
    TimelineRuntimeState::new(root.join("editing-state.timeline.json"), data)
}

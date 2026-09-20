use super::*;
use crate::editor::tests::TimelineTestExt;
use crate::editor::timeline_clip::AudioClipProperties;

fn audio_asset(id: u64) -> MediaAsset {
    MediaAsset {
        id: ulid(id),
        kind: MediaKind::Audio,
        path: PathBuf::from("audio.mp3"),
        name: "Audio".to_string(),
        duration: 10.0,
        width: 0,
        height: 0,
        framerate: 0.0,
        frame_rate_numerator: 0,
        frame_rate_denominator: 0,
        codec: "mp3".to_string(),
        has_audio: true,
    }
}

fn audio_clip(id: u64, start: i64, duration: i64) -> Clip {
    Clip::Audio(AudioClip {
        id: ulid(id),
        track_id: ulid(2),
        asset_id: ulid(100),
        timeline_start: TimelineTime::from_frames(start),
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::from_frames(duration),
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    })
}

#[test]
fn audio_clips_from_video_assets_cannot_move_to_video_tracks() {
    let mut project = TimelineSerialization::with_test_tracks();
    let mut asset = audio_asset(100);
    asset.kind = MediaKind::Video;
    project.assets.push(asset);
    project.clips = vec![audio_clip(10, 0, 30), audio_clip(11, 30, 30)];
    let selected = HashSet::from([ulid(10), ulid(11)]);

    for placements in [
        vec![(ulid(10), ulid(1), TimelineTime::from_frames(120))],
        vec![
            (ulid(10), ulid(2), TimelineTime::from_frames(120)),
            (ulid(11), ulid(1), TimelineTime::from_frames(150)),
        ],
    ] {
        let error = project
            .validate_clip_move_placements(&placements, &selected)
            .unwrap_err();
        assert_eq!(
            error.downcast::<ClipPlacementRejection>().unwrap(),
            ClipPlacementRejection::IncompatibleTrack,
        );
    }
    assert!(
        project
            .validate_clip_move_placements(
                &[
                    (ulid(10), ulid(2), TimelineTime::from_frames(120)),
                    (ulid(11), ulid(2), TimelineTime::from_frames(150)),
                ],
                &selected,
            )
            .is_ok()
    );
}

#[test]
fn clipboard_preserves_relative_timing_tracks_and_primary_selection() {
    let mut project = TimelineSerialization::with_test_tracks();
    project.assets.push(audio_asset(100));
    project.clips = vec![audio_clip(10, 20, 8), audio_clip(11, 40, 12)];
    let selected = HashSet::from([ulid(10), ulid(11)]);
    let clipboard = ClipClipboard::from_selection(
        "one.timeline.json".into(),
        &project,
        &selected,
        Some(ulid(11)),
    )
    .unwrap();

    let pasted = clipboard.clips_at(TimelineTime::from_frames(100), project.settings.frame_rate);
    assert_eq!(pasted[0].timeline_start(), TimelineTime::from_frames(100));
    assert_eq!(pasted[1].timeline_start(), TimelineTime::from_frames(120));
    assert_eq!(pasted[0].track_id(), ulid(2));
    assert_eq!(pasted[1].track_id(), ulid(2));
    assert_eq!(clipboard.primary_index, Some(1));
}

#[test]
fn clipboard_rescales_source_bounds_between_timeline_frame_rates() {
    let mut source = TimelineSerialization::with_test_tracks();
    source.settings.frame_rate = FrameRate::new(24, 1);
    source.assets.push(audio_asset(100));
    let mut clip = audio_clip(10, 12, 24);
    clip.media_mut().unwrap().source_in = TimelineTime::from_frames(24);
    clip.media_mut().unwrap().source_out = TimelineTime::from_frames(48);
    source.clips = vec![clip, audio_clip(11, 36, 24)];
    let clipboard = ClipClipboard::from_selection(
        "one.timeline.json".into(),
        &source,
        &HashSet::from([ulid(10), ulid(11)]),
        Some(ulid(10)),
    )
    .unwrap();

    let mut destination = TimelineSerialization::with_test_tracks();
    destination.settings.frame_rate = FrameRate::new(30, 1);
    destination.tracks[0].id = ulid(201);
    destination.tracks[1].id = ulid(202);
    let (clips, _) = clipboard
        .prepare_paste(
            std::path::Path::new("two.timeline.json"),
            &destination,
            TimelineTime::from_frames(60),
        )
        .unwrap();

    assert_eq!(clips[0].timeline_start(), TimelineTime::from_frames(60));
    assert_eq!(
        clips[0].media().unwrap().source_in,
        TimelineTime::from_frames(30)
    );
    assert_eq!(
        clips[0].media().unwrap().source_out,
        TimelineTime::from_frames(60)
    );
    assert_eq!(
        clips[0].frame_length(destination.settings.frame_rate),
        TimelineTime::from_frames(30)
    );
    assert_eq!(clips[1].timeline_start(), TimelineTime::from_frames(90));
}

#[test]
fn clipboard_remaps_tracks_and_assets_between_timelines() {
    let mut source = TimelineSerialization::with_test_tracks();
    source.assets.push(audio_asset(100));
    source.clips = vec![audio_clip(10, 20, 8), audio_clip(11, 40, 12)];
    let clipboard = ClipClipboard::from_selection(
        "one.timeline.json".into(),
        &source,
        &HashSet::from([ulid(10), ulid(11)]),
        Some(ulid(11)),
    )
    .unwrap();

    let mut destination = TimelineSerialization::with_test_tracks();
    destination.tracks[0].id = ulid(201);
    destination.tracks[1].id = ulid(202);
    let (clips, assets) = clipboard
        .prepare_paste(
            std::path::Path::new("two.timeline.json"),
            &destination,
            TimelineTime::from_frames(100),
        )
        .unwrap();

    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].path, PathBuf::from("audio.mp3"));
    assert_ne!(assets[0].id, ulid(100));
    assert_eq!(clips[0].track_id(), ulid(202));
    assert_eq!(clips[1].track_id(), ulid(202));
    assert_eq!(clips[0].media().unwrap().asset_id, assets[0].id);
    assert_eq!(clips[1].media().unwrap().asset_id, assets[0].id);
    assert_eq!(clips[0].timeline_start(), TimelineTime::from_frames(100));
    assert_eq!(clips[1].timeline_start(), TimelineTime::from_frames(120));
}

#[test]
fn clipboard_reuses_existing_destination_assets() {
    let mut source = TimelineSerialization::with_test_tracks();
    source.assets.push(audio_asset(100));
    source.clips = vec![audio_clip(10, 0, 8)];
    let clipboard = ClipClipboard::from_selection(
        "one.timeline.json".into(),
        &source,
        &HashSet::from([ulid(10)]),
        Some(ulid(10)),
    )
    .unwrap();

    let mut destination = TimelineSerialization::with_test_tracks();
    destination.tracks[1].id = ulid(202);
    let mut existing_asset = audio_asset(300);
    existing_asset.path = PathBuf::from("audio.mp3");
    destination.assets.push(existing_asset);
    let (clips, assets) = clipboard
        .prepare_paste(
            std::path::Path::new("two.timeline.json"),
            &destination,
            TimelineTime::ZERO,
        )
        .unwrap();

    assert!(assets.is_empty());
    assert_eq!(clips[0].media().unwrap().asset_id, ulid(300));
}

#[test]
fn clipboard_paste_rejects_the_complete_selection_on_collision() {
    let mut project = TimelineSerialization::with_test_tracks();
    project.assets.push(audio_asset(100));
    project.clips = vec![audio_clip(20, 105, 10)];
    let candidates = vec![audio_clip(10, 100, 8), audio_clip(11, 120, 12)];
    let rejection = validate_clips_placements(&project, &candidates).unwrap_err();
    let rejection = rejection.downcast::<ClipPlacementRejection>().unwrap();

    assert_eq!(rejection, ClipPlacementRejection::ExistingClipOverlap);
    assert_eq!(rejection.message(), "Placement overlaps an existing clip");
}

#[test]
fn track_magnet_does_not_ripple_multiple_deleted_clips() {
    let mut clips = vec![
        audio_clip(1, 10, 10),
        audio_clip(2, 30, 5),
        audio_clip(3, 50, 10),
        {
            let mut clip = audio_clip(4, 50, 10);
            clip.set_track_id(ulid(3));
            clip
        },
    ];

    ripple_clips_after_deletion(
        &mut clips,
        &HashSet::from([ulid(1), ulid(2)]),
        FrameRate::default(),
    );

    assert_eq!(clips[2].timeline_start(), TimelineTime::from_frames(50));
    assert_eq!(clips[3].timeline_start(), TimelineTime::from_frames(50));
}

#[test]
fn select_all_excludes_clips_on_locked_tracks() {
    let mut project = TimelineSerialization::with_test_tracks();
    project.clips = vec![audio_clip(10, 0, 10), {
        let mut clip = audio_clip(11, 10, 10);
        clip.set_track_id(ulid(1));
        clip
    }];
    project.track_mut(ulid(2)).unwrap().locked = true;

    assert_eq!(unlocked_clip_ids(&project), HashSet::from([ulid(11)]));
}

#[test]
fn edits_do_not_require_source_media_or_a_playback_backend() -> Result<()> {
    let mut data = TimelineSerialization::with_test_tracks();
    data.assets.push(audio_asset(100));
    let mut timeline = TimelineRuntimeState::new("test.timeline.json".into(), data, Path::new("."));
    edit_timeline(
        &mut timeline,
        EditAction::AddClips {
            clips: vec![audio_clip(10, 0, 30), audio_clip(11, 30, 30)],
            assets: Vec::new(),
        },
    )?;
    edit_timeline(
        &mut timeline,
        EditAction::MoveClips {
            placements: vec![
                (ulid(10), ulid(2), TimelineTime::from_frames(90)),
                (ulid(11), ulid(2), TimelineTime::from_frames(120)),
            ],
        },
    )?;
    assert_eq!(
        timeline
            .data
            .clip(ulid(10))
            .unwrap()
            .timeline_start()
            .frames(),
        90
    );
    assert_eq!(
        timeline
            .data
            .clip(ulid(11))
            .unwrap()
            .timeline_start()
            .frames(),
        120
    );
    assert_eq!(timeline.data.content_duration().frames(), 150);
    Ok(())
}

#[test]
fn invalid_edits_leave_the_document_unchanged() -> Result<()> {
    let mut data = TimelineSerialization::with_test_tracks();
    data.assets.push(audio_asset(100));
    data.clips = vec![audio_clip(10, 0, 30), audio_clip(11, 30, 30)];
    let mut timeline = TimelineRuntimeState::new("test.timeline.json".into(), data, Path::new("."));
    let before = serde_json::to_value(&timeline.data)?;
    assert!(
        edit_timeline(
            &mut timeline,
            EditAction::MoveClips {
                placements: vec![(ulid(11), ulid(2), TimelineTime::from_frames(10))],
            }
        )
        .is_err()
    );
    assert_eq!(serde_json::to_value(&timeline.data)?, before);
    let mut invalid = timeline.data.clip(ulid(11)).unwrap().clone();
    invalid.set_timeline_start(TimelineTime::from_frames(10));
    assert!(edit_timeline(&mut timeline, EditAction::UpdateClip { clip: invalid }).is_err());
    assert_eq!(serde_json::to_value(&timeline.data)?, before);
    Ok(())
}

#[test]
fn splitting_trimming_and_ripple_deletion_update_the_model() -> Result<()> {
    let mut data = TimelineSerialization::with_test_tracks();
    data.assets.push(audio_asset(100));
    data.clips = vec![audio_clip(10, 0, 60), audio_clip(11, 60, 30)];
    let mut timeline = TimelineRuntimeState::new("test.timeline.json".into(), data, Path::new("."));
    let (left, right) = timeline.data.clips[0]
        .split_at(
            TimelineTime::from_frames(20),
            timeline.data.settings.frame_rate,
        )
        .unwrap();
    let left_id = left.id();
    let right_id = right.id();
    edit_timeline(
        &mut timeline,
        EditAction::SplitClips {
            removed_clips: HashSet::from([ulid(10)]),
            added_clips: vec![left, right],
        },
    )?;
    assert_eq!(timeline.data.clips.len(), 3);
    assert_eq!(
        timeline
            .data
            .clip(right_id)
            .unwrap()
            .media()
            .unwrap()
            .source_in
            .frames(),
        20
    );
    let mut trimmed = timeline.data.clip(right_id).unwrap().clone();
    trimmed.media_mut().unwrap().source_out = TimelineTime::from_frames(50);
    edit_timeline(&mut timeline, EditAction::UpdateClip { clip: trimmed })?;
    edit_timeline(
        &mut timeline,
        EditAction::RemoveClips {
            clip_ids: HashSet::from([left_id]),
            close_track_gaps: true,
        },
    )?;
    assert_eq!(
        timeline.data.clip(right_id).unwrap().timeline_start(),
        TimelineTime::ZERO
    );
    assert_eq!(
        timeline
            .data
            .clip(right_id)
            .unwrap()
            .media()
            .unwrap()
            .source_out
            .frames(),
        50
    );
    assert_eq!(
        timeline
            .data
            .clip(ulid(11))
            .unwrap()
            .timeline_start()
            .frames(),
        40
    );
    Ok(())
}

#[test]
fn track_controls_and_properties_work_without_preview() -> Result<()> {
    let mut data = TimelineSerialization::with_test_tracks();
    data.assets.push(audio_asset(100));
    data.clips = vec![audio_clip(10, 0, 60)];
    let mut timeline = TimelineRuntimeState::new("test.timeline.json".into(), data, Path::new("."));
    let properties = VideoClipProperties {
        position_x: 25.0,
        position_y: -40.0,
        scale: 0.5,
    };
    edit_timeline(
        &mut timeline,
        EditAction::SetVideoProperties {
            clip_ids: vec![ulid(10)],
            properties,
        },
    )?;
    assert_eq!(
        timeline.data.clips[0].media().unwrap().video_properties,
        properties
    );
    let visible = timeline.data.track(ulid(2)).unwrap().visible;
    edit_timeline(
        &mut timeline,
        EditAction::ToggleTrackVisibility { track_id: ulid(2) },
    )?;
    edit_timeline(
        &mut timeline,
        EditAction::ToggleTrackMute { track_id: ulid(2) },
    )?;
    edit_timeline(
        &mut timeline,
        EditAction::ToggleTrackLock { track_id: ulid(2) },
    )?;
    let track = timeline.data.track(ulid(2)).unwrap();
    assert_eq!(track.visible, !visible);
    assert!(track.muted && track.locked);
    edit_timeline(&mut timeline, EditAction::SetSnapping { enabled: false })?;
    edit_timeline(&mut timeline, EditAction::SetTrackMagnet { enabled: true })?;
    assert!(!timeline.interaction.snapping_enabled && !timeline.data.view.snapping_enabled);
    assert!(timeline.interaction.magnet_enabled && timeline.data.view.track_magnet_enabled);
    edit_timeline(&mut timeline, EditAction::DeleteTrack { track_id: ulid(2) })?;
    assert!(timeline.data.track(ulid(2)).is_none());
    assert!(timeline.data.clips.is_empty());
    Ok(())
}

#[test]
fn text_edits_preserve_timing_without_a_renderer() -> Result<()> {
    let mut data = TimelineSerialization::default();
    data.tracks.push(Track {
        id: ulid(3),
        name: "Text".into(),
        kind: TrackKind::Text,
        locked: false,
        muted: false,
        visible: true,
    });
    let mut timeline = TimelineRuntimeState::new("test.timeline.json".into(), data, Path::new("."));
    let mut clip = TextClip {
        id: ulid(10),
        track_id: ulid(3),
        timeline_start: TimelineTime::from_frames(15),
        length: Duration::from_secs(2),
        properties: TextClipProperties::default(),
    };
    edit_timeline(
        &mut timeline,
        EditAction::AddClips {
            clips: vec![Clip::Text(clip.clone())],
            assets: Vec::new(),
        },
    )?;
    // Track locks restrict placement changes; text properties remain editable.
    edit_timeline(
        &mut timeline,
        EditAction::ToggleTrackLock { track_id: ulid(3) },
    )?;
    clip.properties.text = "Hello 日本語".into();
    edit_timeline(
        &mut timeline,
        EditAction::UpdateClip {
            clip: Clip::Text(clip.clone()),
        },
    )?;
    clip.properties.font_size = 72.0;
    edit_timeline(
        &mut timeline,
        EditAction::SetTextProperties {
            clip_id: clip.id,
            properties: clip.properties.clone(),
        },
    )?;
    let json = serde_json::to_string(&timeline.data)?;
    let restored: TimelineSerialization = serde_json::from_str(&json)?;
    let Clip::Text(restored) = restored.clip(clip.id).unwrap() else {
        panic!("text clip must retain its kind");
    };
    assert_eq!(restored.properties, clip.properties);
    assert_eq!(restored.timeline_start, clip.timeline_start);
    assert_eq!(restored.length, clip.length);
    Ok(())
}

#[test]
fn playhead_is_restored_saved_and_clamped_without_media() -> Result<()> {
    let mut data = TimelineSerialization::with_test_tracks();
    data.assets.push(audio_asset(100));
    data.clips = vec![audio_clip(10, 0, 60)];
    data.view.saved_playhead_frame = TimelineTime::from_frames(35);
    let mut timeline = TimelineRuntimeState::new("test.timeline.json".into(), data, Path::new("."));
    assert_eq!(timeline.playhead().frames(), 35);
    edit_timeline(
        &mut timeline,
        EditAction::SetSavedPlayhead {
            playhead: TimelineTime::from_frames(45),
        },
    )?;
    let directory = std::env::temp_dir().join(format!("opencut-playhead-{}", Ulid::generate()));
    timeline.save_timeline_playhead(&directory)?;
    let restored = TimelineSerialization::load(&directory.join(&timeline.path))?;
    std::fs::remove_dir_all(&directory)?;
    let restored = TimelineRuntimeState::new(timeline.path.clone(), restored, Path::new("."));
    assert_eq!(restored.playhead().frames(), 45);
    edit_timeline(
        &mut timeline,
        EditAction::SetSavedPlayhead {
            playhead: TimelineTime::from_frames(100),
        },
    )?;
    assert_eq!(timeline.playhead().frames(), 60);
    edit_timeline(
        &mut timeline,
        EditAction::RemoveClips {
            clip_ids: HashSet::from([ulid(10)]),
            close_track_gaps: false,
        },
    )?;
    assert_eq!(timeline.playhead(), TimelineTime::ZERO);
    Ok(())
}

#[test]
fn replacing_history_snapshots_preserves_playhead_and_document() -> Result<()> {
    let mut data = TimelineSerialization::with_test_tracks();
    data.assets.push(audio_asset(100));
    data.clips = vec![audio_clip(10, 0, 60)];
    data.view.saved_playhead_frame = TimelineTime::from_frames(15);
    let mut timeline = TimelineRuntimeState::new("test.timeline.json".into(), data, Path::new("."));
    timeline.record_editing_history();
    edit_timeline(
        &mut timeline,
        EditAction::MoveClips {
            placements: vec![(ulid(10), ulid(2), TimelineTime::from_frames(30))],
        },
    )?;
    let redo = timeline.data.clone();
    let undo = timeline.undo_stack.pop().unwrap();
    edit_timeline(
        &mut timeline,
        EditAction::ReplaceTimeline { timeline: undo },
    )?;
    assert_eq!(timeline.data.clips[0].timeline_start(), TimelineTime::ZERO);
    assert_eq!(timeline.playhead().frames(), 15);
    edit_timeline(
        &mut timeline,
        EditAction::ReplaceTimeline { timeline: redo },
    )?;
    assert_eq!(timeline.data.clips[0].timeline_start().frames(), 30);
    assert_eq!(timeline.playhead().frames(), 15);
    Ok(())
}

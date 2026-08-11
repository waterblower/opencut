use super::*;

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

fn audio_clip(id: u64, start: i64, duration: i64) -> TimelineClip {
    TimelineClip {
        id: ulid(id),
        track_id: ulid(2),
        asset_id: ulid(100),
        timeline_start: TimelineTime::from_frames(start),
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::from_frames(duration),
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    }
}

#[test]
fn clipboard_preserves_relative_timing_tracks_and_primary_selection() {
    let mut project = Timeline::with_test_tracks();
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

    let pasted = clipboard.clips_at(TimelineTime::from_frames(100));
    assert_eq!(pasted[0].timeline_start, TimelineTime::from_frames(100));
    assert_eq!(pasted[1].timeline_start, TimelineTime::from_frames(120));
    assert_eq!(pasted[0].track_id, ulid(2));
    assert_eq!(pasted[1].track_id, ulid(2));
    assert_eq!(clipboard.primary_index, Some(1));
}

#[test]
fn clipboard_remaps_tracks_and_assets_between_timelines() {
    let mut source = Timeline::with_test_tracks();
    source.assets.push(audio_asset(100));
    source.clips = vec![audio_clip(10, 20, 8), audio_clip(11, 40, 12)];
    let clipboard = ClipClipboard::from_selection(
        "one.timeline.json".into(),
        &source,
        &HashSet::from([ulid(10), ulid(11)]),
        Some(ulid(11)),
    )
    .unwrap();

    let mut destination = Timeline::with_test_tracks();
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
    assert_eq!(clips[0].track_id, ulid(202));
    assert_eq!(clips[1].track_id, ulid(202));
    assert_eq!(clips[0].asset_id, assets[0].id);
    assert_eq!(clips[1].asset_id, assets[0].id);
    assert_eq!(clips[0].timeline_start, TimelineTime::from_frames(100));
    assert_eq!(clips[1].timeline_start, TimelineTime::from_frames(120));
}

#[test]
fn clipboard_reuses_existing_destination_assets() {
    let mut source = Timeline::with_test_tracks();
    source.assets.push(audio_asset(100));
    source.clips = vec![audio_clip(10, 0, 8)];
    let clipboard = ClipClipboard::from_selection(
        "one.timeline.json".into(),
        &source,
        &HashSet::from([ulid(10)]),
        Some(ulid(10)),
    )
    .unwrap();

    let mut destination = Timeline::with_test_tracks();
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
    assert_eq!(clips[0].asset_id, ulid(300));
}

#[test]
fn clipboard_paste_rejects_the_complete_selection_on_collision() {
    let mut project = Timeline::with_test_tracks();
    project.assets.push(audio_asset(100));
    project.clips = vec![audio_clip(20, 105, 10)];
    let candidates = vec![audio_clip(10, 100, 8), audio_clip(11, 120, 12)];
    let rejection = validate_clipboard_placements(&project, &candidates).unwrap_err();

    assert_eq!(rejection, ClipPlacementRejection::ExistingClipOverlap);
    assert_eq!(rejection.message(), "Placement overlaps an existing clip");
}

#[test]
fn track_magnet_closes_deleted_durations_independently_per_track() {
    let mut clips = vec![
        audio_clip(1, 10, 10),
        audio_clip(2, 30, 5),
        audio_clip(3, 50, 10),
        TimelineClip {
            track_id: ulid(3),
            ..audio_clip(4, 50, 10)
        },
    ];

    ripple_clips_after_deletion(&mut clips, &HashSet::from([ulid(1), ulid(2)]));

    assert_eq!(clips[2].timeline_start, TimelineTime::from_frames(35));
    assert_eq!(clips[3].timeline_start, TimelineTime::from_frames(50));
}

#[test]
fn blade_targets_unselected_clips_crossing_the_playhead() {
    let mut project = Timeline::with_test_tracks();
    project.assets.push(audio_asset(100));
    project.clips = vec![audio_clip(10, 0, 20), audio_clip(11, 30, 20)];

    let mut timeline = TimelineState::new("timeline.json".into(), project);
    timeline.playhead = TimelineTime::from_frames(10);
    let mut updated = blade_at_playhead(&timeline.data, timeline.playhead).unwrap();
    updated.clips.sort_by_key(|clip| clip.timeline_start);

    assert_eq!(
        updated
            .clips
            .iter()
            .map(|clip| clip.timeline_start)
            .collect::<Vec<_>>(),
        [
            TimelineTime::ZERO,
            TimelineTime::from_frames(10),
            TimelineTime::from_frames(30)
        ]
    );
    assert_eq!(timeline.data.clips.len(), 2);
}

#[test]
fn select_all_excludes_clips_on_locked_tracks() {
    let mut project = Timeline::with_test_tracks();
    project.clips = vec![
        audio_clip(10, 0, 10),
        TimelineClip {
            id: ulid(11),
            track_id: ulid(1),
            ..audio_clip(11, 10, 10)
        },
    ];
    project.track_mut(ulid(2)).unwrap().locked = true;

    assert_eq!(unlocked_clip_ids(&project), HashSet::from([ulid(11)]));
}

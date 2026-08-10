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
fn appending_media_requires_a_manually_created_track() {
    let project = Timeline::default();

    assert_eq!(
        find_append_track(&project, &audio_asset(100)).unwrap_err(),
        "Add an unlocked audio track before adding media to the timeline."
    );
    assert!(project.tracks.is_empty());
}

#[test]
fn clipboard_preserves_relative_timing_tracks_and_primary_selection() {
    let mut project = Timeline::with_test_tracks();
    project.assets.push(audio_asset(100));
    project.clips = vec![audio_clip(10, 20, 8), audio_clip(11, 40, 12)];
    let selected = HashSet::from([ulid(10), ulid(11)]);
    let clipboard = ClipClipboard::from_selection(&project, &selected, Some(ulid(11))).unwrap();

    let pasted = clipboard.clips_at(TimelineTime::from_frames(100));
    assert_eq!(pasted[0].timeline_start, TimelineTime::from_frames(100));
    assert_eq!(pasted[1].timeline_start, TimelineTime::from_frames(120));
    assert_eq!(pasted[0].track_id, ulid(2));
    assert_eq!(pasted[1].track_id, ulid(2));
    assert_eq!(clipboard.primary_index, Some(1));
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

    let targets = clips_crossing_playhead(&project, TimelineTime::from_frames(10));

    assert_eq!(
        targets.iter().map(|clip| clip.id).collect::<Vec<_>>(),
        [ulid(10)]
    );
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

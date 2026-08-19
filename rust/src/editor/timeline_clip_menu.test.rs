use super::*;

fn asset(id: u64, kind: MediaKind) -> MediaAsset {
    MediaAsset {
        id: ulid(id),
        kind,
        path: format!("asset-{id}").into(),
        name: format!("Asset {id}"),
        duration: 1.0,
        width: 1920,
        height: 1080,
        framerate: 24.0,
        frame_rate_numerator: 24,
        frame_rate_denominator: 1,
        codec: "test".into(),
        has_audio: kind != MediaKind::Image,
    }
}

fn clip(id: u64, track_id: u64, asset_id: u64) -> Clip {
    Clip {
        id: ulid(id),
        track_id: ulid(track_id),
        asset_id: ulid(asset_id),
        timeline_start: TimelineTime::from_frames(id as i64),
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::ONE_FRAME,
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    }
}

#[test]
fn finds_changed_visual_clips_on_the_same_unlocked_track() {
    let mut project = TimelineSerialization::with_test_tracks();
    project.assets = vec![
        asset(10, MediaKind::Video),
        asset(11, MediaKind::Image),
        asset(12, MediaKind::Audio),
    ];
    let mut source = clip(20, 1, 10);
    source.video_properties.position_x = 120.0;
    let target = clip(21, 1, 11);
    let mut unchanged = clip(22, 1, 10);
    unchanged.video_properties = source.video_properties;
    let audio = clip(23, 2, 12);
    project.clips = vec![source, target, unchanged, audio];

    let (properties, targets) = transform_targets(&project, ulid(20)).unwrap();
    assert_eq!(properties.position_x, 120.0);
    assert_eq!(targets, vec![1]);

    project.track_mut(ulid(1)).unwrap().locked = true;
    assert!(transform_targets(&project, ulid(20)).is_none());
}

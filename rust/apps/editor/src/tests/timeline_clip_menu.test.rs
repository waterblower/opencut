use super::*;
use crate::model::{MediaAsset, MediaKind};
use crate::test_support::{TimelineTestExt, ulid};
use crate::timeline::TimelineFrameIndex;
use crate::timeline_clip::{AudioClipProperties, Clip, VideoClip, VideoClipProperties};
use ::timeline::TimelineEditingState;

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
    let clip = VideoClip::new(
        ulid(id),
        ulid(track_id),
        ulid(asset_id),
        TimelineFrameIndex::from(id as i64),
        TimelineFrameIndex::ZERO,
        TimelineFrameIndex::ONE_FRAME,
        VideoClipProperties::default(),
        AudioClipProperties::default(),
    );
    match track_id {
        1 => Clip::Video(clip),
        2 => Clip::Audio(timeline::AudioClip::new(
            clip.id(),
            clip.track_id,
            clip.asset_id,
            clip.timeline_start,
            clip.source_in,
            clip.source_out,
            clip.audio_properties,
        )),
        _ => panic!("test media clips require a video or audio track"),
    }
}

#[test]
fn finds_changed_visual_clips_on_the_same_unlocked_track() {
    let mut project = TimelineEditingState::with_test_tracks();
    project.assets = vec![
        asset(10, MediaKind::Video),
        asset(11, MediaKind::Image),
        asset(12, MediaKind::Audio),
    ];
    let mut source = clip(20, 1, 10);
    source.video_mut().unwrap().video_properties.position_x = 120.0;
    let target = clip(21, 1, 11);
    let mut unchanged = clip(22, 1, 10);
    unchanged.video_mut().unwrap().video_properties = source.video().unwrap().video_properties;
    let audio = clip(23, 2, 12);
    project.clips = vec![source, target, unchanged, audio];

    let (properties, targets) = transform_targets(&project, ulid(20)).unwrap();
    assert_eq!(properties.position_x, 120.0);
    assert_eq!(targets, vec![1]);

    project.track_mut(ulid(1)).unwrap().locked = true;
    assert!(transform_targets(&project, ulid(20)).is_none());
}

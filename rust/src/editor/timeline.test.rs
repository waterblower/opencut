use super::*;

#[cfg(test)]
impl Timeline {
    pub fn with_test_tracks() -> Self {
        Self {
            tracks: vec![
                TimelineTrack {
                    id: 1,
                    name: "Video 1".into(),
                    kind: TrackKind::Video,
                    locked: false,
                    muted: false,
                    visible: true,
                },
                TimelineTrack {
                    id: 2,
                    name: "Audio 1".into(),
                    kind: TrackKind::Audio,
                    locked: false,
                    muted: false,
                    visible: true,
                },
            ],
            ..Self::default()
        }
    }
}

fn frames(value: i64) -> TimelineTime {
    TimelineTime::from_frames(value)
}

fn video_clip(id: u64, start: i64, duration: i64) -> TimelineClip {
    TimelineClip {
        id,
        track_id: 1,
        asset_id: 100,
        timeline_start: frames(start),
        source_in: TimelineTime::ZERO,
        source_out: frames(duration),
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    }
}

fn video_asset() -> MediaAsset {
    MediaAsset {
        id: 100,
        kind: MediaKind::Video,
        path: "clip.mp4".into(),
        name: "clip".into(),
        duration: 30.0,
        width: 1920,
        height: 1080,
        framerate: 30.0,
        frame_rate_numerator: 30,
        frame_rate_denominator: 1,
        codec: "h264".into(),
        has_audio: true,
    }
}

fn image_asset() -> MediaAsset {
    MediaAsset {
        id: 100,
        kind: MediaKind::Image,
        path: "still.png".into(),
        name: "still".into(),
        duration: DEFAULT_IMAGE_CLIP_DURATION,
        width: 1920,
        height: 1080,
        framerate: 0.0,
        frame_rate_numerator: 0,
        frame_rate_denominator: 0,
        codec: "PNG".into(),
        has_audio: false,
    }
}

#[test]
fn new_timelines_have_no_tracks() {
    assert!(Timeline::default().tracks.is_empty());
}

#[test]
fn frame_rate_labels_use_presets_and_format_custom_rates() {
    assert_eq!(FrameRate::new(24_000, 1_001).label(), "23.976 fps");
    assert_eq!(FrameRate::new(15, 1).label(), "15 fps");
    assert_eq!(FrameRate::new(31, 2).label(), "15.50 fps");
    assert_eq!(FrameRate::new(2_469, 200).label(), "12.35 fps");
}

#[test]
fn repairs_overlapping_clips_when_loading_a_timeline() {
    let mut project = Timeline {
        assets: vec![video_asset()],
        clips: vec![video_clip(10, 0, 150), video_clip(11, 90, 120)],
        ..Timeline::with_test_tracks()
    };

    project.normalize();

    assert_eq!(project.clips[0].timeline_start, frames(0));
    assert_eq!(project.clips[1].timeline_start, frames(150));
}

#[test]
fn still_image_clips_can_extend_beyond_their_default_duration() {
    let mut project = Timeline {
        assets: vec![image_asset()],
        clips: vec![video_clip(10, 0, 300)],
        ..Timeline::with_test_tracks()
    };

    project.normalize();

    assert_eq!(project.clips[0].duration(), frames(300));
    assert_eq!(project.seconds(project.clips[0].duration()), 10.0);
}

#[test]
fn time_based_media_remains_bounded_by_its_source_duration() {
    let mut project = Timeline {
        assets: vec![video_asset()],
        clips: vec![video_clip(10, 0, 1_200)],
        ..Timeline::with_test_tracks()
    };

    project.normalize();

    assert_eq!(project.clips[0].duration(), frames(900));
}

#[test]
fn fractional_frame_rates_round_trip_without_drift() {
    for frame_rate in [
        FrameRate {
            numerator: 24_000,
            denominator: 1_001,
        },
        FrameRate {
            numerator: 30_000,
            denominator: 1_001,
        },
        FrameRate {
            numerator: 60_000,
            denominator: 1_001,
        },
    ] {
        let original = frames(1_000_003);
        let seconds = frame_rate.seconds(original);
        assert_eq!(frame_rate.nearest(seconds), original);
    }
}

#[test]
fn repeated_frame_splits_preserve_the_total_duration() {
    let mut remaining = video_clip(10, 0, 10_000);
    let original_duration = remaining.duration();
    let mut pieces = Vec::new();
    let mut next_id = 11;
    for split in [1, 17, 301, 999, 2_048] {
        let position = remaining.timeline_start + frames(split);
        let (left, right) = remaining.split_at(position, next_id).unwrap();
        pieces.push(left.duration());
        remaining = right;
        next_id += 1;
    }
    let reconstructed = pieces
        .into_iter()
        .fold(remaining.duration(), |duration, piece| duration + piece);
    assert_eq!(reconstructed, original_duration);
}

#[test]
fn long_timeline_duration_uses_exact_frame_counts() {
    let frame_rate = FrameRate {
        numerator: 30_000,
        denominator: 1_001,
    };
    let ten_hours = frame_rate.nearest(10.0 * 60.0 * 60.0);
    assert_eq!(frame_rate.nearest(frame_rate.seconds(ten_hours)), ten_hours);
    assert_eq!(
        frame_rate.floor_duration(frame_rate.duration(ten_hours)),
        ten_hours
    );
}

#[test]
fn preview_and_export_boundaries_share_the_same_frame_time() {
    let project = Timeline {
        settings: TimelineSettings {
            frame_rate: FrameRate {
                numerator: 24_000,
                denominator: 1_001,
            },
            ..TimelineSettings::default()
        },
        ..Timeline::default()
    };
    let boundary = frames(98_765);
    let preview_duration = project.duration(boundary).as_secs_f64();
    let export_seconds = project.seconds(boundary);
    assert!((preview_duration - export_seconds).abs() <= 1.0e-9);
}

#[test]
fn timeline_frames_map_to_exact_audio_samples() {
    let frame_rate = FrameRate {
        numerator: 30_000,
        denominator: 1_001,
    };
    assert_eq!(frame_rate.audio_samples(frames(30_000), 48_000), 48_048_000);
}

#[test]
fn maps_30_fps_source_frames_onto_a_24_fps_timeline() {
    let project = Timeline {
        settings: TimelineSettings {
            frame_rate: FrameRate::new(24, 1),
            ..TimelineSettings::default()
        },
        assets: vec![video_asset()],
        clips: vec![video_clip(10, 0, 24)],
        ..Timeline::default()
    };
    let clip = &project.clips[0];
    let mapped = (0..=8)
        .map(|frame| project.source_frame_at(clip, frames(frame)).unwrap())
        .collect::<Vec<_>>();

    assert_eq!(mapped, vec![0, 1, 2, 3, 5, 6, 7, 8, 10]);
}

#[test]
fn changing_timeline_rate_preserves_elapsed_edit_times() {
    let mut project = Timeline {
        assets: vec![video_asset()],
        clips: vec![video_clip(10, 30, 300)],
        ..Timeline::with_test_tracks()
    };

    project.set_frame_rate(FrameRate::new(24, 1));

    assert_eq!(project.clips[0].timeline_start, frames(24));
    assert_eq!(project.clips[0].duration(), frames(240));
}

#[test]
fn clip_source_time_clamps_to_its_source_range() {
    let mut clip = video_clip(10, 100, 60);
    clip.source_in = frames(30);
    clip.source_out = frames(90);

    assert_eq!(clip.source_time_at(frames(50)), frames(30));
    assert_eq!(clip.source_time_at(frames(100)), frames(30));
    assert_eq!(clip.source_time_at(frames(125)), frames(55));
    assert_eq!(clip.source_time_at(frames(160)), frames(90));
    assert_eq!(clip.source_time_at(frames(200)), frames(90));
}

#[test]
fn splitting_clip_preserves_ranges_and_properties() {
    let mut clip = video_clip(10, 100, 60);
    clip.source_in = frames(30);
    clip.source_out = frames(90);
    clip.video_properties.position_x = 42.0;
    clip.video_properties.opacity = 0.5;
    clip.audio_properties.gain_db = -6.0;
    clip.audio_properties.muted = true;

    let (left, right) = clip.split_at(frames(125), 11).unwrap();

    assert_eq!(left.id, 10);
    assert_eq!(left.timeline_start, frames(100));
    assert_eq!(left.source_in, frames(30));
    assert_eq!(left.source_out, frames(55));
    assert_eq!(right.id, 11);
    assert_eq!(right.timeline_start, frames(125));
    assert_eq!(right.source_in, frames(55));
    assert_eq!(right.source_out, frames(90));
    assert_eq!(left.video_properties, clip.video_properties);
    assert_eq!(right.video_properties, clip.video_properties);
    assert_eq!(left.audio_properties, clip.audio_properties);
    assert_eq!(right.audio_properties, clip.audio_properties);
}

#[test]
fn splitting_clip_rejects_its_outer_frames() {
    let clip = video_clip(10, 100, 60);

    assert!(clip.split_at(frames(100), 11).is_none());
    assert!(clip.split_at(frames(160), 11).is_none());
    assert!(clip.split_at(frames(101), 11).is_some());
    assert!(clip.split_at(frames(159), 11).is_some());
}

#[test]
fn timeline_serialization_stores_integer_frames_and_rational_rate() {
    let project = Timeline {
        assets: vec![video_asset()],
        clips: vec![video_clip(10, 17, 83)],
        ..Timeline::default()
    };
    let json = serde_json::to_value(project).unwrap();
    assert!(json.get("version").is_none());
    assert_eq!(json["settings"]["frame_rate"]["numerator"], 30);
    assert_eq!(json["settings"]["frame_rate"]["denominator"], 1);
    assert_eq!(json["clips"][0]["timeline_start"], 17);
    assert_eq!(json["clips"][0]["source_out"], 83);
}

#[test]
fn clip_properties_have_neutral_defaults() {
    assert_eq!(
        VideoClipProperties::default(),
        VideoClipProperties {
            position_x: 0.0,
            position_y: 0.0,
            scale: 1.0,
            opacity: 1.0,
            crop_left: 0.0,
            crop_right: 0.0,
            crop_top: 0.0,
            crop_bottom: 0.0,
        }
    );
    assert_eq!(
        AudioClipProperties::default(),
        AudioClipProperties {
            gain_db: 0.0,
            muted: false,
        }
    );
}

#[test]
fn clips_without_property_objects_deserialize_with_defaults() {
    let legacy = serde_json::json!({
        "id": 10,
        "track_id": 1,
        "asset_id": 100,
        "timeline_start": 0,
        "source_in": 0,
        "source_out": 30
    });
    let clip = serde_json::from_value::<TimelineClip>(legacy).unwrap();

    assert_eq!(clip.video_properties, VideoClipProperties::default());
    assert_eq!(clip.audio_properties, AudioClipProperties::default());
}

#[test]
fn clip_properties_round_trip_through_timeline_json() {
    let mut clip = video_clip(10, 0, 30);
    clip.video_properties = VideoClipProperties {
        position_x: 120.0,
        position_y: -45.0,
        scale: 1.25,
        opacity: 0.8,
        crop_left: 0.1,
        crop_right: 0.2,
        crop_top: 0.05,
        crop_bottom: 0.15,
    };
    clip.audio_properties = AudioClipProperties {
        gain_db: -6.0,
        muted: true,
    };
    let value = serde_json::to_value(&clip).unwrap();
    let restored = serde_json::from_value::<TimelineClip>(value).unwrap();

    assert_eq!(restored.video_properties, clip.video_properties);
    assert_eq!(restored.audio_properties, clip.audio_properties);
}

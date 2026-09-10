use super::*;
use crate::editor::tests::TimelineTestExt;
use crate::editor::timeline_clip::AudioClipProperties;
use gstreamer_editing_services::prelude::*;

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
fn moves_ges_clip_without_rebuilding_timeline() {
    use gstreamer_editing_services::prelude::*;

    let _gstreamer_test = crate::editor::tests::lock_gstreamer_test();
    gstreamer_editing_services::init().unwrap();
    let track_id = ulid(3);
    let clip_id = ulid(10);
    let mut project = TimelineSerialization {
        tracks: vec![Track {
            id: track_id,
            name: "Text 1".to_string(),
            kind: TrackKind::Text,
            locked: false,
            muted: false,
            visible: true,
        }],
        clips: vec![Clip::Text(TextClip {
            id: clip_id,
            track_id,
            timeline_start: TimelineTime::ZERO,
            length: Duration::from_secs(2),
            properties: TextClipProperties::default(),
        })],
        ..TimelineSerialization::default()
    };
    let ges = build_ges_timeline(
        &project,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        export::ExportOptions::from_timeline(&project),
    )
    .unwrap();
    let start = TimelineTime::from_frames(45);

    ges_move_clips(&ges, &project, &[(clip_id, track_id, start)]).unwrap();
    project.clips[0].set_timeline_start(start);

    let clip = ges
        .layers()
        .into_iter()
        .flat_map(|layer| layer.clips())
        .find(|clip| clip.name().as_deref() == Some(format!("opencut-clip-{clip_id}").as_str()))
        .unwrap();
    let expected_start = project.duration(start);
    // Text starts one nanosecond before the frame boundary to avoid a title gap.
    assert!(
        clip.start()
            .nseconds()
            .abs_diff(expected_start.as_nanos() as u64)
            <= 1
    );

    let background = ges
        .layers()
        .into_iter()
        .flat_map(|layer| layer.clips())
        .find(|clip| clip.name().as_deref() == Some("opencut-black-background"))
        .unwrap();
    assert_eq!(
        background.duration().nseconds(),
        project.duration(project.content_duration()).as_nanos() as u64
    );
}

#[test]
fn moves_adjacent_ges_clips_together_without_transient_overlap() {
    use gstreamer_editing_services::prelude::*;

    let _gstreamer_test = crate::editor::tests::lock_gstreamer_test();
    gstreamer_editing_services::init().unwrap();
    let track_id = ulid(3);
    let first_clip_id = ulid(10);
    let second_clip_id = ulid(11);
    let mut project = TimelineSerialization {
        tracks: vec![Track {
            id: track_id,
            name: "Text 1".to_string(),
            kind: TrackKind::Text,
            locked: false,
            muted: false,
            visible: true,
        }],
        clips: vec![
            Clip::Text(TextClip {
                id: first_clip_id,
                track_id,
                timeline_start: TimelineTime::ZERO,
                length: Duration::from_secs(2),
                properties: TextClipProperties::default(),
            }),
            Clip::Text(TextClip {
                id: second_clip_id,
                track_id,
                timeline_start: TimelineTime::from_frames(60),
                length: Duration::from_secs(2),
                properties: TextClipProperties::default(),
            }),
        ],
        ..TimelineSerialization::default()
    };
    let ges = build_ges_timeline(
        &project,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        export::ExportOptions::from_timeline(&project),
    )
    .unwrap();
    let placements = [
        (first_clip_id, track_id, TimelineTime::from_frames(60)),
        (second_clip_id, track_id, TimelineTime::from_frames(120)),
    ];

    ges_move_clips(&ges, &project, &placements).unwrap();
    for (clip_id, _, start) in placements {
        project.clip_mut(clip_id).unwrap().set_timeline_start(start);
    }

    let starts = ges
        .layers()
        .into_iter()
        .flat_map(|layer| layer.clips())
        .filter_map(|clip| {
            let name = clip.name()?;
            let id = name.strip_prefix("opencut-clip-")?.parse::<Ulid>().ok()?;
            Some((id, clip.start()))
        })
        .collect::<HashMap<_, _>>();
    assert!(
        starts[&first_clip_id]
            .nseconds()
            .abs_diff(project.duration(TimelineTime::from_frames(60)).as_nanos() as u64)
            <= 1
    );
    assert!(
        starts[&second_clip_id]
            .nseconds()
            .abs_diff(project.duration(TimelineTime::from_frames(120)).as_nanos() as u64)
            <= 1
    );
}

#[test]
fn moves_adjacent_ges_clips_beyond_timeline_end_without_parking_overlap() {
    use gstreamer_editing_services::prelude::*;

    let _gstreamer_test = crate::editor::tests::lock_gstreamer_test();
    gstreamer_editing_services::init().unwrap();
    let track_id = ulid(2);
    let first_clip_id = ulid(10);
    let second_clip_id = ulid(11);
    let project = TimelineSerialization {
        tracks: vec![Track {
            id: track_id,
            name: "Audio 1".to_string(),
            kind: TrackKind::Audio,
            locked: false,
            muted: false,
            visible: true,
        }],
        assets: vec![audio_asset(100)],
        clips: vec![audio_clip(10, 0, 90), audio_clip(11, 90, 60)],
        ..TimelineSerialization::default()
    };
    // Synthetic sources exercise GES overlap validation without a media fixture.
    let ges = gstreamer_editing_services::Timeline::new_audio_video();
    let layer = ges.append_layer();
    for data in &project.clips {
        let clip = gstreamer_editing_services::TestClip::new().unwrap();
        clip.set_supported_formats(gstreamer_editing_services::TrackType::AUDIO);
        clip.set_name(Some(&format!("opencut-clip-{}", data.id())))
            .unwrap();
        let (start, duration) = super::super::export_gstreamer::clip_clock_range(
            project.settings.frame_rate,
            data,
            data.timeline_start(),
        );
        assert!(clip.set_start(start));
        assert!(clip.set_duration(duration));
        layer.add_clip(&clip).unwrap();
    }
    let placements = [
        (first_clip_id, track_id, TimelineTime::from_frames(240)),
        (second_clip_id, track_id, TimelineTime::from_frames(330)),
    ];

    let mut runtime =
        TimelineRuntimeState::new("test.timeline.json".into(), project, ges.clone()).unwrap();
    assert_eq!(
        runtime.data.content_duration(),
        TimelineTime::from_frames(150)
    );
    edit_timeline(
        &mut runtime,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        EditAction::MoveClips {
            placements: placements.to_vec(),
        },
    )
    .unwrap();
    let project = &runtime.data;
    assert_eq!(project.content_duration(), TimelineTime::from_frames(390));
    assert_eq!(ges.duration(), gstreamer::ClockTime::from_seconds(13));

    let starts = ges
        .layers()
        .into_iter()
        .flat_map(|layer| layer.clips())
        .filter_map(|clip| {
            let name = clip.name()?;
            let id = name.strip_prefix("opencut-clip-")?.parse::<Ulid>().ok()?;
            Some((id, clip.start()))
        })
        .collect::<HashMap<_, _>>();
    for (clip_id, _, start) in placements {
        let (expected_start, _) = super::super::export_gstreamer::clip_clock_range(
            project.settings.frame_rate,
            project.clip(clip_id).unwrap(),
            start,
        );
        assert_eq!(starts[&clip_id], expected_start);
    }
}

#[test]
fn removes_ges_clip_and_ripples_surviving_clips() {
    use gstreamer_editing_services::prelude::*;

    let _gstreamer_test = crate::editor::tests::lock_gstreamer_test();
    gstreamer_editing_services::init().unwrap();
    let track_id = ulid(3);
    let removed_clip_id = ulid(10);
    let surviving_clip_id = ulid(11);
    let project = TimelineSerialization {
        tracks: vec![Track {
            id: track_id,
            name: "Text 1".to_string(),
            kind: TrackKind::Text,
            locked: false,
            muted: false,
            visible: true,
        }],
        clips: vec![
            Clip::Text(TextClip {
                id: removed_clip_id,
                track_id,
                timeline_start: TimelineTime::ZERO,
                length: Duration::from_secs(2),
                properties: TextClipProperties::default(),
            }),
            Clip::Text(TextClip {
                id: surviving_clip_id,
                track_id,
                timeline_start: TimelineTime::from_frames(60),
                length: Duration::from_secs(2),
                properties: TextClipProperties::default(),
            }),
        ],
        ..TimelineSerialization::default()
    };
    let ges = build_ges_timeline(
        &project,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        export::ExportOptions::from_timeline(&project),
    )
    .unwrap();
    let mut runtime =
        TimelineRuntimeState::new("test.timeline.json".into(), project, ges.clone()).unwrap();

    edit_timeline(
        &mut runtime,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        EditAction::RemoveClips {
            clip_ids: HashSet::from([removed_clip_id]),
            close_track_gaps: true,
        },
    )
    .unwrap();

    assert!(runtime.data.clip(removed_clip_id).is_none());
    assert_eq!(
        runtime
            .data
            .clip(surviving_clip_id)
            .unwrap()
            .timeline_start(),
        TimelineTime::ZERO
    );
    let surviving_clip = ges
        .layers()
        .into_iter()
        .flat_map(|layer| layer.clips())
        .find(|clip| {
            clip.name().as_deref() == Some(format!("opencut-clip-{surviving_clip_id}").as_str())
        })
        .unwrap();
    assert_eq!(surviving_clip.start(), gstreamer::ClockTime::ZERO);
    data_parity_check(&runtime, &ges).unwrap();
}

#[test]
fn splits_ges_clip_with_one_edit_action() {
    let _gstreamer_test = crate::editor::tests::lock_gstreamer_test();
    gstreamer_editing_services::init().unwrap();
    let track_id = ulid(3);
    let original_clip_id = ulid(10);
    let original_clip = Clip::Text(TextClip {
        id: original_clip_id,
        track_id,
        timeline_start: TimelineTime::ZERO,
        length: Duration::from_secs(4),
        properties: TextClipProperties::default(),
    });
    let project = TimelineSerialization {
        tracks: vec![Track {
            id: track_id,
            name: "Text 1".to_string(),
            kind: TrackKind::Text,
            locked: false,
            muted: false,
            visible: true,
        }],
        clips: vec![original_clip.clone()],
        ..TimelineSerialization::default()
    };
    let ges = build_ges_timeline(
        &project,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        export::ExportOptions::from_timeline(&project),
    )
    .unwrap();
    let mut runtime =
        TimelineRuntimeState::new("test.timeline.json".into(), project, ges.clone()).unwrap();
    let split_position = TimelineTime::from_frames(60);
    let (left, right) = original_clip
        .split_at(split_position, runtime.data.settings.frame_rate)
        .unwrap();
    let right_clip_id = right.id();

    edit_timeline(
        &mut runtime,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        EditAction::SplitClips {
            removed_clips: HashSet::from([original_clip_id]),
            added_clips: vec![left, right],
        },
    )
    .unwrap();

    assert!(runtime.data.clip(original_clip_id).is_some());
    assert!(runtime.data.clip(right_clip_id).is_some());
    assert_eq!(runtime.data.clips.len(), 2);
    assert_eq!(
        runtime.data.content_duration(),
        TimelineTime::from_frames(120)
    );
    data_parity_check(&runtime, &ges).unwrap();
}

#[test]
fn detects_timeline_and_ges_data_divergence() {
    let _gstreamer_test = crate::editor::tests::lock_gstreamer_test();
    gstreamer_editing_services::init().unwrap();
    let track_id = ulid(3);
    let clip_id = ulid(10);
    let project = TimelineSerialization {
        tracks: vec![Track {
            id: track_id,
            name: "Text 1".to_string(),
            kind: TrackKind::Text,
            locked: false,
            muted: false,
            visible: true,
        }],
        clips: vec![Clip::Text(TextClip {
            id: clip_id,
            track_id,
            timeline_start: TimelineTime::ZERO,
            length: Duration::from_secs(2),
            properties: TextClipProperties::default(),
        })],
        ..TimelineSerialization::default()
    };
    let ges = build_ges_timeline(
        &project,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        export::ExportOptions::from_timeline(&project),
    )
    .unwrap();
    let mut runtime =
        TimelineRuntimeState::new("test.timeline.json".into(), project, ges.clone()).unwrap();

    let rendered = ges
        .layers()
        .into_iter()
        .flat_map(|layer| layer.clips())
        .find(|clip| clip.name().as_deref() == Some(format!("opencut-clip-{clip_id}").as_str()))
        .unwrap();
    assert!(rendered.set_duration(gstreamer::ClockTime::from_mseconds(1_985)));
    // Parity checks read the edited GES objects, not the streaming state.
    // Waiting for a synchronous commit can deadlock during preview preroll.
    assert!(ges.commit());
    data_parity_check(&runtime, &ges).unwrap();

    runtime.data.clips[0].set_timeline_start(TimelineTime::ONE_FRAME);
    let error = data_parity_check(&runtime, &ges).unwrap_err();
    assert!(
        error.to_string().contains("starts at"),
        "unexpected error: {error}"
    );
}

#[test]
fn updates_video_transform_without_rebuilding_ges_clip() {
    let _gstreamer_test = crate::editor::tests::lock_gstreamer_test();
    gstreamer_editing_services::init().unwrap();
    let mut project = TimelineSerialization::with_test_tracks();
    project.settings.width = 1920;
    project.settings.height = 1080;
    let mut asset = audio_asset(100);
    asset.kind = MediaKind::Video;
    asset.width = 1920;
    asset.height = 1080;
    project.assets.push(asset);
    let Clip::Audio(mut media) = audio_clip(10, 0, 60) else {
        unreachable!();
    };
    media.track_id = ulid(1);
    project.clips.push(Clip::Video(media));
    let ges = gstreamer_editing_services::Timeline::new_audio_video();
    let layer = ges.append_layer();
    let rendered = gstreamer_editing_services::TestClip::new().unwrap();
    rendered.set_supported_formats(gstreamer_editing_services::TrackType::VIDEO);
    rendered
        .set_name(Some(&format!("opencut-clip-{}", ulid(10))))
        .unwrap();
    assert!(rendered.set_duration(gstreamer::ClockTime::from_seconds(2)));
    layer.add_clip(&rendered).unwrap();
    let mut runtime =
        TimelineRuntimeState::new("test.timeline.json".into(), project, ges.clone()).unwrap();
    let properties = VideoClipProperties {
        position_x: 120.0,
        position_y: 60.0,
        scale: 0.5,
    };
    let rebuild = edit_timeline(
        &mut runtime,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        EditAction::SetVideoProperties {
            clip_ids: vec![ulid(10)],
            properties,
        },
    )
    .unwrap();
    assert!(!rebuild);
    assert_eq!(runtime.video_backend.ges_timeline(), &ges);
    assert_eq!(
        layer.clips()[0],
        rendered
            .clone()
            .upcast::<gstreamer_editing_services::Clip>()
    );
    for (name, expected) in [
        ("posx", 600),
        ("posy", 330),
        ("width", 960),
        ("height", 540),
    ] {
        assert_eq!(
            rendered.child_property(name).unwrap().get::<i32>().unwrap(),
            expected
        );
    }
    assert_eq!(
        runtime
            .data
            .clip(ulid(10))
            .unwrap()
            .media()
            .unwrap()
            .video_properties,
        properties
    );
}

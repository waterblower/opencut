use super::*;
use crate::editor::ulid;
use crate::editor::{
    model::{MediaAsset, MediaKind},
    timeline::{AudioClipProperties, FrameRate, TimelineClip, TimelineTime, VideoClipProperties},
};
use std::{path::Path, time::Duration};

fn headless_test_pipeline() -> (gst::Pipeline, gst_app::AppSink, gst_audio::StreamVolume) {
    ges::init().unwrap();
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut project = TimelineSerialization::with_test_tracks();
    project.settings.frame_rate = FrameRate::new(24, 1);
    project.assets.push(MediaAsset {
        id: ulid(10),
        kind: MediaKind::Video,
        path: "data/tests/mini测试/地铁-出站-mini-480.mp4".into(),
        name: "test1".into(),
        duration: 5.0,
        width: 320,
        height: 180,
        framerate: 30.0,
        frame_rate_numerator: 30,
        frame_rate_denominator: 1,
        codec: "h264".into(),
        has_audio: true,
    });
    project.clips.push(TimelineClip {
        id: ulid(11),
        track_id: ulid(1),
        asset_id: ulid(10),
        timeline_start: TimelineTime::ZERO,
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::from_frames(12),
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    });
    project.clips.push(TimelineClip {
        id: ulid(12),
        track_id: ulid(1),
        asset_id: ulid(10),
        timeline_start: TimelineTime::from_frames(12),
        source_in: TimelineTime::from_frames(48),
        source_out: TimelineTime::from_frames(60),
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    });
    let audio_sink = gst::parse::bin_from_description(
        "volume name=gpui_audio_volume ! fakesink sync=false",
        true,
    )
    .unwrap();
    let volume_control = audio_sink
        .by_name("gpui_audio_volume")
        .unwrap()
        .dynamic_cast::<gst_audio::StreamVolume>()
        .unwrap();
    let audio_sink = audio_sink.upcast::<gst::Element>();
    let (pipeline, sink) = create_timeline_pipeline(&project, project_root, &audio_sink).unwrap();
    (pipeline, sink, volume_control)
}

#[test]
fn timeline_pipeline_plays_across_a_discontinuous_source_cut() {
    let _gstreamer_test = crate::editor::lock_gstreamer_test();
    let (pipeline, sink, _) = headless_test_pipeline();
    pipeline.set_state(gst::State::Playing).unwrap();

    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let crossed_cut = loop {
        if let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_mseconds(100))
            && sample
                .buffer()
                .and_then(|buffer| buffer.pts())
                .is_some_and(|pts| pts >= gst::ClockTime::from_mseconds(600))
        {
            break true;
        }
        if std::time::Instant::now() >= deadline {
            break false;
        }
    };
    let _ = pipeline.set_state(gst::State::Null);

    assert!(
        crossed_cut,
        "preview did not produce frames after the source cut"
    );
}

#[test]
fn timeline_pipeline_converts_the_output_frame_rate() {
    let _gstreamer_test = crate::editor::lock_gstreamer_test();
    let (pipeline, sink, _) = headless_test_pipeline();

    let negotiated_frame_rate = (|| -> Result<gst::Fraction, String> {
        pipeline
            .set_state(gst::State::Paused)
            .map_err(|error| format!("could not prepare test pipeline: {error}"))?;
        let sample = sink
            .try_pull_preroll(gst::ClockTime::from_seconds(5))
            .ok_or_else(|| "timed out waiting for test pipeline preroll".to_string())?;
        let caps = sample
            .caps()
            .ok_or_else(|| "test pipeline preroll had no caps".to_string())?;
        caps.structure(0)
            .ok_or_else(|| "test pipeline preroll caps were empty".to_string())?
            .get::<gst::Fraction>("framerate")
            .map_err(|error| format!("test pipeline caps had no frame rate: {error}"))
    })();
    let _ = pipeline.set_state(gst::State::Null);

    assert_eq!(negotiated_frame_rate.unwrap(), gst::Fraction::new(24, 1));
}

#[test]
fn timeline_audio_volume_uses_the_preview_volume_element() {
    let _gstreamer_test = crate::editor::lock_gstreamer_test();
    let (pipeline, sink, volume_control) = headless_test_pipeline();
    let video = VideoBackend::from_pipeline(pipeline, sink, volume_control).unwrap();

    video.set_volume(0.35);
    video.set_muted(false);

    let volume = video.volume();
    assert!((volume - 0.35).abs() < 0.000_001, "volume was {volume}");
}

#[test]
fn timeline_video_backend_toggles_playback_state() {
    let _gstreamer_test = crate::editor::lock_gstreamer_test();
    let (pipeline, sink, volume_control) = headless_test_pipeline();
    let video = VideoBackend::from_pipeline(pipeline, sink, volume_control).unwrap();

    assert!(video.paused());
    video.set_paused(false);
    video
        .pipeline()
        .state(gst::ClockTime::from_seconds(5))
        .0
        .expect("timeline pipeline did not start playing");
    assert!(!video.paused());

    video.set_paused(true);
    video
        .pipeline()
        .state(gst::ClockTime::from_seconds(5))
        .0
        .expect("timeline pipeline did not pause");
    assert!(video.paused());
}

use super::*;
use crate::editor::model::{
    AudioClipProperties, MediaAsset, MediaKind, TimelineClip, TimelineTime, VideoClipProperties,
};
use crate::editor::ulid;
use std::{path::Path, sync::mpsc, time::Duration};

fn headless_test_pipeline() -> (gst::Pipeline, gst_app::AppSink) {
    ges::init().unwrap();
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut project = Timeline::with_test_tracks();
    project.settings.frame_rate = super::super::model::FrameRate::new(24, 1);
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
    let audio_sink = gst::ElementFactory::make("fakesink")
        .property("sync", false)
        .build()
        .unwrap();
    create_timeline_pipeline(&project, project_root, &audio_sink).unwrap()
}

#[test]
fn timeline_pipeline_plays_across_a_discontinuous_source_cut() {
    let _gstreamer_test = crate::editor::lock_gstreamer_test();
    let (pipeline, sink) = headless_test_pipeline();
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
    let (pipeline, sink) = headless_test_pipeline();

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
fn timeline_video_shutdown_does_not_wait_on_the_frame_worker() {
    let _gstreamer_test = crate::editor::lock_gstreamer_test();
    let (pipeline, sink) = headless_test_pipeline();
    let video = Video::from_pipeline(pipeline, sink, false).unwrap();
    let (finished, completion) = mpsc::channel();
    std::thread::spawn(move || {
        drop(video);
        let _ = finished.send(());
    });

    completion
        .recv_timeout(Duration::from_secs(5))
        .expect("video shutdown did not finish within five seconds");
}

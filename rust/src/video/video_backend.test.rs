use super::*;
use std::{path::Path, time::Instant};
use url::Url;

#[test]
fn from_pipeline_accepts_an_empty_pipeline_without_caps() {
    gst::init().expect("could not initialize GStreamer");
    let pipeline = gst::Pipeline::new();
    let sink = gst_app::AppSink::builder().build();
    pipeline
        .add(&sink)
        .expect("could not add test sink to pipeline");
    let started_at = Instant::now();
    let video = VideoBackend::from_pipeline(pipeline, sink, test_volume_control())
        .expect("an empty pipeline should create a video backend");

    assert!(
        started_at.elapsed() < Duration::from_secs(1),
        "an empty pipeline should not wait for preroll"
    );
    assert_eq!(video.frame_size(), (1, 1));
    let element = crate::video::video(&video);

    assert_ne!(video.pipeline.current_state(), gst::State::Null);
    assert_eq!(element.width, gpui::px(1.0));
    assert_eq!(element.height, gpui::px(1.0));
}

#[test]
fn from_pipeline_reads_negotiated_frame_size() {
    gst::init().expect("could not initialize GStreamer");
    let pipeline = gst::parse::launch(
        "videotestsrc ! video/x-raw,width=320,height=180 ! appsink name=test_video_sink",
    )
    .expect("could not create test video pipeline")
    .downcast::<gst::Pipeline>()
    .expect("test video pipeline had an unexpected type");
    let sink = pipeline
        .by_name("test_video_sink")
        .expect("test video sink was not created")
        .downcast::<gst_app::AppSink>()
        .expect("test video sink had an unexpected type");

    let video = VideoBackend::from_pipeline(pipeline, sink, test_volume_control())
        .expect("could not create video backend");
    video
        .pipeline
        .state(gst::ClockTime::from_seconds(5))
        .0
        .expect("test video pipeline did not finish preparing");

    assert_eq!(video.frame_size(), (320, 180));
}

#[test]
fn measures_seek_to_half_of_a_video() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/tests/long video.mp4");
    let uri = Url::from_file_path(&source)
        .unwrap_or_else(|_| panic!("could not convert {} to a file URL", source.display()));
    let mut video = FileVideoBackend::open(&uri).expect("could not open test video");
    video.set_paused(true);
    video
        .pipeline()
        .state(gst::ClockTime::from_seconds(5))
        .0
        .expect("video did not pause before the measured seek");

    let duration = video.duration();
    assert!(!duration.is_zero(), "test video reported a zero duration");
    let target = duration / 2;

    let started_at = Instant::now();
    video
        .seek(target)
        .expect("could not seek test video to its midpoint");
    let elapsed = started_at.elapsed();

    eprintln!("FileVideoBackend::seek to 50% ({target:?} of {duration:?}) took {elapsed:?}");
}

fn test_volume_control() -> gst_audio::StreamVolume {
    gst::ElementFactory::make("volume")
        .build()
        .expect("could not create test volume control")
        .dynamic_cast::<gst_audio::StreamVolume>()
        .expect("test volume element does not implement StreamVolume")
}

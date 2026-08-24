use super::*;
use std::{path::Path, time::Instant};
use url::Url;

#[test]
fn missing_caps_does_not_stop_the_pipeline_or_panic_during_rendering() {
    gst::init().expect("could not initialize GStreamer");
    let pipeline = gst::Pipeline::new();
    let sink = gst_app::AppSink::builder().build();
    pipeline
        .add(&sink)
        .expect("could not add test sink to pipeline");
    let volume_control = gst::ElementFactory::make("volume")
        .build()
        .expect("could not create test volume control")
        .dynamic_cast::<gst_audio::StreamVolume>()
        .expect("test volume element does not implement StreamVolume");
    let video = VideoBackend {
        pipeline,
        sink,
        volume_control,
        current_frame: Arc::new(Mutex::new(None)),
        cached_position: Duration::ZERO,
        _frame_size: (1, 1),
    };
    video
        .pipeline
        .set_state(gst::State::Ready)
        .expect("could not prepare test pipeline");

    assert_eq!(video.frame_size(), (1, 1));
    let element = crate::video::video(&video);

    assert_eq!(video.pipeline.current_state(), gst::State::Ready);
    assert_eq!(element.width, gpui::px(1.0));
    assert_eq!(element.height, gpui::px(1.0));
}

#[test]
fn measures_seek_to_half_of_a_video() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/tests/long video.mp4");
    let uri = Url::from_file_path(&source)
        .unwrap_or_else(|_| panic!("could not convert {} to a file URL", source.display()));
    let mut video = VideoBackend::open(&uri).expect("could not open test video");
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
        .seek(target, false)
        .expect("could not seek test video to its midpoint");
    let elapsed = started_at.elapsed();

    eprintln!("VideoBackend::seek to 50% ({target:?} of {duration:?}) took {elapsed:?}");
}

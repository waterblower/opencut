use super::*;
use std::time::{Duration, Instant};

#[test]
fn repeated_drag_refreshes_allow_slow_frames_to_finish() {
    let _gstreamer_test = crate::editor::tests::lock_gstreamer_test();
    ges::init().unwrap();
    let pipeline = gst::parse::launch(
        "videotestsrc name=source pattern=black num-buffers=300 ! video/x-raw,format=RGBA,width=16,height=16,framerate=30/1 ! identity sleep-time=50000 ! appsink name=sink",
    ).unwrap().downcast::<gst::Pipeline>().unwrap();
    let sink = pipeline
        .by_name("sink")
        .unwrap()
        .downcast::<gst_app::AppSink>()
        .unwrap();
    let volume = gst::ElementFactory::make("volume")
        .build()
        .unwrap()
        .dynamic_cast::<gst_audio::StreamVolume>()
        .unwrap();
    let mut playback = VideoBackend::from_pipeline(pipeline.clone(), sink, volume).unwrap();
    pipeline.state(gst::ClockTime::from_seconds(5)).0.unwrap();
    let initial = playback.get_current_frame().unwrap();
    assert_eq!(
        initial.buffer().unwrap().map_readable().unwrap().as_slice()[0],
        0
    );
    pipeline
        .by_name("source")
        .unwrap()
        .set_property_from_str("pattern", "white");

    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(2) {
        try_refresh_timeline_video_frame(&mut playback).unwrap();
        std::thread::sleep(Duration::from_millis(16));
        let frame = playback.get_current_frame().unwrap();
        if frame.buffer().unwrap().map_readable().unwrap().as_slice()[0] == 255 {
            return;
        }
    }
    panic!("new pixels never arrived while drag refreshes continued");
}

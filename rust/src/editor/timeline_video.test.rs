use super::*;
use std::{path::Path, sync::mpsc, time::Duration};

fn headless_test_pipeline() -> (gst::Pipeline, gst_app::AppSink) {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/gpui-video-player/assets/test1.mp4");
    let url = Url::from_file_path(path).unwrap();
    let (pipeline, sink) = create_timeline_pipeline(&url, 24, 1).unwrap();
    let audio_sink = gst::ElementFactory::make("fakesink")
        .property("sync", false)
        .build()
        .unwrap();
    pipeline.set_property("audio-sink", &audio_sink);
    (pipeline, sink)
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
    let video =
        Video::from_gst_pipeline_with_options(pipeline, sink, None, VideoOptions::default())
            .unwrap();
    let (finished, completion) = mpsc::channel();
    std::thread::spawn(move || {
        drop(video);
        let _ = finished.send(());
    });

    completion
        .recv_timeout(Duration::from_secs(5))
        .expect("video shutdown did not finish within five seconds");
}

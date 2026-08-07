use crate::video_backend::{Video, VideoOptions};
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use url::Url;

pub(super) fn create_timeline_video(
    uri: &Url,
    options: VideoOptions,
    frame_rate_numerator: u32,
    frame_rate_denominator: u32,
) -> Result<Video, String> {
    let (pipeline, sink) =
        create_timeline_pipeline(uri, frame_rate_numerator, frame_rate_denominator)?;
    Video::from_gst_pipeline_with_options(pipeline, sink, None, options)
        .map_err(|error| format!("could not initialize timeline video: {error}"))
}

fn create_timeline_pipeline(
    uri: &Url,
    frame_rate_numerator: u32,
    frame_rate_denominator: u32,
) -> Result<(gst::Pipeline, gst_app::AppSink), String> {
    gst::init().map_err(|error| format!("could not initialize GStreamer: {error}"))?;
    let description = format!(
        "playbin uri=\"{}\" video-sink=\"videorate ! videoscale ! videoconvert ! appsink name=gpui_video drop=true max-buffers=200 enable-last-sample=false caps=video/x-raw,format=NV12,pixel-aspect-ratio=1/1,framerate={}/{}\"",
        uri.as_str(),
        frame_rate_numerator.max(1),
        frame_rate_denominator.max(1),
    );
    let pipeline = gst::parse::launch(&description)
        .map_err(|error| format!("could not create timeline video pipeline: {error}"))?
        .downcast::<gst::Pipeline>()
        .map_err(|_| "timeline video pipeline had an unexpected type".to_string())?;
    let video_sink: gst::Element = pipeline.property("video-sink");
    let pad = video_sink
        .pads()
        .first()
        .cloned()
        .ok_or_else(|| "timeline video sink had no pad".to_string())?
        .dynamic_cast::<gst::GhostPad>()
        .map_err(|_| "timeline video sink pad had an unexpected type".to_string())?;
    let bin = pad
        .parent_element()
        .ok_or_else(|| "timeline video sink had no parent".to_string())?
        .downcast::<gst::Bin>()
        .map_err(|_| "timeline video sink parent had an unexpected type".to_string())?;
    let sink = bin
        .by_name("gpui_video")
        .ok_or_else(|| "timeline video appsink was not created".to_string())?
        .downcast::<gst_app::AppSink>()
        .map_err(|_| "timeline video sink had an unexpected type".to_string())?;

    Ok((pipeline, sink))
}

#[cfg(test)]
mod tests {
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
}

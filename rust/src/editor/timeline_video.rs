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
#[path = "timeline_video.test.rs"]
mod tests;

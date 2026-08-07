use super::{export::ExportOptions, export_gstreamer::build_timeline, model::Project};
use crate::video_backend::{Video, VideoOptions};
use ges::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_editing_services as ges;
use std::path::Path;

pub(super) fn create_timeline_video(
    project: &Project,
    project_root: &Path,
    options: VideoOptions,
) -> Result<Video, String> {
    initialize_gstreamer()?;
    let audio_sink = preview_audio_sink()?;
    let (pipeline, sink) = create_timeline_pipeline(project, project_root, &audio_sink)?;
    Video::from_gst_pipeline_with_options(pipeline, sink, None, options)
        .map_err(|error| format!("could not initialize timeline video: {error}"))
}

pub(super) fn set_timeline_audio(video: &Video, volume: f64, muted: bool) {
    if let Some(control) = video.pipeline().by_name("gpui_audio_volume") {
        control.set_property("volume", volume.clamp(0.0, 1.0));
        control.set_property("mute", muted);
    }
}

fn create_timeline_pipeline(
    project: &Project,
    project_root: &Path,
    audio_sink: &gst::Element,
) -> Result<(gst::Pipeline, gst_app::AppSink), String> {
    initialize_gstreamer()?;
    let options = ExportOptions::from_project(project);
    let timeline = build_timeline(project, project_root, options)?;
    let video_sink = gst::parse::bin_from_description(
        "queue ! videoconvert ! appsink name=gpui_video drop=true max-buffers=200 enable-last-sample=false caps=video/x-raw,format=NV12,pixel-aspect-ratio=1/1",
        true,
    )
    .map_err(|error| format!("could not create timeline preview video sink: {error}"))?;
    let sink = video_sink
        .by_name("gpui_video")
        .ok_or_else(|| "timeline video appsink was not created".to_string())?
        .downcast::<gst_app::AppSink>()
        .map_err(|_| "timeline video sink had an unexpected type".to_string())?;

    let pipeline = ges::Pipeline::new();
    pipeline.preview_set_video_sink(Some(&video_sink));
    pipeline.preview_set_audio_sink(Some(audio_sink));
    pipeline
        .set_timeline(&timeline)
        .map_err(|error| format!("could not attach the preview timeline: {error}"))?;
    pipeline
        .set_mode(ges::PipelineFlags::FULL_PREVIEW)
        .map_err(|error| format!("could not enable GStreamer preview mode: {error}"))?;

    Ok((pipeline.upcast(), sink))
}

fn initialize_gstreamer() -> Result<(), String> {
    ges::init().map_err(|error| format!("could not initialize GStreamer Editing Services: {error}"))
}

fn preview_audio_sink() -> Result<gst::Element, String> {
    let sink = gst::parse::bin_from_description(
        "audioconvert ! audioresample ! volume name=gpui_audio_volume ! autoaudiosink",
        true,
    )
    .map_err(|error| format!("could not create timeline preview audio sink: {error}"))?;
    if let Some(control) = sink.by_name("gpui_audio_volume") {
        control.set_property("mute", true);
    }
    Ok(sink.upcast())
}

#[cfg(test)]
#[path = "timeline_video.test.rs"]
mod tests;

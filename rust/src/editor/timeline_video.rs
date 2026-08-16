use super::{
    clip_render_plan::resolve_visual_clip_render_plan, export::ExportOptions,
    export_gstreamer::build_timeline, timeline::Timeline,
};
use crate::video::VideoBackend;
use ges::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_editing_services as ges;
use std::path::Path;
use ulid::Ulid;

pub(super) fn create_timeline_video(
    timeline: &Timeline,
    project_root: &Path,
) -> Result<VideoBackend, String> {
    initialize_gstreamer()?;
    let audio_sink = preview_audio_sink()?;
    let (pipeline, sink) = create_timeline_pipeline(timeline, project_root, &audio_sink)?;
    VideoBackend::from_pipeline(pipeline, sink)
        .map_err(|error| format!("could not initialize timeline video: {error}"))
}

pub(super) fn set_timeline_audio(pipeline: &gst::Pipeline, volume: f64, muted: bool) {
    if let Some(control) = pipeline.by_name("gpui_audio_volume") {
        control.set_property("volume", volume.clamp(0.0, 1.0));
        control.set_property("mute", muted);
    }
}

pub(super) fn update_timeline_video_position(
    video: &mut VideoBackend,
    timeline_data: &Timeline,
    clip_id: Ulid,
    refresh_frame: bool,
) -> Result<(), String> {
    let clip = timeline_data
        .clip(clip_id)
        .ok_or_else(|| format!("Clip {clip_id} is unavailable."))?;
    let asset = timeline_data
        .asset(clip.asset_id)
        .ok_or_else(|| format!("Clip {clip_id} has no source media."))?;
    let pipeline = video
        .pipeline()
        .downcast::<ges::Pipeline>()
        .map_err(|_| "timeline preview pipeline had an unexpected type".to_string())?;
    let timeline = pipeline
        .timeline()
        .ok_or_else(|| "timeline preview pipeline has no timeline".to_string())?;
    let clip_name = format!("opencut-clip-{clip_id}");
    let rendered_clip = timeline
        .layers()
        .into_iter()
        .flat_map(|layer| layer.clips())
        .find(|rendered_clip| rendered_clip.name().as_deref() == Some(clip_name.as_str()))
        .ok_or_else(|| format!("timeline preview has no rendered clip for {clip_id}"))?;
    let options = ExportOptions::from_timeline(timeline_data);
    let plan = resolve_visual_clip_render_plan(
        clip.video_properties,
        asset.width,
        asset.height,
        timeline_data.settings.width,
        timeline_data.settings.height,
        options.width.max(2) as f64,
        options.height.max(2) as f64,
    );
    for (name, value) in [
        (
            "posx",
            plan.visible
                .left
                .round()
                .clamp(i32::MIN as f64, i32::MAX as f64) as i32,
        ),
        (
            "posy",
            plan.visible
                .top
                .round()
                .clamp(i32::MIN as f64, i32::MAX as f64) as i32,
        ),
    ] {
        rendered_clip
            .set_child_property(name, value)
            .map_err(|error| format!("could not update preview video {name}: {error}"))?;
    }
    if !refresh_frame {
        return Ok(());
    }
    if !timeline.commit_sync() {
        return Err("GStreamer could not commit the preview position.".to_string());
    }
    video.seek(video.position(), true)
}

fn create_timeline_pipeline(
    timeline: &Timeline,
    project_root: &Path,
    audio_sink: &gst::Element,
) -> Result<(gst::Pipeline, gst_app::AppSink), String> {
    initialize_gstreamer()?;
    let options = ExportOptions::from_timeline(timeline);
    let ges_timeline = build_timeline(timeline, project_root, options)?;
    let video_sink = gst::parse::bin_from_description(
        "queue ! videoconvert ! appsink name=opencut_timeline_video drop=true max-buffers=3 enable-last-sample=false caps=video/x-raw,format=NV12,pixel-aspect-ratio=1/1",
        true,
    )
    .map_err(|error| format!("could not create timeline preview video sink: {error}"))?;
    let sink = video_sink
        .by_name("opencut_timeline_video")
        .ok_or_else(|| "timeline video appsink was not created".to_string())?
        .downcast::<gst_app::AppSink>()
        .map_err(|_| "timeline video sink had an unexpected type".to_string())?;

    let pipeline = ges::Pipeline::new();
    pipeline.preview_set_video_sink(Some(&video_sink));
    pipeline.preview_set_audio_sink(Some(audio_sink));
    pipeline
        .set_timeline(&ges_timeline)
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

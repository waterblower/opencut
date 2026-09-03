use super::{
    clip_render_plan::resolve_visual_clip_render_plan, export::ExportOptions,
    timeline::TimelineSerialization,
};
use crate::video::VideoBackend;
use anyhow::{Context as _, Result};
use ges::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_audio as gst_audio;
use gstreamer_editing_services as ges;

use ulid::Ulid;

pub struct TimelineVideoBackend {
    ges_timeline: ges::Timeline,
    playback: VideoBackend,
}

impl TimelineVideoBackend {
    pub fn new(ges_timeline: ges::Timeline) -> Result<TimelineVideoBackend> {
        let playback =
            create_timeline_playback(&ges_timeline).context("TimelineVideoBackend::new failed")?;
        Ok(Self {
            ges_timeline,
            playback,
        })
    }

    pub fn ges_timeline(&self) -> &ges::Timeline {
        &self.ges_timeline
    }

    pub fn playback(&self) -> &VideoBackend {
        &self.playback
    }

    pub fn playback_mut(&mut self) -> &mut VideoBackend {
        &mut self.playback
    }
}

pub(super) fn update_timeline_video_position(
    video: &mut TimelineVideoBackend,
    timeline_data: &TimelineSerialization,
    clip_id: Ulid,
    refresh_frame: bool,
) -> anyhow::Result<()> {
    let clip = timeline_data
        .clip(clip_id)
        .ok_or_else(|| anyhow::anyhow!("Clip {clip_id} is unavailable."))?;
    let clip = clip
        .media()
        .ok_or_else(|| anyhow::anyhow!("Clip {clip_id} is not a media clip."))?;
    let asset = timeline_data
        .asset(clip.asset_id)
        .ok_or_else(|| anyhow::anyhow!("Clip {clip_id} has no source media."))?;
    let timeline = &video.ges_timeline;
    let clip_name = format!("opencut-clip-{clip_id}");
    let rendered_clip = timeline
        .layers()
        .into_iter()
        .flat_map(|layer| layer.clips())
        .find(|rendered_clip| rendered_clip.name().as_deref() == Some(clip_name.as_str()))
        .ok_or_else(|| anyhow::anyhow!("timeline preview has no rendered clip for {clip_id}"))?;
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
            .map_err(|error| anyhow::anyhow!("could not update preview video {name}: {error}"))?;
    }
    if !refresh_frame {
        return Ok(());
    }
    if !timeline.commit_sync() {
        anyhow::bail!("GStreamer could not commit the preview position.");
    }
    let playback = &mut video.playback;
    let position = playback.position();
    playback.seek(position)
}

pub fn create_timeline_pipeline_v2(
    ges_timeline: &ges::Timeline,
    audio_sink: &gst::Element,
) -> anyhow::Result<(gst::Pipeline, gst_app::AppSink)> {
    let video_sink = gst::parse::bin_from_description(
        "queue ! videoconvert ! appsink name=opencut_timeline_video drop=true max-buffers=3 enable-last-sample=false caps=video/x-raw,format=NV12,pixel-aspect-ratio=1/1",
        true,
    )
    .map_err(|error| anyhow::anyhow!("could not create timeline preview video sink: {error}"))?;
    let sink = video_sink
        .by_name("opencut_timeline_video")
        .ok_or_else(|| anyhow::anyhow!("timeline video appsink was not created"))?
        .downcast::<gst_app::AppSink>()
        .map_err(|_| anyhow::anyhow!("timeline video sink had an unexpected type"))?;

    let pipeline = ges::Pipeline::new();
    pipeline.preview_set_video_sink(Some(&video_sink));
    pipeline.preview_set_audio_sink(Some(audio_sink));
    pipeline
        .set_timeline(ges_timeline)
        .map_err(|error| anyhow::anyhow!("could not attach the preview timeline: {error}"))?;
    pipeline
        .set_mode(ges::PipelineFlags::FULL_PREVIEW)
        .map_err(|error| anyhow::anyhow!("could not enable GStreamer preview mode: {error}"))?;

    Ok((pipeline.upcast(), sink))
}

fn create_timeline_playback(timeline: &ges::Timeline) -> Result<VideoBackend> {
    (|| -> Result<VideoBackend> {
        initialize_gstreamer()?;
        let (audio_sink, volume_control) = preview_audio_sink()?;
        let (pipeline, sink) = create_timeline_pipeline_v2(timeline, &audio_sink)?;
        VideoBackend::from_pipeline(pipeline, sink, volume_control)
    })()
    .context("create_timeline_playback failed")
}

fn initialize_gstreamer() -> anyhow::Result<()> {
    ges::init().map_err(|error| {
        anyhow::anyhow!("could not initialize GStreamer Editing Services: {error}")
    })
}

fn preview_audio_sink() -> anyhow::Result<(gst::Element, gst_audio::StreamVolume)> {
    let sink = gst::parse::bin_from_description(
        "audioconvert ! audioresample ! volume name=gpui_audio_volume ! autoaudiosink",
        true,
    )
    .map_err(|error| anyhow::anyhow!("could not create timeline preview audio sink: {error}"))?;
    let control = sink
        .by_name("gpui_audio_volume")
        .ok_or_else(|| anyhow::anyhow!("timeline preview volume control was not created"))?
        .dynamic_cast::<gst_audio::StreamVolume>()
        .map_err(|_| anyhow::anyhow!("timeline preview volume control has an unexpected type"))?;
    Ok((sink.upcast(), control))
}

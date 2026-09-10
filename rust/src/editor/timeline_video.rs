use crate::video::VideoBackend;
use anyhow::{Context as _, Result};
use ges::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_audio as gst_audio;
use gstreamer_editing_services as ges;

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

pub fn refresh_timeline_video_frame(playback: &mut VideoBackend) -> anyhow::Result<()> {
    let position = playback.position();
    playback.seek(position).with_context(|| {
        format!(
            "could not refresh timeline preview at {}:{}",
            file!(),
            line!()
        )
    })
}

pub fn try_refresh_timeline_video_frame(playback: &mut VideoBackend) -> anyhow::Result<()> {
    // A flushing seek cancels pending preroll. Let that frame reach the sink
    // before requesting another one, even when mouse events arrive faster.
    let (result, _, _) = playback.pipeline().state(gst::ClockTime::ZERO);
    match result {
        Ok(gst::StateChangeSuccess::Async) => return Ok(()),
        Err(error) => {
            anyhow::bail!(
                "preview pipeline could not finish rendering: {error} at {}:{}",
                file!(),
                line!(),
            );
        }
        _ => {}
    }
    refresh_timeline_video_frame(playback)
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

#[cfg(test)]
#[path = "tests/timeline_video.test.rs"]
mod tests;

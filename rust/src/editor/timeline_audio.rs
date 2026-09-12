use super::*;
use anyhow::{anyhow, bail};
use ges::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_editing_services as ges;
use std::path::Path;
use std::time::{Duration, Instant};

use opencut_player::transcribe::MAX_TRANSCRIPTION_DURATION;

/// Render the saved timeline's audio into a normalized WAV in memory.
pub fn render_audio_wav(timeline: &TimelineSerialization, project_root: &Path) -> Result<Vec<u8>> {
    let duration = timeline.duration(timeline.content_duration());
    if duration.is_zero() || duration > MAX_TRANSCRIPTION_DURATION {
        bail!(
            "timeline duration must be positive and at most {} seconds (got {duration:?})",
            MAX_TRANSCRIPTION_DURATION.as_secs(),
        );
    }
    let has_audio = timeline.tracks.iter().any(|track| {
        timeline.clips_on_track(track.id).any(|clip| {
            let Some(media) = clip.media() else {
                return false;
            };
            let Some(asset) = timeline.asset(media.asset_id) else {
                return false;
            };
            asset.has_audio
                && !clip_render_plan::resolve_audio_clip_render_plan(
                    track.muted,
                    media.audio_properties,
                )
                .muted
        })
    });
    if !has_audio {
        bail!(
            "timeline contains no enabled audio clips at {}:{}",
            file!(),
            line!()
        );
    }
    ges::init()?;
    let ges_timeline = export_gstreamer::build_ges_timeline(
        timeline,
        project_root,
        export::ExportOptions::from_timeline(timeline),
        true,
    )?;
    let audio_sink = gst::parse::bin_from_description(
        "audioconvert ! audioresample ! audio/x-raw,format=S16LE,rate=16000,channels=1,layout=interleaved ! appsink name=transcription_audio sync=false max-buffers=8 drop=false enable-last-sample=false",
        true,
    )?;
    let sink = audio_sink
        .by_name("transcription_audio")
        .ok_or_else(|| anyhow!("missing audio sink at {}:{}", file!(), line!()))?
        .downcast::<gst_app::AppSink>()
        .map_err(|_| anyhow!("invalid audio sink type at {}:{}", file!(), line!()))?;
    let pipeline = ges::Pipeline::new();
    pipeline.preview_set_audio_sink(Some(&audio_sink));
    pipeline.set_timeline(&ges_timeline)?;
    pipeline.set_mode(ges::PipelineFlags::AUDIO_PREVIEW)?;
    let bus = pipeline
        .bus()
        .ok_or_else(|| anyhow!("missing audio bus at {}:{}", file!(), line!()))?;
    let result = collect_audio_wav(&pipeline, &sink, &bus, duration);
    let stopped = pipeline.set_state(gst::State::Null);
    let wav = result?;
    stopped?;
    Ok(wav)
}

fn collect_audio_wav(
    pipeline: &ges::Pipeline,
    sink: &gst_app::AppSink,
    bus: &gst::Bus,
    duration: Duration,
) -> Result<Vec<u8>> {
    let samples = (duration.as_secs_f64() * 16_000.0).round() as usize;
    let mut wav = vec![0; 44 + samples * 2];
    pipeline.set_state(gst::State::Playing)?;
    let mut last_sample = Instant::now();
    let mut received_audio = false;
    loop {
        if let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_mseconds(100)) {
            received_audio = true;
            last_sample = Instant::now();
            let buffer = sample
                .buffer()
                .ok_or_else(|| anyhow!("missing audio buffer at {}:{}", file!(), line!()))?;
            let pts = buffer
                .pts()
                .ok_or_else(|| anyhow!("missing audio timestamp at {}:{}", file!(), line!()))?;
            let start = ((pts.nseconds() as u128 * 16_000 + 500_000_000) / 1_000_000_000) as usize;
            let data = buffer.map_readable()?;
            if data.len() % 2 != 0 {
                bail!("unaligned audio buffer at {}:{}", file!(), line!());
            }
            if start < samples {
                let count = data.len().min((samples - start) * 2);
                wav[44 + start * 2..44 + start * 2 + count].copy_from_slice(&data[..count]);
            }
        }
        while let Some(message) = bus.pop() {
            if let gst::MessageView::Error(error) = message.view() {
                bail!(
                    "timeline audio rendering failed: {:?} ({:?}) at {}:{}",
                    error.error(),
                    error.debug(),
                    file!(),
                    line!()
                );
            }
        }
        if sink.is_eos() {
            break;
        }
        if last_sample.elapsed() > Duration::from_secs(60) {
            bail!(
                "timeline audio rendering stalled at {}:{}",
                file!(),
                line!()
            );
        }
    }
    if !received_audio {
        bail!("timeline produced no audio at {}:{}", file!(), line!());
    }
    opencut_player::transcribe::audio::write_wav_header(wav)
}

#[cfg(test)]
#[path = "tests/timeline_audio.test.rs"]
mod tests;

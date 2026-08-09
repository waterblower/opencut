use super::{
    clip_render_plan::{resolve_audio_clip_render_plan, resolve_visual_clip_render_plan},
    export::{ExportEncoder, ExportOptions},
    model::{MediaAsset, MediaKind, TimelineClip, TrackKind, VideoClipProperties},
    timeline::Timeline,
};
use ges::prelude::*;
use gstreamer as gst;
use gstreamer_editing_services as ges;
use gstreamer_pbutils as gst_pbutils;
use std::{collections::HashMap, fs, path::Path, sync::Mutex, time::Duration};
use url::Url;

const AUDIO_BIT_RATE: i32 = 192_000;
static EXPORT_ENCODER_LOCK: Mutex<()> = Mutex::new(());

impl ExportEncoder {
    fn factory_name(self) -> &'static str {
        match self {
            Self::Hardware => "vtenc_h264_hw",
            Self::Software => "x264enc",
        }
    }
}

pub(super) fn export_timeline(
    timeline: &Timeline,
    project_root: &Path,
    output: &Path,
    options: ExportOptions,
    mut report_progress: impl FnMut(f32),
) -> Result<(), String> {
    if timeline.clips.is_empty() {
        return Err("Add at least one clip before exporting.".to_string());
    }
    ges::init()
        .map_err(|error| format!("could not initialize GStreamer Editing Services: {error}"))?;
    report_progress(0.0);

    let temporary_output = TemporaryOutput::new(temporary_output_path(output))?;
    let _encoder_lock = EXPORT_ENCODER_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    export_timeline_with_encoder(
        timeline,
        project_root,
        &temporary_output.path,
        options,
        options.encoder,
        &mut report_progress,
    )?;

    if output.is_file() {
        fs::remove_file(output)
            .map_err(|error| format!("could not replace {}: {error}", output.display()))?;
    }
    fs::rename(&temporary_output.path, output).map_err(|error| {
        format!(
            "could not move completed export to {}: {error}",
            output.display()
        )
    })?;
    report_progress(1.0);
    Ok(())
}

fn export_timeline_with_encoder(
    timeline_data: &Timeline,
    project_root: &Path,
    temporary_output: &Path,
    options: ExportOptions,
    encoder: ExportEncoder,
    report_progress: &mut impl FnMut(f32),
) -> Result<(), String> {
    let timeline = build_timeline(timeline_data, project_root, options)?;
    let profile = encoding_profile(options);
    let _encoder_selection = EncoderSelection::for_export(encoder)?;
    let pipeline = ges::Pipeline::new();
    configure_export_elements(&pipeline, options.video_bit_rate);
    pipeline
        .set_timeline(&timeline)
        .map_err(|error| format!("could not attach the export timeline: {error}"))?;

    let output_uri = Url::from_file_path(temporary_output).map_err(|_| {
        format!(
            "could not convert {} to a file URL",
            temporary_output.display()
        )
    })?;
    pipeline
        .set_render_settings(output_uri.as_str(), &profile)
        .map_err(|error| format!("could not configure GStreamer export: {error}"))?;
    pipeline
        .set_mode(ges::PipelineFlags::RENDER)
        .map_err(|error| format!("could not enable GStreamer render mode: {error}"))?;

    log::info!("Starting GStreamer export with {}", encoder.factory_name());
    let result = render_pipeline(
        &pipeline,
        timeline_data.duration(timeline_data.content_duration()),
        report_progress,
    );
    let _ = pipeline.set_state(gst::State::Null);
    result
}

pub(super) fn build_timeline(
    timeline_data: &Timeline,
    project_root: &Path,
    options: ExportOptions,
) -> Result<ges::Timeline, String> {
    let timeline = ges::Timeline::new_audio_video();
    let video_caps = gst::Caps::builder("video/x-raw")
        .field("width", options.width.max(2) as i32)
        .field("height", options.height.max(2) as i32)
        .field(
            "framerate",
            gst::Fraction::new(
                options.frame_rate.numerator as i32,
                options.frame_rate.denominator as i32,
            ),
        )
        .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
        .build();
    for track in timeline.tracks() {
        if track.track_type().contains(ges::TrackType::VIDEO) {
            track.set_restriction_caps(&video_caps);
        }
        track.set_mixing(true);
    }

    let mut assets: HashMap<u64, ges::UriClipAsset> = HashMap::new();
    for timeline_track in &timeline_data.tracks {
        let layer = timeline.append_layer();
        let mut clips = timeline_data
            .clips_on_track(timeline_track.id)
            .collect::<Vec<_>>();
        clips.sort_by_key(|clip| clip.timeline_start);
        for clip in clips {
            let asset = timeline_data
                .asset(clip.asset_id)
                .ok_or_else(|| format!("Clip {} has no source media.", clip.id))?;
            let track_types =
                exported_track_types(timeline_track, clip, asset.kind, asset.has_audio);
            if track_types.is_empty() {
                continue;
            }
            let uri_asset = if let Some(asset) = assets.get(&asset.id) {
                asset.clone()
            } else {
                let source = project_root.join(&asset.path);
                let uri = Url::from_file_path(&source)
                    .map_err(|_| format!("could not convert {} to a file URL", source.display()))?;
                let uri_asset = ges::UriClipAsset::request_sync(uri.as_str())
                    .map_err(|error| format!("could not inspect {}: {error}", source.display()))?;
                assets.insert(asset.id, uri_asset.clone());
                uri_asset
            };

            let start = clock_time(timeline_data.duration(clip.timeline_start));
            let inpoint = source_in(timeline_data, clip, asset.kind, track_types);
            let duration = clock_time(timeline_data.duration(clip.duration()));
            let ges_clip = layer
                .add_asset(&uri_asset, start, inpoint, duration, track_types)
                .map_err(|error| {
                    format!(
                        "could not add {} to the export timeline: {error}",
                        asset.name
                    )
                })?;
            ges_clip
                .set_name(Some(&format!("opencut-clip-{}", clip.id)))
                .map_err(|error| format!("could not identify clip {}: {error}", clip.id))?;
            if track_types.contains(ges::TrackType::VIDEO) {
                apply_video_transform(
                    &ges_clip,
                    timeline_data,
                    asset,
                    options,
                    clip.video_properties,
                )?;
            }
            if track_types.contains(ges::TrackType::AUDIO) {
                let audio_plan =
                    resolve_audio_clip_render_plan(timeline_track.muted, clip.audio_properties);
                let gain = if audio_plan.muted {
                    0.0
                } else {
                    audio_plan.gain_linear
                };
                // URI clips expose the audio source's `volume` child property.
                let _ = ges_clip.set_child_property("volume", gain);
            }
        }
    }
    if !timeline.commit_sync() {
        return Err("GStreamer could not commit the export timeline.".to_string());
    }
    Ok(timeline)
}

pub(super) fn apply_video_transform(
    clip: &ges::Clip,
    timeline: &Timeline,
    asset: &MediaAsset,
    options: ExportOptions,
    properties: VideoClipProperties,
) -> Result<(), String> {
    let plan = resolve_visual_clip_render_plan(
        properties,
        asset.width,
        asset.height,
        timeline.settings.width,
        timeline.settings.height,
        options.width.max(2) as f64,
        options.height.max(2) as f64,
    );

    if plan.crop.left != 0 || plan.crop.right != 0 || plan.crop.top != 0 || plan.crop.bottom != 0 {
        let effect = ges::Effect::new(&format!(
            "videocrop left={} right={} top={} bottom={}",
            plan.crop.left, plan.crop.right, plan.crop.top, plan.crop.bottom
        ))
        .map_err(|error| format!("could not create video crop effect: {error}"))?;
        clip.add_top_effect(&effect, 0)
            .map_err(|error| format!("could not apply video crop: {error}"))?;
    }

    for (name, value) in [
        ("posx", rounded_i32(plan.visible.left)),
        ("posy", rounded_i32(plan.visible.top)),
        ("width", rounded_i32(plan.visible.width).max(1)),
        ("height", rounded_i32(plan.visible.height).max(1)),
    ] {
        clip.set_child_property(name, value)
            .map_err(|error| format!("could not apply video {name}: {error}"))?;
    }
    clip.set_child_property("alpha", plan.opacity)
        .map_err(|error| format!("could not apply video opacity: {error}"))?;
    Ok(())
}

fn rounded_i32(value: f64) -> i32 {
    value.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32
}

fn exported_track_types(
    track: &super::model::TimelineTrack,
    clip: &TimelineClip,
    asset_kind: MediaKind,
    has_audio: bool,
) -> ges::TrackType {
    let mut types = ges::TrackType::empty();
    if track.kind == TrackKind::Video
        && track.visible
        && matches!(asset_kind, MediaKind::Video | MediaKind::Image)
    {
        types |= ges::TrackType::VIDEO;
    }
    if has_audio && !resolve_audio_clip_render_plan(track.muted, clip.audio_properties).muted {
        types |= ges::TrackType::AUDIO;
    }
    types
}

fn source_in(
    timeline: &Timeline,
    clip: &TimelineClip,
    asset_kind: MediaKind,
    track_types: ges::TrackType,
) -> gst::ClockTime {
    if asset_kind == MediaKind::Image {
        return gst::ClockTime::ZERO;
    }
    if track_types.contains(ges::TrackType::VIDEO) {
        return clock_time(Duration::from_secs_f64(timeline.source_start_seconds(clip)));
    }
    clock_time(timeline.audio_duration(clip.source_in))
}

fn encoding_profile(options: ExportOptions) -> gst_pbutils::EncodingContainerProfile {
    let container_caps = gst::Caps::builder("video/quicktime")
        .field("variant", "iso")
        .build();
    let video_caps = gst::Caps::builder("video/x-h264")
        .field("stream-format", "avc")
        .field("alignment", "au")
        .build();
    let video_restriction = gst::Caps::builder("video/x-raw")
        .field("width", options.width.max(2) as i32)
        .field("height", options.height.max(2) as i32)
        .field(
            "framerate",
            gst::Fraction::new(
                options.frame_rate.numerator as i32,
                options.frame_rate.denominator as i32,
            ),
        )
        .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
        .build();
    let audio_caps = gst::Caps::builder("audio/mpeg")
        .field("mpegversion", 4i32)
        .field("stream-format", "raw")
        .build();

    let video = gst_pbutils::EncodingVideoProfile::builder(&video_caps)
        .name("OpenCut H.264")
        .restriction(&video_restriction)
        .presence(1)
        .build();
    let audio = gst_pbutils::EncodingAudioProfile::builder(&audio_caps)
        .name("OpenCut AAC")
        .presence(1)
        .build();
    gst_pbutils::EncodingContainerProfile::builder(&container_caps)
        .name("OpenCut MP4")
        .add_profile(video)
        .add_profile(audio)
        .build()
}

fn configure_export_elements(pipeline: &ges::Pipeline, video_bit_rate: usize) {
    let kilobits_per_second = (video_bit_rate / 1_000).clamp(1, u32::MAX as usize) as u32;
    pipeline.connect_deep_element_added(move |_, _, element| {
        let Some(factory) = element.factory() else {
            return;
        };
        let factory_name = factory.name();
        match factory_name.as_str() {
            "x264enc" => {
                element.set_property("bitrate", kilobits_per_second);
                element.set_property_from_str("pass", "cbr");
                element.set_property_from_str("speed-preset", "veryfast");
            }
            "vtenc_h264" | "vtenc_h264_hw" => {
                element.set_property("bitrate", kilobits_per_second);
                // Avoid reordered frames because qtmux requires stable PTS/DTS
                // when GES switches or trims timeline sources.
                element.set_property("allow-frame-reordering", false);
            }
            "faac" => element.set_property("bitrate", AUDIO_BIT_RATE),
            _ => {}
        }

        if matches!(
            factory_name.as_str(),
            "videoconvert" | "videoscale" | "videoconvertscale"
        ) && element.find_property("n-threads").is_some()
        {
            // Zero lets GStreamer select a thread count based on the machine.
            element.set_property("n-threads", 0u32);
        }

        if matches!(
            factory_name.as_str(),
            "x264enc" | "vtenc_h264" | "vtenc_h264_hw"
        ) {
            log::info!("GStreamer export is using {factory_name}");
        }
    });
}

struct EncoderSelection {
    previous_ranks: Vec<(gst::ElementFactory, gst::Rank)>,
}

impl EncoderSelection {
    fn for_export(video_encoder: ExportEncoder) -> Result<Self, String> {
        let selected_video =
            gst::ElementFactory::find(video_encoder.factory_name()).ok_or_else(|| {
                format!(
                    "GStreamer H.264 encoder `{}` is unavailable.",
                    video_encoder.factory_name()
                )
            })?;
        let faac = gst::ElementFactory::find("faac").ok_or_else(|| {
            "GStreamer AAC encoder `faac` is unavailable; install the bad plugin set.".to_string()
        })?;
        let mut previous_ranks = vec![
            (selected_video.clone(), selected_video.rank()),
            (faac.clone(), faac.rank()),
        ];
        selected_video.set_rank(gst::Rank::PRIMARY + 100);
        faac.set_rank(gst::Rank::PRIMARY + 100);
        if let Some(atenc) = gst::ElementFactory::find("atenc") {
            previous_ranks.push((atenc.clone(), atenc.rank()));
            atenc.set_rank(gst::Rank::NONE);
        }

        for name in ["x264enc", "vtenc_h264", "vtenc_h264_hw"] {
            if name == video_encoder.factory_name() {
                continue;
            }
            if let Some(other_encoder) = gst::ElementFactory::find(name) {
                previous_ranks.push((other_encoder.clone(), other_encoder.rank()));
                other_encoder.set_rank(gst::Rank::NONE);
            }
        }
        Ok(Self { previous_ranks })
    }
}

impl Drop for EncoderSelection {
    fn drop(&mut self) {
        for (factory, rank) in self.previous_ranks.drain(..) {
            factory.set_rank(rank);
        }
    }
}

fn render_pipeline(
    pipeline: &ges::Pipeline,
    duration: Duration,
    report_progress: &mut impl FnMut(f32),
) -> Result<(), String> {
    pipeline
        .set_state(gst::State::Playing)
        .map_err(|error| format!("could not start GStreamer export: {error}"))?;
    let bus = pipeline
        .bus()
        .ok_or_else(|| "GStreamer export pipeline has no message bus.".to_string())?;
    let total = duration.as_secs_f64().max(f64::EPSILON);
    loop {
        if let Some(message) = bus.timed_pop(gst::ClockTime::from_mseconds(100)) {
            match message.view() {
                gst::MessageView::Eos(..) => return Ok(()),
                gst::MessageView::Error(error) => {
                    return Err(format!(
                        "GStreamer export failed: {}{}",
                        error.error(),
                        error
                            .debug()
                            .map(|debug| format!(" ({debug})"))
                            .unwrap_or_default()
                    ));
                }
                _ => {}
            }
        }
        let position = pipeline
            .query_position::<gst::ClockTime>()
            .map(|position| position.seconds_f64())
            .unwrap_or(0.0);
        report_progress((position / total).clamp(0.0, 0.999) as f32);
    }
}

fn clock_time(duration: Duration) -> gst::ClockTime {
    gst::ClockTime::from_nseconds(duration.as_nanos().min(u64::MAX as u128) as u64)
}

fn temporary_output_path(output: &Path) -> std::path::PathBuf {
    let name = output
        .file_stem()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "export".into());
    output.with_file_name(format!(".{name}.opencut-exporting.mp4"))
}

struct TemporaryOutput {
    path: std::path::PathBuf,
}

impl TemporaryOutput {
    fn new(path: std::path::PathBuf) -> Result<Self, String> {
        if path.is_file() {
            fs::remove_file(&path).map_err(|error| {
                format!(
                    "could not replace temporary export {}: {error}",
                    path.display()
                )
            })?;
        }
        Ok(Self { path })
    }
}

impl Drop for TemporaryOutput {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
#[path = "export_gstreamer.test.rs"]
mod integration_tests;

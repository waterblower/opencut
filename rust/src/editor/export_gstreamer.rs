use super::{
    clip_render_plan::{resolve_audio_clip_render_plan, resolve_visual_clip_render_plan},
    export::{ExportEncoder, ExportOptions},
    model::{MediaAsset, MediaKind},
    timeline::{FrameRate, TimelineSerialization, TimelineTime},
    timeline_clip::{Clip, TextClipProperties, VideoClipProperties},
    track::{Track, TrackKind},
};
use ges::prelude::*;
use gstreamer as gst;
use gstreamer_editing_services as ges;
use gstreamer_pbutils as gst_pbutils;
use std::{collections::HashMap, fs, path::Path, sync::Mutex, time::Duration};
use ulid::Ulid;
use url::Url;

const AUDIO_BIT_RATE: i32 = 192_000;
static EXPORT_ENCODER_LOCK: Mutex<()> = Mutex::new(());

pub(super) fn export_timeline(
    timeline: &TimelineSerialization,
    project_root: &Path,
    output: &Path,
    options: ExportOptions,
    mut report_progress: impl FnMut(f32),
) -> anyhow::Result<()> {
    if timeline.clips.is_empty() {
        anyhow::bail!("Add at least one clip before exporting.");
    }
    ges::init().map_err(|error| {
        anyhow::anyhow!("could not initialize GStreamer Editing Services: {error}")
    })?;
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
            .map_err(|error| anyhow::anyhow!("could not replace {}: {error}", output.display()))?;
    }
    fs::rename(&temporary_output.path, output).map_err(|error| {
        anyhow::anyhow!(
            "could not move completed export to {}: {error}",
            output.display()
        )
    })?;
    report_progress(1.0);
    Ok(())
}

/// Resolve frame boundaries for both initial construction and incremental edits.
pub fn clip_clock_range(
    rate: FrameRate,
    clip: &Clip,
    timeline_start: TimelineTime,
) -> (gst::ClockTime, gst::ClockTime) {
    let mut start = frame_clock_time(rate, timeline_start).nseconds();
    if matches!(clip, Clip::Text(_)) {
        // GES title sources can emit a gap when starting exactly on a sample.
        start = start.saturating_sub(1);
    }
    let end = frame_clock_time(rate, timeline_start + clip.frame_length(rate)).nseconds();
    (
        gst::ClockTime::from_nseconds(start),
        gst::ClockTime::from_nseconds(end.saturating_sub(start)),
    )
}

/// Text occupies its own transparent canvas, independently of media transforms.
pub fn configure_text_clip(
    clip: &ges::TitleClip,
    properties: &TextClipProperties,
    output_scale: f64,
) -> anyhow::Result<()> {
    let font_size = (properties.font_size * output_scale).clamp(1.0, 1000.0);
    let Some((text_overlay, _)) = clip.lookup_child("font-desc") else {
        return Err(anyhow::anyhow!(
            "text renderer has no font property at {}:{}",
            file!(),
            line!()
        ));
    };
    // Font size is already in output pixels, so disable GStreamer's screen-size scaling.
    text_overlay.set_property("auto-resize", false);
    for (name, value) in [
        ("text", properties.text.to_value()),
        (
            "font-desc",
            format!("{} {font_size}px", properties.font).to_value(),
        ),
        ("color", properties.color.to_value()),
        ("foreground-color", 0_u32.to_value()),
        ("halignment", ges::TextHAlign::Position.to_value()),
        ("valignment", ges::TextVAlign::Position.to_value()),
        ("xpos", properties.position_x.to_value()),
        ("ypos", properties.position_y.to_value()),
    ] {
        if let Err(error) = clip.set_child_property(name, value) {
            return Err(anyhow::anyhow!(
                "could not set text {name}: {error} at {}:{}",
                file!(),
                line!()
            ));
        }
    }
    Ok(())
}

pub(super) fn build_ges_timeline(
    timeline_data: &TimelineSerialization,
    project_root: &Path,
    options: ExportOptions,
) -> anyhow::Result<ges::Timeline> {
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

    let mut assets: HashMap<Ulid, ges::UriClipAsset> = HashMap::new();
    for timeline_track in timeline_data
        .tracks
        .iter()
        .filter(|track| track.kind == TrackKind::Text)
        .chain(
            timeline_data
                .tracks
                .iter()
                .filter(|track| track.kind != TrackKind::Text),
        )
    {
        let layer = timeline.append_layer();
        if timeline_track.kind == TrackKind::Text {
            if !timeline_track.visible {
                continue;
            }
            let output_scale = (options.width.max(2) as f64
                / timeline_data.settings.width.max(2) as f64)
                .min(options.height.max(2) as f64 / timeline_data.settings.height.max(2) as f64);
            let mut clips = timeline_data
                .clips_on_track(timeline_track.id)
                .collect::<Vec<_>>();
            clips.sort_by_key(|clip| clip.timeline_start());
            for clip in clips {
                let text = clip.text().ok_or_else(|| {
                    anyhow::anyhow!("Media clip {} is on a text track.", clip.id())
                })?;
                let overlay = ges::TitleClip::new().ok_or_else(|| {
                    anyhow::anyhow!(
                        "could not create text clip {} at {}:{}",
                        clip.id(),
                        file!(),
                        line!()
                    )
                })?;
                overlay
                    .set_name(Some(&format!("opencut-clip-{}", clip.id())))
                    .map_err(|error| {
                        anyhow::anyhow!("could not identify clip {}: {error}", clip.id())
                    })?;
                let (start, duration) = clip_clock_range(
                    timeline_data.settings.frame_rate,
                    clip,
                    clip.timeline_start(),
                );
                if !overlay.set_start(start) {
                    return Err(anyhow::anyhow!(
                        "could not set text clip {} start at {}:{}",
                        clip.id(),
                        file!(),
                        line!()
                    ));
                }
                if !overlay.set_duration(duration) {
                    return Err(anyhow::anyhow!(
                        "could not set text clip {} duration at {}:{}",
                        clip.id(),
                        file!(),
                        line!()
                    ));
                }
                layer.add_clip(&overlay).map_err(|error| {
                    anyhow::anyhow!(
                        "could not add text clip {} to the timeline: {error}",
                        clip.id()
                    )
                })?;
                configure_text_clip(&overlay, &text.properties, output_scale)?;
            }
            continue;
        }
        let mut clips = timeline_data
            .clips_on_track(timeline_track.id)
            .collect::<Vec<_>>();
        clips.sort_by_key(|clip| clip.timeline_start());
        for clip in clips {
            let media = clip
                .media()
                .ok_or_else(|| anyhow::anyhow!("Text clip {} is on a media track.", clip.id()))?;
            let asset = timeline_data
                .asset(media.asset_id)
                .ok_or_else(|| anyhow::anyhow!("Clip {} has no source media.", clip.id()))?;
            let track_types =
                exported_track_types(timeline_track, clip, asset.kind, asset.has_audio);
            if track_types.is_empty() {
                continue;
            }
            let uri_asset = if let Some(asset) = assets.get(&asset.id) {
                asset.clone()
            } else {
                let source = project_root.join(&asset.path);
                let uri = Url::from_file_path(&source).map_err(|_| {
                    anyhow::anyhow!("could not convert {} to a file URL", source.display())
                })?;
                let uri_asset = ges::UriClipAsset::request_sync(uri.as_str()).map_err(|error| {
                    anyhow::anyhow!(
                        "build_ges_timeline failed: could not inspect {}: {error}",
                        source.display()
                    )
                })?;
                assets.insert(asset.id, uri_asset.clone());
                uri_asset
            };

            let (start, duration) = clip_clock_range(
                timeline_data.settings.frame_rate,
                clip,
                clip.timeline_start(),
            );
            let inpoint = source_in(timeline_data, clip, asset.kind, track_types);
            let ges_clip = layer
                .add_asset(&uri_asset, start, inpoint, duration, track_types)
                .map_err(|error| {
                    anyhow::anyhow!(
                        "could not add {} to the export timeline: {error}",
                        asset.name
                    )
                })?;
            ges_clip
                .set_name(Some(&format!("opencut-clip-{}", clip.id())))
                .map_err(|error| {
                    anyhow::anyhow!("could not identify clip {}: {error}", clip.id())
                })?;
            if track_types.contains(ges::TrackType::VIDEO) {
                apply_video_transform(
                    &ges_clip,
                    timeline_data,
                    asset,
                    options,
                    media.video_properties,
                )?;
            }
            if track_types.contains(ges::TrackType::AUDIO) {
                let audio_plan =
                    resolve_audio_clip_render_plan(timeline_track.muted, media.audio_properties);
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
    let content_duration = timeline_data.duration(timeline_data.content_duration());
    if !content_duration.is_zero() {
        // Appended layers have lower visual precedence, so this preserves the
        // timeline duration and supplies black frames without covering media.
        let background_layer = timeline.append_layer();
        let background = ges::TestClip::new()
            .ok_or_else(|| anyhow::anyhow!("could not create the timeline background"))?;
        background.set_supported_formats(ges::TrackType::VIDEO);
        background.set_vpattern(ges::VideoTestPattern::Black);
        background.set_mute(true);
        background
            .set_name(Some("opencut-black-background"))
            .map_err(|error| {
                anyhow::anyhow!("could not identify the timeline background: {error}")
            })?;
        if !background.set_duration(frame_clock_time(
            timeline_data.settings.frame_rate,
            timeline_data.content_duration(),
        )) {
            anyhow::bail!(
                "could not set the timeline background duration at {}:{}",
                file!(),
                line!()
            );
        }
        background_layer
            .add_clip(&background)
            .map_err(|error| anyhow::anyhow!("could not add the timeline background: {error}"))?;
    }
    if !timeline.commit_sync() {
        anyhow::bail!("GStreamer could not commit the export timeline.");
    }
    Ok(timeline)
}

pub(super) fn apply_video_transform(
    clip: &ges::Clip,
    timeline: &TimelineSerialization,
    asset: &MediaAsset,
    options: ExportOptions,
    properties: VideoClipProperties,
) -> anyhow::Result<()> {
    let plan = resolve_visual_clip_render_plan(
        properties,
        asset.width,
        asset.height,
        timeline.settings.width,
        timeline.settings.height,
        options.width.max(2) as f64,
        options.height.max(2) as f64,
    );

    for (name, value) in [
        ("posx", rounded_i32(plan.visible.left)),
        ("posy", rounded_i32(plan.visible.top)),
        ("width", rounded_i32(plan.visible.width).max(1)),
        ("height", rounded_i32(plan.visible.height).max(1)),
    ] {
        clip.set_child_property(name, value)
            .map_err(|error| anyhow::anyhow!("could not apply video {name}: {error}"))?;
    }
    Ok(())
}

impl ExportEncoder {
    fn factory_name(self) -> &'static str {
        match self {
            Self::Hardware => "vtenc_h264_hw",
            Self::Software => "x264enc",
        }
    }
}

fn export_timeline_with_encoder(
    timeline_data: &TimelineSerialization,
    project_root: &Path,
    temporary_output: &Path,
    options: ExportOptions,
    encoder: ExportEncoder,
    report_progress: &mut impl FnMut(f32),
) -> anyhow::Result<()> {
    let timeline = build_ges_timeline(timeline_data, project_root, options)?;
    let profile = encoding_profile(options);
    let _encoder_selection = EncoderSelection::for_export(encoder)?;
    let pipeline = ges::Pipeline::new();
    configure_export_elements(&pipeline, options.video_bit_rate);
    pipeline
        .set_timeline(&timeline)
        .map_err(|error| anyhow::anyhow!("could not attach the export timeline: {error}"))?;

    let output_uri = Url::from_file_path(temporary_output).map_err(|_| {
        anyhow::anyhow!(
            "could not convert {} to a file URL",
            temporary_output.display()
        )
    })?;
    pipeline
        .set_render_settings(output_uri.as_str(), &profile)
        .map_err(|error| anyhow::anyhow!("could not configure GStreamer export: {error}"))?;
    pipeline
        .set_mode(ges::PipelineFlags::RENDER)
        .map_err(|error| anyhow::anyhow!("could not enable GStreamer render mode: {error}"))?;

    log::info!("Starting GStreamer export with {}", encoder.factory_name());
    let result = render_pipeline(
        &pipeline,
        timeline_data.duration(timeline_data.content_duration()),
        report_progress,
    );
    let _ = pipeline.set_state(gst::State::Null);
    result
}

fn rounded_i32(value: f64) -> i32 {
    value.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32
}

fn exported_track_types(
    track: &Track,
    clip: &Clip,
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
    if has_audio
        && clip.media().is_some_and(|clip| {
            !resolve_audio_clip_render_plan(track.muted, clip.audio_properties).muted
        })
    {
        types |= ges::TrackType::AUDIO;
    }
    types
}

fn source_in(
    timeline: &TimelineSerialization,
    clip: &Clip,
    asset_kind: MediaKind,
    track_types: ges::TrackType,
) -> gst::ClockTime {
    if asset_kind == MediaKind::Image {
        return gst::ClockTime::ZERO;
    }
    if track_types.contains(ges::TrackType::VIDEO) {
        return clock_time(Duration::from_secs_f64(timeline.source_start_seconds(clip)));
    }
    clock_time(
        timeline.audio_duration(
            clip.media()
                .expect("export source clips are media clips")
                .source_in,
        ),
    )
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
        .preset_name(AUDIO_ENCODER_FACTORY)
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
            "atenc" => {
                element.set_property("bitrate", AUDIO_BIT_RATE as u32);
                element.set_property_from_str("rate-control", "cbr");
            }
            "avenc_aac" => element.set_property("bitrate", AUDIO_BIT_RATE),
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
            "x264enc" | "vtenc_h264" | "vtenc_h264_hw" | "atenc" | "avenc_aac"
        ) {
            log::info!("GStreamer export is using {factory_name}");
        }
    });
}

const AUDIO_ENCODER_FACTORY: &str = if cfg!(target_os = "macos") {
    "atenc"
} else {
    "avenc_aac"
};

struct EncoderSelection {
    previous_ranks: Vec<(gst::ElementFactory, gst::Rank)>,
}

impl EncoderSelection {
    fn for_export(video_encoder: ExportEncoder) -> anyhow::Result<Self> {
        let Some(selected_video) = gst::ElementFactory::find(video_encoder.factory_name()) else {
            anyhow::bail!(
                "GStreamer H.264 encoder `{}` is unavailable. [{}:{}]",
                video_encoder.factory_name(),
                file!(),
                line!()
            );
        };
        if gst::ElementFactory::find(AUDIO_ENCODER_FACTORY).is_none() {
            anyhow::bail!(
                "GStreamer AAC encoder `{AUDIO_ENCODER_FACTORY}` is unavailable. [{}:{}]",
                file!(),
                line!()
            );
        }
        let mut previous_ranks = vec![(selected_video.clone(), selected_video.rank())];
        selected_video.set_rank(gst::Rank::PRIMARY + 100);

        for name in ["x264enc", "vtenc_h264", "vtenc_h264_hw"] {
            if name == video_encoder.factory_name() {
                continue;
            }
            let Some(other_encoder) = gst::ElementFactory::find(name) else {
                continue;
            };
            previous_ranks.push((other_encoder.clone(), other_encoder.rank()));
            other_encoder.set_rank(gst::Rank::NONE);
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
) -> anyhow::Result<()> {
    pipeline
        .set_state(gst::State::Playing)
        .map_err(|error| anyhow::anyhow!("could not start GStreamer export: {error}"))?;
    let bus = pipeline
        .bus()
        .ok_or_else(|| anyhow::anyhow!("GStreamer export pipeline has no message bus."))?;
    let total = duration.as_secs_f64().max(f64::EPSILON);
    loop {
        if let Some(message) = bus.timed_pop(gst::ClockTime::from_mseconds(100)) {
            match message.view() {
                gst::MessageView::Eos(..) => return Ok(()),
                gst::MessageView::Error(error) => {
                    return Err(anyhow::anyhow!(
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

// GStreamer timestamps output frames with integer division. Rounding a start up
// by one nanosecond would place the clip after its intended first output frame.
fn frame_clock_time(rate: FrameRate, time: TimelineTime) -> gst::ClockTime {
    let numerator = time.frames().max(0) as u128 * rate.denominator.max(1) as u128 * 1_000_000_000;
    let nanos = numerator / rate.numerator.max(1) as u128;
    gst::ClockTime::from_nseconds(nanos.min(u64::MAX as u128 - 1) as u64)
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
    fn new(path: std::path::PathBuf) -> anyhow::Result<Self> {
        if path.is_file() {
            fs::remove_file(&path).map_err(|error| {
                anyhow::anyhow!(
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

#[cfg(all(test, feature = "cli"))]
#[path = "shared_timeline.test.rs"]
mod shared_timeline_tests;

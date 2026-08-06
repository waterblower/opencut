use super::{
    clip_render_plan::{resolve_audio_clip_render_plan, resolve_visual_clip_render_plan},
    export::{ExportEncoder, ExportOptions},
    model::{MediaAsset, MediaKind, Project, TimelineClip, TrackKind, VideoClipProperties},
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

pub(super) fn export_project(
    project: &Project,
    project_root: &Path,
    output: &Path,
    options: ExportOptions,
    mut report_progress: impl FnMut(f32),
) -> Result<(), String> {
    if project.clips.is_empty() {
        return Err("Add at least one clip before exporting.".to_string());
    }
    ges::init()
        .map_err(|error| format!("could not initialize GStreamer Editing Services: {error}"))?;
    report_progress(0.0);

    let temporary_output = TemporaryOutput::new(temporary_output_path(output))?;
    let _encoder_lock = EXPORT_ENCODER_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    export_project_with_encoder(
        project,
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

fn export_project_with_encoder(
    project: &Project,
    project_root: &Path,
    temporary_output: &Path,
    options: ExportOptions,
    encoder: ExportEncoder,
    report_progress: &mut impl FnMut(f32),
) -> Result<(), String> {
    let timeline = build_timeline(project, project_root, options)?;
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
        project.duration(project.content_duration()),
        report_progress,
    );
    let _ = pipeline.set_state(gst::State::Null);
    result
}

fn build_timeline(
    project: &Project,
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
    for project_track in &project.tracks {
        let layer = timeline.append_layer();
        let mut clips = project.clips_on_track(project_track.id).collect::<Vec<_>>();
        clips.sort_by_key(|clip| clip.timeline_start);
        for clip in clips {
            let asset = project
                .asset(clip.asset_id)
                .ok_or_else(|| format!("Clip {} has no source media.", clip.id))?;
            let track_types =
                exported_track_types(project_track, clip, asset.kind, asset.has_audio);
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

            let start = clock_time(project.duration(clip.timeline_start));
            let inpoint = source_in(project, clip, asset.kind, track_types);
            let duration = clock_time(project.duration(clip.duration()));
            let ges_clip = layer
                .add_asset(&uri_asset, start, inpoint, duration, track_types)
                .map_err(|error| {
                    format!(
                        "could not add {} to the export timeline: {error}",
                        asset.name
                    )
                })?;
            if track_types.contains(ges::TrackType::VIDEO) {
                apply_video_transform(&ges_clip, project, asset, options, clip.video_properties)?;
            }
            if track_types.contains(ges::TrackType::AUDIO) {
                let audio_plan =
                    resolve_audio_clip_render_plan(project_track.muted, clip.audio_properties);
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

fn apply_video_transform(
    clip: &ges::Clip,
    project: &Project,
    asset: &MediaAsset,
    options: ExportOptions,
    properties: VideoClipProperties,
) -> Result<(), String> {
    let plan = resolve_visual_clip_render_plan(
        properties,
        asset.width,
        asset.height,
        project.settings.width,
        project.settings.height,
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
    project: &Project,
    clip: &TimelineClip,
    asset_kind: MediaKind,
    track_types: ges::TrackType,
) -> gst::ClockTime {
    if asset_kind == MediaKind::Image {
        return gst::ClockTime::ZERO;
    }
    if track_types.contains(ges::TrackType::VIDEO) {
        return clock_time(Duration::from_secs_f64(project.source_start_seconds(clip)));
    }
    clock_time(project.audio_duration(clip.source_in))
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

#[cfg(all(test, target_os = "macos"))]
#[path = "export_gstreamer_videotoolbox.test.rs"]
mod videotoolbox_integration_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::model::{
        AudioClipProperties, MediaAsset, TimelineTime, VideoClipProperties,
    };

    #[test]
    fn video_track_exports_visible_video_and_unmuted_audio() {
        let project = Project::default();
        let track = project
            .tracks
            .iter()
            .find(|track| track.kind == TrackKind::Video)
            .unwrap();
        let clip = TimelineClip {
            id: 1,
            track_id: track.id,
            asset_id: 2,
            timeline_start: TimelineTime::ZERO,
            source_in: TimelineTime::ZERO,
            source_out: TimelineTime::ONE_FRAME,
            video_properties: VideoClipProperties::default(),
            audio_properties: AudioClipProperties::default(),
        };
        let types = exported_track_types(track, &clip, MediaKind::Video, true);
        assert!(types.contains(ges::TrackType::VIDEO));
        assert!(types.contains(ges::TrackType::AUDIO));
    }

    #[test]
    fn hidden_video_track_can_still_export_audio() {
        let mut project = Project::default();
        let track = project
            .tracks
            .iter_mut()
            .find(|track| track.kind == TrackKind::Video)
            .unwrap();
        track.visible = false;
        let clip = TimelineClip {
            id: 1,
            track_id: track.id,
            asset_id: 2,
            timeline_start: TimelineTime::ZERO,
            source_in: TimelineTime::ZERO,
            source_out: TimelineTime::ONE_FRAME,
            video_properties: VideoClipProperties::default(),
            audio_properties: AudioClipProperties::default(),
        };
        assert_eq!(
            exported_track_types(track, &clip, MediaKind::Video, true),
            ges::TrackType::AUDIO
        );
    }

    #[test]
    fn applies_the_requested_bitrate_to_x264() {
        ges::init().unwrap();
        let pipeline = ges::Pipeline::new();
        configure_export_elements(&pipeline, 12_345_000);
        let encoder = gst::ElementFactory::make("x264enc").build().unwrap();
        pipeline.add(&encoder).unwrap();
        assert_eq!(encoder.property::<u32>("bitrate"), 12_345);
        let audio_encoder = gst::ElementFactory::make("faac").build().unwrap();
        pipeline.add(&audio_encoder).unwrap();
        assert_eq!(audio_encoder.property::<i32>("bitrate"), AUDIO_BIT_RATE);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn configures_videotoolbox_for_mp4_timeline_export() {
        ges::init().unwrap();
        let Some(factory) = gst::ElementFactory::find("vtenc_h264_hw") else {
            return;
        };
        let pipeline = ges::Pipeline::new();
        configure_export_elements(&pipeline, 12_345_000);
        let encoder = factory.create().build().unwrap();
        pipeline.add(&encoder).unwrap();
        assert_eq!(encoder.property::<u32>("bitrate"), 12_345);
        assert!(!encoder.property::<bool>("allow-frame-reordering"));
    }

    #[test]
    fn enables_automatic_threading_for_video_conversion_and_scaling() {
        ges::init().unwrap();
        let pipeline = ges::Pipeline::new();
        configure_export_elements(&pipeline, 12_345_000);

        for factory_name in ["videoconvert", "videoscale", "videoconvertscale"] {
            let element = gst::ElementFactory::make(factory_name).build().unwrap();
            pipeline.add(&element).unwrap();
            assert_eq!(
                element.property::<u32>("n-threads"),
                0,
                "{factory_name} should choose its thread count automatically"
            );
        }
    }

    #[test]
    fn creates_gstreamer_timeline_from_real_media() {
        ges::init().unwrap();
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut project = Project::default();
        let video_track = project
            .tracks
            .iter()
            .find(|track| track.kind == TrackKind::Video)
            .unwrap()
            .id;
        project.assets.push(MediaAsset {
            id: 10,
            kind: MediaKind::Video,
            path: "vendor/gpui-video-player/assets/test1.mp4".into(),
            name: "test1".into(),
            duration: 5.0,
            width: 320,
            height: 180,
            framerate: 30.0,
            frame_rate_numerator: 30,
            frame_rate_denominator: 1,
            codec: "h264".into(),
            has_audio: true,
        });
        let video_properties = VideoClipProperties {
            position_x: 120.0,
            position_y: -60.0,
            scale: 0.5,
            opacity: 0.25,
            crop_left: 0.1,
            crop_right: 0.2,
            crop_top: 0.1,
            crop_bottom: 0.2,
        };
        project.clips.push(TimelineClip {
            id: 11,
            track_id: video_track,
            asset_id: 10,
            timeline_start: TimelineTime::ZERO,
            source_in: TimelineTime::ZERO,
            source_out: TimelineTime::from_frames(3),
            video_properties,
            audio_properties: AudioClipProperties::default(),
        });
        let timeline = build_timeline(
            &project,
            project_root,
            ExportOptions::from_project(&project),
        )
        .unwrap();
        assert_eq!(timeline.layers().len(), project.tracks.len());
        let exported_clip = timeline
            .layers()
            .into_iter()
            .flat_map(|layer| layer.clips())
            .next()
            .unwrap();
        assert_eq!(
            exported_clip
                .child_property("posx")
                .unwrap()
                .get::<i32>()
                .unwrap(),
            696
        );
        assert_eq!(
            exported_clip
                .child_property("posy")
                .unwrap()
                .get::<i32>()
                .unwrap(),
            264
        );
        assert_eq!(
            exported_clip
                .child_property("width")
                .unwrap()
                .get::<i32>()
                .unwrap(),
            672
        );
        assert_eq!(
            exported_clip
                .child_property("height")
                .unwrap()
                .get::<i32>()
                .unwrap(),
            378
        );
        assert_eq!(
            exported_clip
                .child_property("alpha")
                .unwrap()
                .get::<f64>()
                .unwrap(),
            0.25
        );
        for (name, expected) in [("left", 32), ("right", 64), ("top", 18), ("bottom", 36)] {
            assert_eq!(
                exported_clip
                    .child_property(name)
                    .unwrap()
                    .get::<i32>()
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn exports_real_media_with_audio() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut project = Project::default();
        project.settings.width = 320;
        project.settings.height = 180;
        let video_track = project
            .tracks
            .iter()
            .find(|track| track.kind == TrackKind::Video)
            .unwrap()
            .id;
        project.assets.push(MediaAsset {
            id: 10,
            kind: MediaKind::Video,
            path: "vendor/gpui-video-player/assets/test1.mp4".into(),
            name: "test1".into(),
            duration: 5.0,
            width: 320,
            height: 180,
            framerate: 30.0,
            frame_rate_numerator: 30,
            frame_rate_denominator: 1,
            codec: "h264".into(),
            has_audio: true,
        });
        project.clips.push(TimelineClip {
            id: 11,
            track_id: video_track,
            asset_id: 10,
            timeline_start: TimelineTime::ZERO,
            source_in: TimelineTime::ZERO,
            source_out: TimelineTime::from_frames(30),
            video_properties: VideoClipProperties::default(),
            audio_properties: AudioClipProperties::default(),
        });

        let output =
            std::env::temp_dir().join(format!("opencut-ges-video-{}.mp4", std::process::id()));
        export_project(
            &project,
            project_root,
            &output,
            ExportOptions::from_project(&project),
            |_| {},
        )
        .unwrap();
        assert!(std::fs::metadata(&output).unwrap().len() > 0);
        std::fs::remove_file(output).unwrap();
    }

    #[test]
    fn exports_an_image_only_timeline() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let project_root = std::env::temp_dir().join(format!("opencut-ges-image-{unique}"));
        std::fs::create_dir_all(&project_root).unwrap();
        let image_path = project_root.join("still.png");
        image::save_buffer(
            &image_path,
            &[0x20; 64 * 64 * 4],
            64,
            64,
            image::ColorType::Rgba8,
        )
        .unwrap();

        let mut project = Project::default();
        project.settings.width = 64;
        project.settings.height = 64;
        let video_track = project
            .tracks
            .iter()
            .find(|track| track.kind == TrackKind::Video)
            .unwrap()
            .id;
        project.assets.push(MediaAsset {
            id: 10,
            kind: MediaKind::Image,
            path: "still.png".into(),
            name: "still".into(),
            duration: 5.0,
            width: 64,
            height: 64,
            framerate: 0.0,
            frame_rate_numerator: 0,
            frame_rate_denominator: 0,
            codec: "png".into(),
            has_audio: false,
        });
        project.clips.push(TimelineClip {
            id: 11,
            track_id: video_track,
            asset_id: 10,
            timeline_start: TimelineTime::ZERO,
            source_in: TimelineTime::ZERO,
            source_out: TimelineTime::from_frames(3),
            video_properties: VideoClipProperties::default(),
            audio_properties: AudioClipProperties::default(),
        });

        let output = project_root.join("image-export.mp4");
        export_project(
            &project,
            &project_root,
            &output,
            ExportOptions::from_project(&project),
            |_| {},
        )
        .unwrap();
        assert!(std::fs::metadata(&output).unwrap().len() > 0);
        std::fs::remove_dir_all(project_root).unwrap();
    }
}

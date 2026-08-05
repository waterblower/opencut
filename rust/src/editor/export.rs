use super::model::{FrameRate, MediaKind, Project, TimelineClip, TrackKind};
use ffmpeg::{
    Dictionary, Packet, Rational,
    channel_layout::ChannelLayout,
    codec::{self, encoder},
    filter, format, frame,
    util::format::Pixel,
};
use ffmpeg_next as ffmpeg;
use std::path::Path;

pub(super) const DEFAULT_VIDEO_BIT_RATE: usize = 8_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExportEncoder {
    Hardware,
    Software,
}

impl ExportEncoder {
    pub(super) const fn default_for_platform() -> Self {
        if cfg!(target_os = "macos") {
            Self::Hardware
        } else {
            Self::Software
        }
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Hardware => "Hardware",
            Self::Software => "Software",
        }
    }

    pub(super) const fn implementation(self) -> &'static str {
        match self {
            Self::Hardware => "VideoToolbox",
            Self::Software => "x264",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ExportOptions {
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub video_bit_rate: usize,
    pub encoder: ExportEncoder,
}

impl ExportOptions {
    #[cfg(test)]
    pub fn from_project(project: &Project) -> Self {
        Self {
            width: project.settings.width,
            height: project.settings.height,
            frame_rate: project.settings.frame_rate,
            video_bit_rate: DEFAULT_VIDEO_BIT_RATE,
            encoder: ExportEncoder::Software,
        }
    }
}

pub(super) fn export_project(
    project: &Project,
    project_root: &Path,
    output: &Path,
    options: ExportOptions,
    report_progress: impl FnMut(f32),
) -> Result<(), String> {
    if std::env::var_os("OPENCUT_FFMPEG_EXPORT").is_some() {
        return export_project_ffmpeg(project, project_root, output, options, report_progress);
    }
    super::export_gstreamer::export_project(project, project_root, output, options, report_progress)
}

fn export_project_ffmpeg(
    project: &Project,
    project_root: &Path,
    output: &Path,
    options: ExportOptions,
    mut report_progress: impl FnMut(f32),
) -> Result<(), String> {
    if project.clips.is_empty() {
        return Err("Add at least one clip before exporting.".to_string());
    }
    report_progress(0.0);
    ffmpeg::init().map_err(|error| format!("could not initialize FFmpeg: {error}"))?;

    let duration = project.seconds(project.content_duration());
    let width = even(options.width.max(2));
    let height = even(options.height.max(2));
    let fps = options.frame_rate.frames_per_second();
    let frame_rate = Rational(
        options.frame_rate.numerator as i32,
        options.frame_rate.denominator as i32,
    );
    let video_time_base = Rational(frame_rate.denominator(), frame_rate.numerator());
    let audio_rate = i32::try_from(project.settings.audio_sample_rate)
        .map_err(|_| "project audio sample rate is too large".to_string())?;

    let mut video_graph = build_video_graph(
        project,
        project_root,
        width,
        height,
        fps,
        video_time_base,
        duration,
    )?;
    let mut audio_graph = build_audio_graph(project, project_root, duration, audio_rate)?;
    let mut output_context = format::output(output)
        .map_err(|error| format!("could not create {}: {error}", output.display()))?;
    let global_header = output_context
        .format()
        .flags()
        .contains(format::Flags::GLOBAL_HEADER);

    let video_codec = encoder::find(codec::Id::H264)
        .ok_or_else(|| "this FFmpeg build has no H.264 encoder".to_string())?;
    let mut video_encoder = codec::context::Context::new_with_codec(video_codec)
        .encoder()
        .video()
        .map_err(|error| format!("could not configure H.264 encoder: {error}"))?;
    video_encoder.set_width(width);
    video_encoder.set_height(height);
    video_encoder.set_format(Pixel::YUV420P);
    video_encoder.set_frame_rate(Some(frame_rate));
    video_encoder.set_time_base(video_time_base);
    video_encoder.set_bit_rate(options.video_bit_rate);
    if global_header {
        video_encoder.set_flags(codec::Flags::GLOBAL_HEADER);
    }
    let video_encoder = video_encoder
        .open_with(video_encoder_options())
        .map_err(|error| format!("could not open H.264 encoder: {error}"))?;
    let (video_stream_index, video_stream_time_base) = {
        let mut stream = output_context
            .add_stream(video_codec)
            .map_err(|error| format!("could not add video stream: {error}"))?;
        stream.set_time_base(video_time_base);
        stream.set_parameters(&video_encoder);
        (stream.index(), stream.time_base())
    };

    let audio_codec = encoder::find(codec::Id::AAC)
        .ok_or_else(|| "this FFmpeg build has no AAC encoder".to_string())?;
    let audio_description = audio_codec
        .audio()
        .map_err(|error| format!("the selected AAC codec is not an audio encoder: {error}"))?;
    let mut audio_formats = audio_description
        .formats()
        .ok_or_else(|| "the AAC encoder reports no supported sample format".to_string())?;
    let audio_format = audio_formats
        .next()
        .ok_or_else(|| "the AAC encoder reports no supported sample format".to_string())?;
    let audio_time_base = Rational(1, audio_rate);
    let mut audio_encoder = codec::context::Context::new_with_codec(audio_codec)
        .encoder()
        .audio()
        .map_err(|error| format!("could not configure AAC encoder: {error}"))?;
    audio_encoder.set_rate(audio_rate);
    audio_encoder.set_channel_layout(ChannelLayout::STEREO);
    audio_encoder.set_format(audio_format);
    audio_encoder.set_bit_rate(192_000);
    audio_encoder.set_time_base(audio_time_base);
    if global_header {
        audio_encoder.set_flags(codec::Flags::GLOBAL_HEADER);
    }
    let audio_encoder = audio_encoder
        .open_as(audio_codec)
        .map_err(|error| format!("could not open AAC encoder: {error}"))?;
    let (audio_stream_index, audio_stream_time_base) = {
        let mut stream = output_context
            .add_stream(audio_codec)
            .map_err(|error| format!("could not add audio stream: {error}"))?;
        stream.set_time_base(audio_time_base);
        stream.set_parameters(&audio_encoder);
        (stream.index(), stream.time_base())
    };
    audio_graph
        .get("audio_out")
        .ok_or_else(|| "audio filter output is missing".to_string())?
        .sink()
        .set_frame_size(audio_encoder.frame_size());

    output_context
        .write_header()
        .map_err(|error| format!("could not write MP4 header: {error}"))?;

    encode_media(
        &mut video_graph,
        &mut audio_graph,
        video_encoder,
        audio_encoder,
        video_stream_index,
        audio_stream_index,
        video_time_base,
        video_stream_time_base,
        audio_time_base,
        audio_stream_time_base,
        &mut output_context,
        duration,
        &mut report_progress,
    )?;
    output_context
        .write_trailer()
        .map_err(|error| format!("could not finish MP4 export: {error}"))?;
    report_progress(1.0);
    Ok(())
}

fn build_video_graph(
    project: &Project,
    project_root: &Path,
    width: u32,
    height: u32,
    fps: f64,
    time_base: Rational,
    duration: f64,
) -> Result<filter::Graph, String> {
    let mut filters = vec![format!(
        "color=c=black:s={width}x{height}:r={}:d={}[visual0]",
        decimal(fps),
        decimal(duration)
    )];
    let mut visual_label = "visual0".to_string();
    let mut visual_number = 0;
    for track in project
        .tracks
        .iter()
        .rev()
        .filter(|track| track.visible && track.kind == TrackKind::Video)
    {
        let mut clips = project.clips_on_track(track.id).collect::<Vec<_>>();
        clips.sort_by_key(|clip| clip.timeline_start);
        for clip in clips {
            visual_number += 1;
            let output_label = format!("visual{visual_number}");
            let asset = clip
                .asset_id
                .and_then(|id| project.asset(id))
                .ok_or_else(|| format!("Clip {} has no source media.", clip.id))?;
            let source = escape_filter_value(&project_root.join(&asset.path).to_string_lossy());
            let prepared = format!("prepared{visual_number}");
            let source_filter = if asset.kind == MediaKind::Image {
                format!(
                    "movie=filename='{source}',setpts=PTS-STARTPTS,tpad=stop_mode=clone:stop_duration={},trim=duration={}",
                    decimal(project.seconds(clip.duration())),
                    decimal(project.seconds(clip.duration()))
                )
            } else {
                format!(
                    "movie=filename='{source}',trim=start={}:duration={},setpts=PTS-STARTPTS",
                    decimal(project.source_start_seconds(clip)),
                    decimal(project.seconds(clip.duration()))
                )
            };
            filters.push(format!(
                "{source_filter},scale={width}:{height}:force_original_aspect_ratio=decrease,format=rgba,setpts=PTS+{}/TB[{prepared}]",
                decimal(project.seconds(clip.timeline_start))
            ));
            filters.push(format!(
                "[{visual_label}][{prepared}]overlay=x=(W-w)/2:y=(H-h)/2:eof_action=pass:enable='between(t,{},{})'[{output_label}]",
                decimal(project.seconds(clip.timeline_start)),
                decimal(project.seconds(clip.timeline_end()))
            ));
            visual_label = output_label;
        }
    }
    filters.push(format!(
        "[{visual_label}]fps={},settb=expr={}/{},setpts=N,format=yuv420p",
        decimal(fps),
        time_base.numerator(),
        time_base.denominator()
    ));
    build_source_graph("buffersink", "video_out", &filters.join(";"))
}

fn build_audio_graph(
    project: &Project,
    project_root: &Path,
    duration: f64,
    audio_rate: i32,
) -> Result<filter::Graph, String> {
    let audio_clips = project
        .tracks
        .iter()
        .filter(|track| !track.muted)
        .flat_map(|track| project.clips_on_track(track.id))
        .filter(|clip| clip_has_audio(project, clip))
        .collect::<Vec<_>>();
    let mut filters = Vec::new();
    if audio_clips.is_empty() {
        filters.push(format!(
            "anullsrc=r={audio_rate}:cl=stereo,atrim=duration={},asetpts=N/SR/TB",
            decimal(duration)
        ));
    } else {
        let mut inputs = String::new();
        for (number, clip) in audio_clips.iter().enumerate() {
            let asset = clip
                .asset_id
                .and_then(|id| project.asset(id))
                .ok_or_else(|| format!("Clip {} has no audio source.", clip.id))?;
            let source = escape_filter_value(&project_root.join(&asset.path).to_string_lossy());
            filters.push(format!(
                "amovie=filename='{source}',atrim=start={}:duration={},aresample={audio_rate},aformat=sample_fmts=fltp:channel_layouts=stereo,asetpts=PTS-STARTPTS+{}/TB[audio{number}]",
                decimal(project.audio_seconds(clip.source_in)),
                decimal(project.audio_seconds(clip.duration())),
                decimal(project.audio_seconds(clip.timeline_start))
            ));
            inputs.push_str(&format!("[audio{number}]"));
        }
        filters.push(format!(
            "{inputs}amix=inputs={}:duration=longest:normalize=0,atrim=duration={},asetpts=N/SR/TB",
            audio_clips.len(),
            decimal(duration)
        ));
    }
    build_source_graph("abuffersink", "audio_out", &filters.join(";"))
}

fn build_source_graph(
    sink_filter: &str,
    sink_name: &str,
    spec: &str,
) -> Result<filter::Graph, String> {
    let mut graph = filter::Graph::new();
    let sink = filter::find(sink_filter)
        .ok_or_else(|| format!("FFmpeg filter {sink_filter} is unavailable"))?;
    graph
        .add(&sink, sink_name, "")
        .map_err(|error| format!("could not create {sink_filter}: {error}"))?;
    graph
        .input(sink_name, 0)
        .and_then(|parser| parser.parse(&format!("{spec}[{sink_name}]")))
        .map_err(|error| format!("could not build export filter graph: {error}"))?;
    graph
        .validate()
        .map_err(|error| format!("could not validate export filter graph: {error}"))?;
    Ok(graph)
}

#[allow(clippy::too_many_arguments)]
fn encode_media(
    video_graph: &mut filter::Graph,
    audio_graph: &mut filter::Graph,
    mut video_encoder: ffmpeg::encoder::Video,
    mut audio_encoder: ffmpeg::encoder::Audio,
    video_stream_index: usize,
    audio_stream_index: usize,
    video_encoder_time_base: Rational,
    video_stream_time_base: Rational,
    audio_encoder_time_base: Rational,
    audio_stream_time_base: Rational,
    output: &mut format::context::Output,
    duration: f64,
    report_progress: &mut dyn FnMut(f32),
) -> Result<(), String> {
    let video_filter_time_base = video_graph
        .get("video_out")
        .ok_or_else(|| "video filter output is missing".to_string())?
        .sink()
        .time_base();
    let audio_filter_time_base = audio_graph
        .get("audio_out")
        .ok_or_else(|| "audio filter output is missing".to_string())?
        .sink()
        .time_base();
    let mut video_frame = pull_video_frame(video_graph);
    let mut audio_frame = pull_audio_frame(audio_graph);

    while video_frame.is_some() || audio_frame.is_some() {
        let video_time = video_frame
            .as_ref()
            .map(|frame| timestamp_seconds(frame.pts(), video_filter_time_base))
            .unwrap_or(f64::INFINITY);
        let audio_time = audio_frame
            .as_ref()
            .map(|frame| timestamp_seconds(frame.pts(), audio_filter_time_base))
            .unwrap_or(f64::INFINITY);
        if video_time <= audio_time {
            let frame = video_frame.take().unwrap();
            video_encoder
                .send_frame(&frame)
                .map_err(|error| format!("could not encode video frame: {error}"))?;
            write_video_packets(
                &mut video_encoder,
                video_stream_index,
                video_encoder_time_base,
                video_stream_time_base,
                output,
            )?;
            video_frame = pull_video_frame(video_graph);
        } else {
            let frame = audio_frame.take().unwrap();
            audio_encoder
                .send_frame(&frame)
                .map_err(|error| format!("could not encode audio frame: {error}"))?;
            write_audio_packets(
                &mut audio_encoder,
                audio_stream_index,
                audio_encoder_time_base,
                audio_stream_time_base,
                output,
            )?;
            audio_frame = pull_audio_frame(audio_graph);
        }

        if duration > 0.0 {
            report_progress((video_time.min(audio_time) / duration).clamp(0.0, 0.999) as f32);
        }
    }

    video_encoder
        .send_eof()
        .map_err(|error| format!("could not flush video encoder: {error}"))?;
    write_video_packets(
        &mut video_encoder,
        video_stream_index,
        video_encoder_time_base,
        video_stream_time_base,
        output,
    )?;
    audio_encoder
        .send_eof()
        .map_err(|error| format!("could not flush audio encoder: {error}"))?;
    write_audio_packets(
        &mut audio_encoder,
        audio_stream_index,
        audio_encoder_time_base,
        audio_stream_time_base,
        output,
    )
}

fn pull_video_frame(graph: &mut filter::Graph) -> Option<frame::Video> {
    let mut frame = frame::Video::empty();
    graph
        .get("video_out")?
        .sink()
        .frame(&mut frame)
        .ok()
        .map(|()| frame)
}

fn pull_audio_frame(graph: &mut filter::Graph) -> Option<frame::Audio> {
    let mut frame = frame::Audio::empty();
    graph
        .get("audio_out")?
        .sink()
        .frame(&mut frame)
        .ok()
        .map(|()| frame)
}

fn timestamp_seconds(timestamp: Option<i64>, time_base: Rational) -> f64 {
    timestamp.unwrap_or(0) as f64 * f64::from(time_base)
}

fn write_video_packets(
    encoder: &mut ffmpeg::encoder::Video,
    stream_index: usize,
    encoder_time_base: Rational,
    stream_time_base: Rational,
    output: &mut format::context::Output,
) -> Result<(), String> {
    let mut packet = Packet::empty();
    while encoder.receive_packet(&mut packet).is_ok() {
        packet.set_stream(stream_index);
        packet.rescale_ts(encoder_time_base, stream_time_base);
        packet
            .write_interleaved(output)
            .map_err(|error| format!("could not write video packet: {error}"))?;
    }
    Ok(())
}

fn write_audio_packets(
    encoder: &mut ffmpeg::encoder::Audio,
    stream_index: usize,
    encoder_time_base: Rational,
    stream_time_base: Rational,
    output: &mut format::context::Output,
) -> Result<(), String> {
    let mut packet = Packet::empty();
    while encoder.receive_packet(&mut packet).is_ok() {
        packet.set_stream(stream_index);
        packet.rescale_ts(encoder_time_base, stream_time_base);
        packet
            .write_interleaved(output)
            .map_err(|error| format!("could not write audio packet: {error}"))?;
    }
    Ok(())
}

fn clip_has_audio(project: &Project, clip: &TimelineClip) -> bool {
    clip.asset_id
        .and_then(|id| project.asset(id))
        .is_some_and(|asset| asset.has_audio)
}

/// Escapes a value that the caller wraps in single quotes inside a filter graph.
///
/// The backslashes survive the quotes so that libavfilter's second pass, which splits
/// filter options on `:`, still sees them escaped. A quote cannot be escaped inside
/// quotes at all, so it has to close them, escape itself, and reopen.
fn escape_filter_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace(',', "\\,")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('\'', "'\\\\\\''")
}

fn even(value: u32) -> u32 {
    value - value % 2
}

fn video_encoder_options<'a>() -> Dictionary<'a> {
    let mut options = Dictionary::new();
    options.set("preset", "medium");
    // CRF selects constant-quality rate control and overrides the bitrate set
    // on AVCodecContext. Omitting it leaves libx264 in average-bitrate mode.
    options
}

fn decimal(value: f64) -> String {
    format!("{value:.6}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::model::{
        AudioClipProperties, MediaAsset, TimelineTime, VideoClipProperties,
    };

    #[test]
    fn video_encoder_options_do_not_override_the_requested_bitrate() {
        let options = video_encoder_options();
        assert_eq!(options.get("preset"), Some("medium"));
        assert_eq!(options.get("crf"), None);
    }

    #[test]
    fn builds_and_reads_source_only_video_graph() {
        ffmpeg::init().unwrap();
        let mut graph = build_source_graph(
            "buffersink",
            "video_out",
            "color=c=black:s=64x64:r=30:d=0.1,fps=30,settb=expr=1/30,setpts=N,format=yuv420p",
        )
        .unwrap();
        let mut output = frame::Video::empty();
        assert!(
            graph
                .get("video_out")
                .unwrap()
                .sink()
                .frame(&mut output)
                .is_ok()
        );
        assert_eq!((output.width(), output.height()), (64, 64));
    }

    #[test]
    fn builds_and_reads_source_only_audio_graph() {
        ffmpeg::init().unwrap();
        let mut graph = build_source_graph(
            "abuffersink",
            "audio_out",
            "anullsrc=r=48000:cl=stereo,atrim=duration=0.1,asetpts=N/SR/TB",
        )
        .unwrap();
        let mut output = frame::Audio::empty();
        assert!(
            graph
                .get("audio_out")
                .unwrap()
                .sink()
                .frame(&mut output)
                .is_ok()
        );
        assert!(output.samples() > 0);
    }

    #[test]
    fn opens_media_sources_through_libavfilter() {
        ffmpeg::init().unwrap();
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
        project.clips.push(TimelineClip {
            id: 11,
            track_id: video_track,
            asset_id: Some(10),
            timeline_start: TimelineTime::ZERO,
            source_in: TimelineTime::ZERO,
            source_out: TimelineTime::from_frames(3),
            video_properties: VideoClipProperties::default(),
            audio_properties: AudioClipProperties::default(),
        });

        let mut video =
            build_video_graph(&project, project_root, 320, 180, 30.0, Rational(1, 30), 0.1)
                .unwrap();
        let mut audio = build_audio_graph(&project, project_root, 0.1, 48_000).unwrap();

        assert!(pull_video_frame(&mut video).is_some());
        assert!(pull_audio_frame(&mut audio).is_some());

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let output = std::env::temp_dir().join(format!("opencut-api-export-{unique}.mp4"));
        let mut progress = Vec::new();
        export_project(
            &project,
            project_root,
            &output,
            ExportOptions::from_project(&project),
            |fraction| progress.push(fraction),
        )
        .unwrap();
        assert_eq!(progress.first(), Some(&0.0));
        assert_eq!(progress.last(), Some(&1.0));
        assert!(progress.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(std::fs::metadata(&output).unwrap().len() > 0);
        std::fs::remove_file(output).unwrap();
    }

    #[test]
    fn escapes_quotes_by_reopening_the_surrounding_quotes() {
        assert_eq!(escape_filter_value("a:b,c[d]"), "a\\:b\\,c\\[d\\]");
        assert_eq!(escape_filter_value("It's"), "It'\\\\\\''s");
    }

    #[test]
    fn reads_a_source_whose_path_contains_a_quote() {
        ffmpeg::init().unwrap();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let project_root = std::env::temp_dir().join(format!("opencut-quoted-{unique}"));
        std::fs::create_dir_all(&project_root).unwrap();
        let quoted = project_root.join("it's a clip.mp4");
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/gpui-video-player/assets/test1.mp4"),
            &quoted,
        )
        .unwrap();

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
            path: "it's a clip.mp4".into(),
            name: "it's a clip".into(),
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
            asset_id: Some(10),
            timeline_start: TimelineTime::ZERO,
            source_in: TimelineTime::ZERO,
            source_out: TimelineTime::from_frames(3),
            video_properties: VideoClipProperties::default(),
            audio_properties: AudioClipProperties::default(),
        });

        let mut video = build_video_graph(
            &project,
            &project_root,
            320,
            180,
            30.0,
            Rational(1, 30),
            0.1,
        )
        .unwrap();
        let mut audio = build_audio_graph(&project, &project_root, 0.1, 48_000).unwrap();
        assert!(pull_video_frame(&mut video).is_some());
        assert!(pull_audio_frame(&mut audio).is_some());

        std::fs::remove_dir_all(project_root).unwrap();
    }
}

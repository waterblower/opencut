use gst::prelude::*;
use gst_video::VideoFrameExt as _;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use std::{fs, path::Path};
use url::Url;
use yuv::{YuvBiPlanarImage, YuvConversionMode, YuvRange, YuvStandardMatrix, yuv_nv12_to_rgba};

pub use gpui_video_player::{Video, VideoOptions, video};

pub fn create_timeline_video(
    uri: &Url,
    options: VideoOptions,
    frame_rate_numerator: u32,
    frame_rate_denominator: u32,
) -> Result<Video, String> {
    gst::init().map_err(|error| format!("could not initialize GStreamer: {error}"))?;
    let pipeline = format!(
        "playbin uri=\"{}\" video-sink=\"videorate ! videoscale ! videoconvert ! appsink name=gpui_video drop=true max-buffers=200 enable-last-sample=false caps=video/x-raw,format=NV12,pixel-aspect-ratio=1/1,framerate={}/{}\"",
        uri.as_str(),
        frame_rate_numerator.max(1),
        frame_rate_denominator.max(1),
    );
    let pipeline = gst::parse::launch(&pipeline)
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

    Video::from_gst_pipeline_with_options(pipeline, sink, None, options)
        .map_err(|error| format!("could not initialize timeline video: {error}"))
}

pub fn read_video_codec(video: &Video) -> Option<String> {
    let pipeline = video.pipeline();
    let stream_index = pipeline.property::<i32>("current-video");
    if stream_index < 0 {
        return None;
    }

    let tags = pipeline.emit_by_name::<Option<gst::TagList>>("get-video-tags", &[&stream_index])?;
    let codec_tag = tags.get::<gst::tags::VideoCodec>()?;
    Some(codec_tag.get().to_string())
}

pub fn current_frame_rgba(video: &Video) -> Option<(Vec<u8>, u32, u32)> {
    let (nv12, width, height) = video.current_frame_data()?;
    let width_usize = width as usize;
    let height_usize = height as usize;
    let y_size = width_usize.checked_mul(height_usize)?;
    let uv_size = width_usize.checked_mul(height_usize.div_ceil(2))?;
    if width == 0 || height == 0 || nv12.len() < y_size + uv_size {
        return None;
    }

    let image = YuvBiPlanarImage {
        y_plane: &nv12[..y_size],
        y_stride: width,
        uv_plane: &nv12[y_size..y_size + uv_size],
        uv_stride: width,
        width,
        height,
    };
    let mut rgba = vec![0; y_size.checked_mul(4)?];
    yuv_nv12_to_rgba(
        &image,
        &mut rgba,
        width.checked_mul(4)?,
        YuvRange::Full,
        YuvStandardMatrix::Bt709,
        YuvConversionMode::Balanced,
    )
    .ok()?;

    Some((rgba, width, height))
}

pub fn generate_thumbnail(video_path: &Path, output_path: &Path) -> Result<(), String> {
    gst::init().map_err(|error| format!("could not initialize GStreamer: {error}"))?;
    if let Some(directory) = output_path.parent() {
        fs::create_dir_all(directory)
            .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
    }
    if output_path.is_file() {
        return Ok(());
    }

    let uri = Url::from_file_path(video_path)
        .map_err(|_| format!("could not convert {} to a file URL", video_path.display()))?;
    let description = format!(
        "uridecodebin uri=\"{}\" name=decoder decoder. ! queue ! videoconvert ! videoscale ! video/x-raw,format=RGBA,width=320,height=180,pixel-aspect-ratio=1/1 ! appsink name=history_thumbnail sync=false max-buffers=1 drop=true",
        uri.as_str()
    );
    let pipeline = gst::parse::launch(&description)
        .map_err(|error| format!("could not create thumbnail pipeline: {error}"))?
        .downcast::<gst::Pipeline>()
        .map_err(|_| "thumbnail pipeline had an unexpected type".to_string())?;
    let sink = pipeline
        .by_name("history_thumbnail")
        .ok_or_else(|| "thumbnail sink was not created".to_string())?
        .downcast::<gst_app::AppSink>()
        .map_err(|_| "thumbnail sink had an unexpected type".to_string())?;

    pipeline
        .set_state(gst::State::Paused)
        .map_err(|error| format!("could not start thumbnail pipeline: {error}"))?;
    let result = (|| -> Result<(), String> {
        let sample = sink
            .try_pull_preroll(gst::ClockTime::from_seconds(10))
            .ok_or_else(|| "timed out waiting for the first frame".to_string())?;
        let caps = sample
            .caps()
            .ok_or_else(|| "first frame had no video format".to_string())?;
        let info = gst_video::VideoInfo::from_caps(caps)
            .map_err(|error| format!("could not read first-frame format: {error}"))?;
        let buffer = sample
            .buffer()
            .ok_or_else(|| "first frame had no pixel buffer".to_string())?;
        let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info)
            .map_err(|error| format!("could not map first-frame pixels: {error}"))?;

        let width = frame.width();
        let height = frame.height();
        let row_bytes = width as usize * 4;
        let stride = usize::try_from(frame.info().stride()[0])
            .map_err(|_| "first frame had a negative row stride".to_string())?;
        let source = frame
            .plane_data(0)
            .map_err(|error| format!("could not read first-frame pixels: {error}"))?;
        let mut pixels = Vec::with_capacity(row_bytes * height as usize);
        for row in 0..height as usize {
            let start = row * stride;
            let end = start + row_bytes;
            pixels.extend_from_slice(
                source
                    .get(start..end)
                    .ok_or_else(|| "first-frame row was truncated".to_string())?,
            );
        }

        let temporary_path = output_path.with_extension("png.part");
        image::save_buffer_with_format(
            &temporary_path,
            &pixels,
            width,
            height,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .map_err(|error| format!("could not encode thumbnail: {error}"))?;
        fs::rename(&temporary_path, output_path)
            .map_err(|error| format!("could not finish thumbnail: {error}"))
    })();
    let _ = pipeline.set_state(gst::State::Null);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_pipeline_converts_the_output_frame_rate() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/gpui-video-player/assets/test1.mp4");
        let url = Url::from_file_path(path).unwrap();
        let video = create_timeline_video(&url, VideoOptions::default(), 24, 1).unwrap();

        assert!((video.framerate() - 24.0).abs() < 0.001);
    }
}

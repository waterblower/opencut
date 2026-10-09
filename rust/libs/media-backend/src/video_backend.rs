use crate::{AudioDecoder, MediaInfo, VideoDecoder};
use anyhow::{Context, Result};
use ffmpeg_next::format;
use std::path::Path;

/// 同步解码同时包含视频流和音频流的媒体文件；缺少任一流时打开失败。
/// 纯音频文件使用 AudioBackend。
pub struct VideoBackend {
    pub metadata: MediaInfo,
    pub video: VideoDecoder,
    pub audio: AudioDecoder,
}

impl VideoBackend {
    /// 打开视频和音频；缺少音轨或任一解码器打开失败都返回错误。
    /// 元数据和视频解码器共用一次打开；音频需要独立的读取位置，单独打开。
    pub fn open(path: &Path) -> Result<Self> {
        let input = format::input(path).context("opening video media")?;
        let metadata = MediaInfo::from_av_input(&input, path)?;
        let audio = AudioDecoder::open(path, &metadata)?;
        let video = VideoDecoder::from_av_input(
            input,
            metadata.video.stream_index,
            metadata.origin_microseconds,
        )?;
        Ok(Self {
            metadata,
            video,
            audio,
        })
    }
}

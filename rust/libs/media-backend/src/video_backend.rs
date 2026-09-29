use crate::{
    AudioDecoder, AudioInfo, MediaInfo, VideoDecoder, VideoInfo, time::timestamp_microseconds,
};
use anyhow::{Context, Error, Result, bail};
use ffmpeg_next::{ffi::AV_NOPTS_VALUE, format, media::Type};
use std::{path::Path, time::Duration};

/// 同步解码同时包含视频流和音频流的媒体文件；缺少任一流时打开失败。
/// 纯音频文件使用 AudioBackend。
pub struct VideoBackend {
    pub metadata: MediaInfo,
    pub video: VideoDecoder,
    pub audio: AudioDecoder,
}

impl VideoBackend {
    /// 打开视频和音频；缺少音轨或任一解码器打开失败都返回错误。
    pub fn open(path: &Path) -> Result<Self> {
        let metadata = Self::probe(path)?;
        let audio = AudioDecoder::open(path, &metadata)?;
        let video = VideoDecoder::open(
            path,
            metadata.video.stream_index,
            metadata.origin_microseconds,
        )?;
        Ok(Self {
            metadata,
            video,
            audio,
        })
    }

    /// Requires a video stream and known duration. Probe resources are dropped before returning.
    pub fn probe(path: &Path) -> Result<MediaInfo> {
        let input = format::input(path).context("opening media for metadata")?;
        let Some(video) = input.streams().best(Type::Video) else {
            bail!("media has no video stream");
        };
        let audio = input.streams().best(Type::Audio);

        // SAFETY: input owns the format context for the duration of this read.
        let mut origin = unsafe { (*input.as_ptr()).start_time };
        if origin == AV_NOPTS_VALUE {
            for stream in [Some(&video), audio.as_ref()].into_iter().flatten() {
                if stream.start_time() == AV_NOPTS_VALUE {
                    continue;
                }
                let start = timestamp_microseconds(stream.start_time(), stream.time_base())?;
                if origin == AV_NOPTS_VALUE || start < origin {
                    origin = start;
                }
            }
            if origin == AV_NOPTS_VALUE {
                origin = 0;
            }
        }

        let duration_microseconds = input.duration();
        let duration = if duration_microseconds > 0 {
            Duration::from_micros(duration_microseconds as u64)
        } else {
            bail!("media duration is unavailable: {}", path.display());
        };
        let rate = video.avg_frame_rate();
        let average_frame_interval = if rate.numerator() > 0 && rate.denominator() > 0 {
            Some(Duration::from_secs_f64(
                f64::from(rate.denominator()) / f64::from(rate.numerator()),
            ))
        } else {
            None
        };
        let video_parameters = video.parameters();
        // SAFETY: the parameters and input remain alive; only scalar fields escape.
        let (width, height) = unsafe {
            let parameters = &*video_parameters.as_ptr();
            (parameters.width, parameters.height)
        };
        let video = VideoInfo {
            stream_index: video.index(),
            width: u32::try_from(width).context("invalid video width")?,
            height: u32::try_from(height).context("invalid video height")?,
            average_frame_interval,
        };
        let audio = (|| {
            let Some(stream) = audio else {
                return Ok(None);
            };
            let parameters = stream.parameters();
            // SAFETY: stream parameters are borrowed only while input is alive.
            let (sample_rate, channels) = unsafe {
                let parameters = &*parameters.as_ptr();
                (parameters.sample_rate, parameters.ch_layout.nb_channels)
            };
            Ok::<_, Error>(Some(AudioInfo {
                stream_index: stream.index(),
                sample_rate: u32::try_from(sample_rate).context("invalid audio sample rate")?,
                channels: u16::try_from(channels).context("invalid audio channel count")?,
            }))
        })()?;
        Ok(MediaInfo {
            origin_microseconds: origin,
            duration,
            video,
            audio,
        })
    }
}

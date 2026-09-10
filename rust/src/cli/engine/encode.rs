use super::decode::again;
use crate::{
    cli::{error::Result, time::FrameRate},
    cli_error, cli_try,
};
use ffmpeg_next as ffmpeg;
use image::RgbaImage;
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, SyncSender},
    thread::JoinHandle,
};

pub struct EncoderWorker {
    sender: Option<SyncSender<EncodeMessage>>,
    worker: Option<JoinHandle<Result<()>>>,
    pub audio_frame_size: usize,
}

pub struct VideoEncoding {
    pub codec: String,
    pub preset: String,
    pub bitrate: u64,
}

impl EncoderWorker {
    pub fn open(
        path: PathBuf,
        dimensions: (u32, u32),
        fps: FrameRate,
        rate: u32,
        video: VideoEncoding,
        metadata: Option<String>,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(2);
        let (ready, initialized) = mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            let mut encoder =
                Encoder::open(&path, dimensions, fps, rate, &video, metadata.as_deref())?;
            cli_try!(
                ready.send(encoder.audio_frame_size()),
                "render_cancelled",
                "",
                5
            );
            for message in receiver {
                match message {
                    EncodeMessage::Video(image, frame) => encoder.video(&image, frame)?,
                    EncodeMessage::Audio(samples, start) => encoder.audio(&samples, start)?,
                    EncodeMessage::Finish => return encoder.finish(),
                }
            }
            Err(cli_error!(
                "render_cancelled",
                "",
                5,
                "encoder input closed before completion"
            ))
        });
        let mut result = Self {
            sender: Some(sender),
            worker: Some(worker),
            audio_frame_size: 0,
        };
        match initialized.recv() {
            Ok(size) => {
                result.audio_frame_size = size;
                Ok(result)
            }
            Err(_) => {
                result.join()?;
                Err(cli_error!(
                    "encode_failure",
                    "",
                    5,
                    "encoder did not initialize"
                ))
            }
        }
    }

    pub fn video(&mut self, image: RgbaImage, frame: i64) -> Result<()> {
        if self
            .sender
            .as_ref()
            .unwrap()
            .send(EncodeMessage::Video(image, frame))
            .is_err()
        {
            self.join()?;
            return Err(cli_error!("encode_failure", "", 5, "encoder closed"));
        }
        Ok(())
    }

    pub fn audio(&mut self, samples: Vec<[f32; 2]>, start: i64) -> Result<()> {
        if self
            .sender
            .as_ref()
            .unwrap()
            .send(EncodeMessage::Audio(samples, start))
            .is_err()
        {
            self.join()?;
            return Err(cli_error!("encode_failure", "", 5, "encoder closed"));
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<()> {
        let _ = self.sender.as_ref().unwrap().send(EncodeMessage::Finish);
        self.join()
    }
}

impl Drop for EncoderWorker {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub struct Encoder {
    output: ffmpeg::format::context::Output,
    video: ffmpeg::encoder::Video,
    audio: ffmpeg::encoder::Audio,
    scaler: ffmpeg::software::scaling::Context,
    video_base: ffmpeg::Rational,
    audio_base: ffmpeg::Rational,
    rate: u32,
}

impl Encoder {
    pub fn open(
        path: &Path,
        dimensions: (u32, u32),
        fps: FrameRate,
        rate: u32,
        settings: &VideoEncoding,
        metadata: Option<&str>,
    ) -> Result<Self> {
        super::probe::init()?;
        let (width, height) = dimensions;
        let codec_name = settings.codec.as_str();
        let preset = settings.preset.as_str();
        let codec = video_codec(codec_name)?;
        let mut output = cli_try!(ffmpeg::format::output(path), "encode_failure", "", 5);
        let global = output
            .format()
            .flags()
            .contains(ffmpeg::format::Flags::GLOBAL_HEADER);
        let mut video = cli_try!(
            ffmpeg::codec::context::Context::new_with_codec(codec)
                .encoder()
                .video(),
            "encode_failure",
            "",
            5
        );
        video.set_width(width);
        video.set_height(height);
        let pixel = if codec_name == "prores" {
            ffmpeg::format::Pixel::YUV422P10LE
        } else {
            ffmpeg::format::Pixel::YUV420P
        };
        video.set_format(pixel);
        let video_base = ffmpeg::Rational(fps.denominator as i32, fps.numerator as i32);
        video.set_time_base(video_base);
        video.set_frame_rate(Some((fps.numerator as i32, fps.denominator as i32)));
        let bitrate = cli_try!(usize::try_from(settings.bitrate), "invalid_bitrate", "", 2);
        video.set_bit_rate(bitrate);
        video.set_max_b_frames(0);
        video.set_gop((fps.numerator as f64 / fps.denominator as f64 * 2.0).round() as u32);
        video.set_colorspace(ffmpeg::color::Space::BT709);
        video.set_color_range(ffmpeg::color::Range::MPEG);
        unsafe {
            (*video.as_mut_ptr()).color_primaries = ffmpeg::ffi::AVColorPrimaries::AVCOL_PRI_BT709;
            (*video.as_mut_ptr()).color_trc =
                ffmpeg::ffi::AVColorTransferCharacteristic::AVCOL_TRC_BT709;
        }
        if global {
            video.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
        }
        let mut options = ffmpeg::Dictionary::new();
        if codec.name().ends_with("_videotoolbox") {
            options.set("allow_sw", "1");
        }
        if codec.name() == "libx264" {
            options.set(
                "preset",
                if preset == "draft" {
                    "ultrafast"
                } else if preset == "high" {
                    "slow"
                } else {
                    "medium"
                },
            );
        }
        if codec_name == "prores" {
            options.set(
                "profile",
                if preset == "draft" {
                    "0"
                } else if preset == "high" {
                    "3"
                } else {
                    "2"
                },
            );
        }
        let video = cli_try!(video.open_with(options), "encoder_unavailable", "", 5);
        {
            let mut stream = cli_try!(output.add_stream(codec), "encode_failure", "", 5);
            stream.set_time_base(video_base);
            stream.set_parameters(&video);
            if codec_name == "hevc" {
                // Apple playback expects hvc1. Together with GLOBAL_HEADER above,
                // this tells the MOV muxer to store complete parameter sets in hvcC.
                // ffmpeg-next does not expose a codec-tag setter.
                unsafe {
                    (*stream.parameters().as_mut_ptr()).codec_tag = u32::from_le_bytes(*b"hvc1");
                }
            }
        }
        let Some(aac) = ffmpeg::encoder::find(ffmpeg::codec::Id::AAC) else {
            return Err(cli_error!(
                "encoder_unavailable",
                "",
                5,
                "AAC encoder is not compiled in"
            ));
        };
        let mut audio = cli_try!(
            ffmpeg::codec::context::Context::new_with_codec(aac)
                .encoder()
                .audio(),
            "encode_failure",
            "",
            5
        );
        audio.set_rate(rate as i32);
        audio.set_channel_layout(ffmpeg::ChannelLayout::STEREO);
        audio.set_format(ffmpeg::format::Sample::F32(
            ffmpeg::format::sample::Type::Planar,
        ));
        audio.set_bit_rate(192000);
        audio.set_time_base((1, rate as i32));
        if global {
            audio.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
        }
        let audio = cli_try!(audio.open_as(aac), "encode_failure", "", 5);
        let audio_base = ffmpeg::Rational(1, rate as i32);
        {
            let mut stream = cli_try!(output.add_stream(aac), "encode_failure", "", 5);
            stream.set_time_base(audio_base);
            stream.set_parameters(&audio);
        }
        if let Some(metadata) = metadata {
            let mut dictionary = ffmpeg::Dictionary::new();
            dictionary.set("opencut.timeline", metadata);
            output.set_metadata(dictionary);
        }
        let mut mux_options = ffmpeg::Dictionary::new();
        mux_options.set("movflags", "+use_metadata_tags");
        cli_try!(
            output.write_header_with(mux_options),
            "encode_failure",
            "",
            5
        );
        let scaler = cli_try!(
            ffmpeg::software::scaling::Context::get(
                ffmpeg::format::Pixel::RGBA,
                width,
                height,
                pixel,
                width,
                height,
                ffmpeg::software::scaling::Flags::BILINEAR
            ),
            "encode_failure",
            "",
            5
        );
        unsafe {
            let coefficients = ffmpeg::ffi::sws_getCoefficients(ffmpeg::ffi::SWS_CS_ITU709);
            let result = ffmpeg::ffi::sws_setColorspaceDetails(
                scaler.as_ptr() as *mut _,
                coefficients,
                1,
                coefficients,
                0,
                0,
                1 << 16,
                1 << 16,
            );
            if result < 0 {
                return Err(cli_error!(
                    "encode_failure",
                    "",
                    5,
                    "cannot configure BT.709 conversion"
                ));
            }
        }
        Ok(Self {
            output,
            video,
            audio,
            scaler,
            video_base,
            audio_base,
            rate,
        })
    }

    pub fn audio_frame_size(&self) -> usize {
        self.audio.frame_size() as usize
    }

    pub fn video(&mut self, image: &RgbaImage, frame: i64) -> Result<()> {
        let mut rgba =
            ffmpeg::frame::Video::new(ffmpeg::format::Pixel::RGBA, image.width(), image.height());
        let row = image.width() as usize * 4;
        let stride = rgba.stride(0);
        for y in 0..image.height() as usize {
            rgba.data_mut(0)[y * stride..y * stride + row]
                .copy_from_slice(&image.as_raw()[y * row..(y + 1) * row]);
        }
        let mut converted = ffmpeg::frame::Video::empty();
        cli_try!(
            self.scaler.run(&rgba, &mut converted),
            "encode_failure",
            "",
            5
        );
        converted.set_pts(Some(frame));
        converted.set_color_space(ffmpeg::color::Space::BT709);
        converted.set_color_range(ffmpeg::color::Range::MPEG);
        converted.set_color_primaries(ffmpeg::color::Primaries::BT709);
        converted.set_color_transfer_characteristic(ffmpeg::color::TransferCharacteristic::BT709);
        cli_try!(self.video.send_frame(&converted), "encode_failure", "", 5);
        self.drain_video()
    }

    pub fn audio(&mut self, samples: &[[f32; 2]], start: i64) -> Result<()> {
        let mut frame = ffmpeg::frame::Audio::new(
            self.audio.format(),
            samples.len(),
            ffmpeg::ChannelLayout::STEREO,
        );
        frame.set_rate(self.rate);
        frame.set_pts(Some(start));
        for channel in 0..2 {
            for (i, sample) in samples.iter().enumerate() {
                frame.plane_mut::<f32>(channel)[i] = sample[channel];
            }
        }
        cli_try!(self.audio.send_frame(&frame), "encode_failure", "", 5);
        self.drain_audio()
    }

    pub fn finish(mut self) -> Result<()> {
        cli_try!(self.video.send_eof(), "encode_failure", "", 5);
        self.drain_video()?;
        cli_try!(self.audio.send_eof(), "encode_failure", "", 5);
        self.drain_audio()?;
        cli_try!(self.output.write_trailer(), "encode_failure", "", 5);
        Ok(())
    }
}

pub fn video_codec(name: &str) -> Result<ffmpeg::Codec> {
    let encoder = match name {
        "h264" => {
            #[cfg(feature = "gpl")]
            {
                "libx264"
            }
            #[cfg(not(feature = "gpl"))]
            {
                "h264_videotoolbox"
            }
        }
        "hevc" => "hevc_videotoolbox",
        "prores" => "prores_ks",
        _ => {
            return Err(cli_error!(
                "encoder_unavailable",
                "",
                5,
                "unsupported codec {name}"
            ));
        }
    };
    let Some(codec) = ffmpeg::encoder::find_by_name(encoder) else {
        return Err(cli_error!(
            "encoder_unavailable",
            "",
            5,
            "encoder {encoder} is not compiled in; use ProRes or an appropriate build"
        ));
    };
    Ok(codec)
}

pub fn bitrate(width: u32, height: u32, fps: FrameRate, preset: &str) -> usize {
    let factor = match preset {
        "draft" => 0.07,
        "high" => 0.25,
        _ => 0.14,
    };
    (width as f64 * height as f64 * fps.numerator as f64 / fps.denominator as f64 * factor)
        .max(128000.0) as usize
}

enum EncodeMessage {
    Video(RgbaImage, i64),
    Audio(Vec<[f32; 2]>, i64),
    Finish,
}

impl EncoderWorker {
    fn join(&mut self) -> Result<()> {
        self.sender.take();
        let Some(worker) = self.worker.take() else {
            return Err(cli_error!(
                "encode_failure",
                "",
                5,
                "encoder already joined"
            ));
        };
        match worker.join() {
            Ok(result) => result,
            Err(_) => Err(cli_error!(
                "encode_failure",
                "",
                5,
                "encoder worker panicked"
            )),
        }
    }
}

impl Encoder {
    fn drain_video(&mut self) -> Result<()> {
        loop {
            let mut packet = ffmpeg::Packet::empty();
            match self.video.receive_packet(&mut packet) {
                Ok(()) => {
                    packet.set_stream(0);
                    if packet.duration() <= 0 {
                        packet.set_duration(1);
                    }
                    packet.rescale_ts(self.video_base, self.output.stream(0).unwrap().time_base());
                    cli_try!(
                        packet.write_interleaved(&mut self.output),
                        "encode_failure",
                        "",
                        5
                    );
                }
                Err(ffmpeg::Error::Eof) => return Ok(()),
                Err(error) if again(error) => return Ok(()),
                Err(error) => return Err(cli_error!("encode_failure", "", 5, "{error}")),
            }
        }
    }

    fn drain_audio(&mut self) -> Result<()> {
        loop {
            let mut packet = ffmpeg::Packet::empty();
            match self.audio.receive_packet(&mut packet) {
                Ok(()) => {
                    packet.set_stream(1);
                    packet.rescale_ts(self.audio_base, self.output.stream(1).unwrap().time_base());
                    cli_try!(
                        packet.write_interleaved(&mut self.output),
                        "encode_failure",
                        "",
                        5
                    );
                }
                Err(ffmpeg::Error::Eof) => return Ok(()),
                Err(error) if again(error) => return Ok(()),
                Err(error) => return Err(cli_error!("encode_failure", "", 5, "{error}")),
            }
        }
    }
}

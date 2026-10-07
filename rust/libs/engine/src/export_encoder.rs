use anyhow::{Context as _, Result, bail};
use ffmpeg_next as ffmpeg;
use image::RgbaImage;
use std::path::Path;
use timeline::TimelineSettings;

pub struct ExportEncoder {
    output: ffmpeg::format::context::Output,
    video: ffmpeg::encoder::Video,
    audio: ffmpeg::encoder::Audio,
    scaler: ffmpeg::software::scaling::Context,
}

impl ExportEncoder {
    pub fn open(path: &Path, settings: TimelineSettings, bitrate: u64) -> Result<Self> {
        let mut output = ffmpeg::format::output(path).context("Opening export container")?;
        let global = output
            .format()
            .flags()
            .contains(ffmpeg::format::Flags::GLOBAL_HEADER);
        let codec = ffmpeg::encoder::find_by_name("h264_videotoolbox")
            .context("H.264 VideoToolbox encoder is unavailable")?;
        let mut video = ffmpeg::codec::context::Context::new_with_codec(codec)
            .encoder()
            .video()?;
        video.set_width(settings.width);
        video.set_height(settings.height);
        video.set_format(ffmpeg::format::Pixel::YUV420P);
        let fps = settings.frame_rate;
        video.set_time_base((fps.denominator as i32, fps.numerator as i32));
        video.set_frame_rate(Some((fps.numerator as i32, fps.denominator as i32)));
        video.set_bit_rate(usize::try_from(bitrate).context("Export bitrate is too large")?);
        video.set_max_b_frames(0);
        video.set_gop(
            ((f64::from(fps.numerator) / f64::from(fps.denominator)) * 2.0)
                .round()
                .clamp(1.0, i32::MAX as f64) as u32,
        );
        video.set_colorspace(ffmpeg::color::Space::BT709);
        video.set_color_range(ffmpeg::color::Range::MPEG);
        // SAFETY: this encoder exclusively owns its live codec context.
        unsafe {
            (*video.as_mut_ptr()).color_primaries = ffmpeg::ffi::AVColorPrimaries::AVCOL_PRI_BT709;
            (*video.as_mut_ptr()).color_trc =
                ffmpeg::ffi::AVColorTransferCharacteristic::AVCOL_TRC_BT709;
        }
        if global {
            video.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
        }
        let mut options = ffmpeg::Dictionary::new();
        options.set("allow_sw", "1");
        let video = video
            .open_with(options)
            .context("Opening H.264 export encoder")?;
        {
            let mut stream = output.add_stream(codec)?;
            stream.set_time_base(video.time_base());
            stream.set_parameters(&video);
        }

        let aac =
            ffmpeg::encoder::find(ffmpeg::codec::Id::AAC).context("AAC encoder is unavailable")?;
        let mut audio = ffmpeg::codec::context::Context::new_with_codec(aac)
            .encoder()
            .audio()?;
        audio.set_rate(settings.audio_sample_rate as i32);
        audio.set_time_base((1, settings.audio_sample_rate as i32));
        audio.set_channel_layout(ffmpeg::ChannelLayout::STEREO);
        audio.set_format(ffmpeg::format::Sample::F32(
            ffmpeg::format::sample::Type::Planar,
        ));
        audio.set_bit_rate(192_000);
        if global {
            audio.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
        }
        let audio = audio.open_as(aac).context("Opening AAC export encoder")?;
        if audio.frame_size() == 0 {
            bail!("AAC encoder has no fixed frame size");
        }
        {
            let mut stream = output.add_stream(aac)?;
            stream.set_time_base(audio.time_base());
            stream.set_parameters(&audio);
        }
        output.write_header().context("Writing export header")?;
        let mut scaler = ffmpeg::software::scaling::Context::get(
            ffmpeg::format::Pixel::RGBA,
            settings.width,
            settings.height,
            ffmpeg::format::Pixel::YUV420P,
            settings.width,
            settings.height,
            ffmpeg::software::scaling::Flags::BILINEAR,
        )?;
        // SAFETY: scaler owns the context; FFmpeg returns a static coefficient table.
        unsafe {
            let matrix = ffmpeg::ffi::sws_getCoefficients(ffmpeg::ffi::SWS_CS_ITU709);
            if ffmpeg::ffi::sws_setColorspaceDetails(
                scaler.as_mut_ptr(),
                matrix,
                1,
                matrix,
                0,
                0,
                1 << 16,
                1 << 16,
            ) < 0
            {
                bail!("Could not configure export BT.709 conversion");
            }
        }
        Ok(Self {
            output,
            video,
            audio,
            scaler,
        })
    }

    pub fn video(&mut self, image: &RgbaImage, index: i64) -> Result<()> {
        if image.dimensions() != (self.video.width(), self.video.height()) {
            bail!(
                "Export renderer returned {}x{} pixels; expected {}x{}",
                image.width(),
                image.height(),
                self.video.width(),
                self.video.height(),
            );
        }
        let mut rgba =
            ffmpeg::frame::Video::new(ffmpeg::format::Pixel::RGBA, image.width(), image.height());
        let row = image.width() as usize * 4;
        let stride = rgba.stride(0);
        for y in 0..image.height() as usize {
            rgba.data_mut(0)[y * stride..y * stride + row]
                .copy_from_slice(&image.as_raw()[y * row..(y + 1) * row]);
        }
        let mut frame = ffmpeg::frame::Video::empty();
        self.scaler
            .run(&rgba, &mut frame)
            .context("Converting export pixels")?;
        frame.set_pts(Some(index));
        frame.set_color_space(ffmpeg::color::Space::BT709);
        frame.set_color_range(ffmpeg::color::Range::MPEG);
        frame.set_color_primaries(ffmpeg::color::Primaries::BT709);
        frame.set_color_transfer_characteristic(ffmpeg::color::TransferCharacteristic::BT709);
        self.video
            .send_frame(&frame)
            .context("Encoding export video")?;
        self.drain_video(false)
    }

    pub fn audio(&mut self, samples: &[[f32; 2]], start: i64, total_samples: i64) -> Result<()> {
        let mut frame = ffmpeg::frame::Audio::new(
            self.audio.format(),
            samples.len(),
            ffmpeg::ChannelLayout::STEREO,
        );
        frame.set_rate(self.audio.rate());
        frame.set_pts(Some(start));
        for channel in 0..2 {
            for (index, sample) in samples.iter().enumerate() {
                frame.plane_mut::<f32>(channel)[index] = sample[channel];
            }
        }
        self.audio
            .send_frame(&frame)
            .context("Encoding export audio")?;
        self.drain_audio(false, total_samples)
    }

    pub fn finish(mut self, total_samples: i64) -> Result<()> {
        self.video.send_eof().context("Finishing video encoder")?;
        self.drain_video(true)?;
        self.audio.send_eof().context("Finishing audio encoder")?;
        self.drain_audio(true, total_samples)?;
        self.output
            .write_trailer()
            .context("Finishing export container")?;
        Ok(())
    }

    pub fn audio_frame_size(&self) -> u32 {
        self.audio.frame_size()
    }

    fn drain_video(&mut self, finishing: bool) -> Result<()> {
        loop {
            let mut packet = ffmpeg::Packet::empty();
            match self.video.receive_packet(&mut packet) {
                Ok(()) => {
                    packet.set_stream(0);
                    if packet.duration() <= 0 {
                        packet.set_duration(1);
                    }
                    packet.rescale_ts(
                        self.video.time_base(),
                        self.output.stream(0).unwrap().time_base(),
                    );
                    packet
                        .write_interleaved(&mut self.output)
                        .context("Writing export video")?;
                }
                Err(ffmpeg::Error::Eof) => return Ok(()),
                Err(ffmpeg::Error::Other { errno })
                    if errno == ffmpeg::error::EAGAIN && !finishing =>
                {
                    return Ok(());
                }
                Err(error) => return Err(error).context("Draining export video encoder"),
            }
        }
    }

    fn drain_audio(&mut self, finishing: bool, total_samples: i64) -> Result<()> {
        loop {
            let mut packet = ffmpeg::Packet::empty();
            match self.audio.receive_packet(&mut packet) {
                Ok(()) => {
                    // 编码填充不延长容器时长；负 PTS 保留 AAC priming 信息。
                    if let Some(pts) = packet.pts() {
                        if pts >= total_samples {
                            continue;
                        }
                        if pts >= 0 {
                            packet.set_duration(packet.duration().min(total_samples - pts));
                        }
                    }
                    packet.set_stream(1);
                    packet.rescale_ts(
                        self.audio.time_base(),
                        self.output.stream(1).unwrap().time_base(),
                    );
                    packet
                        .write_interleaved(&mut self.output)
                        .context("Writing export audio")?;
                }
                Err(ffmpeg::Error::Eof) => return Ok(()),
                Err(ffmpeg::Error::Other { errno })
                    if errno == ffmpeg::error::EAGAIN && !finishing =>
                {
                    return Ok(());
                }
                Err(error) => return Err(error).context("Draining export audio encoder"),
            }
        }
    }
}

//! Writes a lower-resolution copy of a video. The video stream is re-encoded to H.264 at the
//! target height; the audio stream is copied unchanged, and timestamps are kept, so a time in
//! the copy is the same time in the source.
use anyhow::{Context as _, Result, bail};
use ffmpeg_next as ffmpeg;
use std::fs;
use std::path::Path;

pub struct ResizeOption {
    pub height: u32,        // 输出高度；宽度按原比例取偶数
    pub video_bitrate: u64, // 比特每秒
    pub overwrite: bool,
}

/// Returns the output width and height.
pub fn resize_video(source: &Path, output: &Path, option: &ResizeOption) -> Result<(u32, u32)> {
    if output.extension().and_then(|value| value.to_str()) != Some("mp4") {
        bail!("Resize output must have an .mp4 extension");
    }
    match fs::symlink_metadata(output) {
        Ok(_) if !option.overwrite => bail!("Resize output already exists: {}", output.display()),
        Ok(_) => {
            if fs::canonicalize(output).context("Resolving resize output")?
                == fs::canonicalize(source).context("Resolving resize source")?
            {
                bail!(
                    "Resize output would replace the source: {}",
                    output.display()
                );
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("Inspecting resize output"),
    }

    ffmpeg::init().context("Initializing FFmpeg")?;
    let mut input =
        ffmpeg::format::input(source).with_context(|| format!("Opening {}", source.display()))?;
    let video_stream = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .with_context(|| format!("{} has no video stream", source.display()))?;
    let video_index = video_stream.index();
    let video_time_base = video_stream.time_base();
    let frame_rate = video_stream.avg_frame_rate();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())
        .context("Reading video parameters")?
        .decoder()
        .video()
        .context("Opening video decoder")?;
    let audio = input
        .streams()
        .best(ffmpeg::media::Type::Audio)
        .map(|stream| (stream.index(), stream.time_base(), stream.parameters()));

    if decoder.height() <= option.height {
        bail!(
            "{} is {}p; it must be taller than {}p to resize",
            source.display(),
            decoder.height(),
            option.height
        );
    }
    let height = option.height;
    // 按原宽高比缩放，宽度取偶数（YUV 4:2:0 要求）。
    let width =
        ((u64::from(decoder.width()) * u64::from(height) / u64::from(decoder.height())) as u32 + 1)
            & !1;

    let mut output_context =
        ffmpeg::format::output(output).with_context(|| format!("Creating {}", output.display()))?;
    let global = output_context
        .format()
        .flags()
        .contains(ffmpeg::format::Flags::GLOBAL_HEADER);
    let codec = ffmpeg::encoder::find_by_name("h264_videotoolbox")
        .context("H.264 VideoToolbox encoder is unavailable")?;
    let mut encoder = ffmpeg::codec::context::Context::new_with_codec(codec)
        .encoder()
        .video()?;
    encoder.set_width(width);
    encoder.set_height(height);
    encoder.set_format(ffmpeg::format::Pixel::YUV420P);
    encoder.set_time_base(video_time_base);
    if frame_rate.numerator() > 0 && frame_rate.denominator() > 0 {
        encoder.set_frame_rate(Some(frame_rate));
        // 每 2 秒一个关键帧，便于拖动与按时间截取。
        encoder.set_gop(
            (2.0 * f64::from(frame_rate))
                .round()
                .clamp(1.0, f64::from(i32::MAX)) as u32,
        );
    }
    encoder.set_bit_rate(
        usize::try_from(option.video_bitrate).context("Resize bitrate is too large")?,
    );
    encoder.set_max_b_frames(0);
    encoder.set_colorspace(decoder.color_space());
    encoder.set_color_range(ffmpeg::color::Range::MPEG);
    if global {
        encoder.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
    }
    let mut options = ffmpeg::Dictionary::new();
    options.set("allow_sw", "1");
    let mut encoder = encoder
        .open_with(options)
        .context("Opening H.264 encoder")?;
    let video_output_index = {
        let mut stream = output_context.add_stream(codec)?;
        stream.set_time_base(video_time_base);
        stream.set_parameters(&encoder);
        stream.index()
    };
    let audio_output = match audio {
        Some((index, time_base, parameters)) => {
            let mut stream =
                output_context.add_stream(ffmpeg::encoder::find(ffmpeg::codec::Id::None))?;
            stream.set_parameters(parameters);
            // SAFETY: the stream belongs to the open output; clearing the tag lets the muxer
            // choose one valid for MP4.
            unsafe {
                (*stream.parameters().as_mut_ptr()).codec_tag = 0;
            }
            stream.set_time_base(time_base);
            Some((index, time_base, stream.index()))
        }
        None => None,
    };
    output_context
        .write_header()
        .context("Writing resize header")?;

    let mut scaler = ffmpeg::software::scaling::Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        ffmpeg::format::Pixel::YUV420P,
        width,
        height,
        ffmpeg::software::scaling::Flags::BILINEAR,
    )
    .context("Creating scaler")?;

    let mut decoded = ffmpeg::frame::Video::empty();
    for (stream, mut packet) in input.packets() {
        if stream.index() == video_index {
            decoder.send_packet(&packet).context("Decoding video")?;
            while decoder.receive_frame(&mut decoded).is_ok() {
                encode_frame(&mut scaler, &decoded, &mut encoder)?;
                write_video_packets(
                    &mut encoder,
                    &mut output_context,
                    video_output_index,
                    video_time_base,
                    false,
                )?;
            }
            continue;
        }
        let Some((audio_index, audio_time_base, audio_output_index)) = audio_output else {
            continue;
        };
        if stream.index() != audio_index {
            continue;
        }
        packet.set_stream(audio_output_index);
        packet.set_position(-1);
        packet.rescale_ts(
            audio_time_base,
            output_context
                .stream(audio_output_index)
                .unwrap()
                .time_base(),
        );
        packet
            .write_interleaved(&mut output_context)
            .context("Writing audio")?;
    }

    decoder.send_eof().context("Finishing video decoder")?;
    while decoder.receive_frame(&mut decoded).is_ok() {
        encode_frame(&mut scaler, &decoded, &mut encoder)?;
        write_video_packets(
            &mut encoder,
            &mut output_context,
            video_output_index,
            video_time_base,
            false,
        )?;
    }
    encoder.send_eof().context("Finishing video encoder")?;
    write_video_packets(
        &mut encoder,
        &mut output_context,
        video_output_index,
        video_time_base,
        true,
    )?;
    output_context
        .write_trailer()
        .context("Finishing resize output")?;
    Ok((width, height))
}

fn encode_frame(
    scaler: &mut ffmpeg::software::scaling::Context,
    decoded: &ffmpeg::frame::Video,
    encoder: &mut ffmpeg::encoder::Video,
) -> Result<()> {
    let mut scaled = ffmpeg::frame::Video::empty();
    scaler.run(decoded, &mut scaled).context("Scaling video")?;
    // 保留源时间戳，输出与源的时间一一对应。
    scaled.set_pts(decoded.timestamp());
    scaled.set_color_space(decoded.color_space());
    scaled.set_color_range(ffmpeg::color::Range::MPEG);
    scaled.set_color_primaries(decoded.color_primaries());
    scaled.set_color_transfer_characteristic(decoded.color_transfer_characteristic());
    encoder.send_frame(&scaled).context("Encoding video")
}

fn write_video_packets(
    encoder: &mut ffmpeg::encoder::Video,
    output: &mut ffmpeg::format::context::Output,
    stream_index: usize,
    encoder_time_base: ffmpeg::Rational,
    finishing: bool,
) -> Result<()> {
    loop {
        let mut packet = ffmpeg::Packet::empty();
        match encoder.receive_packet(&mut packet) {
            Ok(()) => {
                packet.set_stream(stream_index);
                packet.rescale_ts(
                    encoder_time_base,
                    output.stream(stream_index).unwrap().time_base(),
                );
                packet.write_interleaved(output).context("Writing video")?;
            }
            Err(ffmpeg::Error::Eof) => return Ok(()),
            Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN && !finishing => {
                return Ok(());
            }
            Err(error) => return Err(error).context("Draining H.264 encoder"),
        }
    }
}

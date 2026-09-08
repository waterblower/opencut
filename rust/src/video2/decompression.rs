use anyhow::{Context as _, Result};
use ffmpeg_next as ffmpeg;

#[cfg(target_os = "macos")]
mod videotoolbox;

pub enum Decompression {
    Software(ffmpeg::decoder::Video),
    #[cfg(target_os = "macos")]
    Hardware(videotoolbox::Decoder),
}

impl Decompression {
    pub fn open(
        parameters: ffmpeg::codec::Parameters,
        base: ffmpeg::Rational,
        container: &str,
    ) -> Result<Self> {
        #[cfg(target_os = "macos")]
        // MOV demuxing supplies decode-order DTS and normalizes negative
        // composition offsets. Other containers keep the software decoder.
        if container
            .split(',')
            .any(|name| matches!(name, "mov" | "mp4"))
            && let Some(decoder) = videotoolbox::Decoder::open(&parameters, base)?
        {
            return Ok(Self::Hardware(decoder));
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (base, container);
        let mut context = ffmpeg::codec::context::Context::from_parameters(parameters).context(
            format!("Reading video parameters at {}:{}", file!(), line!()),
        )?;
        context.set_threading(ffmpeg::codec::threading::Config::kind(
            ffmpeg::codec::threading::Type::Frame,
        ));
        Ok(Self::Software(context.decoder().video().context(
            format!("Opening software video decoder at {}:{}", file!(), line!()),
        )?))
    }

    pub fn send_packet(&mut self, packet: &ffmpeg::Packet) -> Result<()> {
        match self {
            Self::Software(decoder) => decoder.send_packet(packet).context(format!(
                "Sending software video packet at {}:{}",
                file!(),
                line!()
            )),
            #[cfg(target_os = "macos")]
            Self::Hardware(decoder) => decoder.send_packet(packet),
        }
    }

    pub fn send_eof(&mut self) -> Result<()> {
        match self {
            Self::Software(decoder) => decoder.send_eof().context(format!(
                "Draining software video at {}:{}",
                file!(),
                line!()
            )),
            #[cfg(target_os = "macos")]
            Self::Hardware(decoder) => decoder.send_eof(),
        }
    }

    pub fn flush(&mut self) -> Result<()> {
        match self {
            Self::Software(decoder) => {
                decoder.flush();
                Ok(())
            }
            #[cfg(target_os = "macos")]
            Self::Hardware(decoder) => decoder.flush(),
        }
    }

    pub fn seek_target(&mut self, target: Option<i64>) {
        #[cfg(target_os = "macos")]
        if let Self::Hardware(decoder) = self {
            decoder.seek = target;
        }
        #[cfg(not(target_os = "macos"))]
        let _ = target;
    }

    /// The boolean proves that no presentation pictures were suppressed.
    pub fn receive(&mut self) -> Result<Option<(ffmpeg::frame::Video, bool)>> {
        match self {
            Self::Software(decoder) => {
                let mut image = ffmpeg::frame::Video::empty();
                match decoder.receive_frame(&mut image) {
                    Ok(()) => Ok(Some((image, true))),
                    Err(ffmpeg::Error::Eof) => Ok(None),
                    Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => {
                        Ok(None)
                    }
                    Err(error) => Err(anyhow::Error::new(error).context(format!(
                        "Decoding software video at {}:{}",
                        file!(),
                        line!()
                    ))),
                }
            }
            #[cfg(target_os = "macos")]
            Self::Hardware(decoder) => decoder.receive(),
        }
    }
}

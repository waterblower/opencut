use crate::{cli::error::Result, cli_error, cli_try};
use ffmpeg_next as ffmpeg;
use image::RgbaImage;
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender},
    thread::JoinHandle,
};

pub struct VideoWorker {
    sender: Option<SyncSender<f64>>,
    receiver: Receiver<Result<RgbaImage>>,
    thread: Option<JoinHandle<()>>,
}

impl VideoWorker {
    pub fn new(path: PathBuf) -> Self {
        let (sender, requests) = mpsc::sync_channel(1);
        let (results, receiver) = mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || {
            let mut reader = match VideoReader::open(&path) {
                Ok(reader) => reader,
                Err(error) => {
                    let _ = results.send(Err(error));
                    return;
                }
            };
            for time in requests {
                let result = reader.at(time);
                let failed = result.is_err();
                if results.send(result).is_err() || failed {
                    break;
                }
            }
        });
        Self {
            sender: Some(sender),
            receiver,
            thread: Some(thread),
        }
    }
    pub fn at(&self, seconds: f64) -> Result<RgbaImage> {
        if self.sender.as_ref().unwrap().send(seconds).is_err() {
            if let Ok(result) = self.receiver.try_recv() {
                return result;
            }
            return Err(cli_error!(
                "decode_failure",
                "",
                5,
                "decoder worker stopped"
            ));
        }
        cli_try!(self.receiver.recv(), "decode_failure", "", 5)
    }
}

impl Drop for VideoWorker {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn again(error: ffmpeg::Error) -> bool {
    matches!(error, ffmpeg::Error::Other { errno } if errno == ffmpeg::error::EAGAIN)
}

pub fn origin(input: &ffmpeg::format::context::Input) -> f64 {
    let start = unsafe { (*input.as_ptr()).start_time };
    if start == ffmpeg::ffi::AV_NOPTS_VALUE {
        0.0
    } else {
        start as f64 / ffmpeg::ffi::AV_TIME_BASE as f64
    }
}

struct VideoReader {
    input: ffmpeg::format::context::Input,
    decoder: ffmpeg::decoder::Video,
    scaler: ffmpeg::software::scaling::Context,
    stream: usize,
    time_base: f64,
    origin: f64,
    eof: bool,
    previous: Option<(f64, RgbaImage)>,
    next: Option<(f64, RgbaImage)>,
    requested: Option<f64>,
    rotation: i32,
}

impl VideoReader {
    fn open(path: &Path) -> Result<Self> {
        super::probe::init()?;
        let input = cli_try!(ffmpeg::format::input(path), "unreadable_media", "", 4);
        let Some(stream) = input.streams().best(ffmpeg::media::Type::Video) else {
            return Err(cli_error!(
                "missing_video",
                "",
                4,
                "{} has no video stream",
                path.display()
            ));
        };
        let context = cli_try!(
            ffmpeg::codec::context::Context::from_parameters(stream.parameters()),
            "decode_failure",
            "",
            5
        );
        let decoder = cli_try!(context.decoder().video(), "decode_failure", "", 5);
        let scaler = cli_try!(
            ffmpeg::software::scaling::Context::get(
                decoder.format(),
                decoder.width(),
                decoder.height(),
                ffmpeg::format::Pixel::RGBA,
                decoder.width(),
                decoder.height(),
                ffmpeg::software::scaling::Flags::BILINEAR
            ),
            "decode_failure",
            "",
            5
        );
        let mut rotation = 0;
        for side in stream.side_data() {
            if side.kind() == ffmpeg::codec::packet::side_data::Type::DisplayMatrix
                && side.data().len() >= 36
            {
                let mut matrix = [0_i32; 9];
                for (i, chunk) in side.data()[..36].chunks_exact(4).enumerate() {
                    matrix[i] = i32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                }
                rotation = (-unsafe { ffmpeg::ffi::av_display_rotation_get(matrix.as_ptr()) })
                    .round() as i32;
            }
        }
        Ok(Self {
            stream: stream.index(),
            time_base: f64::from(stream.time_base()),
            origin: origin(&input),
            input,
            decoder,
            scaler,
            eof: false,
            previous: None,
            next: None,
            requested: None,
            rotation,
        })
    }

    fn at(&mut self, seconds: f64) -> Result<RgbaImage> {
        let seek = match self.requested {
            Some(last) => seconds < last || seconds - last > 2.0,
            None => seconds > 0.0,
        };
        if seek {
            let timestamp =
                ((seconds + self.origin) * ffmpeg::ffi::AV_TIME_BASE as f64).floor() as i64;
            cli_try!(
                self.input.seek(timestamp, ..timestamp),
                "seek_failure",
                "",
                5
            );
            self.decoder.flush();
            self.eof = false;
            self.previous = None;
            self.next = None;
        }
        self.requested = Some(seconds);
        loop {
            if let Some((time, image)) = &self.next
                && *time >= seconds
            {
                if let Some((previous_time, previous)) = &self.previous
                    && seconds - previous_time <= time - seconds
                {
                    return Ok(previous.clone());
                }
                return Ok(image.clone());
            }
            if self.next.is_some() {
                self.previous = self.next.take();
            }
            self.next = self.read()?;
            if self.next.is_none() {
                let Some((_, image)) = &self.previous else {
                    return Err(cli_error!(
                        "decode_failure",
                        "",
                        5,
                        "no decoded video frames"
                    ));
                };
                return Ok(image.clone());
            }
        }
    }

    fn read(&mut self) -> Result<Option<(f64, RgbaImage)>> {
        loop {
            let mut decoded = ffmpeg::frame::Video::empty();
            match self.decoder.receive_frame(&mut decoded) {
                Ok(()) => {
                    let Some(pts) = decoded.timestamp() else {
                        return Err(cli_error!(
                            "missing_pts",
                            "",
                            5,
                            "video frame has no presentation timestamp"
                        ));
                    };
                    unsafe {
                        let space = match decoded.color_space() {
                            ffmpeg::color::Space::BT709 => ffmpeg::ffi::SWS_CS_ITU709,
                            ffmpeg::color::Space::BT2020NCL | ffmpeg::color::Space::BT2020CL => {
                                ffmpeg::ffi::SWS_CS_BT2020
                            }
                            _ => ffmpeg::ffi::SWS_CS_ITU601,
                        };
                        let coefficients = ffmpeg::ffi::sws_getCoefficients(space);
                        let full = i32::from(decoded.color_range() == ffmpeg::color::Range::JPEG);
                        let status = ffmpeg::ffi::sws_setColorspaceDetails(
                            self.scaler.as_ptr() as *mut _,
                            coefficients,
                            full,
                            coefficients,
                            1,
                            0,
                            1 << 16,
                            1 << 16,
                        );
                        if status < 0 {
                            return Err(cli_error!(
                                "decode_failure",
                                "",
                                5,
                                "cannot configure source color matrix"
                            ));
                        }
                    }
                    let mut rgba = ffmpeg::frame::Video::empty();
                    cli_try!(
                        self.scaler.run(&decoded, &mut rgba),
                        "decode_failure",
                        "",
                        5
                    );
                    let mut image = RgbaImage::new(rgba.width(), rgba.height());
                    let row = rgba.width() as usize * 4;
                    for y in 0..rgba.height() as usize {
                        image.as_mut()[y * row..(y + 1) * row].copy_from_slice(
                            &rgba.data(0)[y * rgba.stride(0)..y * rgba.stride(0) + row],
                        );
                    }
                    image = match self.rotation.rem_euclid(360) {
                        90 => image::imageops::rotate90(&image),
                        180 => image::imageops::rotate180(&image),
                        270 => image::imageops::rotate270(&image),
                        _ => image,
                    };
                    return Ok(Some((pts as f64 * self.time_base - self.origin, image)));
                }
                Err(ffmpeg::Error::Eof) => return Ok(None),
                Err(error) if again(error) => {}
                Err(error) => return Err(cli_error!("decode_failure", "", 5, "{error}")),
            }
            if self.eof {
                return Ok(None);
            }
            let mut packet = ffmpeg::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) => {
                    if packet.stream() == self.stream {
                        cli_try!(self.decoder.send_packet(&packet), "decode_failure", "", 5);
                    }
                }
                Err(ffmpeg::Error::Eof) => {
                    cli_try!(self.decoder.send_eof(), "decode_failure", "", 5);
                    self.eof = true;
                }
                Err(error) => return Err(cli_error!("decode_failure", "", 5, "{error}")),
            }
        }
    }
}

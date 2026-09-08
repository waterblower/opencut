use std::{
    path::Path,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};
use ffmpeg_next as ffmpeg;

use super::{Message, SeekRequest, State, Status, VideoFrame, audio::AudioWorker, lock, seconds};

/// Every potentially full media queue also services transport and shutdown.
pub struct Control<'a, T = SeekRequest> {
    shared: &'a Arc<Mutex<State>>,
    commands: &'a mpsc::Receiver<T>,
    pub request: Option<T>,
}

impl<'a, T> Control<'a, T> {
    pub fn new(shared: &'a Arc<Mutex<State>>, commands: &'a mpsc::Receiver<T>) -> Self {
        Self {
            shared,
            commands,
            request: None,
        }
    }

    pub fn interrupted(&mut self) -> bool {
        while let Ok(request) = self.commands.try_recv() {
            self.request = Some(request);
        }
        let state = lock(self.shared);
        self.request.is_some() || matches!(state.status, Status::Stopped | Status::Failed(_))
    }
}

pub fn run(
    path: &Path,
    pictures: &mpsc::SyncSender<Message>,
    commands: &mpsc::Receiver<SeekRequest>,
    shared: &Arc<Mutex<State>>,
) -> Result<()> {
    let mut media = Decoder::open(path, shared)?;
    let mut control = Control::new(shared, commands);
    let mut generation = 0;
    let mut displayed = None;
    loop {
        control.interrupted();
        {
            let state = lock(shared);
            if matches!(state.status, Status::Stopped | Status::Failed(_)) {
                return Ok(());
            }
        }
        if let Some(request) = control.request.take() {
            generation = request.generation;
            let started = Instant::now();
            media.seek(request.position)?;
            let demux_elapsed = started.elapsed();
            let duration = lock(shared).duration;
            let Some(Located {
                frame,
                following,
                position,
                draining,
            }) = locate(&mut media, request.position, duration, &mut control)?
            else {
                continue;
            };
            let video_elapsed = started.elapsed();
            if let Some(audio) = &media.audio
                && !audio.seek(position, generation, &mut control)?
            {
                continue;
            }
            eprintln!(
                "seek stages: demux={:.2}ms decode={:.2}ms audio={:.2}ms",
                demux_elapsed.as_secs_f64() * 1000.0,
                (video_elapsed - demux_elapsed).as_secs_f64() * 1000.0,
                (started.elapsed() - video_elapsed).as_secs_f64() * 1000.0,
            );
            displayed = Some(frame.timestamp);
            send(
                pictures,
                Message::Seeked {
                    request,
                    frame,
                    position,
                },
                &mut control,
            )?;
            if let Some(frame) = following {
                send(pictures, Message::Frame { generation, frame }, &mut control)?;
            }
            // Continue from the decoder's existing position, including frames
            // buffered by B-frame reordering. Never decode the same GOP twice.
            receive_video(&mut media, pictures, &mut control, generation, displayed)?;
            if control.interrupted() {
                continue;
            }
            if draining {
                if media.audio.is_none() {
                    send(
                        pictures,
                        Message::End {
                            generation,
                            position: media.video_end,
                        },
                        &mut control,
                    )?;
                }
                media.eof = true;
            }
        }
        if media.eof {
            if let Some(audio) = &media.audio {
                match audio.ends.try_recv() {
                    Ok((epoch, end)) if epoch == generation => {
                        send(
                            pictures,
                            Message::End {
                                generation,
                                position: media.video_end.max(end),
                            },
                            &mut control,
                        )?;
                    }
                    Ok(_) | Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        bail!(
                            "Audio decoder stopped unexpectedly at {}:{}",
                            file!(),
                            line!()
                        )
                    }
                }
            }
            thread::sleep(Duration::from_millis(2));
            continue;
        }
        let mut packet = ffmpeg::Packet::empty();
        match packet.read(&mut media.input) {
            Ok(()) => {
                if packet.stream() == media.track.index {
                    media.video.send_packet(&packet).context(format!(
                        "Sending video packet at {}:{}",
                        file!(),
                        line!()
                    ))?;
                    receive_video(&mut media, pictures, &mut control, generation, displayed)?;
                }
            }
            Err(ffmpeg::Error::Eof) => {
                media.video.send_eof().context(format!(
                    "Draining video decoder at {}:{}",
                    file!(),
                    line!()
                ))?;
                receive_video(&mut media, pictures, &mut control, generation, displayed)?;
                if control.interrupted() {
                    continue;
                }
                if media.video_end.is_zero() {
                    bail!(
                        "Video contains no decodable frames at {}:{}",
                        file!(),
                        line!()
                    );
                }
                if media.audio.is_none() {
                    send(
                        pictures,
                        Message::End {
                            generation,
                            position: media.video_end,
                        },
                        &mut control,
                    )?;
                }
                media.eof = true;
            }
            Err(error) => {
                return Err(anyhow::Error::new(error).context(format!(
                    "Reading media packet at {}:{}",
                    file!(),
                    line!()
                )));
            }
        }
    }
}

struct Track {
    index: usize,
    time_base: f64,
    origin: f64,
    frame_duration: f64,
}

struct Decoder {
    input: ffmpeg::format::context::Input,
    video: ffmpeg::decoder::Video,
    audio: Option<AudioWorker>,
    track: Track,
    next_time: f64,
    video_end: Duration,
    eof: bool,
}

impl Decoder {
    fn open(path: &Path, shared: &Arc<Mutex<State>>) -> Result<Self> {
        // Reject FIFOs/devices: this backend promises local, finite media files.
        let metadata = path.metadata().context(format!(
            "Reading metadata for {} at {}:{}",
            path.display(),
            file!(),
            line!()
        ))?;
        if !metadata.is_file() {
            bail!(
                "Video source must be a regular file at {}:{}",
                file!(),
                line!()
            );
        }
        ffmpeg::init().context(format!("Initializing FFmpeg at {}:{}", file!(), line!()))?;
        let input = ffmpeg::format::input(path).context(format!(
            "Opening {} at {}:{}",
            path.display(),
            file!(),
            line!()
        ))?;
        let Some(stream) = input.streams().best(ffmpeg::media::Type::Video) else {
            bail!(
                "No video track in {} at {}:{}",
                path.display(),
                file!(),
                line!()
            );
        };
        let time_base = f64::from(stream.time_base());
        if !time_base.is_finite() || time_base <= 0.0 {
            bail!("Invalid video time base at {}:{}", file!(), line!());
        }
        let fps = f64::from(stream.avg_frame_rate());
        let origin = if stream.start_time() == ffmpeg::ffi::AV_NOPTS_VALUE {
            0.0
        } else {
            stream.start_time() as f64 * time_base
        };
        let track = Track {
            index: stream.index(),
            time_base,
            origin,
            frame_duration: if fps.is_finite() && fps > 0.0 {
                1.0 / fps
            } else {
                1.0 / 30.0
            },
        };
        let mut duration = if stream.duration() > 0 {
            seconds(stream.duration() as f64 * time_base)
        } else if input.duration() > 0 {
            seconds(input.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64)
        } else {
            Duration::ZERO
        };
        if let Some(audio) = input.streams().best(ffmpeg::media::Type::Audio)
            && audio.duration() > 0
        {
            let base = f64::from(audio.time_base());
            let start = if audio.start_time() == ffmpeg::ffi::AV_NOPTS_VALUE {
                origin
            } else {
                audio.start_time() as f64 * base
            };
            duration = duration.max(seconds(start - origin + audio.duration() as f64 * base));
        }
        let mut context =
            ffmpeg::codec::context::Context::from_parameters(stream.parameters()).context(
                format!("Reading video parameters at {}:{}", file!(), line!()),
            )?;
        context.set_threading(ffmpeg::codec::threading::Config::kind(
            ffmpeg::codec::threading::Type::Frame,
        ));
        let video = context.decoder().video().context(format!(
            "Opening video decoder at {}:{}",
            file!(),
            line!()
        ))?;
        let audio = if input.streams().best(ffmpeg::media::Type::Audio).is_some() {
            Some(AudioWorker::open(path, origin, shared)?)
        } else {
            None
        };
        {
            let mut state = lock(shared);
            state.framerate = if fps.is_finite() && fps > 0.0 {
                Some(fps)
            } else {
                None
            };
            state.duration = duration;
        }
        Ok(Self {
            input,
            video,
            audio,
            track,
            next_time: 0.0,
            video_end: Duration::ZERO,
            eof: false,
        })
    }

    fn seek(&mut self, position: Duration) -> Result<()> {
        let absolute =
            (position.as_secs_f64() + self.track.origin) * ffmpeg::ffi::AV_TIME_BASE as f64;
        if absolute < i64::MIN as f64 || absolute >= i64::MAX as f64 {
            bail!(
                "Seek position exceeds FFmpeg's timestamp range at {}:{}",
                file!(),
                line!()
            );
        }
        let timestamp = absolute as i64;
        // ffmpeg-next passes RangeTo's end directly as FFmpeg's inclusive max_ts.
        if self.input.seek(timestamp, ..timestamp).is_err() {
            let beginning = (self.track.origin * ffmpeg::ffi::AV_TIME_BASE as f64) as i64;
            self.input.seek(beginning, ..beginning).context(format!(
                "Seeking from video origin at {}:{}",
                file!(),
                line!()
            ))?;
        }
        self.video.flush();
        self.next_time = 0.0;
        self.video_end = Duration::ZERO;
        self.eof = false;
        Ok(())
    }
}

fn receive_video(
    media: &mut Decoder,
    pictures: &mpsc::SyncSender<Message>,
    control: &mut Control<'_>,
    generation: u64,
    displayed: Option<Duration>,
) -> Result<()> {
    loop {
        if control.interrupted() {
            return Ok(());
        }
        let Some(frame) = receive_frame(
            &mut media.video,
            &media.track,
            &mut media.next_time,
            &mut media.video_end,
        )?
        else {
            return Ok(());
        };
        if let Some(displayed) = displayed
            && frame.timestamp <= displayed
        {
            continue;
        }
        send(pictures, Message::Frame { generation, frame }, control)?;
    }
}

fn receive_frame(
    decoder: &mut ffmpeg::decoder::Video,
    track: &Track,
    next_time: &mut f64,
    end: &mut Duration,
) -> Result<Option<Arc<VideoFrame>>> {
    let mut image = ffmpeg::frame::Video::empty();
    match decoder.receive_frame(&mut image) {
        Ok(()) => {}
        Err(ffmpeg::Error::Eof) => return Ok(None),
        Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => return Ok(None),
        Err(error) => {
            return Err(anyhow::Error::new(error).context(format!(
                "Decoding video frame at {}:{}",
                file!(),
                line!()
            )));
        }
    }
    let time = match image.timestamp() {
        Some(timestamp) => (timestamp as f64 * track.time_base - track.origin).max(0.0),
        None => *next_time,
    };
    let duration = if image.packet().duration > 0 {
        image.packet().duration as f64 * track.time_base
    } else {
        track.frame_duration
    };
    *next_time = time + duration;
    *end = (*end).max(seconds(*next_time));
    Ok(Some(Arc::new(VideoFrame {
        timestamp: seconds(time),
        image,
    })))
}

struct Located {
    frame: Arc<VideoFrame>,
    following: Option<Arc<VideoFrame>>,
    position: Duration,
    draining: bool,
}

fn locate(
    media: &mut Decoder,
    target: Duration,
    duration: Duration,
    control: &mut Control<'_>,
) -> Result<Option<Located>> {
    let mut best: Option<Arc<VideoFrame>> = None;
    let mut draining = false;
    loop {
        if control.interrupted() {
            return Ok(None);
        }
        if let Some(frame) = receive_frame(
            &mut media.video,
            &media.track,
            &mut media.next_time,
            &mut media.video_end,
        )? {
            if frame.timestamp <= target {
                best = Some(frame);
                continue;
            }
            let (selected, following) = match best {
                Some(selected) => (selected, Some(frame)),
                None => (frame, None),
            };
            let position = target.max(selected.timestamp);
            return Ok(Some(Located {
                frame: selected,
                following,
                position,
                draining,
            }));
        }
        if draining {
            let Some(frame) = best else {
                bail!(
                    "No decodable frame at seek target at {}:{}",
                    file!(),
                    line!()
                );
            };
            let position = if duration.is_zero() || target >= duration {
                frame.timestamp
            } else {
                target.max(frame.timestamp)
            };
            return Ok(Some(Located {
                frame,
                following: None,
                position,
                draining,
            }));
        }
        let mut packet = ffmpeg::Packet::empty();
        match packet.read(&mut media.input) {
            Ok(()) => {
                if packet.stream() == media.track.index {
                    media.video.send_packet(&packet).context(format!(
                        "Sending video packet while seeking at {}:{}",
                        file!(),
                        line!()
                    ))?;
                }
            }
            Err(ffmpeg::Error::Eof) => {
                media.video.send_eof().context(format!(
                    "Draining video while seeking at {}:{}",
                    file!(),
                    line!()
                ))?;
                draining = true;
            }
            Err(error) => {
                return Err(anyhow::Error::new(error).context(format!(
                    "Reading media while seeking at {}:{}",
                    file!(),
                    line!()
                )));
            }
        }
    }
}

fn send(
    pictures: &mpsc::SyncSender<Message>,
    message: Message,
    control: &mut Control<'_>,
) -> Result<()> {
    let mut message = message;
    loop {
        if control.interrupted() {
            return Ok(());
        }
        match pictures.try_send(message) {
            Ok(()) => return Ok(()),
            Err(mpsc::TrySendError::Full(returned)) => {
                message = returned;
                thread::sleep(Duration::from_millis(2));
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                bail!("Video presenter disconnected at {}:{}", file!(), line!())
            }
        }
    }
}

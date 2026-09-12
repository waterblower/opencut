use anyhow::{Context as _, Error, Result, bail};
use std::{
    collections::VecDeque,
    path::Path,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use ffmpeg_next as ffmpeg;

use super::decompression::Decompression;
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

    pub fn superseded(&self, generation: u64) -> bool {
        lock(self.shared).generation != generation
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
    let mut restart = None;
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
            restart = None;
            if let Some(frame) = media.cache.get(request.position) {
                let position = request.position;
                if let Some(audio) = &media.audio
                    && !audio.seek(position, generation, &mut control)?
                {
                    continue;
                }
                displayed = Some(frame.timestamp);
                // The requested pixels and audio reset are complete. A paused
                // preview needs no decoder replay; defer it until playback resumes.
                restart = Some(position);
                send(
                    pictures,
                    Message::Seeked {
                        request,
                        frame,
                        position,
                    },
                    &mut control,
                )?;
                continue;
            }
            let started = Instant::now();
            if !media.can_continue(request.position) {
                media.seek(request.position)?;
            }
            let demux_elapsed = started.elapsed();
            media.video.seek_target(Some(
                ((request.position.as_secs_f64() + media.track.origin) / media.track.time_base)
                    as i64,
            ));
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
            media.video.seek_target(None);
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
        if let Some(position) = restart {
            if lock(shared).clock.paused() {
                thread::sleep(Duration::from_millis(1));
                continue;
            }
            media.seek(position)?;
            restart = None;
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
                media.eof = true;
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
                return Err(Error::new(error).context(format!(
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
    video: Decompression,
    audio: Option<AudioWorker>,
    track: Track,
    next_time: f64,
    video_end: Duration,
    eof: bool,
    cache: FrameCache,
    previous: Option<Arc<VideoFrame>>,
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
        let video = Decompression::open(
            stream.parameters(),
            stream.time_base(),
            input.format().name(),
        )?;
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
            cache: FrameCache::default(),
            previous: None,
        })
    }

    fn can_continue(&self, target: Duration) -> bool {
        let Some(previous) = &self.previous else {
            return false;
        };
        if self.eof || target < previous.timestamp {
            return false;
        }
        let Some(stream) = self.input.stream(self.track.index) else {
            return false;
        };
        let timestamp = ((target.as_secs_f64() + self.track.origin) / self.track.time_base) as i64;
        // SAFETY: the input owns this stream; the returned index entry is read
        // immediately and never retained across a demux operation.
        let entry = unsafe {
            ffmpeg::ffi::avformat_index_get_entry_from_timestamp(
                stream.as_ptr().cast_mut(),
                timestamp,
                ffmpeg::ffi::AVSEEK_FLAG_BACKWARD,
            )
        };
        if entry.is_null() {
            return false;
        }
        let key_time =
            unsafe { (*entry).timestamp } as f64 * self.track.time_base - self.track.origin;
        // Continue only when the existing decoder is closer than a new keyframe
        // seek. This avoids re-decoding the GOP during forward scrubbing.
        seconds(key_time) <= previous.timestamp
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
        self.video.flush()?;
        self.previous = None;
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
        let Some(frame) = receive_frame(media)? else {
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

fn receive_frame(media: &mut Decoder) -> Result<Option<Arc<VideoFrame>>> {
    let Some((image, contiguous)) = media.video.receive()? else {
        return Ok(None);
    };
    let time = match image.timestamp() {
        Some(timestamp) => (timestamp as f64 * media.track.time_base - media.track.origin).max(0.0),
        None => media.next_time,
    };
    let duration = if image.packet().duration > 0 {
        image.packet().duration as f64 * media.track.time_base
    } else {
        media.track.frame_duration
    };
    media.next_time = time + duration;
    media.video_end = media.video_end.max(seconds(media.next_time));
    let frame = Arc::new(VideoFrame {
        timestamp: seconds(time),
        image,
    });
    if let Some(previous) = media.previous.replace(Arc::clone(&frame))
        && contiguous
    {
        media.cache.insert(previous, frame.timestamp);
    }
    Ok(Some(frame))
}

struct Located {
    frame: Arc<VideoFrame>,
    following: Option<Arc<VideoFrame>>,
    position: Duration,
    draining: bool,
}

// Own at most 128 MiB of visible plane storage, not an unbounded decoded video.
// Spans are proven by consecutive presentation timestamps, never guessed from FPS.
const CACHE_BYTES: usize = 128 * 1024 * 1024;

#[derive(Default)]
struct FrameCache {
    spans: VecDeque<(Arc<VideoFrame>, Duration, usize)>,
    bytes: usize,
}

impl FrameCache {
    fn get(&self, position: Duration) -> Option<Arc<VideoFrame>> {
        for (frame, end, _) in self.spans.iter().rev() {
            if frame.timestamp <= position && position < *end {
                return Some(Arc::clone(frame));
            }
        }
        None
    }

    fn insert(&mut self, frame: Arc<VideoFrame>, end: Duration) {
        if end <= frame.timestamp {
            return;
        }
        let bytes = (0..frame.image.planes()).fold(0usize, |total, plane| {
            total.saturating_add(frame.image.data(plane).len())
        });
        if bytes > CACHE_BYTES {
            return;
        }
        // A repeated decode replaces its span rather than retaining duplicates.
        if let Some(index) = self
            .spans
            .iter()
            .position(|(cached, _, _)| cached.timestamp == frame.timestamp)
        {
            let (_, _, removed) = self.spans.remove(index).expect("located cached frame");
            self.bytes -= removed;
        }
        while self.bytes + bytes > CACHE_BYTES {
            let Some((_, _, removed)) = self.spans.pop_front() else {
                break;
            };
            self.bytes -= removed;
        }
        self.bytes += bytes;
        self.spans.push_back((frame, end, bytes));
    }
}

fn locate(
    media: &mut Decoder,
    target: Duration,
    duration: Duration,
    control: &mut Control<'_>,
) -> Result<Option<Located>> {
    let mut best = media.previous.clone();
    let mut draining = false;
    loop {
        if control.interrupted() {
            return Ok(None);
        }
        if let Some(frame) = receive_frame(media)? {
            if frame.timestamp <= target {
                best = Some(frame);
                continue;
            }
            let (selected, following) = match best {
                Some(selected) => (selected, Some(frame)),
                None => (frame, None),
            };
            let position = target.max(selected.timestamp);
            // Preroll may suppress output for dependencies, but the selected
            // frame and first later frame prove this exact covering interval.
            if let Some(following) = &following {
                media
                    .cache
                    .insert(Arc::clone(&selected), following.timestamp);
            }
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
                media.eof = true;
                draining = true;
            }
            Err(error) => {
                return Err(Error::new(error).context(format!(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obsolete_audio_is_detected_before_its_seek_command_arrives() {
        let shared = Arc::new(Mutex::new(State::new()));
        let (_sender, receiver) = mpsc::channel::<()>();
        let mut control = Control::new(&shared, &receiver);
        assert!(!control.superseded(0));
        lock(&shared).generation = 1;
        assert!(!control.interrupted());
        assert!(control.superseded(0));
        assert!(!control.superseded(1));
    }

    #[test]
    fn cache_uses_exact_half_open_intervals_and_evicts_to_its_budget() {
        let mut cache = FrameCache::default();
        for index in 0..48 {
            let frame = Arc::new(VideoFrame {
                timestamp: Duration::from_millis(index * 40),
                image: ffmpeg::frame::Video::new(ffmpeg::format::Pixel::YUV420P, 2048, 1024),
            });
            cache.insert(frame, Duration::from_millis(index * 40 + 40));
            assert!(cache.bytes <= CACHE_BYTES);
        }
        assert!(cache.get(Duration::ZERO).is_none());
        let target = Duration::from_millis(47 * 40);
        let frame = cache.get(target).expect("newest span retained");
        assert_eq!(frame.timestamp, target);
        assert!(cache.get(target + Duration::from_millis(39)).is_some());
        assert!(cache.get(target + Duration::from_millis(40)).is_none());
        let bytes = cache.bytes;
        cache.insert(frame, target + Duration::from_millis(40));
        assert_eq!(cache.bytes, bytes);
        // No inferred coverage across a discontinuity between decoded segments.
        assert!(cache.get(Duration::from_secs(100)).is_none());
    }
}

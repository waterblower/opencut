//! Autonomous local-file playback. Frame reads observe playback; they do not drive it.
//!
//! Opening starts paused with a frame ready. `seek` awaits completion without
//! blocking its executor; `seek_sync` waits for the same operation on its thread.
//! Frames retain FFmpeg's software pixel format, strides, and color metadata.
//! Independent video/audio decoding keeps device backpressure out of the video
//! path. A presentation worker publishes snapshots against their shared clock.
//! Enable the `ffmpeg-backend` Cargo feature to use this module.
//! Enable `ffmpeg-video` for GPUI's `video(&backend)?.id(...).size(...)` element.

use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};
use ffmpeg_next::frame::Video;

mod audio;
mod decoder;

#[cfg(feature = "ffmpeg-video")]
mod video_element;
#[cfg(feature = "ffmpeg-video")]
pub use video_element::{VideoElement, video};

pub struct VideoFrame {
    /// Presentation time relative to the video stream's origin.
    pub timestamp: Duration,
    pub image: Video,
}

pub struct VideoBackend {
    shared: Arc<Mutex<State>>,
    commands: mpsc::Sender<SeekRequest>,
    workers: Vec<JoinHandle<()>>,
}

impl VideoBackend {
    /// Initializes playback on a worker and returns paused with the first frame ready.
    pub async fn open(path: &Path) -> Result<Self> {
        let (backend, ready) = Self::start(path)?;
        ready.recv().await.context(format!(
            "Waiting for video initialization at {}:{}",
            file!(),
            line!()
        ))??;
        Ok(backend)
    }

    /// Blocking counterpart of [`Self::open`].
    pub fn open_sync(path: &Path) -> Result<Self> {
        let (backend, ready) = Self::start(path)?;
        ready.recv_blocking().context(format!(
            "Waiting for video initialization at {}:{}",
            file!(),
            line!()
        ))??;
        Ok(backend)
    }

    pub fn frame_size(&self) -> (u32, u32) {
        let state = lock(&self.shared);
        let frame = state
            .frame
            .as_ref()
            .expect("successful open prepares a frame");
        (frame.image.width(), frame.image.height())
    }

    /// Reported average frame rate; `None` when the metadata is unusable.
    /// An average rate does not describe individual variable-rate frame intervals.
    pub fn framerate(&self) -> Option<f64> {
        lock(&self.shared).framerate
    }

    /// Zero when the file does not declare a duration.
    pub fn duration(&self) -> Duration {
        lock(&self.shared).duration
    }

    pub fn position(&self) -> Duration {
        lock(&self.shared).clock.position(Instant::now())
    }

    pub fn paused(&self) -> bool {
        lock(&self.shared).clock.paused()
    }

    pub fn set_paused(&mut self, paused: bool) -> Result<()> {
        let mut state = lock(&self.shared);
        state.check()?;
        if matches!(state.status, Status::Ended) && !paused {
            return Ok(());
        }
        state.clock.set_paused(paused, Instant::now());
        Ok(())
    }

    /// Completes after the target frame is published and internal audio is reset.
    /// Preserves play/pause state. Dropping the future does not cancel an already
    /// submitted seek; a later seek supersedes it.
    pub async fn seek(&mut self, position: Duration) -> Result<()> {
        let reply = self.request_seek(position)?;
        seek_result(&self.shared, reply.recv().await)
    }

    /// Blocking counterpart of [`Self::seek`].
    pub fn seek_sync(&mut self, position: Duration) -> Result<()> {
        let reply = self.request_seek(position)?;
        seek_result(&self.shared, reply.recv_blocking())
    }

    /// Effective gain: zero while muted.
    pub fn volume(&self) -> f64 {
        let state = lock(&self.shared);
        if state.muted { 0.0 } else { state.volume }
    }

    /// Clamps finite values to 0..=1; rejects NaN and infinity.
    pub fn set_volume(&mut self, volume: f64) -> Result<()> {
        if !volume.is_finite() {
            bail!("Volume must be finite at {}:{}", file!(), line!());
        }
        let mut state = lock(&self.shared);
        state.check()?;
        state.volume = volume.clamp(0.0, 1.0);
        Ok(())
    }

    pub fn muted(&self) -> bool {
        lock(&self.shared).muted
    }

    pub fn set_muted(&mut self, muted: bool) -> Result<()> {
        let mut state = lock(&self.shared);
        state.check()?;
        state.muted = muted;
        Ok(())
    }

    /// Clones a snapshot handle, without consuming or waiting for a frame.
    /// Previously returned snapshots stay valid when playback advances.
    pub fn get_current_frame(&self) -> Result<Arc<VideoFrame>> {
        let state = lock(&self.shared);
        state.check()?;
        let Some(frame) = &state.frame else {
            bail!("Video has no prepared frame at {}:{}", file!(), line!());
        };
        Ok(Arc::clone(frame))
    }

    pub fn ended(&self) -> bool {
        matches!(lock(&self.shared).status, Status::Ended)
    }
}

impl Drop for VideoBackend {
    fn drop(&mut self) {
        lock(&self.shared).status = Status::Stopped;
        for worker in self.workers.drain(..) {
            // Drop cannot return an error; worker errors are exposed by the API.
            let _ = worker.join();
        }
    }
}

impl VideoBackend {
    fn start(path: &Path) -> Result<(Self, async_channel::Receiver<Result<()>>)> {
        let shared = Arc::new(Mutex::new(State::new()));
        let (commands, requests) = mpsc::channel();
        let (pictures, receiver) = mpsc::sync_channel(6);
        let (ready, completion) = async_channel::bounded(1);
        let mut backend = Self {
            shared: Arc::clone(&shared),
            commands,
            workers: Vec::new(),
        };
        let worker_path = path.to_owned();
        let decoding_state = Arc::clone(&shared);
        let decoder = thread::Builder::new()
            .name("video2-decode".into())
            .spawn(move || {
                if let Err(error) =
                    decoder::run(&worker_path, &pictures, &requests, &decoding_state)
                {
                    lock(&decoding_state).fail(&error);
                }
            })
            .context(format!("Starting video decoder at {}:{}", file!(), line!()))?;
        backend.workers.push(decoder);
        let presenter = thread::Builder::new()
            .name("video2-present".into())
            .spawn(move || {
                if let Err(error) = present(&receiver, &shared, &ready) {
                    lock(&shared).fail(&error);
                    let _ = ready.try_send(Err(error));
                }
            })
            .context(format!(
                "Starting video presentation at {}:{}",
                file!(),
                line!()
            ))?;
        backend.workers.push(presenter);
        Ok((backend, completion))
    }

    fn request_seek(&mut self, position: Duration) -> Result<async_channel::Receiver<Result<()>>> {
        let (reply, completion) = async_channel::bounded(1);
        let mut state = lock(&self.shared);
        state.check()?;
        state.generation = state.generation.checked_add(1).context(format!(
            "Seek generation exhausted at {}:{}",
            file!(),
            line!()
        ))?;
        let position = if state.duration.is_zero() {
            position
        } else {
            position.min(state.duration)
        };
        let resume = !state.clock.paused();
        state.clock = Clock::Seeking {
            position: state.clock.position(Instant::now()),
            resume,
        };
        state.status = Status::Active;
        let request = SeekRequest {
            generation: state.generation,
            position,
            reply,
        };
        if self.commands.send(request).is_err() {
            let error = anyhow::anyhow!("Video decoder disconnected at {}:{}", file!(), line!());
            state.fail(&error);
            return Err(error);
        }
        Ok(completion)
    }
}

struct SeekRequest {
    generation: u64,
    position: Duration,
    reply: async_channel::Sender<Result<()>>,
}

enum Message {
    Frame {
        generation: u64,
        frame: Arc<VideoFrame>,
    },
    Seeked {
        request: SeekRequest,
        frame: Arc<VideoFrame>,
        position: Duration,
    },
    End {
        generation: u64,
        position: Duration,
    },
}

enum Status {
    Active,
    Ended,
    Failed(String),
    Stopped,
}

struct State {
    clock: Clock,
    generation: u64,
    frame: Option<Arc<VideoFrame>>,
    framerate: Option<f64>,
    duration: Duration,
    volume: f64,
    muted: bool,
    status: Status,
}

impl State {
    fn new() -> Self {
        Self {
            clock: Clock::Paused(Duration::ZERO),
            generation: 0,
            frame: None,
            framerate: None,
            duration: Duration::ZERO,
            volume: 1.0,
            muted: false,
            status: Status::Active,
        }
    }

    fn check(&self) -> Result<()> {
        if let Status::Failed(error) = &self.status {
            bail!("Video playback failed: {error} at {}:{}", file!(), line!());
        }
        if matches!(self.status, Status::Stopped) {
            bail!("Video playback stopped at {}:{}", file!(), line!());
        }
        Ok(())
    }

    fn fail(&mut self, error: &anyhow::Error) {
        if matches!(self.status, Status::Failed(_) | Status::Stopped) {
            return;
        }
        self.clock = Clock::Paused(self.clock.position(Instant::now()));
        self.status = Status::Failed(format!("{error:#}"));
    }
}

#[derive(Clone, Copy)]
enum Clock {
    Paused(Duration),
    Playing { position: Duration, since: Instant },
    Seeking { position: Duration, resume: bool },
}

impl Clock {
    fn position(self, now: Instant) -> Duration {
        match self {
            Self::Paused(position) | Self::Seeking { position, .. } => position,
            Self::Playing { position, since } => {
                position.saturating_add(now.saturating_duration_since(since))
            }
        }
    }

    fn paused(self) -> bool {
        match self {
            Self::Paused(_) => true,
            Self::Playing { .. } => false,
            Self::Seeking { resume, .. } => !resume,
        }
    }

    fn set_paused(&mut self, paused: bool, now: Instant) {
        if let Self::Seeking { resume, .. } = self {
            *resume = !paused;
            return;
        }
        if self.paused() == paused {
            return;
        }
        let position = self.position(now);
        *self = if paused {
            Self::Paused(position)
        } else {
            Self::Playing {
                position,
                since: now,
            }
        };
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn seconds(value: f64) -> Duration {
    if !value.is_finite() || value <= 0.0 {
        return Duration::ZERO;
    }
    Duration::try_from_secs_f64(value).unwrap_or(Duration::MAX)
}

fn seek_result(
    shared: &Mutex<State>,
    completion: std::result::Result<Result<()>, async_channel::RecvError>,
) -> Result<()> {
    match completion {
        Ok(result) => result,
        Err(error) => {
            // Preserve the decoder's actual failure rather than hiding it behind
            // a disconnected completion channel.
            lock(shared).check()?;
            Err(anyhow::Error::new(error).context(format!(
                "Waiting for video seek at {}:{}",
                file!(),
                line!()
            )))
        }
    }
}

fn present(
    messages: &mpsc::Receiver<Message>,
    shared: &Arc<Mutex<State>>,
    ready: &async_channel::Sender<Result<()>>,
) -> Result<()> {
    let mut pending = None;
    loop {
        {
            let state = lock(shared);
            if matches!(state.status, Status::Stopped) {
                return Ok(());
            }
            state.check()?;
        }
        if pending.is_none() {
            match messages.recv_timeout(Duration::from_millis(2)) {
                Ok(message) => pending = Some(message),
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    lock(shared).check()?;
                    bail!(
                        "Video decoder stopped unexpectedly at {}:{}",
                        file!(),
                        line!()
                    );
                }
            }
        }
        let mut state = lock(shared);
        if matches!(state.status, Status::Stopped) {
            return Ok(());
        }
        state.check()?;
        let Some(message) = pending.as_ref() else {
            continue;
        };
        match message {
            Message::Frame { generation, frame } => {
                if *generation != state.generation {
                    pending = None;
                    continue;
                }
                let initial = state.frame.is_none();
                if initial
                    || matches!(state.clock, Clock::Playing { .. })
                        && frame.timestamp <= state.clock.position(Instant::now())
                {
                    state.frame = Some(Arc::clone(frame));
                    pending = None;
                    if initial {
                        let _ = ready.try_send(Ok(()));
                    }
                    continue;
                }
            }
            Message::Seeked {
                request,
                frame,
                position,
            } => {
                if request.generation == state.generation {
                    let resume = !state.clock.paused();
                    state.frame = Some(Arc::clone(frame));
                    state.clock = Clock::Paused(*position);
                    state.clock.set_paused(!resume, Instant::now());
                    let _ = request.reply.try_send(Ok(()));
                } else {
                    let _ = request.reply.try_send(Err(anyhow::anyhow!(
                        "Seek superseded at {}:{}",
                        file!(),
                        line!()
                    )));
                }
                pending = None;
                continue;
            }
            Message::End {
                generation,
                position,
            } => {
                if *generation != state.generation {
                    pending = None;
                    continue;
                }
                if state.frame.is_none() {
                    bail!(
                        "Video contains no decodable frames at {}:{}",
                        file!(),
                        line!()
                    );
                }
                if matches!(state.clock, Clock::Playing { .. })
                    && state.clock.position(Instant::now()) >= *position
                {
                    state.clock = Clock::Paused(*position);
                    state.status = Status::Ended;
                    pending = None;
                    continue;
                }
            }
        }
        drop(state);
        thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(test)]
mod tests;

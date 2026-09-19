//! Audio-only playback using the same decoder and device output as video playback.

use anyhow::{Context as _, Result, anyhow, bail};
use ffmpeg_next as ffmpeg;
use std::{
    path::Path,
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::video2::{
    Clock, SeekRequest, State, Status, audio::AudioWorker, decoder::Control, lock, seconds,
};

pub struct AudioBackend {
    shared: Arc<Mutex<State>>,
    commands: mpsc::Sender<SeekRequest>,
    worker: Option<JoinHandle<()>>,
}

impl AudioBackend {
    /// Opens a local audio file on a worker. Playback starts paused.
    pub async fn open(path: &Path) -> Result<Self> {
        let shared = Arc::new(Mutex::new(State::new()));
        let (commands, requests) = mpsc::channel();
        let (ready, completion) = async_channel::bounded(1);
        let path = path.to_owned();
        let worker_state = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("audio-playback".into())
            .spawn(move || {
                if let Err(error) = run(&path, &worker_state, &requests, &ready) {
                    lock(&worker_state).fail(&error);
                    let _ = ready.try_send(Err(error));
                }
            })
            .context("Starting audio playback")?;
        let backend = Self {
            shared,
            commands,
            worker: Some(worker),
        };
        completion
            .recv()
            .await
            .context("Waiting for audio initialization")??;
        Ok(backend)
    }

    pub fn position(&self) -> Duration {
        lock(&self.shared).clock.position(Instant::now())
    }

    pub fn duration(&self) -> Duration {
        lock(&self.shared).duration
    }

    pub fn paused(&self) -> bool {
        lock(&self.shared).clock.paused()
    }

    pub fn ended(&self) -> bool {
        matches!(lock(&self.shared).status, Status::Ended)
    }

    pub fn check(&self) -> Result<()> {
        lock(&self.shared).check()
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

    pub fn volume(&self) -> f64 {
        lock(&self.shared).volume
    }

    pub fn set_volume(&mut self, volume: f64) -> Result<()> {
        if !volume.is_finite() {
            bail!("Volume must be finite");
        }
        let mut state = lock(&self.shared);
        state.check()?;
        state.volume = volume.clamp(0.0, 1.0);
        Ok(())
    }

    /// Submits immediately. Completion means the decoder and audio queue have
    /// switched to the requested position; it does not wait for audible output.
    pub fn seek(
        &mut self,
        position: Duration,
    ) -> impl Future<Output = Result<()>> + Send + 'static {
        let completion = self.request_seek(position);
        let shared = Arc::clone(&self.shared);
        async move {
            match completion?.recv().await {
                Ok(result) => result,
                Err(error) => {
                    lock(&shared).check()?;
                    Err(error).context("Waiting for audio seek")
                }
            }
        }
    }
}

impl Drop for AudioBackend {
    fn drop(&mut self) {
        lock(&self.shared).status = Status::Stopped;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl AudioBackend {
    fn request_seek(&mut self, position: Duration) -> Result<async_channel::Receiver<Result<()>>> {
        let (reply, completion) = async_channel::bounded(1);
        let mut state = lock(&self.shared);
        state.check()?;
        let position = if state.duration.is_zero() {
            position
        } else {
            position.min(state.duration)
        };
        state.generation = state
            .generation
            .checked_add(1)
            .context("Audio seek generation exhausted")?;
        state.clock = Clock::Seeking {
            position: state.clock.position(Instant::now()),
            resume: !state.clock.paused(),
        };
        state.status = Status::Active;
        if self
            .commands
            .send(SeekRequest {
                generation: state.generation,
                position,
                reply,
            })
            .is_err()
        {
            let error = anyhow!("Audio playback worker disconnected");
            state.fail(&error);
            return Err(error);
        }
        Ok(completion)
    }
}

fn run(
    path: &Path,
    shared: &Arc<Mutex<State>>,
    requests: &mpsc::Receiver<SeekRequest>,
    ready: &async_channel::Sender<Result<()>>,
) -> Result<()> {
    if !path
        .metadata()
        .with_context(|| format!("Reading {}", path.display()))?
        .is_file()
    {
        bail!("Audio source must be a regular file: {}", path.display());
    }
    ffmpeg::init().context("Initializing FFmpeg")?;
    let input =
        ffmpeg::format::input(path).with_context(|| format!("Opening {}", path.display()))?;
    let track = input
        .streams()
        .best(ffmpeg::media::Type::Audio)
        .context("File has no audio stream")?;
    let time_base = f64::from(track.time_base());
    let origin = if track.start_time() == ffmpeg::ffi::AV_NOPTS_VALUE {
        0.0
    } else {
        track.start_time() as f64 * time_base
    };
    let duration = if track.duration() > 0 {
        seconds(track.duration() as f64 * time_base)
    } else {
        seconds(input.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64)
    };
    lock(shared).duration = duration;
    drop(input);
    let audio = AudioWorker::open(path, origin, shared)?;
    let _ = ready.try_send(Ok(()));
    let mut control = Control::new(shared, requests);
    let mut end = None;
    loop {
        control.interrupted();
        {
            let state = lock(shared);
            if matches!(state.status, Status::Stopped) {
                return Ok(());
            }
            state.check()?;
        }
        if let Some(request) = control.request.take() {
            end = None;
            if !audio.seek(request.position, request.generation, &mut control)? {
                let _ = request
                    .reply
                    .try_send(Err(anyhow!("Audio seek superseded")));
                continue;
            }
            let mut state = lock(shared);
            state.check()?;
            if state.generation != request.generation {
                let _ = request
                    .reply
                    .try_send(Err(anyhow!("Audio seek superseded")));
                continue;
            }
            let paused = state.clock.paused();
            state.clock = Clock::Paused(request.position);
            state.clock.set_paused(paused, Instant::now());
            let _ = request.reply.try_send(Ok(()));
        }
        while let Ok(message) = audio.ends.try_recv() {
            end = Some(message);
        }
        if let Some((generation, position)) = end {
            let mut state = lock(shared);
            if generation != state.generation {
                end = None;
            } else if !state.clock.paused() && state.clock.position(Instant::now()) >= position {
                state.clock = Clock::Paused(position);
                state.status = Status::Ended;
                end = None;
            }
        }
        thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(test)]
#[path = "tests/audio_backend.test.rs"]
mod tests;

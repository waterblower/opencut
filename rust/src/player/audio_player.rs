use crate::audio_output::AudioOutput;
use anyhow::Result;
use gpui::{AsyncApp, Context, FocusHandle, WeakEntity, Window, actions};
use opencut_player::video3::{AudioBackend, AudioSamples};
use std::{
    future::poll_fn,
    path::PathBuf,
    task::{Poll, Waker},
    time::{Duration, Instant},
};

pub struct AudioPlayer {
    pub audio_backend: AudioBackend,
    audio_output: AudioOutput,
    pub playback_state: PlaybackState,
    play_waker: Option<Waker>,
    pub position: Duration,
    pub title: String,
    pub focus: FocusHandle,
}

impl AudioPlayer {
    pub fn new(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Result<Self> {
        let mut audio_backend = AudioBackend::open(&path)?;
        let audio_output = AudioOutput::open()?;
        audio_backend.audio.configure_output(&audio_output.format)?;
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let player = Self {
            audio_backend,
            audio_output,
            playback_state: PlaybackState::Playing,
            play_waker: None,
            position: Duration::ZERO,
            title: path.display().to_string(),
            focus,
        };
        cx.spawn(async move |player, cx| {
            if let Err(error) = run_playback(player, cx).await {
                eprintln!("Audio player failed: {error:?}");
                std::process::exit(1);
            }
        })
        .detach();
        Ok(player)
    }
}

actions!(opencut, [ToggleAudio]);

pub enum PlaybackState {
    Playing,
    Paused,
    Ended,
}

impl AudioPlayer {
    pub fn seek(&mut self, position: Duration, cx: &mut Context<Self>) -> Result<()> {
        let position = position.min(self.audio_backend.metadata.duration);
        self.audio_output.clear()?;
        self.audio_backend.audio.seek(position)?;
        self.position = position;
        if matches!(self.playback_state, PlaybackState::Ended) {
            self.playback_state = PlaybackState::Paused;
        }
        cx.notify();
        Ok(())
    }

    pub fn toggle_playback(&mut self, cx: &mut Context<Self>) -> Result<()> {
        if matches!(self.playback_state, PlaybackState::Ended) {
            self.seek(Duration::ZERO, cx)?;
        }
        let playing = !matches!(self.playback_state, PlaybackState::Playing);
        self.audio_output.set_playing(playing)?;
        self.playback_state = if playing {
            PlaybackState::Playing
        } else {
            PlaybackState::Paused
        };
        if playing && let Some(waker) = self.play_waker.take() {
            waker.wake();
        }
        cx.notify();
        Ok(())
    }
}

impl AudioPlayer {
    fn get_next_samples(&mut self, cx: &mut Context<Self>) -> Result<Option<AudioSamples>> {
        let samples = match self.audio_backend.audio.next_samples() {
            // A decoded PCM block is available; update its position below.
            Ok(Some(samples)) => samples,
            // Decoder EOF: the playback loop must still wait for output to drain.
            Ok(None) => return Ok(None),
            // Reading or decoding failed; propagate the error to the playback task.
            Err(error) => return Err(error),
        };
        self.position = Duration::from_micros(samples.timestamp.0.max(0) as u64)
            .min(self.audio_backend.metadata.duration);
        cx.notify();
        Ok(Some(samples))
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        if let Some(waker) = self.play_waker.take() {
            waker.wake();
        }
    }
}

trait WaitUntilPlaying {
    async fn wait_until_playing(&self, cx: &mut AsyncApp) -> Result<()>;
}

impl WaitUntilPlaying for WeakEntity<AudioPlayer> {
    async fn wait_until_playing(&self, cx: &mut AsyncApp) -> Result<()> {
        poll_fn(|task_cx| {
            self.update(cx, |player, _| {
                if matches!(player.playback_state, PlaybackState::Playing) {
                    player.play_waker = None;
                    Poll::Ready(Ok(()))
                } else {
                    player.play_waker = Some(task_cx.waker().clone());
                    Poll::Pending
                }
            })?
        })
        .await
    }
}

async fn run_playback(player: WeakEntity<AudioPlayer>, cx: &mut AsyncApp) -> Result<()> {
    let bge = cx.background_executor().clone();
    loop {
        player.wait_until_playing(cx).await?;
        let time_to_wait = player.update(cx, |player, cx| -> Result<Duration> {
            let cycle_start = Instant::now();
            // Some(samples) continues to submission below. None means decoding
            // has ended, but previously submitted audio may still be playing.
            let Some(samples) = player.get_next_samples(cx)? else {
                let remaining = player.audio_output.remaining_duration()?;
                if !remaining.is_zero() {
                    // Stay Playing. Return from this update so the outer timer
                    // waits for the remaining tail, then the next cycle rechecks it.
                    return Ok(remaining);
                }
                // Both the software queue and device tail have drained. Stop
                // output and show the completed position in the UI.
                player.audio_output.set_playing(false)?;
                player.playback_state = PlaybackState::Ended;
                player.position = player.audio_backend.metadata.duration;
                cx.notify();
                // This ends only the current update, not the playback task.
                // The next cycle waits at the play gate until the user replays.
                return Ok(Duration::ZERO);
            };
            player.audio_output.push_samples(samples)?;
            let time_to_wait = player
                .audio_output
                .compute_time_to_wait(cycle_start.elapsed())?;
            Ok(time_to_wait)
        })??;
        // Recheck the decoder and output after waiting so pause and seek still
        // apply during the tail. Once output drains, the play gate waits for replay.
        bge.timer(time_to_wait).await;
    }
}

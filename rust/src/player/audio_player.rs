use crate::audio_output::AudioOutput;
use anyhow::Result;
use gpui::{
    AsyncApp, ClickEvent, Context, FocusHandle, Render, WeakEntity, Window, actions, div,
    prelude::*, px, relative, rgb,
};
use opencut_player::video3::{AudioBackend, AudioSamples};
use std::{
    future::poll_fn,
    path::PathBuf,
    task::{Poll, Waker},
    time::{Duration, Instant},
};

pub struct AudioPlayer {
    audio_backend: AudioBackend,
    audio_output: AudioOutput,
    playback_state: PlaybackState,
    play_waker: Option<Waker>,
    position: Duration,
    title: String,
    focus: FocusHandle,
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

impl Render for AudioPlayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = f32::from(window.viewport_size().width).max(1.0);
        let duration = self.audio_backend.metadata.duration;
        let position = self.position;
        let label = match self.playback_state {
            PlaybackState::Playing => "Pause",
            PlaybackState::Paused => "Play",
            PlaybackState::Ended => "Replay",
        };
        let progress = if duration.is_zero() {
            0.0
        } else {
            (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
        };
        let detail = format!(
            "{}:{:02} / {}:{:02}",
            position.as_secs() / 60,
            position.as_secs() % 60,
            duration.as_secs() / 60,
            duration.as_secs() % 60
        );
        div()
            .id("audio-player")
            .track_focus(&self.focus)
            .on_action(cx.listener(|player, _: &ToggleAudio, _, cx| {
                if let Err(error) = player.toggle_playback(cx) {
                    eprintln!("Toggling audio playback failed: {error:?}");
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x080808))
            .text_color(rgb(0xeeeeee))
            .child(
                div()
                    .flex_1()
                    .p_4()
                    .child(self.title.clone())
                    .child(div().mt_4().child(detail)),
            )
            .child(
                div()
                    .id("audio-seek")
                    .w_full()
                    .h(px(20.0))
                    .bg(rgb(0x303030))
                    .cursor_pointer()
                    .on_click(cx.listener(move |player, event: &ClickEvent, window, cx| {
                        let fraction = (f32::from(event.position().x) / width).clamp(0.0, 1.0);
                        if let Err(error) = player.seek(duration.mul_f64(f64::from(fraction)), cx) {
                            eprintln!("Seeking audio failed: {error:?}");
                        }
                        player.focus.focus(window, cx);
                    }))
                    .child(div().h_full().w(relative(progress)).bg(rgb(0xdba34b))),
            )
            .child(
                div()
                    .id("audio-play-pause")
                    .p_4()
                    .cursor_pointer()
                    .child(label)
                    .on_click(cx.listener(|player, _, _, cx| {
                        if let Err(error) = player.toggle_playback(cx) {
                            eprintln!("Toggling audio playback failed: {error:?}");
                        }
                    })),
            )
    }
}

actions!(opencut, [ToggleAudio]);

enum PlaybackState {
    Playing,
    Paused,
    Ended,
}

impl AudioPlayer {
    fn get_next_samples(&mut self, cx: &mut Context<Self>) -> Result<Option<AudioSamples>> {
        let samples = match self.audio_backend.audio.next_samples() {
            // A decoded PCM block is available; update its position below.
            Ok(Some(samples)) => samples,
            // EOF: the decoder and resampler have no remaining samples.
            Ok(None) => {
                self.playback_state = PlaybackState::Ended;
                self.position = self.audio_backend.metadata.duration;
                cx.notify();
                return Ok(None);
            }
            // Reading or decoding failed; propagate the error to the playback task.
            Err(error) => return Err(error),
        };
        self.position = Duration::from_micros(samples.timestamp.0.max(0) as u64)
            .min(self.audio_backend.metadata.duration);
        cx.notify();
        Ok(Some(samples))
    }

    fn seek(&mut self, position: Duration, cx: &mut Context<Self>) -> Result<()> {
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

    fn toggle_playback(&mut self, cx: &mut Context<Self>) -> Result<()> {
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
            let Some(samples) = player.get_next_samples(cx)? else {
                return Ok(Duration::ZERO);
            };
            player.audio_output.push_samples(samples)?;
            let time_to_wait = player
                .audio_output
                .compute_time_to_wait(cycle_start.elapsed())?;
            Ok(time_to_wait)
        })??;
        // At EOF, the next iteration waits at the play gate until restarted.
        bge.timer(time_to_wait).await;
    }
}

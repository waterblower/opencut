use crate::audio_output::{AudioOutput, OutputEvent, validate_samples};
use anyhow::{Context as _, Result, bail};
use gpui::{
    ClickEvent, Context, FocusHandle, Render, Window, actions, div, prelude::*, px, relative, rgb,
};
use opencut_player::video3::AudioBackend;
use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, Sender, SyncSender, TryRecvError, TrySendError, channel, sync_channel},
    time::{Duration, Instant},
};

pub struct AudioPlayer {
    commands: Sender<Command>,
    status: Status,
    title: String,
    focus: FocusHandle,
}

impl AudioPlayer {
    pub fn new(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (commands, receiver) = channel();
        let (updates, statuses) = sync_channel(8);
        let title = path.display().to_string();
        std::thread::spawn(move || {
            if let Err(error) = run_audio(path, receiver, &updates) {
                eprintln!("Audio player failed: {error:?}");
                let _ = updates.send(Status::Failed(format!("{error:#}")));
            }
        });
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        // let bge = cx.background_executor().clone();
        cx.spawn(async move |player, cx| {
            loop {
                // old code:
                // cx.background_executor()
                //     .timer(Duration::from_millis(30))
                //     .await;
                // let mut latest = None;
                // while let Ok(status) = statuses.try_recv() {
                //     latest = Some(status);
                // }
                // if player
                //     .update(cx, |player, cx| {
                //         if let Some(status) = latest {
                //             player.status = status;
                //             cx.notify();
                //         }
                //     })
                //     .is_err()
                // {
                //     break;
                // }
                //
                // new design should be as close to video player control structure
                // as possible
                // player.wait_until_playing(cx).await?;
                // // Return the time to wait, or propagate a playback error.
                // let res = player.update(cx, |player, cx| -> Result<Duration> {
                //     let cycle_start = Instant::now();
                //     let samples = player.get_next_samples(cx)?;
                //     push_samples_to_output_device(samples)?
                //     let time_to_wait = compute_time_to_wait(...)?;
                //     Ok(time_to_wait)
                // })??;
                // // At EOF, the next iteration waits at the play gate until restarted.
                // bge.timer(res).await;
            }
        })
        .detach();
        Self {
            commands,
            status: Status::Loading,
            title,
            focus,
        }
    }
}

impl Render for AudioPlayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = f32::from(window.viewport_size().width).max(1.0);
        let (position, duration, label) = match &self.status {
            Status::Ready {
                position,
                duration,
                state,
            } => (
                *position,
                *duration,
                match state {
                    PlaybackState::Playing => "Pause",
                    PlaybackState::Paused => "Play",
                    PlaybackState::Ended => "Replay",
                },
            ),
            _ => (Duration::ZERO, Duration::ZERO, "Play"),
        };
        let progress = if duration.is_zero() {
            0.0
        } else {
            (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
        };
        let detail = match &self.status {
            Status::Loading => "Opening audio…".to_owned(),
            Status::Failed(error) => error.clone(),
            Status::Ready { .. } => format!(
                "{}:{:02} / {}:{:02}",
                position.as_secs() / 60,
                position.as_secs() % 60,
                duration.as_secs() / 60,
                duration.as_secs() % 60
            ),
        };
        div()
            .id("audio-player")
            .track_focus(&self.focus)
            .on_action(cx.listener(|player, _: &ToggleAudio, _, _| {
                let _ = player.commands.send(Command::Toggle);
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
                    .on_click(cx.listener(move |player, event: &ClickEvent, _, _| {
                        let fraction = (f32::from(event.position().x) / width).clamp(0.0, 1.0);
                        let _ = player
                            .commands
                            .send(Command::Seek(duration.mul_f64(f64::from(fraction))));
                    }))
                    .child(div().h_full().w(relative(progress)).bg(rgb(0xdba34b))),
            )
            .child(
                div()
                    .id("audio-play-pause")
                    .p_4()
                    .cursor_pointer()
                    .child(label)
                    .on_click(cx.listener(|player, _, _, _| {
                        let _ = player.commands.send(Command::Toggle);
                    })),
            )
    }
}

actions!(opencut, [ToggleAudio]);

enum Command {
    Toggle,
    Seek(Duration),
}
#[derive(Clone, Copy)]
enum PlaybackState {
    Playing,
    Paused,
    Ended,
}
enum Status {
    Loading,
    Ready {
        position: Duration,
        duration: Duration,
        state: PlaybackState,
    },
    Failed(String),
}

// This worker owns the synchronous backend. The device callback only copies PCM;
// all file I/O, decoding, seeking and stream lifecycle operations run here.
fn run_audio(
    path: PathBuf,
    commands: Receiver<Command>,
    updates: &SyncSender<Status>,
) -> Result<()> {
    let mut backend = AudioBackend::open(&path)?;
    let duration = backend.metadata.duration;
    let mut output = AudioOutput::open()?;
    backend.audio.configure_output(&output.format)?;
    eprintln!(
        "Audio output: {} Hz, {} channels",
        output.format.sample_rate,
        output.format.channel_layout.len()
    );
    let mut state = PlaybackState::Playing;
    let mut position = Duration::ZERO;
    let mut pending = None;
    let mut eof = false;
    let mut tail: Option<Duration> = None;
    let mut started = false;
    let mut tick = Instant::now();
    loop {
        let elapsed = tick.elapsed();
        tick = Instant::now();
        if matches!(state, PlaybackState::Playing) {
            if let Some(remaining) = tail {
                if elapsed >= remaining {
                    output.set_playing(false)?;
                    state = PlaybackState::Ended;
                    tail = None;
                    eprintln!("Audio playback ended at {} µs", position.as_micros());
                } else {
                    tail = Some(remaining - elapsed);
                }
            }
        }
        let mut seek = None;
        loop {
            match commands.try_recv() {
                Ok(Command::Seek(target)) => {
                    seek = Some(target.min(duration));
                    if matches!(state, PlaybackState::Ended) {
                        state = PlaybackState::Paused;
                    }
                }
                Ok(Command::Toggle) => {
                    state = match state {
                        PlaybackState::Playing => PlaybackState::Paused,
                        PlaybackState::Paused => PlaybackState::Playing,
                        PlaybackState::Ended => {
                            seek = Some(Duration::ZERO);
                            PlaybackState::Playing
                        }
                    };
                    if started {
                        output.set_playing(matches!(state, PlaybackState::Playing))?;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        if let Some(target) = seek {
            // Dropping the old stream discards its queued and partially read PCM.
            // A fresh channel prevents pre-seek samples or events crossing the seek.
            drop(output);
            backend.audio.seek(target)?;
            output = AudioOutput::open()?;
            backend.audio.configure_output(&output.format)?;
            pending = None;
            eof = false;
            tail = None;
            started = false;
            position = target;
        }
        if let Ok(error) = output.errors.try_recv() {
            return Err(error).context("audio output failed");
        }
        while let Ok(event) = output.events.try_recv() {
            match event {
                OutputEvent::Position(value) => position = value.min(duration),
                OutputEvent::Finished(delay) => tail = Some(delay),
            }
        }
        if !eof && matches!(state, PlaybackState::Playing) {
            if pending.is_none() {
                let block = backend.audio.next_samples()?;
                if let Some(block) = &block {
                    validate_samples(block, &output.format)?;
                }
                pending = Some(block);
            }
            if let Some(block) = pending.take() {
                let last = block.is_none();
                match output.samples.try_send(block) {
                    Ok(()) => {
                        eof = last;
                        if !started {
                            output.set_playing(true)?;
                            started = true;
                        }
                    }
                    Err(TrySendError::Full(block)) => pending = Some(block),
                    Err(TrySendError::Disconnected(_)) => bail!("audio output disconnected"),
                }
            }
        }
        let _ = updates.try_send(Status::Ready {
            position,
            duration,
            state,
        });
        std::thread::sleep(Duration::from_millis(2));
    }
}

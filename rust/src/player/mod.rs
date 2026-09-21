use crate::player::session::{
    PlaybackSnapshot, PlaybackState, PreparedFrame, SessionCommand, SessionHandle, SessionUpdate,
};
use gpui::{App, Context, FocusHandle, KeyBinding, Window, actions};
use opencut_player::video3::MediaInfo;
use std::path::PathBuf;

mod lanes;
mod prepare;
mod session;
mod view;

pub struct Player {
    session: SessionHandle,
    metadata: Option<MediaInfo>,
    snapshot: PlaybackSnapshot,
    displayed: Option<PreparedFrame>,
    title: String,
    focus_handle: FocusHandle,
}

impl Player {
    pub fn new(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let title = path.display().to_string();

        let (session, updates) = SessionHandle::open(path, &cx.to_async());
        cx.spawn(async move |player, cx| {
            while let Ok(update) = updates.recv().await {
                let result = player.update(cx, |player, cx| {
                    // There is one source for this window. Metadata is valid even
                    // if a playback command changed the request while opening.
                    match update.value {
                        SessionUpdate::Opened(metadata) => player.metadata = Some(metadata),
                        SessionUpdate::Display(frame)
                            if player.session.current(update.revision) =>
                        {
                            player.displayed = Some(frame);
                        }
                        SessionUpdate::Snapshot(snapshot)
                            if player.session.current(update.revision) =>
                        {
                            player.snapshot = snapshot;
                        }
                        _ => return,
                    }
                    cx.notify();
                });
                if result.is_err() {
                    break;
                }
            }
        })
        .detach();

        Self {
            session,
            metadata: None,
            snapshot: PlaybackSnapshot::default(),
            displayed: None,
            title,
            focus_handle: {
                let focus_handle = cx.focus_handle();
                focus_handle.focus(window, cx);
                focus_handle
            },
        }
    }
}

impl Player {
    fn playing(&self) -> bool {
        matches!(
            self.snapshot.state,
            PlaybackState::Loading | PlaybackState::Preparing | PlaybackState::Playing
        )
    }

    fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        let playing = !self.playing();
        let command = if playing && matches!(self.snapshot.state, PlaybackState::Ended) {
            SessionCommand::Seek {
                position: std::time::Duration::ZERO,
                resume: true,
            }
        } else {
            SessionCommand::SetPlaying(playing)
        };
        if let Err(error) = self.session.command(command) {
            eprintln!("Could not toggle playback: {error:?}");
            return;
        }
        self.snapshot.state = if playing {
            PlaybackState::Preparing
        } else {
            PlaybackState::Paused
        };
        cx.notify();
    }

    fn step(&mut self, direction: i8, cx: &mut Context<Self>) {
        if let Err(error) = self.session.command(SessionCommand::Step { direction }) {
            eprintln!("Could not step video: {error:?}");
            return;
        }
        self.snapshot.state = PlaybackState::Paused;
        cx.notify();
    }
}

actions!(opencut, [TogglePlayback, StepBackward, StepForward]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("space", TogglePlayback, None),
        KeyBinding::new("left", StepBackward, None),
        KeyBinding::new("right", StepForward, None),
    ]);
}

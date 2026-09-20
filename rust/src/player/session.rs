//! Application contracts for V6; the current UI remains connected until V7.
//!
//! Open on background lanes, returning metadata and handles only. Start audio
//! refill and video preparation concurrently. The controller serializes controls,
//! advances revisions, and cancels stale publications/enqueues. Each lane owns its
//! native resources until active work returns; never join/drop them on GPUI.
//!
//! Keep one video request and one prepared lookahead. Coalesce pending seeks in
//! the controller. Audio queue bounds are duration-based, including split blocks.

#![allow(dead_code, reason = "V1 contracts are connected in V6/V7")]

use anyhow::Result;
use async_channel::{Receiver, Sender};
use gpui::RenderImage;
use opencut_player::video3::{AudioSamples, DecodeDiagnostics, MediaInfo, MediaTime, PcmFormat};
use std::{path::PathBuf, sync::Arc, time::Duration};

pub const AUDIO_QUEUE_LIMIT: Duration = Duration::from_millis(200);
pub const AUDIO_PRIME_TARGET: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revision {
    pub session: u64,
    pub request: u64,
}

pub struct Stamped<T> {
    pub revision: Revision,
    pub value: T,
}

pub struct PreparedFrame {
    /// BGRA prepared off the foreground thread for ordinary GPUI image rendering.
    pub image: Arc<RenderImage>,
    pub timestamp: MediaTime,
    pub diagnostics: DecodeDiagnostics,
}

pub enum PlaybackState {
    Loading,
    Paused,
    Priming,
    Playing,
    Ended,
    Failed(String),
}

pub struct PlaybackSnapshot {
    pub state: PlaybackState,
    pub position: MediaTime,
    pub volume: f32,
    pub muted: bool,
}

pub enum SessionCommand {
    SetPlaying(bool),
    Seek { position: Duration, resume: bool },
    Step { direction: i8 },
    SetGain { volume: f32, muted: bool },
    Close,
}

pub enum SessionUpdate {
    Opened(MediaInfo),
    Display(PreparedFrame),
    Snapshot(PlaybackSnapshot),
}

/// Entity owns this handle, the latest displayed frame, and a control snapshot.
/// Dropping the handle cancels/retire lanes asynchronously in V6.
pub struct SessionHandle {
    commands: Sender<SessionCommand>,
}

impl SessionHandle {
    /// Implementation uses GPUI execution, with no second runtime.
    pub async fn open(
        _path: PathBuf,
        _session: u64,
        _cx: &mut gpui::AsyncApp,
    ) -> Result<(Self, Receiver<Stamped<SessionUpdate>>)> {
        todo!("V6: probe/open background lanes, then start concurrent tasks")
    }

    /// Submit without waiting on native work. V6 coalesces pending seek targets
    /// and reports a closed session at the application boundary.
    pub fn command(&self, _command: SessionCommand) -> Result<()> {
        todo!("V6: submit controls with bounded/coalesced pending work")
    }
}

// Native decoders/converter never appear in request or reply messages.
// Channels are bounded; one reply channel per active operation, capacity one.
struct VideoLane {
    requests: Sender<Stamped<VideoRequest>>,
}

enum VideoRequest {
    Next {
        /// Discard stale native candidates before conversion where possible.
        discard_before: MediaTime,
        reply: Sender<Result<Stamped<Option<PreparedFrame>>>>,
    },
    Seek {
        position: Duration,
        reply: Sender<Result<Stamped<PreparedFrame>>>,
    },
}

struct AudioLane {
    requests: Sender<Stamped<AudioRequest>>,
}

enum AudioRequest {
    Next {
        reply: Sender<Result<Stamped<Option<AudioSamples>>>>,
    },
    Seek {
        position: Duration,
        reply: Sender<Result<Revision>>,
    },
}

/// CPAL callback consumes prepared PCM only; it never awaits or calls the UI.
/// Reject stale revisions at enqueue AND consumption. Gaps are media silence;
/// starvation freezes time. Device errors stop the session.
struct AudioOutput;

impl AudioOutput {
    fn format(&self) -> PcmFormat {
        todo!("V5: selected device format")
    }

    async fn enqueue(&self, _samples: Stamped<AudioSamples>) -> Result<()> {
        todo!("V5: split blocks and apply duration-based backpressure")
    }

    fn played_position(&self) -> Option<MediaTime> {
        todo!("V5: consumed samples with device presentation latency counted once")
    }

    async fn next_progress(&self) -> Result<AudioProgress> {
        todo!("V5: asynchronous callback progress without polling")
    }

    async fn reset(&self, _revision: Revision, _position: Duration) -> Result<()> {
        todo!("V5: suspend and invalidate application PCM; device latency persists")
    }

    fn set_playing(&self, _playing: bool) -> Result<()> {
        todo!("V5: pause freezes media time; resume follows priming")
    }

    fn set_gain(&self, _volume: f32, _muted: bool) {
        todo!("V5: callback gain; mute preserves clock advancement")
    }

    async fn finish(&self, _revision: Revision) -> Result<()> {
        todo!("V5: mark decoded EOF and notify once submitted audio drains")
    }
}

enum AudioProgress {
    Advanced(Stamped<MediaTime>),
    Underrun(Revision),
    Drained(Stamped<MediaTime>),
}

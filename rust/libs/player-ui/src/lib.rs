//! Player entities with their GPUI views and playback controls.

mod audio_output;
pub mod audio_player;
mod audio_player_view;
mod seek_bar;
pub mod timeline_player;
pub mod timeline_player_view;
pub mod video_player;
mod video_player_view;

use anyhow::Result;
use std::time::Duration;

pub trait Seeker {
    /// Moves playback to `position`. Does not notify; the caller does.
    fn seek(&mut self, position: Duration) -> Result<()>;
}

trait WaitUntilPlaying {
    /// Waits for Playing without decoding, changing clocks, or performing device I/O.
    async fn wait_until_playing(&self, cx: &mut gpui::AsyncApp);
}

pub(crate) fn format_time(time: Duration) -> String {
    let seconds = time.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

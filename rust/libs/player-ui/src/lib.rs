//! Player entities with their GPUI views and playback controls.

mod audio_output;
pub mod audio_player;
mod audio_player_view;
mod gpu;
pub mod timeline_player;
pub mod timeline_player_view;
pub mod video_player;
mod video_player_view;

trait WaitUntilPlaying {
    /// Waits for Playing without decoding, changing clocks, or performing device I/O.
    async fn wait_until_playing(&self, cx: &mut gpui::AsyncApp);
}

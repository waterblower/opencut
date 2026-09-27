//! Timeline file persistence and shared value types; no runtime-only state.
mod serialization;
pub use serialization::{ParseError, TimelineSerialization, parse};
mod asset;
mod clip;
mod editing_state;
mod time;
mod track;
pub use asset::*;
pub use clip::*;
pub use editing_state::TimelineEditingState;
pub use time::*;
pub use track::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimelineSettings {
    pub frame_rate: FrameRate,
    pub width: u32,
    pub height: u32,
    pub audio_sample_rate: u32,
}

impl Default for TimelineSettings {
    fn default() -> Self {
        Self {
            frame_rate: FrameRate::default(),
            width: 1920,
            height: 1080,
            audio_sample_rate: 48_000,
        }
    }
}

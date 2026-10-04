//! Synchronous native decoding. Callers own execution, scheduling, and presentation.
//!
//! Returns native frames and PCM; the application prepares images for rendering.

mod audio;
mod audio_backend;
mod hardware;
mod media_info;
mod time;
mod video;
mod video_backend;

pub use audio::{AudioDecoder, AudioSamples, PcmFormat};
pub use audio_backend::{AudioBackend, AudioMediaInfo};
pub use media_info::{AudioInfo, MediaInfo, VideoInfo};
pub use time::MediaTime;
pub use video::{DecodeMode, VideoDecoder, VideoFrame};
pub use video_backend::VideoBackend;

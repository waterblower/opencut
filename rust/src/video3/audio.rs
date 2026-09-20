use crate::video3::{MediaInfo, MediaTime};
use anyhow::Result;
use ffmpeg_next::ffi::AVChannel;
use std::{marker::PhantomData, path::Path, rc::Rc, time::Duration};

/// Owned channel positions in interleaving order, with no native layout pointers.
/// PCM is interleaved f32; the application adapts it to the device sample type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcmFormat {
    pub sample_rate: u32,
    pub channel_layout: Vec<AVChannel>,
}

pub struct AudioSamples {
    pub samples: Vec<f32>,
    pub timestamp: MediaTime,
    pub format: PcmFormat,
    /// Per-channel sample frames, not interleaved scalar count.
    pub frame_count: usize,
}

pub struct AudioDecoder {
    _lane_local: PhantomData<Rc<()>>,
}

impl AudioDecoder {
    /// Opens the selected audio stream in its own demuxer on the audio lane.
    pub fn open(_path: &Path, _metadata: &MediaInfo) -> Result<Self> {
        todo!("V5: independent audio decoder")
    }

    /// Required before pulling. Select the device format before initial playback;
    /// a later reconfiguration requires a coordinated seek and reprime.
    pub fn configure_output(&mut self, _format: &PcmFormat) -> Result<()> {
        todo!("V5: configure resampling for the selected device")
    }

    /// None means both decoder and resampler have drained. Preserve stream gaps.
    pub fn next_samples(&mut self) -> Result<Option<AudioSamples>> {
        todo!("V5: pull owned PCM with normalized timestamps")
    }

    /// Flush decoder/resampler and trim pre-target samples on subsequent pulls.
    pub fn seek(&mut self, _position: Duration) -> Result<()> {
        todo!("V5: seek audio without retaining pre-target PCM")
    }
}

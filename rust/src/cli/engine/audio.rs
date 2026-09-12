use crate::transcribe::audio::AudioReader;
pub use crate::transcribe::audio::extract_audio_as_wav;
use crate::{
    cli::{error::Result, validate::MediaInfo},
    timeline::TimelineSerialization,
};
use std::{collections::HashMap, path::Path};
use ulid::Ulid;

#[derive(Default)]
pub struct Mixer {
    readers: HashMap<Ulid, AudioReader>,
}

impl Mixer {
    pub fn block(
        &mut self,
        doc: &TimelineSerialization,
        base: &Path,
        media: &HashMap<Ulid, MediaInfo>,
        start: i64,
        count: usize,
    ) -> Result<Vec<[f32; 2]>> {
        let fps = doc.settings.frame_rate;
        let rate = doc.settings.audio_sample_rate;
        let mut mixed = vec![[0.0_f32; 2]; count];
        let mut active = Vec::new();
        for clip in &doc.clips {
            let Some(data) = clip.media() else {
                continue;
            };
            let Some(track) = doc.track(data.track_id) else {
                continue;
            };
            let Some(asset) = doc.asset(data.asset_id) else {
                continue;
            };
            let Some(info) = media.get(&data.asset_id) else {
                continue;
            };
            let gain = 10.0_f64.powf(data.audio_properties.gain_db.clamp(-96.0, 24.0) / 20.0);
            if track.muted || data.audio_properties.muted || !asset.has_audio || !info.audio {
                continue;
            }
            let clip_start = fps.samples(data.timeline_start.frames(), rate);
            let clip_end = fps.samples(clip.timeline_end(fps).frames(), rate);
            let from = start.max(clip_start);
            let end = (start + count as i64).min(clip_end);
            if from >= end {
                continue;
            }
            active.push(data.id);
            if !self.readers.contains_key(&data.id) {
                self.readers
                    .insert(data.id, AudioReader::open(&base.join(&asset.path), rate)?);
            }
            let source = fps.samples(data.source_in.frames(), rate) + from - clip_start;
            let samples = self
                .readers
                .get_mut(&data.id)
                .unwrap()
                .read(source, (end - from) as usize)?;
            for (index, sample) in samples.iter().enumerate() {
                let output_index = (from - start) as usize + index;
                for channel in 0..2 {
                    mixed[output_index][channel] += sample[channel] * gain as f32;
                }
            }
        }
        self.readers.retain(|id, _| active.contains(id));
        for sample in &mut mixed {
            for channel in sample {
                *channel = channel.clamp(-1.0, 1.0);
            }
        }
        Ok(mixed)
    }
}

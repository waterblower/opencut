use crate::timeline_document;
use ::transcribe;
use anyhow::{Result, bail};
use std::path::PathBuf;

/// Validate the source and prepare audio on a background executor.
pub fn prepare_transcription(source: PathBuf, api_key: &str) -> Result<Vec<u8>> {
    if !source.is_absolute() {
        bail!("transcription source must be an absolute file path");
    }
    if api_key.trim().is_empty() {
        bail!("Set your MiniMax API key in Settings before generating SRT");
    }
    log::info!("Transcribing audio with MiniMax: {}", source.display());
    if timeline_document::is_timeline_path(&source) {
        bail!("Timeline transcription is unavailable; select an audio or video file");
    }
    if let Some(duration) = transcribe::audio::audio_duration(&source)?
        && duration > transcribe::MAX_TRANSCRIPTION_DURATION
    {
        bail!(
            "Audio exceeds the transcription limit of {} seconds",
            transcribe::MAX_TRANSCRIPTION_DURATION.as_secs()
        );
    }
    transcribe::audio::extract_audio_as_wav(&source)
}

#[cfg(test)]
#[path = "tests/transcription.test.rs"]
mod tests;

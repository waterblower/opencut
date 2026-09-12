use super::*;
use opencut_player::transcribe::{self, Format, Options, SRT};

/// Return merged SRT subtitles. The caller owns serialization and publication.
/// Run on a Tokio executor; audio preparation runs directly on the caller's thread.
pub async fn start_transcription(
    source: PathBuf,
    project_root: PathBuf,
    api_key: String,
) -> Result<SRT> {
    if !source.is_absolute() {
        anyhow::bail!("transcription source must be an absolute file path");
    }
    if api_key.trim().is_empty() {
        anyhow::bail!("Set your MiniMax API key in Settings before generating SRT");
    }
    let options = Options {
        format: Format::Srt,
        ..Options::default()
    };
    log::info!("Transcribing audio with MiniMax: {}", source.display());
    let srt = if timeline_document::is_timeline_path(&source) {
        let timeline = TimelineSerialization::load(&source)?;
        log::info!("Rendering timeline audio: {}", source.display());
        let wav = super::timeline_audio::render_audio_wav(&timeline, &project_root)?;
        transcribe::transcribe_wav(wav, &api_key, &options).await?
    } else {
        transcribe::transcribe(&source, &api_key, &options).await?
    };
    transcribe::subtitles::merge_srt_sections(&srt)
}

#[cfg(test)]
#[path = "tests/transcription.test.rs"]
mod tests;

use super::*;
use opencut_player::transcribe::{self, Format, Options};

/// Return SRT text without publishing it. The caller owns the output destination.
/// Run on a Tokio executor; audio preparation runs directly on the caller's thread.
pub async fn start_transcription(source: PathBuf, api_key: String) -> Result<String> {
    if !source.is_absolute() {
        anyhow::bail!("transcription source must be an absolute file path");
    }
    if api_key.trim().is_empty() {
        anyhow::bail!("Set your MiniMax API key in Settings before generating SRT");
    }
    if timeline_document::is_timeline_path(&source) {
        anyhow::bail!("timeline transcription is not supported");
    }
    let options = Options {
        format: Format::Srt,
        ..Options::default()
    };
    log::info!("Transcribing audio with MiniMax: {}", source.display());
    let response = transcribe::transcribe(&source, &api_key, &options).await?;
    let Some(srt) = response.as_str() else {
        anyhow::bail!("expected SRT response at {}:{}", file!(), line!());
    };
    super::srt::parse_srt_text_clips(srt, FrameRate::new(30, 1))?;
    Ok(srt.to_owned())
}

#[cfg(test)]
#[path = "tests/transcription.test.rs"]
mod tests;

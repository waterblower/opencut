//! MiniMax ASR for local media. Credentials belong to the caller, not library state.
use self::audio::extract_audio_as_wav;
use anyhow::Result;
use reqwest::{
    Client,
    header::{AUTHORIZATION, HeaderValue},
    multipart,
};
use serde_json::Value;
use std::{path::Path, time::Duration};
pub mod audio;
pub mod subtitles;
pub use subtitles::{SRT, Subtitle};

#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum Format {
    Json,
    #[default]
    #[cfg_attr(feature = "cli", value(name = "verbose_json"))]
    VerboseJson,
    Srt,
    Vtt,
}

impl Format {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::VerboseJson => "verbose_json",
            Self::Srt => "srt",
            Self::Vtt => "vtt",
        }
    }
}

#[derive(Debug, Default)]
pub struct Options {
    pub format: Format,
    /// Optional BCP-47 language hint; None enables mixed-language recognition.
    pub language: Option<String>,
}

/// Transcribe into parsed SRT subtitles. The response format is always SRT.
pub async fn transcribe(path: &Path, api_key: &str, options: &Options) -> Result<SRT> {
    let options = Options {
        format: Format::Srt,
        language: options.language.clone(),
    };
    let response = transcribe_response(path, api_key, &options).await?;
    let Some(text) = response.as_str() else {
        anyhow::bail!("expected SRT response at {}:{}", file!(), line!());
    };
    SRT::from_string(text)
}

/// Returns the provider's JSON object, or a JSON string containing SRT/VTT text.
/// HTTP is asynchronous; FFmpeg decoding blocks the caller's background thread.
pub async fn transcribe_response(path: &Path, api_key: &str, options: &Options) -> Result<Value> {
    if api_key.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "missing_api_key: {} at {}:{}",
            "an API key is required for transcription",
            file!(),
            line!()
        ));
    }
    if let Some(duration) = audio::audio_duration(path)?
        && duration > Duration::from_secs(500)
    {
        let duration = duration.as_secs_f64();
        return Err(anyhow::anyhow!(
            "audio_too_long: {} at {}:{}",
            format!(
                "audio duration is {duration:.6} seconds; transcription accepts at most 500 seconds"
            ),
            file!(),
            line!()
        ));
    }
    let wav = extract_audio_as_wav(path)?;
    transcribe_wav_response(wav, api_key, options).await
}

/// Transcribe normalized mono 16 kHz PCM WAV bytes without filesystem access.
pub async fn transcribe_wav(wav: Vec<u8>, api_key: &str, options: &Options) -> Result<SRT> {
    let options = Options {
        format: Format::Srt,
        language: options.language.clone(),
    };
    let response = transcribe_wav_response(wav, api_key, &options).await?;
    let Some(text) = response.as_str() else {
        anyhow::bail!("expected SRT response at {}:{}", file!(), line!());
    };
    SRT::from_string(text)
}

async fn transcribe_wav_response(wav: Vec<u8>, api_key: &str, options: &Options) -> Result<Value> {
    if api_key.trim().is_empty() {
        anyhow::bail!(
            "missing_api_key: an API key is required at {}:{}",
            file!(),
            line!()
        );
    }
    if wav.len() <= 44 || wav.len() % 2 != 0 {
        anyhow::bail!("invalid WAV data at {}:{}", file!(), line!());
    }
    let header = wav[..44].to_vec();
    let wav = audio::write_wav_header(wav)?;
    if header != wav[..44] {
        anyhow::bail!(
            "expected mono 16 kHz 16-bit PCM WAV at {}:{}",
            file!(),
            line!()
        );
    }
    // Extraction produces a 44-byte header followed by mono 16 kHz, 16-bit PCM.
    let audio_bytes = wav.len() - 44;
    if audio_bytes > 500 * 16_000 * 2 {
        let duration = audio_bytes as f64 / (16_000.0 * 2.0);
        return Err(anyhow::anyhow!(
            "audio_too_long: {} at {}:{}",
            format!(
                "audio duration is {duration:.6} seconds; transcription accepts at most 500 seconds"
            ),
            file!(),
            line!()
        ));
    }
    let client = match Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(600))
        .retry(reqwest::retry::never())
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(value) => value,
        Err(error) => {
            return Err(anyhow::anyhow!(
                "transcription_request: {} at {}:{}",
                error,
                file!(),
                line!()
            ));
        }
    };
    request(
        &client,
        "https://api.minimaxi.com/v1/speech_to_text",
        api_key,
        wav,
        options,
    )
    .await
}

async fn request(
    client: &Client,
    endpoint: &str,
    api_key: &str,
    wav: Vec<u8>,
    options: &Options,
) -> Result<Value> {
    let mut authorization = match HeaderValue::from_str(&format!("Bearer {api_key}")) {
        Ok(value) => value,
        Err(error) => {
            return Err(anyhow::anyhow!(
                "invalid_api_key: {} at {}:{}",
                error,
                file!(),
                line!()
            ));
        }
    };
    authorization.set_sensitive(true);
    let file = match multipart::Part::bytes(wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
    {
        Ok(value) => value,
        Err(error) => {
            return Err(anyhow::anyhow!(
                "transcription_request: {} at {}:{}",
                error,
                file!(),
                line!()
            ));
        }
    };
    let form = multipart::Form::new()
        .text("model", "asr-1.0")
        .text("response_format", options.format.as_str())
        .text("timestamp_level", "word")
        .text("stream", "false")
        .part("file", file);
    let mut request = client
        .post(endpoint)
        .header(AUTHORIZATION, authorization)
        .multipart(form);
    if let Some(language) = &options.language {
        request = request.header(
            "language",
            match HeaderValue::from_str(language) {
                Ok(value) => value,
                Err(error) => {
                    return Err(anyhow::anyhow!(
                        "invalid_language: {} at {}:{}",
                        error,
                        file!(),
                        line!()
                    ));
                }
            },
        );
    }
    let response = match request.send().await {
        Ok(value) => value,
        Err(error) => {
            return Err(anyhow::anyhow!(
                "transcription_request: {} at {}:{}",
                error,
                file!(),
                line!()
            ));
        }
    };
    let status = response.status();
    let bytes = match response.bytes().await {
        Ok(value) => value,
        Err(error) => {
            return Err(anyhow::anyhow!(
                "transcription_request: {} at {}:{}",
                error,
                file!(),
                line!()
            ));
        }
    };
    let body = match std::str::from_utf8(&bytes) {
        Ok(value) => value,
        Err(error) => {
            return Err(anyhow::anyhow!(
                "invalid_transcription_response: {} at {}:{}",
                error,
                file!(),
                line!()
            ));
        }
    };
    if !status.is_success() {
        let error: Value = serde_json::from_str(body).unwrap_or(Value::Null);
        let message = error
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("speech-to-text request failed");
        let request_id = error
            .get("request_id")
            .and_then(Value::as_str)
            .unwrap_or("unavailable");
        return Err(anyhow::anyhow!(
            "transcription_api: {} at {}:{}",
            format!(
                "HTTP {status}: {} (request_id: {})",
                message.replace(api_key, "[redacted]"),
                request_id.replace(api_key, "[redacted]")
            ),
            file!(),
            line!()
        ));
    }
    if matches!(options.format, Format::Srt | Format::Vtt) {
        return Ok(Value::String(body.into()));
    }
    let value: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(error) => {
            return Err(anyhow::anyhow!(
                "invalid_transcription_response: {} at {}:{}",
                error,
                file!(),
                line!()
            ));
        }
    };
    if !value.get("text").is_some_and(Value::is_string)
        || !value
            .get("duration")
            .and_then(Value::as_f64)
            .is_some_and(|n| n.is_finite() && n >= 0.0)
    {
        return Err(anyhow::anyhow!(
            "invalid_transcription_response: {} at {}:{}",
            "expected transcript text and duration",
            file!(),
            line!()
        ));
    }
    if matches!(options.format, Format::VerboseJson) {
        let Some(segments) = value.get("segments").and_then(Value::as_array) else {
            return Err(anyhow::anyhow!(
                "invalid_transcription_response: {} at {}:{}",
                "expected timestamped segments",
                file!(),
                line!()
            ));
        };
        if value.get("n_speakers").and_then(Value::as_u64).is_none()
            || segments.iter().any(|segment| {
                let (Some(start), Some(end)) = (
                    segment.get("start").and_then(Value::as_f64),
                    segment.get("end").and_then(Value::as_f64),
                ) else {
                    return true;
                };
                start < 0.0
                    || end < start
                    || segment.get("id").and_then(Value::as_u64).is_none()
                    || !segment.get("speaker").is_some_and(Value::is_string)
                    || !segment.get("text").is_some_and(Value::is_string)
            })
        {
            return Err(anyhow::anyhow!(
                "invalid_transcription_response: {} at {}:{}",
                "invalid speaker or segment fields",
                file!(),
                line!()
            ));
        }
    }
    Ok(value)
}

#[cfg(test)]
#[path = "tests/transcribe.test.rs"]
mod tests;

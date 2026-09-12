//! MiniMax ASR for local media. Credentials belong to the caller, not library state.
pub mod error;
use self::{
    audio::extract_audio_as_wav,
    error::{Error, Result},
};
use reqwest::{
    Client,
    header::{AUTHORIZATION, HeaderValue},
    multipart,
};
use serde_json::Value;
use std::{path::Path, time::Duration};
pub mod audio;
pub mod subtitles;

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

#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum TimestampLevel {
    #[default]
    Sentence,
    Word,
}

#[derive(Debug, Default)]
pub struct Options {
    pub format: Format,
    pub timestamp_level: TimestampLevel,
    /// Optional BCP-47 language hint; None enables mixed-language recognition.
    pub language: Option<String>,
}

/// Returns the provider's JSON object, or a JSON string containing SRT/VTT text.
/// HTTP is asynchronous; FFmpeg decoding blocks the caller's background thread.
pub async fn transcribe(path: &Path, api_key: &str, options: &Options) -> Result<Value> {
    if api_key.trim().is_empty() {
        return Err(Error::new(
            "missing_api_key",
            "an API key is required for transcription",
            file!(),
            line!(),
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
        Err(error) => return Err(Error::new("transcription_request", error, file!(), line!())),
    };
    let wav = extract_audio_as_wav(path, 500)?;
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
        Err(error) => return Err(Error::new("invalid_api_key", error, file!(), line!())),
    };
    authorization.set_sensitive(true);
    let file = match multipart::Part::bytes(wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
    {
        Ok(value) => value,
        Err(error) => return Err(Error::new("transcription_request", error, file!(), line!())),
    };
    let form = multipart::Form::new()
        .text("model", "asr-1.0")
        .text("response_format", options.format.as_str())
        .text(
            "timestamp_level",
            match options.timestamp_level {
                TimestampLevel::Sentence => "sentence",
                TimestampLevel::Word => "word",
            },
        )
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
                Err(error) => return Err(Error::new("invalid_language", error, file!(), line!())),
            },
        );
    }
    let response = match request.send().await {
        Ok(value) => value,
        Err(error) => return Err(Error::new("transcription_request", error, file!(), line!())),
    };
    let status = response.status();
    let bytes = match response.bytes().await {
        Ok(value) => value,
        Err(error) => return Err(Error::new("transcription_request", error, file!(), line!())),
    };
    let body = match std::str::from_utf8(&bytes) {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::new(
                "invalid_transcription_response",
                error,
                file!(),
                line!(),
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
        return Err(Error::new(
            "transcription_api",
            format!(
                "HTTP {status}: {} (request_id: {})",
                message.replace(api_key, "[redacted]"),
                request_id.replace(api_key, "[redacted]")
            ),
            file!(),
            line!(),
        ));
    }
    if matches!(options.format, Format::Srt | Format::Vtt) {
        return Ok(Value::String(body.into()));
    }
    let value: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(error) => {
            return Err(Error::new(
                "invalid_transcription_response",
                error,
                file!(),
                line!(),
            ));
        }
    };
    if !value.get("text").is_some_and(Value::is_string)
        || !value
            .get("duration")
            .and_then(Value::as_f64)
            .is_some_and(|n| n.is_finite() && n >= 0.0)
    {
        return Err(Error::new(
            "invalid_transcription_response",
            "expected transcript text and duration",
            file!(),
            line!(),
        ));
    }
    if matches!(options.format, Format::VerboseJson) {
        let Some(segments) = value.get("segments").and_then(Value::as_array) else {
            return Err(Error::new(
                "invalid_transcription_response",
                "expected timestamped segments",
                file!(),
                line!(),
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
            return Err(Error::new(
                "invalid_transcription_response",
                "invalid speaker or segment fields",
                file!(),
                line!(),
            ));
        }
    }
    Ok(value)
}

#[cfg(test)]
#[path = "tests/transcribe.test.rs"]
mod tests;

//! MiniMax ASR for local media. Credentials belong to the caller, not library state.
use crate::{
    cli::{engine::audio::transcription_wav, error::Result},
    cli_error, cli_try,
};
use reqwest::{
    Client,
    header::{AUTHORIZATION, HeaderValue},
    multipart,
};
use serde_json::Value;
use std::{path::Path, time::Duration};
use tokio::fs;

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum Format {
    Json,
    #[default]
    #[value(name = "verbose_json")]
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

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
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
/// HTTP is asynchronous; FFmpeg decoding runs on the blocking task pool.
pub async fn transcribe(path: &Path, api_key: &str, options: &Options) -> Result<Value> {
    if api_key.trim().is_empty() {
        return Err(cli_error!(
            "missing_api_key",
            "",
            2,
            "set MINIMAX_API_KEY before transcribing"
        ));
    }
    let client = cli_try!(
        Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(600))
            .retry(reqwest::retry::never())
            .redirect(reqwest::redirect::Policy::none())
            .build(),
        "transcription_request",
        "",
        5
    );
    let path = path.to_path_buf();
    let wav = cli_try!(
        tokio::task::spawn_blocking(move || transcription_wav(&path)).await,
        "decode_failure",
        "",
        5
    )?;
    request(
        &client,
        "https://api.minimaxi.com/v1/speech_to_text",
        api_key,
        wav,
        options,
    )
    .await
}

/// Check before the request and again before writing so output cannot replace input.
pub async fn check_output(input: &Path, output: &Path, overwrite: bool) -> Result<()> {
    if !cli_try!(fs::try_exists(output).await, "io_error", "", 6) {
        return Ok(());
    }
    let source = cli_try!(fs::canonicalize(input).await, "io_error", "", 6);
    let target = cli_try!(fs::canonicalize(output).await, "io_error", "", 6);
    if source == target {
        return Err(cli_error!(
            "output_is_source",
            "",
            6,
            "output would overwrite input media"
        ));
    }
    if !overwrite {
        return Err(cli_error!(
            "output_exists",
            "",
            6,
            "use --overwrite to replace {}",
            output.display()
        ));
    }
    Ok(())
}

async fn request(
    client: &Client,
    endpoint: &str,
    api_key: &str,
    wav: Vec<u8>,
    options: &Options,
) -> Result<Value> {
    let mut authorization = cli_try!(
        HeaderValue::from_str(&format!("Bearer {api_key}")),
        "invalid_api_key",
        "",
        2
    );
    authorization.set_sensitive(true);
    let file = cli_try!(
        multipart::Part::bytes(wav)
            .file_name("audio.wav")
            .mime_str("audio/wav"),
        "transcription_request",
        "",
        5
    );
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
            cli_try!(HeaderValue::from_str(language), "invalid_language", "", 2),
        );
    }
    let response = cli_try!(request.send().await, "transcription_request", "", 5);
    let status = response.status();
    let bytes = cli_try!(response.bytes().await, "transcription_request", "", 5);
    let body = cli_try!(
        std::str::from_utf8(&bytes),
        "invalid_transcription_response",
        "",
        5
    );
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
        return Err(cli_error!(
            "transcription_api",
            "",
            5,
            "HTTP {status}: {} (request_id: {})",
            message.replace(api_key, "[redacted]"),
            request_id.replace(api_key, "[redacted]")
        ));
    }
    if matches!(options.format, Format::Srt | Format::Vtt) {
        return Ok(Value::String(body.into()));
    }
    let value: Value = cli_try!(
        serde_json::from_str(body),
        "invalid_transcription_response",
        "",
        5
    );
    if !value.get("text").is_some_and(Value::is_string)
        || !value
            .get("duration")
            .and_then(Value::as_f64)
            .is_some_and(|n| n.is_finite() && n >= 0.0)
    {
        return Err(cli_error!(
            "invalid_transcription_response",
            "",
            5,
            "expected transcript text and duration"
        ));
    }
    if matches!(options.format, Format::VerboseJson) {
        let Some(segments) = value.get("segments").and_then(Value::as_array) else {
            return Err(cli_error!(
                "invalid_transcription_response",
                "",
                5,
                "expected timestamped segments"
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
            return Err(cli_error!(
                "invalid_transcription_response",
                "",
                5,
                "invalid speaker or segment fields"
            ));
        }
    }
    Ok(value)
}

#[cfg(test)]
#[path = "tests/transcribe.test.rs"]
mod tests;

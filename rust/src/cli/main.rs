use anyhow::{Context as _, Error, Result, anyhow};
use opencut_player::timeline::TimelineSerialization;
mod args;
mod docs;
mod render;

use args::{Args, Command};
use clap::Parser;
use opencut_player::{
    cli::{document, transcribe},
    engine::probe,
};
use serde_json::{Value, json};
use std::{
    io::{self, Write},
    process::{ExitCode, Termination},
    time::Instant,
};

#[tokio::main]
async fn main() -> CliExitCode {
    let started = Instant::now();
    let json_mode = std::env::args().any(|a| a == "--json");
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                let _ = error.print();
                return CliExitCode::Success;
            }
            let error = anyhow!("usage_error: {error:?} at {}:{}", file!(), line!());
            print_error(&error, json_mode);
            return CliExitCode::UsageError;
        }
    };
    let api_key = std::env::var("MINIMAX_API_KEY").ok();
    let result = run(args.command, args.json, api_key.as_deref()).await;
    let elapsed_seconds = started.elapsed().as_secs_f64();
    let _ = writeln!(io::stderr().lock(), "elapsed_seconds: {elapsed_seconds:.6}");
    print_result(result, args.json)
}

fn print_result(result: Result<Value>, json_mode: bool) -> CliExitCode {
    match result {
        Ok(value) => {
            let text = if json_mode {
                serde_json::to_string(&value)
            } else if let Some(text) = value.as_str() {
                Ok(text.to_string())
            } else {
                serde_json::to_string_pretty(&value)
            };
            let result = match text {
                Ok(text) => writeln!(io::stdout().lock(), "{text}"),
                Err(_) => return CliExitCode::OutputError,
            };
            if result.is_err() {
                return CliExitCode::OutputError;
            }
            CliExitCode::Success
        }
        Err(error) => {
            print_error(&error, json_mode);
            CliExitCode::CommandError
        }
    }
}

fn print_error(error: &Error, json: bool) {
    if json {
        let _ = writeln!(
            io::stdout().lock(),
            "{}",
            json!({"error": {"message": format!("{error:?}")}})
        );
    } else {
        let _ = writeln!(io::stderr().lock(), "{error:?}");
    }
}

async fn run(command: Command, json_mode: bool, api_key: Option<&str>) -> Result<Value> {
    match command {
        Command::Render { output } => render::render(&output),
        Command::Transcribe {
            media_file,
            format,
            post_merge,
            language,
            output,
            overwrite,
        } => {
            if post_merge && !matches!(format, transcribe::Format::Srt) {
                return Err(anyhow!(
                    "usage_error: --post-merge requires --format srt at {}:{}",
                    file!(),
                    line!()
                ));
            }
            if let Some(output) = &output {
                transcribe::check_output(&media_file, output, overwrite).await?;
            }
            let Some(api_key) = api_key else {
                return Err(anyhow!(
                    "missing_api_key: set MINIMAX_API_KEY before transcribing at {}:{}",
                    file!(),
                    line!()
                ));
            };
            let mut result = transcribe::transcribe_response(
                &media_file,
                api_key,
                &transcribe::Options { format, language },
            )
            .await?;
            if post_merge {
                let Some(srt) = result.as_str() else {
                    return Err(anyhow!(
                        "invalid_transcription_response: expected SRT text at {}:{}",
                        file!(),
                        line!()
                    ));
                };
                result = Value::String(
                    opencut_player::cli::subtitles::merge_srt_sections(
                        &opencut_player::transcribe::SRT::from_string(srt)?,
                    )?
                    .to_string(),
                );
            }
            let Some(output) = output else {
                return Ok(result);
            };
            transcribe::check_output(&media_file, &output, overwrite).await?;
            let bytes = if let Some(text) = result.as_str() {
                text.as_bytes().to_vec()
            } else {
                serde_json::to_vec_pretty(&result).context(format!(
                    "serialization_error at {}:{}",
                    file!(),
                    line!()
                ))?
            };
            document::write_atomic_bytes(&output, bytes, overwrite).await?;
            Ok(json!({"path": output, "format": format.as_str()}))
        }
        Command::Probe { file } => {
            if file
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            {
                let (_, doc) = document::load(&file)?;
                doc.validate()?;
                return Ok(document::summary(&doc));
            }
            match serde_json::to_value(probe::probe(&file)?) {
                Ok(value) => Ok(value),
                Err(error) => Err(anyhow!(
                    "serialization_error: {error:?} at {}:{}",
                    file!(),
                    line!()
                )),
            }
        }
        Command::Schema { .. } => Ok(serde_json::to_value(schemars::schema_for!(
            TimelineSerialization
        ))
        .context(format!("serialization_error at {}:{}", file!(), line!()))?),
        Command::Doc => Ok(json!(docs::generate()?)),
        Command::Validate { timeline } => {
            let (_, doc) = document::load(&timeline)?;
            let base = document::asset_base(&timeline)?;
            doc.validate()?;
            probe::assets(&doc.assets, &base)?;
            Ok(json!({"valid": true, "findings": []}))
        }
    }
}

#[repr(u8)]
enum CliExitCode {
    Success = 0,
    CommandError = 1,
    UsageError = 2,
    OutputError = 6,
}

impl Termination for CliExitCode {
    fn report(self) -> ExitCode {
        ExitCode::from(self as u8)
    }
}

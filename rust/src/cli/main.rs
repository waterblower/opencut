use anyhow::{Context as _, Error, Result, anyhow};
mod args;
mod docs;

use args::{Args, Command};
use clap::Parser;
use opencut_player::timeline::{TimelineSettings as Settings, Track, TrackKind};
use opencut_player::{
    cli::engine::probe,
    cli::{
        document::{self, Document},
        time::parse_rate,
        transcribe, validate,
    },
};
use serde_json::{Value, json};
use std::{
    io::{self, Write},
    path::Path,
    process::ExitCode,
};
use ulid::Ulid;

#[tokio::main]
async fn main() -> ExitCode {
    let json_mode = std::env::args().any(|a| a == "--json");
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                let _ = error.print();
                return ExitCode::SUCCESS;
            }
            let error = anyhow!("usage_error: {error:?} at {}:{}", file!(), line!());
            print_error(&error, json_mode);
            return ExitCode::from(2);
        }
    };
    let api_key = std::env::var("MINIMAX_API_KEY").ok();
    match run(
        args.command,
        args.json,
        &args.project_root,
        api_key.as_deref(),
    )
    .await
    {
        Ok(value) => {
            let text = if args.json {
                serde_json::to_string(&value)
            } else if let Some(text) = value.as_str() {
                Ok(text.to_string())
            } else {
                serde_json::to_string_pretty(&value)
            };
            let result = match text {
                Ok(text) => writeln!(io::stdout().lock(), "{text}"),
                Err(_) => return ExitCode::from(6),
            };
            if result.is_err() {
                return ExitCode::from(6);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            print_error(&error, args.json);
            ExitCode::FAILURE
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

async fn run(
    command: Command,
    json_mode: bool,
    base: &Path,
    api_key: Option<&str>,
) -> Result<Value> {
    let project_root = std::path::absolute(base).context(format!(
        "could not resolve project root {} at {}:{}",
        base.display(),
        file!(),
        line!()
    ))?;
    let base = project_root.as_path();
    match command {
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
        Command::Assemble {
            recipe,
            output,
            dry_run,
            overwrite,
        } => {
            opencut_player::cli::assemble::run(&recipe, base, output.as_deref(), dry_run, overwrite)
        }
        Command::Probe { file } => {
            if file
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            {
                let (_, doc) = document::load(&file)?;
                validate::require_valid(&doc, None)?;
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
        Command::New {
            timeline,
            width,
            height,
            fps,
        } => {
            let doc = Document {
                settings: Settings {
                    width,
                    height,
                    frame_rate: parse_rate(&fps)?,
                    ..Settings::default()
                },
                assets: vec![],
                clips: vec![],
                view: Default::default(),
                tracks: vec![
                    Track {
                        id: Ulid::generate(),
                        kind: TrackKind::Video,
                        name: "Video".into(),
                        muted: false,
                        visible: true,
                        locked: false,
                    },
                    Track {
                        id: Ulid::generate(),
                        kind: TrackKind::Audio,
                        name: "Audio".into(),
                        muted: false,
                        visible: true,
                        locked: false,
                    },
                ],
            };
            validate::require_valid(&doc, None)?;
            let raw = serde_json::to_value(&doc).context(format!(
                "serialization_error at {}:{}",
                file!(),
                line!()
            ))?;
            document::write_atomic(&timeline, &raw, false)?;
            if json_mode {
                Ok(json!({"path": timeline, "document": raw}))
            } else {
                Ok(json!(timeline))
            }
        }
        Command::Schema { kind, .. } => Ok(serde_json::to_value(if kind == "recipe" {
            schemars::schema_for!(opencut_player::cli::assemble::Recipe)
        } else {
            schemars::schema_for!(Document)
        })
        .context(format!("serialization_error at {}:{}", file!(), line!()))?),
        Command::Doc => Ok(json!(docs::generate()?)),
        Command::Validate { timeline } => {
            let (_, doc) = document::load(&timeline)?;
            let base = document::asset_base(&timeline)?;
            let media = probe::assets(&doc.assets, &base)?;
            let findings = validate::validate(&doc, Some(&media));
            if !findings.is_empty() {
                let exit = 1;
                let value = json!({"valid": false, "findings": findings});
                let text = if json_mode {
                    value.to_string()
                } else {
                    serde_json::to_string_pretty(&value).context(format!(
                        "serialization_error at {}:{}",
                        file!(),
                        line!()
                    ))?
                };
                writeln!(io::stdout().lock(), "{text}").context(format!(
                    "io_error at {}:{}",
                    file!(),
                    line!()
                ))?;
                std::process::exit(exit);
            }
            Ok(json!({"valid": true, "findings": []}))
        }
    }
}

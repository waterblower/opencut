mod args;
mod edit;

use args::{Args, Command};
use clap::Parser;
use opencut_player::{
    cli_error, cli_try,
    core::{
        document::{self, Document, Settings, Track, TrackKind},
        error::Result,
        time::parse_rate,
        validate,
    },
    engine::{probe, render},
};
use serde_json::{Value, json};
use std::{
    io::{self, Write},
    path::Path,
    process::ExitCode,
};
use ulid::Ulid;

fn main() -> ExitCode {
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
            let error = cli_error!("usage_error", "", 2, "{error}");
            print_error(&error, json_mode);
            return ExitCode::from(2);
        }
    };
    match run(args.command, args.json) {
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
            ExitCode::from(error.exit)
        }
    }
}

fn print_error(error: &opencut_player::core::error::Error, json: bool) {
    if json {
        let _ = writeln!(io::stdout().lock(), "{}", json!({"error": error}));
    } else {
        let _ = writeln!(io::stderr().lock(), "{error}");
    }
}

fn run(command: Command, json_mode: bool) -> Result<Value> {
    match command {
        Command::Probe { media_file } => Ok(cli_try!(
            serde_json::to_value(probe::probe(&media_file)?),
            "serialization_error",
            "",
            6
        )),
        Command::New {
            timeline,
            width,
            height,
            fps,
        } => {
            let doc = Document {
                version: 1,
                settings: Settings {
                    width,
                    height,
                    frame_rate: parse_rate(&fps)?,
                    ..Settings::default()
                },
                assets: vec![],
                clips: vec![],
                transitions: vec![],
                tracks: vec![
                    Track {
                        id: Ulid::generate().to_string(),
                        kind: TrackKind::Video,
                        name: "Video".into(),
                        muted: false,
                    },
                    Track {
                        id: Ulid::generate().to_string(),
                        kind: TrackKind::Audio,
                        name: "Audio".into(),
                        muted: false,
                    },
                ],
            };
            validate::require_valid(&doc, None)?;
            let raw = cli_try!(serde_json::to_value(&doc), "serialization_error", "", 6);
            document::write_atomic(&timeline, &raw, false)?;
            if json_mode {
                Ok(json!({"path": timeline, "document": raw}))
            } else {
                Ok(json!(timeline))
            }
        }
        Command::Schema { .. } => Ok(cli_try!(
            serde_json::to_value(schemars::schema_for!(Document)),
            "serialization_error",
            "",
            6
        )),
        Command::Docs => Ok(json!(include_str!("llms.txt"))),
        Command::Edit { timeline, command } => edit::edit(&timeline, command),
        Command::Validate { timeline } => {
            let (_, doc) = document::load(&timeline)?;
            let base = timeline.parent().unwrap_or(Path::new("."));
            let (media, media_findings) = probe::inspect_assets(&doc, base);
            let mut findings = validate::validate(&doc, Some(&media));
            findings.extend(media_findings);
            if !findings.is_empty() {
                let exit = if findings.iter().any(|f| f.error.exit == 4) {
                    4
                } else {
                    3
                };
                let value = json!({"valid": false, "findings": findings});
                let text = if json_mode {
                    value.to_string()
                } else {
                    cli_try!(
                        serde_json::to_string_pretty(&value),
                        "serialization_error",
                        "",
                        6
                    )
                };
                cli_try!(writeln!(io::stdout().lock(), "{text}"), "io_error", "", 6);
                std::process::exit(exit);
            }
            Ok(json!({"valid": true, "findings": []}))
        }
        Command::Inspect { timeline } => {
            let (_, doc) = document::load(&timeline)?;
            validate::require_valid(&doc, None)?;
            Ok(render::summary(&doc))
        }
        Command::Still {
            timeline,
            at,
            output,
            scale,
            overwrite,
        } => {
            let (_, doc) = document::load(&timeline)?;
            validate::require_valid(&doc, None)?;
            let frame = doc
                .settings
                .frame_rate
                .parse_time(&at, Some(doc.duration()))?;
            let base = timeline.parent().unwrap_or(Path::new("."));
            render::still(&doc, base, frame, &output, scale, overwrite)?;
            Ok(json!({"path": output, "frame": frame}))
        }
        Command::Render {
            timeline,
            output,
            range,
            scale,
            preset,
            video_codec,
            bitrate,
            audio_codec: _,
            progress,
            overwrite,
            dry_run,
            no_metadata,
        } => {
            let (raw, doc) = document::load(&timeline)?;
            validate::require_valid(&doc, None)?;
            let (start, end) = match range {
                Some(value) => {
                    let Some((start, end)) = value.split_once("..") else {
                        return Err(cli_error!("invalid_range", "", 2, "expected start..end"));
                    };
                    (
                        doc.settings.frame_rate.parse_time(start, None)?,
                        doc.settings.frame_rate.parse_time(end, None)?,
                    )
                }
                None => (0, doc.duration()),
            };
            let options = render::Options {
                start,
                end,
                scale,
                preset,
                video_codec,
                bitrate: match bitrate {
                    Some(value) => Some(render::parse_bitrate(&value)?),
                    None => None,
                },
                overwrite,
                metadata: if no_metadata {
                    None
                } else {
                    Some(raw.to_string())
                },
            };
            let base = timeline.parent().unwrap_or(Path::new("."));
            let plan = render::plan(&doc, base, &output, &options)?;
            if dry_run {
                return Ok(plan);
            }
            let (sender, receiver) = std::sync::mpsc::sync_channel(8);
            let base = base.to_path_buf();
            let worker =
                std::thread::spawn(move || render::render(&doc, &base, &output, &options, sender));
            for update in receiver {
                if progress == "json" {
                    eprintln!("{}", update);
                } else if progress == "bar" {
                    eprint!(
                        "\rframe {}/{}  {:.1} fps",
                        update["frame"],
                        update["total"],
                        update["fps"].as_f64().unwrap_or(0.0)
                    );
                }
            }
            if progress == "bar" {
                eprintln!();
            }
            match worker.join() {
                Ok(result) => result?,
                Err(_) => {
                    return Err(cli_error!(
                        "render_failure",
                        "",
                        5,
                        "render worker panicked"
                    ));
                }
            }
            Ok(plan)
        }
    }
}

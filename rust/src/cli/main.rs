mod args;

use args::{Args, Command};
use clap::Parser;
use opencut_player::timeline::{TimelineSettings as Settings, Track, TrackKind};
use opencut_player::{
    cli::engine::{probe, render},
    cli::{
        document::{self, Document},
        error::Result,
        time::parse_rate,
        validate,
    },
    cli_error, cli_try,
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
    match run(args.command, args.json, &args.project_root) {
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

fn print_error(error: &opencut_player::cli::error::Error, json: bool) {
    if json {
        let _ = writeln!(io::stdout().lock(), "{}", json!({"error": error}));
    } else {
        let _ = writeln!(io::stderr().lock(), "{error}");
    }
}

fn run(command: Command, json_mode: bool, base: &Path) -> Result<Value> {
    match command {
        Command::Assemble {
            recipe,
            output,
            dry_run,
            overwrite,
        } => {
            opencut_player::cli::assemble::run(&recipe, base, output.as_deref(), dry_run, overwrite)
        }
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
            let raw = cli_try!(serde_json::to_value(&doc), "serialization_error", "", 6);
            document::write_atomic(&timeline, &raw, false)?;
            if json_mode {
                Ok(json!({"path": timeline, "document": raw}))
            } else {
                Ok(json!(timeline))
            }
        }
        Command::Schema { kind, .. } => Ok(cli_try!(
            serde_json::to_value(if kind == "recipe" {
                schemars::schema_for!(opencut_player::cli::assemble::Recipe)
            } else {
                schemars::schema_for!(Document)
            }),
            "serialization_error",
            "",
            6
        )),
        Command::Docs => Ok(json!(include_str!("llms.txt"))),
        Command::Validate { timeline } => {
            let (_, doc) = document::load(&timeline)?;
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
                .parse_time(&at, Some(doc.content_duration().frames()))?;
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
                None => (0, doc.content_duration().frames()),
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
            let prepared = render::prepare(&doc, base, &output, &options)?;
            if dry_run {
                return Ok(prepared.summary);
            }
            let plan = prepared.summary.clone();
            let (sender, receiver) = std::sync::mpsc::sync_channel(8);
            let base = base.to_path_buf();
            let worker = std::thread::spawn(move || {
                render::render_prepared(&doc, &base, &output, &options, prepared, sender)
            });
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

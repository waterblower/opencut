use crate::args::Args;
use anyhow::Result;
use clap::CommandFactory;
use opencut_player::transcribe::MAX_TRANSCRIPTION_DURATION;
use std::fmt::Write;

/// Generate a usage guide with command details taken from Clap definitions.
pub fn generate() -> Result<String> {
    let mut command = Args::command();
    command.build();
    let mut output = String::from(
        r#"# OpenCut CLI

Use OpenCut to inspect media, author timeline JSON, validate timelines, and transcribe audio. The CLI uses FFmpeg and shares the editor's timeline format.

## Recommended workflow

1. Use `probe <file>` for video, audio, images, or timeline JSON. Media paths must be absolute. Probe source media before choosing cuts.
2. Create a timeline with `new`, or compile explicit cut and camera decisions with `assemble`.
3. Use `schema` when authoring JSON; do not guess document fields.
4. Validate referenced media and probe the timeline.

```sh
opencut probe /path/to/recording.mp4 --json
opencut new episode.timeline.json --fps 30 --json
opencut schema --json
opencut validate /project/episode.timeline.json --json
opencut probe /project/episode.timeline.json --json
```

## Paths and output

- Timeline and output arguments resolve from the working directory.
- Relative timeline asset paths resolve from the timeline file's directory. Absolute asset paths are unchanged. `--project-root` applies only to assembly recipe sources.
- Keep original media available; the timeline references it.
- Use `--json` for machine-readable stdout. Diagnostics go to stderr.
- Treat any nonzero exit code as failure. JSON errors contain `error.message`. Validation stops at the first media probe failure; independent document rule violations are reported as findings.
- `new` refuses existing files. Use `--overwrite` explicitly when replacing supported outputs. Outputs cannot replace source media.

## Authoring

`assemble` compiles decisions supplied by the caller; it does not choose cuts or camera switches automatically. Obtain the recipe contract before writing a recipe:

```sh
opencut schema --kind recipe --json
opencut --project-root /project assemble recipe.json --dry-run --json
opencut --project-root /project assemble recipe.json -o episode.timeline.json --json
```


## Transcription

Set `MINIMAX_API_KEY` in the environment. Transcription uploads the selected audio/video file's audio to MiniMax.

```sh
opencut transcribe recording.mp4 --format srt --post-merge -o subtitles.srt
opencut transcribe recording.wav --format verbose_json --language zh --json
```

All requests use word-level timestamps. `--post-merge` joins nearby SRT cues. Without `-o`, the result is printed; with `-o`, it is saved. The CLI accepts media files for transcription; timeline transcription is available in the editor.

"#,
    );
    writeln!(
        output,
        "Audio must be at most {} seconds, including timestamp gaps. Longer input is rejected rather than truncated.\n",
        MAX_TRANSCRIPTION_DURATION.as_secs()
    )?;
    output.push_str("## Command reference\n\nOptions, defaults, and choices below are generated from the CLI definitions. Use `opencut COMMAND --help` for detailed help. Full JSON schemas are available through `opencut schema` and `opencut schema --kind recipe`.\n");
    for child in command.get_subcommands_mut() {
        if child.is_hide_set() || child.get_name() == "help" {
            continue;
        }
        writeln!(output, "\n### `opencut {}`\n", child.get_name())?;
        if let Some(about) = child.get_about() {
            writeln!(output, "{about}\n")?;
        }
        writeln!(output, "```text\n{}\n```\n", child.render_usage())?;
        for arg in child.get_arguments() {
            if arg.is_hide_set() {
                continue;
            }
            write!(output, "- `{arg}`")?;
            if let Some(help) = arg.get_help() {
                write!(output, ": {help}")?;
            }
            let defaults = arg.get_default_values();
            if !defaults.is_empty() {
                write!(
                    output,
                    " Default: `{}`.",
                    defaults
                        .iter()
                        .map(|value| value.to_string_lossy())
                        .collect::<Vec<_>>()
                        .join(", ")
                )?;
            }
            let choices = arg.get_possible_values();
            if !choices.is_empty() {
                write!(
                    output,
                    " Choices: {}.",
                    choices
                        .iter()
                        .map(|value| format!("`{}`", value.get_name()))
                        .collect::<Vec<_>>()
                        .join(", ")
                )?;
            }
            writeln!(output)?;
        }
    }
    Ok(output)
}

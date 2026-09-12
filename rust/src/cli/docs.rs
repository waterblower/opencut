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

Use OpenCut to inspect media, author timeline JSON, preview frames, render video, and transcribe audio. The CLI uses FFmpeg and shares the editor's timeline format.

## Recommended workflow

1. Probe source media before choosing cuts.
2. Create a timeline with `new`, or compile explicit cut and camera decisions with `assemble`.
3. Use `schema` when authoring JSON; do not guess document fields.
4. Validate referenced media and inspect the timeline.
5. Render a still to check composition, then run a dry run before the final render.

```sh
opencut probe recording.mp4 --json
opencut new episode.timeline.json --fps 30 --json
opencut schema --json
opencut --project-root /project validate episode.timeline.json --json
opencut --project-root /project inspect episode.timeline.json --json
opencut --project-root /project still episode.timeline.json --at 50% -o preview.png
opencut --project-root /project render episode.timeline.json -o output.mp4 --dry-run --json
opencut --project-root /project render episode.timeline.json -o output.mp4 --progress json --json
```

## Paths and output

- Timeline and output arguments resolve from the working directory.
- Relative asset paths resolve from `--project-root`, which defaults to the working directory, not the timeline's directory.
- Keep original media available; the timeline references it.
- Use `--json` for machine-readable stdout. Progress goes to stderr; `render --progress none` disables it.
- Treat any nonzero exit code as failure. JSON errors contain `error.message`; validation reports findings.
- `new` refuses existing files. Use `--overwrite` explicitly when replacing supported outputs. Outputs cannot replace source media.

## Authoring and previewing

`assemble` compiles decisions supplied by the caller; it does not choose cuts or camera switches automatically. Obtain the recipe contract before writing a recipe:

```sh
opencut schema --kind recipe --json
opencut --project-root /project assemble recipe.json --dry-run --json
opencut --project-root /project assemble recipe.json -o episode.timeline.json --json
```

Time arguments accept seconds (`12.5` or `12.5s`), frames (`375f`), and timestamps (`00:00:12.500`). Only `still --at` accepts percentages; `100%` selects the final frame. Use `render --range START..END` for a portion of a timeline.

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

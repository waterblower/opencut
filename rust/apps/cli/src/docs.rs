use crate::args::Args;
use ::transcribe::MAX_TRANSCRIPTION_DURATION;
use anyhow::Result;
use clap::CommandFactory;
use std::fmt::Write;

/// Generate a usage guide with command details taken from Clap definitions.
pub fn generate() -> Result<String> {
    let mut command = Args::command();
    command.build();
    let mut output = String::from(
        r#"# OpenCut CLI

Use OpenCut to inspect media, author timeline JSON, validate and export timelines, and transcribe audio. The CLI uses FFmpeg and shares the editor's timeline format.

## Recommended workflow

1. Use `probe <file>` for video, audio, images, or timeline JSON. Media paths must be absolute. Probe source media before choosing cuts.
2. Create a timeline in the editor or author JSON using the schema.
3. Use `schema` when authoring JSON; do not guess document fields.
4. Validate referenced media and probe the timeline.

```sh
opencut probe /path/to/recording.mp4 --json
opencut schema --json
opencut validate /project/episode.timeline.json --json
opencut probe /project/episode.timeline.json --json
```

## Paths and output

- Timeline and output arguments resolve from the working directory.
- Relative timeline asset paths resolve from the timeline file's directory. Absolute asset paths are unchanged.
- For export, `--project-root` overrides the base directory for relative assets.
- Keep original media available; the timeline references it.
- Use `--json` for machine-readable stdout. Diagnostics go to stderr.
- Every executed command reports `elapsed_seconds` on stderr, excluding Cargo
  build time. This also applies to failed commands.
- Treat any nonzero exit code as failure. JSON errors contain `error.message`. Validation uses the shared timeline validator and stops at the first document or media probe error.
- Use `--overwrite` explicitly when replacing supported outputs. Outputs cannot replace source media.

## Timeline export (macOS)

```sh
opencut export /project/episode.timeline.json -o episode.mp4
opencut export /project/timelines/episode.json -o episode.mp4 --project-root /project --video-bitrate 8000 --json
```

Exports the complete timeline synchronously to H.264 video and stereo AAC audio.
Canvas size, frame rate, and audio sample rate come from the timeline settings.
`--video-bitrate` is in kbps (1 kbps = 1,000 bits per second) and defaults to 8,000.
Requires macOS Metal and VideoToolbox services. Existing output files are refused.
On success, stdout reports the output path and frame count.

## GPUI rendering demo (macOS)

`render -o hello.mp4` produces 150 frames at 30 fps: five seconds of centered
white "hello gpui" text on black. It uses GPUI's Metal renderer and FFmpeg H.264
encoding. The canvas is 640×360 logical pixels, producing 1280×720 output at the
test platform's fixed 2× scale. This initial demo uses GPUI's test-support headless
context with the real Metal renderer, without creating a native window. It requires
macOS Metal and VideoToolbox services.
It refuses existing output files and does not yet accept timelines.
Elapsed time includes argument parsing, rendering, encoding, and cleanup.

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
    output.push_str("## Command reference\n\nOptions, defaults, and choices below are generated from the CLI definitions. Use `opencut COMMAND --help` for detailed help. The full timeline JSON schema is available through `opencut schema`.\n");
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

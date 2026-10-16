# OpenCut

OpenCut is an experimental desktop video tool written in Rust with
[GPUI](https://gpui.rs/). The `rust` workspace contains shared libraries under
`rust/libs` and these applications under `rust/apps`:

- `editor`: a non-destructive, folder-based multi-track editor.
- `player`: a debugging player for a video, audio, or timeline file.
- `cli`: a headless tool for probing, validating, and exporting timelines; see
  [its README](rust/apps/cli/README.md).

The project is an active prototype rather than a production-ready editor.

Devlog: https://www.youtube.com/playlist?list=PLRz1nfZl0jMU

## Requirements

- Latest stable Rust (edition 2024)
- Existing FFmpeg development libraries in `rust/vendor/ffmpeg-8.1.2/`; do not
  build FFmpeg locally
- On macOS: Xcode command line tools and `pkg-config` (`brew install pkg-config`)

## Build and run

Run commands from `rust`. Each command selects a platform explicitly:

```sh
cargo editor-mac                 # Windows: cargo editor-win
cargo player-mac -- <file>       # Windows: cargo player-win -- <file>
cargo build-editor-mac
cargo build-player-mac
cargo test-mac                   # Windows: cargo test-win
```

Extra Cargo flags such as `--release` are forwarded; application arguments
follow `--`. These are native host commands, not cross-compilation commands.
Editor aliases load `.cargo/macos.toml` or `.cargo/windows.toml`; player aliases
use the FFmpeg-only `.cargo/cli.toml` or `.cargo/ffmpeg-windows.toml`. Small
shell runners set the runtime library paths for the vendored FFmpeg.

For other Cargo operations, pass the platform configuration:

```sh
cargo check --config .cargo/macos.toml -p editor   # Windows: .cargo/windows.toml
```

## Documentation

- [Design spec](design/spec.md): user stories and acceptance criteria for the
  editor, including supported files, shortcuts, and project data.
- [Agent Skill](.agents/skills/opencut-development/SKILL.md):
  implementation and code review conventions.
- [Thoughts](thoughts/): essays on the project.

## Development

```sh
cargo test-mac # Windows: cargo test-win
cargo check --config .cargo/macos.toml --all-targets --all-features
cargo clippy --config .cargo/macos.toml --all-features
cargo loc
```

`cargo loc` reports the project's Rust line count. Use `.cargo/windows.toml` on
Windows. Plain `cargo test` loads no platform configuration; use `test-mac` or
`test-win` for the editor and its native media dependencies.

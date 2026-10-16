# Audio transcription

The standalone `transcribe` crate provides async MiniMax speech recognition
for local audio and video. Both the CLI and editor depend on it. Its optional
`cli` feature adds Clap value-enum derives; the editor uses the crate without
that feature. There are no GUI dependencies.

In the editor, open **Settings** in the top bar to save a MiniMax API key in the
global settings JSON. Right-click an audio or video file in the project
explorer and choose **Generate SRT**. Output is saved in the project root as
`<source-stem>.srt`, replacing any existing file at that path.
This sends the extracted audio to MiniMax; it does not insert subtitle clips.

The parsed-SRT APIs, `transcribe` and `transcribe_wav`, automatically split audio
into sections of at most 500 seconds, transcribe them sequentially, and offset
subtitle timestamps to produce one SRT. Files without audio are rejected.
`transcribe_response` requires at most 500 seconds total in every format,
including SRT. The CLI uses this raw-response API; the editor uses the chunked
parsed-SRT API. Timeline transcription is not currently supported.

Call `transcribe(path, api_key, &Options).await` from a Tokio runtime. Credentials
are supplied by application initialization. The result is a parsed `SRT`; this
function always requests SRT. Use `transcribe_response` for raw JSON or subtitle
text in the requested format. FFmpeg decoding runs synchronously on the calling
thread; callers choose where to run it. HTTP uses async reqwest. File publication
remains the caller's job.

`subtitles::merge_srt_sections(&srt)` optionally merges cues with gaps under
100 ms. `audio::extract_audio_as_wav(path)` exposes full-file synchronous
normalization to mono 16 kHz, 16-bit PCM WAV. `audio::AudioReader` supports this
normalization; the timeline engine uses its own audio reader and mixer.

Errors use anyhow with context; some diagnostics also include a source location.
Applications decide how to display them or map them to exit codes.

Link against the existing vendored FFmpeg libraries; do not build FFmpeg.
From the repository root, run module tests with:

```sh
bash rust/scripts/cargo-cli.sh test --locked -p transcribe --lib
```

Tests use local mock HTTP servers and require no API key or paid requests.
The editor supports Generate SRT for individual audio/video files. Its
transcription flow is not connected to the timeline audio mixer.

Call `transcribe_wav(wav, api_key, &Options).await` to transcribe normalized WAV
bytes directly. It validates the WAV format and splits long audio before uploading, then
returns parsed SRT. The editor merges cues and writes only the final SRT to the
project root; it creates no intermediate audio files.

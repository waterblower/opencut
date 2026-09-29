use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use ulid::Ulid;
use {
    engine::probe,
    opencut::{
        document,
        time::{ParseTime, parse_rate},
    },
    timeline::*,
};

#[test]
fn removed_commands_are_unavailable() {
    let help = cli(&["--help"]);
    let help = String::from_utf8(help.stdout).unwrap();
    for command in ["still", "render"] {
        assert!(!help.contains(&format!("  {command} ")));
        let output = cli(&[command, "--json"]);
        assert_eq!(output.status.code(), Some(2));
        assert!(
            decode(&output)["error"]["message"]
                .as_str()
                .unwrap()
                .contains("usage_error")
        );
    }
}

#[test]
fn schema_uses_the_gui_document_contract() {
    let dir = Temp::new();
    let file = dir.0.join("timeline.json");
    let schema = cli(&["schema", "--json"]);
    assert!(schema.status.success());
    assert_eq!(
        decode(&schema),
        serde_json::to_value(schemars::schema_for!(TimelineSerialization)).unwrap()
    );
    let removed = cli(&["new", file.to_str().unwrap(), "--json"]);
    assert_eq!(removed.status.code(), Some(2));
    assert!(!file.exists());
    let removed = cli(&["edit", file.to_str().unwrap(), "--json"]);
    assert_eq!(removed.status.code(), Some(2));
    fs::write(&file, r#"{"version":1,"clips":[]}"#).unwrap();
    let legacy = cli(&["probe", file.to_str().unwrap(), "--json"]);
    assert_eq!(legacy.status.code(), Some(1));
    assert!(
        decode(&legacy)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("legacy_cli_format")
    );
}

#[test]
fn probe_summarizes_timelines_without_opening_referenced_media() {
    let dir = Temp::new();
    let mut doc = empty();
    doc.assets
        .push(asset(100, "missing.mp4", MediaKind::Video, false));
    doc.clips
        .push(Clip::Video(media_clip(200, 1, 100, 15, 0, 30)));
    let expected = document::summary(&doc);
    for name in ["project.timeline.json", "project.json", "uppercase.JSON"] {
        let file = dir.0.join(name);
        document::write_atomic(
            &file,
            &serde_json::to_value(TimelineSerialization::from_editing_state(&doc)).unwrap(),
            false,
        )
        .unwrap();
        let output = cli(&["probe", file.to_str().unwrap(), "--json"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert_eq!(decode(&output), expected);
    }
    let removed = cli(&["inspect", "project.timeline.json", "--json"]);
    assert_eq!(removed.status.code(), Some(2));
    let help = cli(&["--help"]);
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("probe"));
    assert!(!help.contains("  inspect"));
}

#[test]
fn probe_rejects_relative_paths_before_opening_media() {
    for path in ["", "video.mp4", "./audio.wav", "../image.png", "image.svg"] {
        let error = probe::probe(Path::new(path)).unwrap_err();
        let message = format!("{error:?}");
        assert!(message.contains("media path must be absolute"), "{message}");
        assert!(message.contains("probe.rs:"), "{message}");
    }
}

#[test]
fn probe_reports_audio_and_image_metadata() {
    let dir = Temp::new();
    let audio = dir.0.join("tone.wav");
    write_tone(&audio, 44100, 1);
    let output = cli(&["probe", audio.to_str().unwrap(), "--json"]);
    assert!(output.status.success());
    let value = decode(&output);
    assert_eq!(value["streams"][0]["kind"], "audio");
    assert_eq!(value["streams"][0]["sample_rate"], 44100);
    let image = dir.0.join("image.png");
    RgbaImage::from_pixel(64, 48, Rgba([220, 20, 10, 255]))
        .save(&image)
        .unwrap();
    let output = cli(&["probe", image.to_str().unwrap(), "--json"]);
    assert!(output.status.success());
    let value = decode(&output);
    assert_eq!(value["container"], "image");
    assert_eq!(value["streams"][0]["width"], 64);
    assert_eq!(value["streams"][0]["height"], 48);
}

#[test]
fn probe_reports_invalid_timeline_and_missing_file_errors() {
    let dir = Temp::new();
    let file = dir.0.join("invalid.timeline.json");
    fs::write(&file, "{").unwrap();
    let output = cli(&["probe", file.to_str().unwrap(), "--json"]);
    assert!(!output.status.success());
    assert!(
        decode(&output)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("invalid_json")
    );
    for name in ["missing.json", "missing.mp4"] {
        let file = dir.0.join(name);
        let output = cli(&["probe", file.to_str().unwrap(), "--json"]);
        assert!(!output.status.success());
        assert!(decode(&output)["error"]["message"].is_string());
    }
}

#[test]
fn timeline_assets_resolve_from_timeline_directory() {
    let dir = Temp::new();
    fs::create_dir(dir.0.join("scenes")).unwrap();
    RgbaImage::from_pixel(64, 48, Rgba([220, 20, 10, 255]))
        .save(dir.0.join("scenes/image.png"))
        .unwrap();
    RgbaImage::from_pixel(64, 48, Rgba([0, 0, 255, 255]))
        .save(dir.0.join("image.png"))
        .unwrap();
    let mut doc = empty();
    doc.assets
        .push(asset(100, "image.png", MediaKind::Image, false));
    doc.clips
        .push(Clip::Video(media_clip(200, 1, 100, 0, 0, 30)));
    let file = dir.0.join("scenes/intro.timeline.json");
    document::write_atomic(
        &file,
        &serde_json::to_value(TimelineSerialization::from_editing_state(&doc)).unwrap(),
        false,
    )
    .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_opencut"))
        .current_dir(&dir.0)
        .args(["validate", "scenes/intro.timeline.json", "--json"])
        .output()
        .unwrap();
    assert!(result.status.success());
    doc.assets[0].path = "../image.png".into();
    document::write_atomic(
        &file,
        &serde_json::to_value(TimelineSerialization::from_editing_state(&doc)).unwrap(),
        true,
    )
    .unwrap();
    assert!(
        cli(&["validate", file.to_str().unwrap(), "--json"])
            .status
            .success()
    );
    doc.assets[0].path = dir.0.join("image.png");
    document::write_atomic(
        &file,
        &serde_json::to_value(TimelineSerialization::from_editing_state(&doc)).unwrap(),
        true,
    )
    .unwrap();
    assert!(
        cli(&["validate", file.to_str().unwrap(), "--json"])
            .status
            .success()
    );
}

#[test]
fn validation_stops_at_first_missing_asset_and_reports_schema_locations() {
    let dir = Temp::new();
    let mut doc = empty();
    doc.assets = vec![
        asset(100, "missing-a.mp4", MediaKind::Video, true),
        asset(101, "missing-b.wav", MediaKind::Audio, true),
    ];
    let file = dir.0.join("missing.timeline.json");
    document::write_atomic(
        &file,
        &serde_json::to_value(TimelineSerialization::from_editing_state(&doc)).unwrap(),
        false,
    )
    .unwrap();
    let result = cli(&["validate", file.to_str().unwrap(), "--json"]);
    assert_eq!(result.status.code(), Some(1));
    let report = decode(&result);
    let message = report["error"]["message"].as_str().unwrap();
    assert!(message.contains("/assets/0/path"));
    assert!(message.contains("missing-a.mp4"));
    assert!(!message.contains("missing-b.wav"));
    assert!(report.get("findings").is_none());

    let valid_image = dir.0.join("image.png");
    RgbaImage::from_pixel(64, 48, Rgba([0, 0, 0, 255]))
        .save(&valid_image)
        .unwrap();
    doc.assets[0] = asset(100, "image.png", MediaKind::Image, false);
    let error = probe::assets(&doc.assets, &dir.0).unwrap_err();
    let message = format!("{error:?}");
    assert!(message.contains("/assets/1/path"));
    assert!(message.contains("missing-b.wav"));
    let mut raw = serde_json::to_value(TimelineSerialization::from_editing_state(&doc)).unwrap();
    raw["editing_state"]["settings"]["width"] = json!("wrong");
    assert!(
        document::parse(&raw)
            .unwrap_err()
            .to_string()
            .contains("/settings/width")
    );
}

#[test]
fn exact_time_and_invalid_inputs() {
    let fps = parse_rate("30000/1001").unwrap();
    assert_eq!(fps.parse_time("1001s", None).unwrap(), 30000);
    assert_eq!(fps.parse_time("00:16:41.000", None).unwrap(), 30000);
    assert_eq!(fps.parse_time("375f", None).unwrap(), 375);
    assert_eq!(fps.parse_time("100%", Some(60)).unwrap(), 59);
    assert_eq!(fps.samples(30000, 48000), 48_048_000);
    for input in [
        "NaN",
        "inf",
        "-1",
        "00:60:00",
        "1.2.3",
        "999999999999999999999999",
        "101%",
    ] {
        assert!(fps.parse_time(input, Some(60)).is_err(), "{input}");
    }
    assert!(parse_rate("30/0").is_err());
}

fn empty() -> TimelineEditingState {
    TimelineEditingState {
        settings: TimelineSettings {
            width: 64,
            height: 48,
            ..Default::default()
        },
        tracks: vec![
            track(1, TrackKind::Video),
            track(2, TrackKind::Audio),
            track(3, TrackKind::Text),
        ],
        ..Default::default()
    }
}
fn track(id: u128, kind: TrackKind) -> Track {
    Track {
        id: Ulid::from(id),
        kind,
        name: format!("Track {id}"),
        muted: false,
        visible: true,
        locked: false,
    }
}
fn asset(id: u128, path: &str, kind: MediaKind, has_audio: bool) -> MediaAsset {
    MediaAsset {
        id: Ulid::from(id),
        kind,
        path: path.into(),
        name: path.into(),
        duration: 4.0,
        width: 64,
        height: 48,
        framerate: 30.0,
        frame_rate_numerator: 30,
        frame_rate_denominator: 1,
        codec: String::new(),
        has_audio,
    }
}
fn media_clip(
    id: u128,
    track: u128,
    asset: u128,
    start: i64,
    input: i64,
    out: i64,
) -> MediaClipData {
    MediaClipData {
        id: Ulid::from(id),
        track_id: Ulid::from(track),
        asset_id: Ulid::from(asset),
        timeline_start: TimelineFrame::from_frames(start),
        source_in: TimelineFrame::from_frames(input),
        source_out: TimelineFrame::from_frames(out),
        video_properties: Default::default(),
        audio_properties: Default::default(),
    }
}
fn write_tone(path: &Path, rate: u32, seconds: u32) {
    let count = rate * seconds;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + count * 2).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 2).to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(count * 2).to_le_bytes());
    for n in 0..count {
        let sample = ((n as f64 * 440.0 * std::f64::consts::TAU / rate as f64).sin() * 8000.0)
            .round() as i16;
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    fs::write(path, wav).unwrap();
}
fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_opencut"))
        .args(args)
        .output()
        .unwrap()
}
fn decode(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap()
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("opencut-test-{}", Ulid::generate()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn generates_agent_docs_from_cli_definitions() {
    let output = Command::new(env!("CARGO_BIN_EXE_opencut"))
        .args(["doc"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("## Recommended workflow"));
    assert!(!text.contains("$schema"));
    assert!(!text.contains("--llm"));
    assert!(text.contains("--post-merge"));
    assert!(text.contains("--project-root"));
    assert!(!text.contains("assemble"));
    assert!(!text.contains("recipe"));
    let json_output = Command::new(env!("CARGO_BIN_EXE_opencut"))
        .args(["docs", "--json"])
        .output()
        .unwrap();
    assert!(json_output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&json_output.stdout)
            .unwrap()
            .as_str()
            .unwrap(),
        text.strip_suffix('\n').unwrap()
    );
}

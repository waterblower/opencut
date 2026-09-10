#![cfg(feature = "cli")]
use image::{Rgba, RgbaImage};
use opencut_player::{
    cli::{
        assemble::{self, Recipe},
        document,
        engine::{
            audio::Mixer,
            decode::VideoWorker,
            encode::{Encoder, VideoEncoding},
            probe::{self, Probe, Stream},
        },
    },
    timeline::*,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};
use ulid::Ulid;

#[test]
fn assembly_maps_cuts_offsets_and_camera_switches() {
    let recipe: Recipe = serde_json::from_value(recipe()).unwrap();
    let (doc, report) = assemble::compile(&recipe, &metadata()).unwrap();
    assert_eq!(doc.content_duration().frames(), 60);
    assert_eq!(
        report["intervals"][1],
        json!({"episode_start":90,"episode_end":120,"output_start":30,"output_end":60})
    );
    let audio: Vec<_> = doc
        .clips
        .iter()
        .filter_map(|clip| match clip {
            Clip::Audio(data) => Some(data),
            _ => None,
        })
        .collect();
    assert_eq!(audio.len(), 2, "camera switch must not split master audio");
    assert_eq!(
        (audio[0].source_in.frames(), audio[0].source_out.frames()),
        (30, 60)
    );
    assert_eq!(
        (
            audio[1].source_in.frames(),
            audio[1].timeline_start.frames()
        ),
        (90, 30)
    );
    assert_eq!(audio[0].audio_properties.gain_db, -6.0);
    let video: Vec<_> = doc
        .clips
        .iter()
        .filter_map(|clip| match clip {
            Clip::Video(data) => Some(data),
            _ => None,
        })
        .collect();
    assert_eq!(video.len(), 3);
    assert_eq!(
        (video[0].source_in.frames(), video[0].source_out.frames()),
        (60, 75)
    );
    assert_eq!(
        (video[1].source_in.frames(), video[1].source_out.frames()),
        (15, 30)
    );
    assert_eq!(
        (video[2].source_in.frames(), video[2].source_out.frames()),
        (60, 90)
    );
    assert!(video.iter().all(|data| data.audio_properties.muted));
    let raw = serde_json::to_value(&doc).unwrap();
    assert_eq!(
        serde_json::to_value(document::parse(&raw).unwrap()).unwrap(),
        raw
    );
}

#[test]
fn recipe_rejects_bad_decisions_and_reports_locations() {
    for (pointer, value, code) in [
        ("/time_base/denominator", json!(0), "invalid_time_base"),
        ("/master_audio", json!("missing"), "unknown_source"),
        ("/master_audio", json!("silent"), "invalid_master_audio"),
        ("/sources/0/source_offset", json!(1), "invalid_master_audio"),
        ("/sources/1/name", json!("master"), "duplicate_source"),
        (
            "/sources/1/source_offset",
            json!(-2000),
            "source_out_of_bounds",
        ),
        ("/retained/1/start", json!(1000), "invalid_range"),
        ("/retained/0/start", json!(-1), "invalid_range"),
        ("/retained/0/end", json!(1001), "collapsed_range"),
        ("/retained", json!([]), "invalid_range"),
        ("/cameras/0/end", json!(1400), "missing_camera_coverage"),
        ("/cameras/1/source", json!("missing"), "unknown_source"),
        ("/cameras/1/source", json!("master"), "invalid_camera"),
        ("/cameras/1/start", json!(1400), "invalid_range"),
        ("/gain_db", json!(25), "invalid_gain"),
        ("/retained/1/end", json!(20000), "source_out_of_bounds"),
        (
            "/settings/frame_rate/numerator",
            json!(0),
            "invalid_frame_rate",
        ),
    ] {
        let mut value_recipe = recipe();
        *value_recipe.pointer_mut(pointer).unwrap() = value;
        let recipe: Recipe = serde_json::from_value(value_recipe).unwrap();
        let error = assemble::compile(&recipe, &metadata()).unwrap_err();
        assert_eq!(error.code, code, "{pointer}: {error}");
        assert!(!error.pointer.is_empty());
        assert!(error.line > 0 && !error.file.is_empty());
    }
    let recipe: Recipe = serde_json::from_value(recipe()).unwrap();
    let mut media = metadata();
    media[1].streams.push(stream("video"));
    assert_eq!(
        assemble::compile(&recipe, &media).unwrap_err().code,
        "unsupported_streams"
    );
    let mut value = serde_json::to_value(recipe).unwrap();
    value["captions"] = json!([]);
    assert!(
        serde_json::from_value::<Recipe>(value).is_err(),
        "unsupported decisions must not be silently ignored"
    );
}

#[test]
fn fractional_quantization_reports_adjustments_and_checks_overflow() {
    let mut value = recipe();
    value["settings"]["frame_rate"] = json!({"numerator":30000,"denominator":1001});
    value["sources"][1]["source_offset"] = json!(11);
    let recipe: Recipe = serde_json::from_value(value).unwrap();
    let (doc, report) = assemble::compile(&recipe, &metadata()).unwrap();
    assert_eq!(doc.content_duration().frames(), 60);
    assert!(
        report["rounding"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["pointer"] == "/sources/1/source_offset"
                && item["effective_frames"] == 0)
    );
    let mut value = serde_json::to_value(recipe).unwrap();
    value["time_base"] = json!({"numerator":u32::MAX,"denominator":1});
    value["sources"][1]["source_offset"] = json!(i64::MAX);
    let recipe: Recipe = serde_json::from_value(value).unwrap();
    assert_eq!(
        assemble::compile(&recipe, &metadata()).unwrap_err().code,
        "time_overflow"
    );
}

#[test]
fn single_camera_and_stream_duration_coverage() {
    let mut value = recipe();
    value["cameras"] = json!([{"source":"camera","start":1000,"end":4000}]);
    let recipe: Recipe = serde_json::from_value(value).unwrap();
    let (doc, _) = assemble::compile(&recipe, &metadata()).unwrap();
    assert_eq!(doc.clips.len(), 4);
    let mut media = metadata();
    media[1].streams[0].duration = Some(4.5);
    assert_eq!(
        assemble::compile(&recipe, &media).unwrap_err().code,
        "source_out_of_bounds"
    );
    let mut media = metadata();
    media[0].streams[0].duration = Some(3.5);
    assert_eq!(
        assemble::compile(&recipe, &media).unwrap_err().code,
        "source_out_of_bounds"
    );
}

#[test]
fn cli_assembles_previews_and_renders_without_overwriting_inputs() {
    let dir = Temp::new();
    probe::init().unwrap();
    let source = dir.0.join("camera.mov");
    let mut encoder = Encoder::open(
        &source,
        (64, 48),
        FrameRate::default(),
        48000,
        &VideoEncoding {
            codec: "prores".into(),
            preset: "standard".into(),
            bitrate: 128000,
        },
        None,
    )
    .unwrap();
    let mut sample = 0;
    for frame in 0..150 {
        encoder
            .video(
                &RgbaImage::from_pixel(64, 48, Rgba([(frame + 40) as u8, 60, 100, 255])),
                frame,
            )
            .unwrap();
        while sample < (frame + 1) * 1600 {
            let count = encoder.audio_frame_size().min((240000 - sample) as usize);
            encoder.audio(&vec![[0.2, 0.2]; count], sample).unwrap();
            sample += count as i64;
        }
    }
    encoder.finish().unwrap();
    let mut value = recipe();
    for source in value["sources"].as_array_mut().unwrap() {
        source["path"] = json!("camera.mov");
    }
    let input = dir.0.join("recipe.json");
    let output = dir.0.join("nested/episode.json");
    fs::create_dir(dir.0.join("nested")).unwrap();
    fs::write(&input, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let dry = cli(&dir, &["assemble", "recipe.json", "--dry-run", "--json"]);
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stdout)
    );
    assert_eq!(decode(&dry)["report"]["frames"], 60);
    assert!(!output.exists());
    let assembled = cli(
        &dir,
        &[
            "assemble",
            "recipe.json",
            "-o",
            "nested/episode.json",
            "--json",
        ],
    );
    assert!(
        assembled.status.success(),
        "{}",
        String::from_utf8_lossy(&assembled.stdout)
    );
    let original = fs::read(&output).unwrap();
    let (_, doc) = document::load(&output).unwrap();
    let media = probe::assets(&doc, &dir.0).unwrap();
    let mut mixer = Mixer::default();
    for at in [12000, 32000, 56000, 80000] {
        let samples = mixer.block(&doc, &dir.0, &media, at, 1024).unwrap();
        let mean = samples.iter().map(|s| s[0] as f64).sum::<f64>() / samples.len() as f64;
        assert!(
            (mean - 0.2 * 10.0_f64.powf(-6.0 / 20.0)).abs() < 0.015,
            "audio doubled or missing: {mean}"
        );
    }
    assert!(
        !cli(
            &dir,
            &[
                "assemble",
                "recipe.json",
                "-o",
                "nested/episode.json",
                "--json"
            ]
        )
        .status
        .success()
    );
    assert_eq!(fs::read(&output).unwrap(), original);
    for target in ["recipe.json", "camera.mov"] {
        let result = cli(
            &dir,
            &[
                "assemble",
                "recipe.json",
                "-o",
                target,
                "--overwrite",
                "--json",
            ],
        );
        assert_eq!(decode(&result)["error"]["code"], "output_is_source");
    }
    value["cameras"] = json!([]);
    fs::write(&input, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(
        !cli(
            &dir,
            &[
                "assemble",
                "recipe.json",
                "-o",
                "nested/episode.json",
                "--overwrite",
                "--json"
            ]
        )
        .status
        .success()
    );
    assert_eq!(fs::read(&output).unwrap(), original);
    let rendered = cli(
        &dir,
        &[
            "render",
            "nested/episode.json",
            "-o",
            "episode.mov",
            "--video-codec",
            "prores",
            "--progress",
            "none",
            "--json",
        ],
    );
    assert!(
        rendered.status.success(),
        "{}",
        String::from_utf8_lossy(&rendered.stdout)
    );
    assert_eq!(decode(&rendered)["frames"], 60);
    let worker = VideoWorker::new(dir.0.join("episode.mov"));
    for (output_frame, source_frame) in [(0, 60), (14, 74), (15, 15), (29, 29), (30, 60), (59, 89)]
    {
        let frame = worker.at(output_frame as f64 / 30.0).unwrap();
        assert!((frame.get_pixel(32, 24)[0] as i32 - (source_frame + 40)).abs() <= 5);
    }
    let schema = cli(&dir, &["schema", "--kind", "recipe", "--json"]);
    assert!(schema.status.success());
    assert_eq!(
        decode(&schema),
        serde_json::to_value(schemars::schema_for!(Recipe)).unwrap()
    );
}

fn recipe() -> Value {
    json!({"settings":{"width":64,"height":48,"frame_rate":{"numerator":30,"denominator":1},"audio_sample_rate":48000},"time_base":{"numerator":1,"denominator":1000},"sources":[{"name":"master","path":"master.wav","source_offset":0},{"name":"camera","path":"camera.mov","source_offset":1000},{"name":"silent","path":"silent.mov","source_offset":-1000}],"master_audio":"master","gain_db":-6.0,"retained":[{"start":1000,"end":2000},{"start":3000,"end":4000}],"cameras":[{"source":"camera","start":1000,"end":1500},{"source":"silent","start":1500,"end":4000}]})
}
fn metadata() -> Vec<Probe> {
    let mut out = Vec::new();
    for kinds in [vec!["audio"], vec!["video", "audio"], vec!["video"]] {
        let mut streams = Vec::new();
        for kind in kinds {
            streams.push(stream(kind));
        }
        out.push(Probe {
            container: "test".into(),
            duration: 10.0,
            streams,
            keyframe_interval_s: None,
        });
    }
    out
}
fn stream(kind: &str) -> Stream {
    Stream {
        index: 0,
        duration: Some(10.0),
        kind: kind.into(),
        codec: "test".into(),
        codec_tag: String::new(),
        bitrate: None,
        width: 64,
        height: 48,
        fps: Some([30, 1]),
        sample_rate: 48000,
        channels: 2,
        rotation: 0.0,
    }
}
fn cli(dir: &Temp, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_opencut"))
        .current_dir(&dir.0)
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
        let path = std::env::temp_dir().join(format!("opencut-assembly-{}", Ulid::generate()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

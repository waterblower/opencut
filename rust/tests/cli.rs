#![cfg(feature = "cli")]
use image::{Rgba, RgbaImage};
use opencut_player::{
    cli::{document, time::parse_rate},
    engine::{
        audio::Mixer,
        decode::VideoWorker,
        encode::{Encoder, VideoEncoding},
        probe,
    },
    timeline::*,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use ulid::Ulid;

#[test]
fn still_command_is_removed() {
    let help = cli(&["--help"]);
    let help = String::from_utf8(help.stdout).unwrap();
    for command in ["still"] {
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
#[cfg(target_os = "macos")]
fn render_refuses_existing_output() {
    let dir = Temp::new();
    let output = dir.0.join("existing.mp4");
    fs::write(&output, b"keep me").unwrap();
    let result = cli(&["render", "-o", output.to_str().unwrap(), "--json"]);
    assert_eq!(result.status.code(), Some(1));
    assert!(
        decode(&result)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("already exists")
    );
    assert_eq!(fs::read(output).unwrap(), b"keep me");
}

#[test]
#[cfg(target_os = "macos")]
#[ignore = "requires a macOS graphical session, Metal, and VideoToolbox"]
fn render_gpui_demo() {
    let dir = Temp::new();
    let output = dir.0.join("hello.mp4");
    let result = cli(&["render", "-o", output.to_str().unwrap(), "--json"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(decode(&result)["frames"], 150);
    probe::init().unwrap();
    let mut input = ffmpeg_next::format::input(&output).unwrap();
    let stream = input
        .streams()
        .best(ffmpeg_next::media::Type::Video)
        .unwrap();
    assert_eq!(stream.avg_frame_rate(), ffmpeg_next::Rational(30, 1));
    assert_eq!(stream.frames(), 150);
    assert!((stream.duration() as f64 * f64::from(stream.time_base()) - 5.0).abs() < 0.001);
    let index = stream.index();
    assert_eq!(
        input
            .packets()
            .filter(|(stream, _)| stream.index() == index)
            .count(),
        150
    );
    let worker = VideoWorker::new(output);
    for time in [0.0, 149.0 / 30.0] {
        let image = worker.at(time).unwrap();
        assert!(
            image.get_pixel(0, 0).0[..3]
                .iter()
                .all(|&channel| channel < 8)
        );
        let white_pixels = image
            .pixels()
            .filter(|pixel| pixel.0[..3].iter().all(|&channel| channel > 220))
            .count();
        assert!(
            white_pixels > 100,
            "text should produce visible white pixels"
        );
        assert!(white_pixels < (image.width() * image.height() / 10) as usize);
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
fn audio_resampling_gaps_mutes_and_hidden_video() {
    let dir = Temp::new();
    write_tone(&dir.0.join("tone.wav"), 44100, 2);
    let mut doc = empty();
    doc.assets
        .push(asset(100, "tone.wav", MediaKind::Audio, true));
    doc.clips
        .push(Clip::Audio(media_clip(200, 2, 100, 15, 0, 30)));
    let infos = probe::assets(&doc.assets, &dir.0).unwrap();
    let gap = Mixer::default()
        .block(&doc, &dir.0, &infos, 0, 1024)
        .unwrap();
    assert!(gap.iter().all(|s| *s == [0.0, 0.0]));
    let samples = Mixer::default()
        .block(&doc, &dir.0, &infos, 30000, 2048)
        .unwrap();
    assert!(power(&samples) > 0.01);
    doc.tracks[1].muted = true;
    assert!(
        Mixer::default()
            .block(&doc, &dir.0, &infos, 30000, 2048)
            .unwrap()
            .iter()
            .all(|s| *s == [0.0, 0.0])
    );
    doc.tracks[1].muted = false;
    let mut hidden = media_clip(201, 1, 100, 15, 0, 30);
    hidden.audio_properties.gain_db = -6.020599913;
    doc.clips = vec![Clip::Video(hidden)];
    doc.tracks[0].visible = false;
    let half = Mixer::default()
        .block(&doc, &dir.0, &infos, 30000, 2048)
        .unwrap();
    assert!((power(&half) / power(&samples) - 0.25).abs() < 0.001);
    doc.clips[0].media_mut().unwrap().audio_properties.muted = true;
    assert!(
        Mixer::default()
            .block(&doc, &dir.0, &infos, 30000, 2048)
            .unwrap()
            .iter()
            .all(|s| *s == [0.0, 0.0])
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

#[test]
fn native_video_seek_and_audio_mix() {
    let dir = Temp::new();
    let source = dir.0.join("source.mov");
    probe::init().unwrap();
    let fps = FrameRate::default();
    let mut encoder = Encoder::open(
        &source,
        (64, 48),
        fps,
        48000,
        &VideoEncoding {
            codec: "prores".into(),
            preset: "standard".into(),
            bitrate: 128000,
        },
        None,
    )
    .unwrap();
    let mut audio_at = 0;
    for f in 0..60 {
        let image = RgbaImage::from_pixel(64, 48, Rgba([(f * 3 + 20) as u8, 60, 100, 255]));
        encoder.encode_new_frame(&image).unwrap();
        while audio_at < (f + 1) * 1600 {
            let count = (encoder.audio_frame_size() as i64).min(96000 - audio_at) as usize;
            let mut samples = Vec::new();
            for n in audio_at..audio_at + count as i64 {
                let v = (n as f64 * 440.0 * std::f64::consts::TAU / 48000.0).sin() as f32 * 0.2;
                samples.push([v, v]);
            }
            encoder.audio(&samples, audio_at).unwrap();
            audio_at += count as i64;
        }
    }
    encoder.finish().unwrap();
    let output = cli(&["probe", source.to_str().unwrap(), "--json"]);
    assert!(output.status.success());
    let info = decode(&output);
    assert!(info["streams"].as_array().unwrap().iter().any(|stream| {
        stream["kind"] == "video" && stream["width"] == 64 && stream["height"] == 48
    }));
    let worker = VideoWorker::new(source.clone());
    for f in [0, 29, 30, 59, 15, 16, 0] {
        let frame = worker.at(f as f64 / 30.0).unwrap();
        let red = frame.get_pixel(32, 24)[0] as i32;
        assert!((red - (f * 3 + 20)).abs() <= 4, "frame {f} red {red}");
    }
    let mut doc = empty();
    doc.assets
        .push(asset(100, "source.mov", MediaKind::Video, true));
    let mut clip = media_clip(200, 1, 100, 15, 15, 45);
    clip.audio_properties.gain_db = -6.020599913;
    doc.clips.push(Clip::Video(clip));
    let media = probe::assets(&doc.assets, &dir.0).unwrap();
    doc.validate().unwrap();
    let mut mixer = Mixer::default();
    assert!(
        mixer
            .block(&doc, &dir.0, &media, 0, 1000)
            .unwrap()
            .iter()
            .all(|s| *s == [0.0, 0.0])
    );
    let samples = mixer.block(&doc, &dir.0, &media, 30000, 4096).unwrap();
    let rms =
        (samples.iter().map(|s| (s[0] * s[0]) as f64).sum::<f64>() / samples.len() as f64).sqrt();
    assert!((rms - 0.07071).abs() < 0.01, "RMS {rms}");
}

#[test]
fn variable_pts_frame_selection() {
    let dir = Temp::new();
    let source = dir.0.join("variable.mov");
    let sequential = dir.0.join("sequential.mov");
    let mut encoder = Encoder::open(
        &sequential,
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
    for red in [20, 80, 140, 200] {
        encoder
            .encode_new_frame(&RgbaImage::from_pixel(64, 48, Rgba([red, 60, 100, 255])))
            .unwrap();
    }
    encoder.finish().unwrap();
    // Remux the sequential export with irregular timestamps to exercise VFR decoding.
    let mut input = ffmpeg_next::format::input(&sequential).unwrap();
    let mut output = ffmpeg_next::format::output(&source).unwrap();
    let video = input
        .streams()
        .best(ffmpeg_next::media::Type::Video)
        .unwrap();
    let video_index = video.index();
    let mut stream = output
        .add_stream(ffmpeg_next::encoder::find(ffmpeg_next::codec::Id::PRORES))
        .unwrap();
    stream.set_parameters(video.parameters());
    stream.set_time_base((1, 30));
    output.write_header().unwrap();
    let time_base = output.stream(0).unwrap().time_base();
    let mut timestamps = [0, 2, 5, 9].into_iter();
    for (stream, mut packet) in input.packets() {
        if stream.index() != video_index {
            continue;
        }
        let pts = timestamps.next().unwrap();
        packet.set_stream(0);
        packet.set_pts(Some(pts));
        packet.set_dts(Some(pts));
        packet.set_duration(1);
        packet.set_position(-1);
        packet.rescale_ts((1, 30), time_base);
        packet.write_interleaved(&mut output).unwrap();
    }
    assert!(timestamps.next().is_none());
    output.write_trailer().unwrap();
    let worker = VideoWorker::new(source.clone());
    for (time, expected) in [
        (0.0, 20),
        (1.0 / 30.0, 20),
        (4.0 / 30.0, 140),
        (8.0 / 30.0, 200),
        (2.0 / 30.0, 80),
    ] {
        let image = worker.at(time).unwrap();
        assert!(
            (image.get_pixel(32, 24)[0] as i32 - expected).abs() <= 4,
            "time {time}"
        );
    }
}

#[test]
#[cfg(target_os = "macos")]
#[ignore = "requires macOS VideoToolbox encoder services"]
fn platform_video_encoders_and_fractional_frame_rate() {
    let dir = Temp::new();
    probe::init().unwrap();
    for (codec, container) in [("h264", "mp4"), ("hevc", "mp4"), ("hevc", "mov")] {
        let output = dir.0.join(format!("{codec}.{container}"));
        let mut encoder = Encoder::open(
            &output,
            (64, 48),
            FrameRate::new(30000, 1001),
            48000,
            &VideoEncoding {
                codec: codec.into(),
                preset: "standard".into(),
                bitrate: 500000,
            },
            None,
        )
        .unwrap();
        let image = RgbaImage::from_pixel(64, 48, Rgba([220, 20, 10, 255]));
        for _ in 0..5 {
            encoder.encode_new_frame(&image).unwrap();
        }
        encoder.finish().unwrap();
        let info = probe::probe(&output).unwrap();
        assert_eq!(info.streams[0].codec, codec);
        assert_eq!(info.streams[0].fps, Some([30000, 1001]));
        assert_eq!(
            info.streams[0].codec_tag,
            if codec == "hevc" { "hvc1" } else { "avc1" }
        );
        if codec == "hevc" {
            let mut input = ffmpeg_next::format::input(&output).unwrap();
            let stream = input
                .streams()
                .best(ffmpeg_next::media::Type::Video)
                .unwrap();
            let index = stream.index();
            let parameters = stream.parameters();
            let config = unsafe {
                let p = &*parameters.as_ptr();
                assert!(!p.extradata.is_null() && p.extradata_size >= 23);
                std::slice::from_raw_parts(p.extradata, p.extradata_size as usize).to_vec()
            };
            assert_eq!(config[0], 1);
            let length_size = (config[21] & 3) as usize + 1;
            let mut offset = 23;
            let mut parameter_sets = Vec::new();
            for _ in 0..config[22] {
                let kind = config[offset] & 63;
                if matches!(kind, 32..=34) {
                    assert_ne!(
                        config[offset] & 128,
                        0,
                        "hvc1 parameter-set array must be complete"
                    );
                    parameter_sets.push(kind);
                }
                let count = u16::from_be_bytes([config[offset + 1], config[offset + 2]]);
                offset += 3;
                for _ in 0..count {
                    let length = u16::from_be_bytes([config[offset], config[offset + 1]]) as usize;
                    offset += 2 + length;
                }
            }
            assert_eq!(parameter_sets, [32, 33, 34]);
            for (stream, packet) in input.packets() {
                if stream.index() != index {
                    continue;
                }
                let data = packet.data().unwrap();
                let mut offset = 0;
                while offset < data.len() {
                    let mut length = 0_usize;
                    for byte in &data[offset..offset + length_size] {
                        length = (length << 8) | *byte as usize;
                    }
                    offset += length_size;
                    assert!(length >= 2 && offset + length <= data.len());
                    assert!(
                        !matches!((data[offset] >> 1) & 63, 32..=34),
                        "hvc1 samples must not contain parameter sets"
                    );
                    offset += length;
                }
            }
        }
    }
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
        timeline_start: TimelineTime::from_frames(start),
        source_in: TimelineTime::from_frames(input),
        source_out: TimelineTime::from_frames(out),
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
fn power(samples: &[[f32; 2]]) -> f64 {
    samples.iter().map(|s| (s[0] * s[0]) as f64).sum::<f64>() / samples.len() as f64
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
    assert!(!text.contains("--project-root"));
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

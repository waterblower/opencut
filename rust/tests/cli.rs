#![cfg(feature = "cli")]
use image::{Rgba, RgbaImage};
use opencut_player::{
    cli::{
        document,
        engine::{
            audio::Mixer,
            compose::Composer,
            decode::VideoWorker,
            encode::{Encoder, VideoEncoding},
            probe, render,
        },
        time::parse_rate,
        validate,
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
#[cfg(target_os = "macos")]
fn chinese_titles_use_distinct_system_font_glyphs() {
    let mut raster = opencut_player::cli::engine::raster::TextRaster::default();
    let mut properties = TextClipProperties {
        text: "景".into(),
        font: "Heiti SC".into(),
        font_size: 48.0,
        ..TextClipProperties::default()
    };
    let first = raster.raster(&properties, 128, 96).unwrap();
    properties.text = "镜".into();
    let second = raster.raster(&properties, 128, 96).unwrap();
    assert!(first.pixels().any(|pixel| pixel[3] > 0));
    assert_ne!(
        first, second,
        "Chinese characters must not render as identical missing-glyph boxes"
    );
}

#[test]
fn schema_and_new_use_the_gui_document_contract() {
    let dir = Temp::new();
    let file = dir.0.join("new.timeline.json");
    let output = cli(&[
        "new",
        file.to_str().unwrap(),
        "--fps",
        "30000/1001",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let raw: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    assert!(raw.get("version").is_none());
    assert_eq!(raw["settings"]["audio_sample_rate"], 48000);
    assert_eq!(raw["tracks"][0]["kind"], "Video");
    assert_eq!(raw["tracks"][0]["visible"], true);
    let doc = document::parse(&raw).unwrap();
    assert_eq!(doc.settings.frame_rate, FrameRate::new(30000, 1001));
    let schema = cli(&["schema", "--json"]);
    assert!(schema.status.success());
    assert_eq!(
        decode(&schema),
        serde_json::to_value(schemars::schema_for!(TimelineSerialization)).unwrap()
    );
    assert!(!cli(&["new", file.to_str().unwrap()]).status.success());
    assert_eq!(
        fs::read(&file).unwrap(),
        serde_json::to_vec_pretty(&raw).unwrap()
    );
    let removed = cli(&["edit", file.to_str().unwrap(), "--json"]);
    assert_eq!(removed.status.code(), Some(2));
    fs::write(&file, r#"{"version":1,"clips":[]}"#).unwrap();
    let legacy = cli(&["inspect", file.to_str().unwrap(), "--json"]);
    assert_eq!(legacy.status.code(), Some(1));
    assert!(
        decode(&legacy)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("legacy_cli_format")
    );
}

#[test]
fn project_root_is_independent_of_timeline_directory() {
    let dir = Temp::new();
    fs::create_dir(dir.0.join("scenes")).unwrap();
    RgbaImage::from_pixel(64, 48, Rgba([220, 20, 10, 255]))
        .save(dir.0.join("image.png"))
        .unwrap();
    let mut doc = empty();
    doc.assets
        .push(asset(100, "image.png", MediaKind::Image, false));
    doc.clips
        .push(Clip::Video(media_clip(200, 1, 100, 0, 0, 30)));
    let file = dir.0.join("scenes/intro.timeline.json");
    document::write_atomic(&file, &serde_json::to_value(&doc).unwrap(), false).unwrap();
    let frame = dir.0.join("still.png");
    let result = cli(&[
        "--project-root",
        dir.0.to_str().unwrap(),
        "still",
        file.to_str().unwrap(),
        "--at",
        "50%",
        "-o",
        frame.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert_eq!(
        image::open(&frame).unwrap().to_rgba8().get_pixel(32, 24).0,
        [220, 20, 10, 255]
    );
    let result = Command::new(env!("CARGO_BIN_EXE_opencut"))
        .current_dir(&dir.0)
        .args(["validate", "scenes/intro.timeline.json", "--json"])
        .output()
        .unwrap();
    assert!(result.status.success());
    doc.assets[0].path = dir.0.join("image.png");
    document::write_atomic(&file, &serde_json::to_value(&doc).unwrap(), true).unwrap();
    assert!(
        cli(&["validate", file.to_str().unwrap(), "--json"])
            .status
            .success()
    );
}

#[test]
fn compositor_matches_gui_layer_visibility_and_pixel_offsets() {
    let dir = Temp::new();
    for (name, color) in [
        ("red.png", [255, 0, 0, 255]),
        ("blue.png", [0, 0, 255, 255]),
    ] {
        RgbaImage::from_pixel(64, 48, Rgba(color))
            .save(dir.0.join(name))
            .unwrap();
    }
    let mut doc = empty();
    doc.tracks.insert(0, track(4, TrackKind::Video));
    doc.assets = vec![
        asset(100, "red.png", MediaKind::Image, false),
        asset(101, "blue.png", MediaKind::Image, false),
    ];
    let mut overlay = media_clip(200, 4, 100, 0, 0, 30);
    overlay.video_properties = VideoClipProperties {
        position_x: 16.0,
        position_y: 0.0,
        scale: 0.5,
    };
    doc.clips = vec![
        Clip::Video(overlay),
        Clip::Video(media_clip(201, 1, 101, 0, 0, 60)),
    ];
    doc.tracks[0].locked = true;
    let frame = Composer::default().frame(&doc, &dir.0, 0).unwrap();
    assert_eq!(frame.get_pixel(10, 24).0, [0, 0, 255, 255]);
    assert_eq!(frame.get_pixel(48, 24).0, [255, 0, 0, 255]);
    doc.tracks[0].visible = false;
    let hidden = Composer::default().frame(&doc, &dir.0, 0).unwrap();
    assert_eq!(hidden.get_pixel(48, 24).0, [0, 0, 255, 255]);
    doc.clips.push(Clip::Text(TextClip {
        id: Ulid::from(202_u128),
        track_id: Ulid::from(3_u128),
        timeline_start: TimelineTime::from_frames(10),
        length: std::time::Duration::from_millis(500),
        properties: TextClipProperties {
            text: "Hi".into(),
            font_size: 14.0,
            color: 0xffff0000,
            ..Default::default()
        },
    }));
    assert_eq!(Composer::default().frame(&doc, &dir.0, 0).unwrap(), hidden);
    let caption = Composer::default().frame(&doc, &dir.0, 10).unwrap();
    assert_ne!(caption, hidden);
    assert!(
        caption
            .pixels()
            .any(|pixel| pixel[0] > 200 && pixel[1] < 30 && pixel[2] < 50)
    );
    assert_eq!(Composer::default().frame(&doc, &dir.0, 25).unwrap(), hidden);
    doc.tracks
        .iter_mut()
        .find(|t| t.kind == TrackKind::Text)
        .unwrap()
        .visible = false;
    assert_eq!(Composer::default().frame(&doc, &dir.0, 10).unwrap(), hidden);
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
    let infos = probe::assets(&doc, &dir.0).unwrap();
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
fn validation_reports_all_missing_assets_and_schema_locations() {
    let dir = Temp::new();
    let mut doc = empty();
    doc.assets = vec![
        asset(100, "missing-a.mp4", MediaKind::Video, true),
        asset(101, "missing-b.wav", MediaKind::Audio, true),
    ];
    let file = dir.0.join("missing.timeline.json");
    document::write_atomic(&file, &serde_json::to_value(&doc).unwrap(), false).unwrap();
    let result = cli(&[
        "validate",
        file.to_str().unwrap(),
        "--project-root",
        dir.0.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(result.status.code(), Some(1));
    let report = decode(&result);
    assert_eq!(report["findings"].as_array().unwrap().len(), 2);
    assert!(
        report["findings"][0]["message"]
            .as_str()
            .unwrap()
            .contains("/assets/0/path")
    );
    let mut raw = serde_json::to_value(&doc).unwrap();
    raw["settings"]["width"] = json!("wrong");
    assert!(
        document::parse(&raw)
            .unwrap_err()
            .to_string()
            .contains("/settings/width")
    );
}

#[test]
fn source_bitrate_respects_range_hidden_and_audio_tracks() {
    let mut doc = empty();
    doc.assets = vec![
        asset(100, "one.mov", MediaKind::Video, true),
        asset(101, "two.mov", MediaKind::Video, true),
    ];
    doc.clips = vec![
        Clip::Video(media_clip(200, 1, 100, 0, 0, 30)),
        Clip::Video(media_clip(201, 1, 101, 30, 0, 60)),
        Clip::Audio(media_clip(202, 2, 100, 0, 0, 90)),
    ];
    let mut media = std::collections::HashMap::new();
    for (id, bitrate) in [(100_u128, 1_000_000), (101, 4_000_000)] {
        media.insert(
            Ulid::from(id),
            validate::MediaInfo {
                duration: 4.0,
                video: true,
                audio: true,
                image: false,
                video_bitrate: Some(bitrate),
            },
        );
    }
    let mut options = render::Options {
        start: 0,
        end: 90,
        scale: 1.0,
        preset: "standard".into(),
        video_codec: "prores".into(),
        bitrate: None,
        overwrite: false,
        metadata: None,
    };
    assert_eq!(
        render::resolve_bitrate(&doc, &media, &options).unwrap(),
        (3_000_000, "source")
    );
    options.end = 30;
    assert_eq!(
        render::resolve_bitrate(&doc, &media, &options).unwrap(),
        (1_000_000, "source")
    );
    doc.tracks[0].visible = false;
    assert_eq!(
        render::resolve_bitrate(&doc, &media, &options).unwrap().1,
        "preset"
    );
    options.bitrate = Some(render::parse_bitrate("2.5M").unwrap());
    assert_eq!(
        render::resolve_bitrate(&doc, &media, &options).unwrap(),
        (2_500_000, "explicit")
    );
    for input in ["0", "-1", "NaN", "inf", "oops"] {
        assert!(render::parse_bitrate(input).is_err());
    }
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
fn native_video_seek_audio_mix_and_full_render() {
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
        encoder.video(&image, f).unwrap();
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
    let raw = serde_json::to_value(&doc).unwrap();
    let media = probe::assets(&doc, &dir.0).unwrap();
    validate::require_valid(&doc, Some(&media)).unwrap();
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
    let path = dir.0.join("timeline.json");
    document::write_atomic(&path, &raw, false).unwrap();
    let out = dir.0.join("output.mov");
    let render_started = std::time::Instant::now();
    let result = cli(&[
        "--project-root",
        dir.0.to_str().unwrap(),
        "render",
        path.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--video-codec",
        "prores",
        "--progress",
        "json",
        "--json",
    ]);
    assert!(
        result.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(decode(&result)["frames"], 45);
    assert_eq!(decode(&result)["bitrate_source"], "source");
    assert_eq!(
        decode(&result)["video_bitrate"],
        media[&Ulid::from(100_u128)].video_bitrate.unwrap()
    );
    let elapsed = render_started.elapsed();
    let progress = String::from_utf8_lossy(&result.stderr);
    let updates: Vec<Value> = progress
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(!updates.is_empty());
    assert!(
        updates.len() as u64 <= elapsed.as_secs() / 5 + 1,
        "progress must be throttled, not emitted per frame"
    );
    assert_eq!(updates.last().unwrap()["frame"], 45);
    assert_eq!(updates.last().unwrap()["total"], 45);
    assert_eq!(updates.last().unwrap()["eta_s"], 0.0);
    let rendered = probe::probe(&out).unwrap();
    assert!(
        (rendered.duration - 1.5).abs() < 0.04,
        "{}",
        rendered.duration
    );
    let input = ffmpeg_next::format::input(&out).unwrap();
    assert_eq!(
        input.metadata().get("opencut.timeline"),
        Some(raw.to_string().as_str())
    );
}

#[test]
fn variable_pts_and_mixed_frame_rate_selection() {
    let dir = Temp::new();
    let source = dir.0.join("variable.mov");
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
    for (pts, red) in [(0, 20), (2, 80), (5, 140), (9, 200)] {
        encoder
            .video(
                &RgbaImage::from_pixel(64, 48, Rgba([red, 60, 100, 255])),
                pts,
            )
            .unwrap();
    }
    encoder.finish().unwrap();
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
    let mut doc = empty();
    doc.settings.frame_rate = FrameRate::new(24, 1);
    doc.assets
        .push(asset(100, "variable.mov", MediaKind::Video, false));
    doc.clips
        .push(Clip::Video(media_clip(200, 1, 100, 0, 0, 7)));
    let image = Composer::default().frame(&doc, &dir.0, 4).unwrap();
    assert!((image.get_pixel(32, 24)[0] as i32 - 140).abs() <= 4);
}

#[test]
#[cfg(target_os = "macos")]
#[ignore = "requires macOS VideoToolbox encoder services"]
fn platform_video_encoders_and_fractional_frame_rate() {
    let dir = Temp::new();
    let timeline = dir.0.join("codecs.json");
    let mut doc = empty();
    doc.settings.frame_rate = FrameRate::new(30000, 1001);
    doc.clips.push(Clip::Text(TextClip {
        id: Ulid::from(200_u128),
        track_id: Ulid::from(3_u128),
        timeline_start: TimelineTime::ZERO,
        length: doc
            .settings
            .frame_rate
            .duration(TimelineTime::from_frames(5)),
        properties: TextClipProperties {
            text: "Hi".into(),
            font_size: 14.0,
            ..Default::default()
        },
    }));
    let raw = serde_json::to_value(&doc).unwrap();
    document::write_atomic(&timeline, &raw, false).unwrap();
    for (codec, container) in [("h264", "mp4"), ("hevc", "mp4"), ("hevc", "mov")] {
        let output = dir.0.join(format!("{codec}.{container}"));
        let result = cli(&[
            "render",
            timeline.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--video-codec",
            codec,
            "--bitrate",
            "500k",
            "--progress",
            "none",
            "--json",
        ]);
        assert!(
            result.status.success(),
            "{codec}: {}",
            String::from_utf8_lossy(&result.stdout)
        );
        assert_eq!(decode(&result)["video_bitrate"], 500000);
        assert_eq!(decode(&result)["bitrate_source"], "explicit");
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

fn empty() -> TimelineSerialization {
    TimelineSerialization {
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
    let output = Command::new(env!("CARGO_BIN_EXE_opencut")).args(["doc"]).output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("## Recommended workflow"));
    assert!(!text.contains("$schema"));
    assert!(!text.contains("--llm"));
    assert!(text.contains("--post-merge"));
    assert!(text.contains("--project-root"));
    let json_output = Command::new(env!("CARGO_BIN_EXE_opencut")).args(["docs", "--json"]).output().unwrap();
    assert!(json_output.status.success());
    assert_eq!(serde_json::from_slice::<Value>(&json_output.stdout).unwrap().as_str().unwrap(), text.strip_suffix('\n').unwrap());
}

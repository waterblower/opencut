#![cfg(feature = "cli")]

use image::{Rgba, RgbaImage};
use opencut_player::{
    core::{
        document::{self},
        time::{FrameRate, parse_rate},
        validate,
    },
    engine::{
        audio::Mixer,
        compose::Composer,
        decode::VideoWorker,
        encode::{Encoder, VideoEncoding},
        probe, render,
    },
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

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
fn document_validation_and_schema() {
    let mut raw = empty();
    raw["clips"] =
        json!([{"type":"text","id":"c","track_id":"missing","timeline_start":0,"length":30}]);
    let doc = document::parse(&raw).unwrap();
    let findings = validate::validate(&doc, None);
    assert!(
        findings
            .iter()
            .any(|f| f.error.pointer == "/clips/0/track_id" && f.error.code == "unknown_track")
    );
    raw["settings"]["width"] = json!("bad");
    assert_eq!(
        document::parse(&raw).unwrap_err().pointer,
        "/settings/width"
    );
    raw["version"] = json!(2);
    assert_eq!(
        document::parse(&raw).unwrap_err().code,
        "unsupported_version"
    );
    let output = cli(&["schema", "--json"]);
    assert!(output.status.success());
    let schema = decode(&output);
    assert!(schema["properties"]["clips"].is_object());
    assert!(schema["$defs"]["Clip"].is_object());
}

#[test]
fn edits_preserve_extensions_and_fail_atomically() {
    let dir = Temp::new();
    let path = dir.0.join("project.json");
    let mut raw = empty();
    raw["extension"] = json!({"future":[1,2,3]});
    raw["clips"] = json!([{"type":"text","id":"c","track_id":"v","timeline_start":0,"length":60,"future":{"x":true},"properties":{"text":"hello","future":"keep"}}]);
    document::write_atomic(&path, &raw, false).unwrap();
    let path = path.to_str().unwrap();
    let output = cli(&[
        "edit",
        path,
        "set",
        "--clip",
        "c",
        "--property",
        "properties.font_size",
        "--value",
        "40",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let (updated, _) = document::load(Path::new(path)).unwrap();
    assert_eq!(updated["extension"], raw["extension"]);
    assert_eq!(updated["clips"][0]["future"], raw["clips"][0]["future"]);
    assert_eq!(updated["clips"][0]["properties"]["future"], "keep");
    let before = fs::read(path).unwrap();
    let output = cli(&[
        "edit",
        path,
        "set",
        "--clip",
        "c",
        "--property",
        "opacity",
        "--value",
        "2",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(before, fs::read(path).unwrap());
    let split = cli(&[
        "edit",
        path,
        "split-clip",
        "--clip",
        "c",
        "--at",
        "30f",
        "--json",
    ]);
    assert!(
        split.status.success(),
        "{}",
        String::from_utf8_lossy(&split.stdout)
    );
    let entities = decode(&split);
    assert_eq!(entities["clips"][0]["length"], 30);
    assert_eq!(entities["clips"][1]["timeline_start"], 30);
    assert_eq!(entities["clips"][1]["future"], raw["clips"][0]["future"]);
}

#[test]
fn command_exit_codes_and_no_overwrite() {
    let dir = Temp::new();
    let path = dir.0.join("new.json");
    let path = path.to_str().unwrap();
    assert!(cli(&["new", path, "--json"]).status.success());
    let before = fs::read(path).unwrap();
    assert_eq!(cli(&["new", path, "--json"]).status.code(), Some(6));
    assert_eq!(before, fs::read(path).unwrap());
    assert_eq!(cli(&["nonsense", "--json"]).status.code(), Some(2));
    assert_eq!(
        cli(&["probe", "/nonexistent/opencut-media.mp4", "--json"])
            .status
            .code(),
        Some(4)
    );
    assert!(cli(&["validate", path, "--json"]).status.success());
    assert_eq!(decode(&cli(&["inspect", path, "--json"]))["frames"], 0);
}

#[test]
fn still_composition_effects_text_svg_and_determinism() {
    let dir = Temp::new();
    RgbaImage::from_pixel(32, 32, Rgba([255, 0, 0, 255]))
        .save(dir.0.join("red.png"))
        .unwrap();
    fs::write(dir.0.join("blue.svg"), r##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="#0000ff"/></svg>"##).unwrap();
    let mut raw = empty();
    raw["assets"] = json!([{"id":"a","path":"red.png"},{"id":"b","path":"blue.svg"}]);
    raw["tracks"] = json!([{"id":"v","kind":"video"},{"id":"overlay","kind":"video"},{"id":"text","kind":"text"}]);
    raw["clips"] = json!([
        {"type":"image","id":"c1","track_id":"v","asset_id":"a","timeline_start":0,"length":30},
        {"type":"image","id":"c2","track_id":"overlay","asset_id":"b","timeline_start":0,"length":30,"opacity":0.5,"video_properties":{"scale":0.5}},
        {"type":"text","id":"c3","track_id":"text","timeline_start":15,"length":15,"properties":{"text":"Hello","font_size":12}}
    ]);
    let doc = document::parse(&raw).unwrap();
    validate::require_valid(&doc, None).unwrap();
    let mut composer = Composer::default();
    let a = composer.frame(&doc, &dir.0, 0).unwrap();
    assert_eq!(a.get_pixel(1, 1).0, [0, 0, 0, 255]);
    let center = a.get_pixel(32, 24).0;
    assert!(
        center[0] >= 126 && center[0] <= 128 && center[2] >= 127 && center[2] <= 129,
        "{center:?}"
    );
    let b = composer.frame(&doc, &dir.0, 15).unwrap();
    assert_ne!(a.as_raw(), b.as_raw());
    let c = Composer::default().frame(&doc, &dir.0, 15).unwrap();
    assert_eq!(b.as_raw(), c.as_raw());
    let path = dir.0.join("timeline.json");
    document::write_atomic(&path, &raw, false).unwrap();
    let output = dir.0.join("still.png");
    let result = cli(&[
        "still",
        path.to_str().unwrap(),
        "--at",
        "15f",
        "-o",
        output.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert_eq!(image::open(output).unwrap().to_rgba8().as_raw(), b.as_raw());
}

#[test]
fn transitions_validate_handles_and_render_midpoints() {
    let dir = Temp::new();
    for (name, color) in [
        ("red.png", [255, 0, 0, 255]),
        ("blue.png", [0, 0, 255, 255]),
    ] {
        RgbaImage::from_pixel(64, 48, Rgba(color))
            .save(dir.0.join(name))
            .unwrap();
    }
    let mut raw = empty();
    raw["assets"] = json!([{"id":"a","path":"red.png"},{"id":"b","path":"blue.png"}]);
    raw["clips"] = json!([{"type":"image","id":"c1","track_id":"v","asset_id":"a","timeline_start":0,"length":30},{"type":"image","id":"c2","track_id":"v","asset_id":"b","timeline_start":30,"length":30}]);
    for effect in [
        json!({"type":"crossfade"}),
        json!({"type":"dip_to_color","color":"#00ff00"}),
        json!({"type":"wipe","direction":"left"}),
        json!({"type":"slide","direction":"up"}),
    ] {
        let mut transition = json!({"id":"t","from_clip":"c1","to_clip":"c2","duration":10});
        transition
            .as_object_mut()
            .unwrap()
            .extend(effect.as_object().unwrap().clone());
        raw["transitions"] = json!([transition]);
        let doc = document::parse(&raw).unwrap();
        validate::require_valid(&doc, None).unwrap();
        let mut c = Composer::default();
        let start = c.frame(&doc, &dir.0, 25).unwrap();
        assert_eq!(start.get_pixel(32, 24).0, [255, 0, 0, 255]);
        let middle = c.frame(&doc, &dir.0, 30).unwrap();
        if effect["type"] == "crossfade" {
            assert_eq!(middle.get_pixel(32, 24).0, [128, 0, 128, 255]);
        }
        if effect["type"] == "dip_to_color" {
            assert_eq!(middle.get_pixel(32, 24).0, [0, 255, 0, 255]);
        }
        assert_eq!(
            c.frame(&doc, &dir.0, 35).unwrap().get_pixel(32, 24).0,
            [0, 0, 255, 255]
        );
    }
    raw["clips"][1] = json!({"type":"media","id":"c2","track_id":"v","asset_id":"b","timeline_start":30,"source_in":0,"source_out":30});
    assert!(
        validate::validate(&document::parse(&raw).unwrap(), None)
            .iter()
            .any(|f| f.error.code == "insufficient_handles")
    );
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
    let mut raw = empty();
    raw["assets"] = json!([{"id":"a","path":"source.mov"}]);
    raw["clips"] = json!([{"type":"media","id":"c","track_id":"v","asset_id":"a","timeline_start":15,"source_in":15,"source_out":45,"audio_properties":{"gain_db":-6.020599913}}]);
    let doc = document::parse(&raw).unwrap();
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
        media["a"].video_bitrate.unwrap()
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
fn all_edit_commands_ranges_and_json_errors() {
    let dir = Temp::new();
    let timeline = dir.0.join("edit.json");
    let path = timeline.to_str().unwrap();
    document::write_atomic(&timeline, &empty(), false).unwrap();
    let image = dir.0.join("asset.png");
    RgbaImage::from_pixel(64, 48, Rgba([10, 20, 30, 255]))
        .save(&image)
        .unwrap();
    let track = cli(&[
        "edit",
        path,
        "add-track",
        "--kind",
        "text",
        "--name",
        "Titles",
        "--json",
    ]);
    assert!(track.status.success());
    let track = decode(&track);
    let id = track["id"].as_str().unwrap();
    let text = cli(&[
        "edit",
        path,
        "add-text",
        "--track",
        id,
        "--text",
        "Title",
        "--at",
        "0",
        "--duration",
        "2s",
        "--json",
    ]);
    assert!(text.status.success());
    let text = decode(&text);
    let text_id = text["id"].as_str().unwrap();
    let moved = cli(&[
        "edit",
        path,
        "move-clip",
        "--clip",
        text_id,
        "--to",
        "1s",
        "--json",
    ]);
    assert!(moved.status.success());
    assert_eq!(decode(&moved)["timeline_start"], 30);
    let clip = cli(&[
        "edit",
        path,
        "add-clip",
        "--track",
        "v",
        "--asset",
        image.to_str().unwrap(),
        "--at",
        "0",
        "--out",
        "3s",
        "--json",
    ]);
    assert!(
        clip.status.success(),
        "{}",
        String::from_utf8_lossy(&clip.stdout)
    );
    let dry = dir.0.join("dry.mov");
    let args = [
        "render",
        path,
        "-o",
        dry.to_str().unwrap(),
        "--range",
        "1s..2s",
        "--video-codec",
        "prores",
        "--dry-run",
        "--json",
    ];
    let dry_run = cli(&args);
    assert!(dry_run.status.success());
    assert_eq!(decode(&dry_run)["frames"], 30);
    assert!(!dry.exists());
    let output = dir.0.join("range.mov");
    let rendered = cli(&[
        "render",
        path,
        "-o",
        output.to_str().unwrap(),
        "--range",
        "1s..2s",
        "--scale",
        "0.5",
        "--video-codec",
        "prores",
        "--progress",
        "none",
        "--no-metadata",
        "--json",
    ]);
    assert!(
        rendered.status.success(),
        "{}",
        String::from_utf8_lossy(&rendered.stdout)
    );
    assert!(
        rendered.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&rendered.stderr)
    );
    let input = ffmpeg_next::format::input(&output).unwrap();
    assert!(input.metadata().get("opencut.timeline").is_none());
    let info = probe::probe(&output).unwrap();
    assert_eq!(info.streams[0].width, 32);
    assert!((info.duration - 1.0).abs() < 0.04);
    let removed = cli(&["edit", path, "remove-clip", "--clip", text_id, "--json"]);
    assert!(removed.status.success());
    let missing = cli(&["edit", path, "remove-clip", "--clip", text_id, "--json"]);
    assert_eq!(missing.status.code(), Some(3));
    let error = decode(&missing);
    assert_eq!(error["error"]["code"], "unknown_clip");
    assert!(error["error"]["file"].is_string());
    assert!(error["error"]["line"].as_u64().unwrap() > 0);
}

#[test]
fn audio_resampling_gaps_and_equal_power_crossfade() {
    let dir = Temp::new();
    let path = dir.0.join("tone.wav");
    let rate = 44100_u32;
    let count = rate * 4;
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
    fs::write(&path, wav).unwrap();
    let mut raw = empty();
    raw["assets"] = json!([{"id":"tone","path":"tone.wav"}]);
    raw["clips"] = json!([
        {"type":"media","id":"c1","track_id":"a-track","asset_id":"tone","timeline_start":0,"source_in":30,"source_out":60},
        {"type":"media","id":"c2","track_id":"a-track","asset_id":"tone","timeline_start":30,"source_in":60,"source_out":90}
    ]);
    raw["transitions"] =
        json!([{"id":"t","type":"crossfade","from_clip":"c1","to_clip":"c2","duration":12}]);
    let doc = document::parse(&raw).unwrap();
    let media = probe::assets(&doc, &dir.0).unwrap();
    validate::require_valid(&doc, Some(&media)).unwrap();
    let before = Mixer::default()
        .block(&doc, &dir.0, &media, 12000, 2048)
        .unwrap();
    let middle = Mixer::default()
        .block(&doc, &dir.0, &media, 47000, 2048)
        .unwrap();
    let before_power = before.iter().map(|s| (s[0] * s[0]) as f64).sum::<f64>();
    let middle_power = middle.iter().map(|s| (s[0] * s[0]) as f64).sum::<f64>();
    // Coherent tones intentionally add: equal-power gains give twice the power at midpoint.
    assert!(
        (middle_power / before_power - 2.0).abs() < 0.12,
        "ratio {}",
        middle_power / before_power
    );
    let silence = Mixer::default()
        .block(&doc, &dir.0, &media, 96000, 2048)
        .unwrap();
    assert!(silence.iter().all(|s| *s == [0.0, 0.0]));
    let mut mixer = Mixer::default();
    let mut samples = 0;
    while samples < 96000 {
        let count = (96000 - samples).min(997) as usize;
        let block = mixer.block(&doc, &dir.0, &media, samples, count).unwrap();
        assert_eq!(block.len(), count);
        samples += count as i64;
    }
    assert_eq!(samples, 96000);
}

#[test]
fn split_keeps_nested_duration_extensions() {
    let dir = Temp::new();
    let path = dir.0.join("duration.json");
    let mut raw = empty();
    raw["clips"] = json!([{"type":"text","id":"c","track_id":"v","timeline_start":0,"length":{"secs":2,"nanos":0,"future":"keep"}}]);
    document::write_atomic(&path, &raw, false).unwrap();
    let result = cli(&[
        "edit",
        path.to_str().unwrap(),
        "split-clip",
        "--clip",
        "c",
        "--at",
        "15f",
        "--json",
    ]);
    assert!(result.status.success());
    let result = decode(&result);
    assert_eq!(result["clips"][0]["length"]["future"], "keep");
    assert_eq!(result["clips"][1]["length"]["future"], "keep");
    let (_, doc) = document::load(&path).unwrap();
    assert_eq!(doc.clips[0].length(doc.settings.frame_rate), 15);
    assert_eq!(doc.clips[1].length(doc.settings.frame_rate), 45);
}

#[test]
fn cpu_effects_and_validation_extremes() {
    use opencut_player::{core::document::Effect, engine::effects};
    let mut source = RgbaImage::from_pixel(4, 2, Rgba([10, 20, 30, 255]));
    source.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
    let flipped = effects::apply(
        source.clone(),
        &[Effect::Flip {
            horizontal: true,
            vertical: true,
        }],
        1.0,
    );
    assert_eq!(flipped.get_pixel(3, 1).0, [255, 0, 0, 255]);
    let cropped = effects::apply(
        source.clone(),
        &[Effect::Crop {
            x: 0.0,
            y: 0.0,
            width: 0.5,
            height: 1.0,
        }],
        0.5,
    );
    assert_eq!(cropped.dimensions(), (2, 2));
    assert_eq!(cropped.get_pixel(0, 0)[3], 128);
    let gray = effects::apply(
        source.clone(),
        &[Effect::ColorAdjust {
            brightness: 0.0,
            contrast: 1.0,
            saturation: 0.0,
        }],
        1.0,
    );
    assert_eq!(gray.get_pixel(0, 0).0, [54, 54, 54, 255]);
    let blur = effects::apply(source.clone(), &[Effect::GaussianBlur { radius: 1.0 }], 1.0);
    assert_ne!(blur, source);
    let mut raw = empty();
    raw["clips"] = json!([{"type":"text","id":"c1","track_id":"v","timeline_start":0,"length":30},{"type":"text","id":"c2","track_id":"v","timeline_start":30,"length":30}]);
    raw["transitions"] =
        json!([{"id":"t","type":"crossfade","from_clip":"c1","to_clip":"c2","duration":i64::MIN}]);
    assert!(!validate::validate(&document::parse(&raw).unwrap(), None).is_empty());
}

#[test]
#[cfg(target_os = "macos")]
#[ignore = "requires access to macOS VideoToolbox services outside a sandbox"]
fn platform_video_encoders_and_fractional_frame_rate() {
    let dir = Temp::new();
    let timeline = dir.0.join("codecs.json");
    let mut raw = empty();
    raw["settings"]["frame_rate"] = json!({"numerator":30000,"denominator":1001});
    raw["clips"] = json!([{"type":"text","id":"c","track_id":"v","timeline_start":0,"length":5,"properties":{"text":"Hi","font_size":14}}]);
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
    let mut raw = empty();
    raw["settings"]["frame_rate"] = json!({"numerator":24,"denominator":1});
    raw["assets"] = json!([{"id":"source","path":"variable.mov"}]);
    raw["clips"] = json!([{"type":"media","id":"clip","track_id":"v","asset_id":"source","timeline_start":0,"source_in":0,"source_out":7}]);
    let doc = document::parse(&raw).unwrap();
    let image = Composer::default().frame(&doc, &dir.0, 4).unwrap();
    assert!((image.get_pixel(32, 24)[0] as i32 - 140).abs() <= 4);
}

#[test]
fn validation_reports_all_missing_assets() {
    let dir = Temp::new();
    let path = dir.0.join("missing.json");
    let mut raw = empty();
    raw["assets"] = json!([{"id":"a","path":"missing-a.mp4"},{"id":"b","path":"missing-b.mp4"}]);
    document::write_atomic(&path, &raw, false).unwrap();
    let result = cli(&["validate", path.to_str().unwrap(), "--json"]);
    assert_eq!(result.status.code(), Some(4));
    let result = decode(&result);
    assert_eq!(result["findings"].as_array().unwrap().len(), 2);
    assert_eq!(result["findings"][0]["pointer"], "/assets/0/path");
    assert_eq!(result["findings"][1]["pointer"], "/assets/1/path");
}

#[test]
fn source_bitrate_default_override_range_and_fallback() {
    let mut raw = empty();
    raw["assets"] = json!([{"id":"one","path":"one.mp4"},{"id":"two","path":"two.mp4"},{"id":"unused","path":"unused.mp4"}]);
    raw["clips"] = json!([
        {"type":"media","id":"c1","track_id":"v","asset_id":"one","timeline_start":0,"source_in":0,"source_out":30},
        {"type":"media","id":"c2","track_id":"v","asset_id":"two","timeline_start":30,"source_in":0,"source_out":60},
        {"type":"media","id":"audio","track_id":"a-track","asset_id":"unused","timeline_start":0,"source_in":0,"source_out":90}
    ]);
    let doc = document::parse(&raw).unwrap();
    let mut media = std::collections::HashMap::new();
    for (id, bitrate) in [
        ("one", 1_000_000),
        ("two", 4_000_000),
        ("unused", 100_000_000),
    ] {
        media.insert(
            id.into(),
            validate::MediaInfo {
                duration: 3.0,
                video: true,
                audio: true,
                image: false,
                video_bitrate: Some(bitrate),
            },
        );
    }
    let mut options = render::Options {
        start: 0,
        end: 30,
        scale: 1.0,
        preset: "high".into(),
        video_codec: "hevc".into(),
        bitrate: None,
        overwrite: false,
        metadata: None,
    };
    assert_eq!(
        render::resolve_bitrate(&doc, &media, &options).unwrap(),
        (1_000_000, "source")
    );
    options.end = 90;
    assert_eq!(
        render::resolve_bitrate(&doc, &media, &options).unwrap(),
        (3_000_000, "source")
    );
    options.start = 30;
    assert_eq!(
        render::resolve_bitrate(&doc, &media, &options).unwrap(),
        (4_000_000, "source")
    );
    options.bitrate = Some(render::parse_bitrate("2.5M").unwrap());
    assert_eq!(
        render::resolve_bitrate(&doc, &media, &options).unwrap(),
        (2_500_000, "explicit")
    );
    options.bitrate = None;
    media.get_mut("two").unwrap().video_bitrate = None;
    assert_eq!(
        render::resolve_bitrate(&doc, &media, &options).unwrap().1,
        "preset"
    );
    for (value, expected) in [
        ("1500000", 1_500_000),
        ("1500k", 1_500_000),
        ("1.5M", 1_500_000),
    ] {
        assert_eq!(render::parse_bitrate(value).unwrap(), expected);
    }
    for invalid in ["0", "-1", "NaN", "inf", "1e100", "oops"] {
        assert!(render::parse_bitrate(invalid).is_err());
    }
}

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("opencut-test-{}", ulid::Ulid::generate()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn empty() -> Value {
    json!({"version":1,"settings":{"width":64,"height":48,"frame_rate":{"numerator":30,"denominator":1},"sample_rate":48000,"background":"#000000"},"assets":[],"tracks":[{"id":"v","kind":"video"},{"id":"a-track","kind":"audio"}],"clips":[]})
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

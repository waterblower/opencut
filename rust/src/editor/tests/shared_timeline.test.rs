use super::super::{FrameRate, TimelineEditorExt, TimelineTime, timeline_document};
use super::*;
use gst_pbutils::prelude::*;
use image::{Rgba, RgbaImage};
use opencut_player::cli::{
    document,
    engine::{
        decode::VideoWorker,
        encode::{Encoder, VideoEncoding},
        probe,
    },
    validate,
};
use std::fs;

#[test]
fn shared_timeline_round_trip_and_editor_rendering() {
    ges::init().unwrap();
    let root = std::env::temp_dir().join(format!("opencut-shared-{}", Ulid::generate()));
    fs::create_dir_all(root.join("media")).unwrap();
    fs::create_dir(root.join("scenes")).unwrap();
    let source = root.join("media/camera.mov");
    write_test_camera(&source);
    RgbaImage::from_pixel(80, 45, Rgba([230, 220, 10, 255]))
        .save(root.join("media/title.png"))
        .unwrap();
    let mut timeline = timeline_document::deserialize_timeline(include_str!(
        "../../../tests/fixtures/shared.timeline.json"
    ))
    .unwrap();
    timeline.assets[1].path = "media/camera.mov".into();
    timeline.assets[1].codec = "aac".into();
    let path = root.join("scenes/shared.timeline.json");
    timeline.save(&path).unwrap();
    let (_, cli_document) = document::load(&path).unwrap();
    assert_eq!(
        serde_json::to_value(&timeline).unwrap(),
        serde_json::to_value(&cli_document).unwrap()
    );
    let mut edited = TimelineSerialization::load(&path).unwrap();
    edited.clips[0].media_mut().unwrap().source_out = TimelineTime::from_frames(74);
    let Clip::Text(text) = &mut edited.clips[3] else {
        panic!("expected text");
    };
    text.properties.text = "After edit".into();
    edited.save(&path).unwrap();
    let (raw, document) = document::load(&path).unwrap();
    assert_eq!(serde_json::to_value(&edited).unwrap(), raw);
    validate::require_valid(
        &document,
        Some(&probe::assets(&document.assets, &root).unwrap()),
    )
    .unwrap();
    let gstreamer_output = root.join("gstreamer.mp4");
    export_test_timeline(&edited, &root, &gstreamer_output);
    let expected = edited.seconds(edited.content_duration());
    {
        let output = &gstreamer_output;
        let info = probe::probe(output).unwrap();
        assert!(
            (info.duration - expected).abs() < 0.08,
            "duration {} expected {expected}",
            info.duration
        );
        assert!(info.streams.iter().any(|s| s.kind == "audio"));
    }
    {
        let output = &gstreamer_output;
        let mut audio_document = document.clone();
        audio_document.assets = vec![document.assets[1].clone()];
        audio_document.assets[0].path = output.clone();
        audio_document
            .tracks
            .retain(|track| track.kind == TrackKind::Audio);
        let mut audio_clip = document.clips[2].clone();
        let data = audio_clip.media_mut().unwrap();
        data.source_in = TimelineTime::ZERO;
        data.source_out = document.content_duration();
        data.audio_properties.gain_db = 0.0;
        audio_document.clips = vec![audio_clip];
        let media = probe::assets(&audio_document.assets, &root).unwrap();
        let samples = opencut_player::cli::engine::audio::Mixer::default()
            .block(&audio_document, &root, &media, 12000, 4096)
            .unwrap();
        let rms = (samples
            .iter()
            .map(|sample| (sample[0] * sample[0]) as f64)
            .sum::<f64>()
            / samples.len() as f64)
            .sqrt();
        assert!(
            (rms - 0.0708).abs() < 0.015,
            "master audio gain/muting for {}: RMS {rms}",
            output.display()
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn shared_timeline_podcast_assembly_exports_in_editor() {
    ges::init().unwrap();
    let root = std::env::temp_dir().join(format!("opencut-podcast-{}", Ulid::generate()));
    fs::create_dir(&root).unwrap();
    let source = root.join("camera.mov");
    write_test_camera(&source);
    let recipe = serde_json::from_value(serde_json::json!({
        "settings": {"width":160,"height":90,"frame_rate":{"numerator":24,"denominator":1},"audio_sample_rate":48000},
        "time_base":{"numerator":1,"denominator":24},
        "sources":[{"name":"master","path":"camera.mov","source_offset":0},
                   {"name":"other","path":"camera.mov","source_offset":24}],
        "master_audio":"master", "gain_db":-6.020599913,
        "retained":[{"start":0,"end":12},{"start":24,"end":48}],
        "cameras":[{"source":"master","start":0,"end":6},
                   {"source":"other","start":6,"end":48}]
    })).unwrap();
    let metadata = vec![
        probe::probe(&source).unwrap(),
        probe::probe(&source).unwrap(),
    ];
    let (document, _) = opencut_player::cli::assemble::compile(&recipe, &metadata).unwrap();
    let path = root.join("episode.json");
    document.save(&path).unwrap();
    let mut edited = TimelineSerialization::load(&path).unwrap();
    edited.tracks[0].name = "Podcast cameras".into();
    edited.save(&path).unwrap();
    let (_, document) = document::load(&path).unwrap();
    let gst_output = root.join("gstreamer.mp4");
    export_test_timeline(&edited, &root, &gst_output);
    {
        let output = &gst_output;
        let info = probe::probe(output).unwrap();
        assert!((info.duration - 1.5).abs() < 0.08);
        let worker = VideoWorker::new(output.clone());
        for frame in [0, 5, 6, 11, 12, 35] {
            let image = worker.at(frame as f64 / 24.0).unwrap();
            let pixel = image.get_pixel(80, 45);
            assert!(
                if frame < 6 {
                    pixel[0] > 180 && pixel[2] < 40
                } else {
                    pixel[2] > 180 && pixel[0] < 40
                },
                "frame {frame}: {pixel:?}"
            );
        }
        let mut audio = document.clone();
        audio.clips.retain(|clip| matches!(clip, Clip::Audio(_)));
        audio.clips.truncate(1);
        let data = audio.clips[0].media_mut().unwrap();
        data.source_in = TimelineTime::ZERO;
        data.source_out = TimelineTime::from_frames(36);
        data.audio_properties.gain_db = 0.0;
        audio.assets[0].path = output.clone();
        let metadata = probe::assets(&audio.assets, &root).unwrap();
        let mut mixer = opencut_player::cli::engine::audio::Mixer::default();
        for at in [2000, 14000, 26000, 50000] {
            let samples = mixer.block(&audio, &root, &metadata, at, 2048).unwrap();
            let rms = (samples
                .iter()
                .map(|sample| (sample[0] * sample[0]) as f64)
                .sum::<f64>()
                / samples.len() as f64)
                .sqrt();
            assert!(
                (rms - 0.0708).abs() < 0.015,
                "{} at {at}: {rms}",
                output.display()
            );
        }
    }
    fs::remove_dir_all(root).unwrap();
}

fn write_test_camera(source: &std::path::Path) {
    let mut encoder = Encoder::open(
        source,
        (160, 90),
        FrameRate::new(24, 1),
        48000,
        &VideoEncoding {
            codec: "prores".into(),
            preset: "standard".into(),
            bitrate: 128000,
        },
        None,
    )
    .unwrap();
    let mut audio_at = 0_i64;
    for frame in 0..96 {
        let color = if frame < 24 {
            [220, 20, 10, 255]
        } else {
            [10, 20, 220, 255]
        };
        encoder
            .video(&RgbaImage::from_pixel(160, 90, Rgba(color)), frame)
            .unwrap();
        while audio_at < (frame + 1) * 2000 {
            let count = (encoder.audio_frame_size() as i64).min(192000 - audio_at) as usize;
            let samples: Vec<_> = (audio_at..audio_at + count as i64)
                .map(|n| {
                    let value =
                        (n as f64 * 440.0 * std::f64::consts::TAU / 48000.0).sin() as f32 * 0.2;
                    [value, value]
                })
                .collect();
            encoder.audio(&samples, audio_at).unwrap();
            audio_at += count as i64;
        }
    }
    encoder.finish().unwrap();
}

fn export_test_timeline(
    timeline: &TimelineSerialization,
    root: &std::path::Path,
    output: &std::path::Path,
) {
    // Exercise the production GES timeline builder and encoding profile with a
    // software AAC encoder; atenc requires macOS services unavailable headlessly.
    let options = ExportOptions::from_timeline(timeline);
    let ges_timeline = build_ges_timeline(timeline, root, options, false).unwrap();
    let _encoder_selection = EncoderSelection::for_export(options.encoder).unwrap();
    let pipeline = ges::Pipeline::new();
    let production_profile = encoding_profile(options);
    let container_caps = production_profile.format();
    let mut builder = gst_pbutils::EncodingContainerProfile::builder(&container_caps);
    for stream in production_profile.profiles() {
        if stream.is::<gst_pbutils::EncodingAudioProfile>() {
            let audio = gst_pbutils::EncodingAudioProfile::builder(&stream.format())
                .preset_name("voaacenc")
                .presence(1)
                .build();
            builder = builder.add_profile(audio);
        } else {
            builder = builder.add_profile(stream);
        }
    }
    let profile = builder.build();
    configure_export_elements(&pipeline, options.video_bit_rate);
    pipeline.set_timeline(&ges_timeline).unwrap();
    pipeline
        .set_render_settings(Url::from_file_path(output).unwrap().as_str(), &profile)
        .unwrap();
    pipeline.set_mode(ges::PipelineFlags::RENDER).unwrap();
    let result = render_pipeline(
        &pipeline,
        timeline.duration(timeline.content_duration()),
        &mut |_| {},
    );
    pipeline.set_state(gst::State::Null).unwrap();
    result.unwrap();
}

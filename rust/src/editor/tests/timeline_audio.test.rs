use super::*;
use crate::editor::tests::{TimelineTestExt, lock_gstreamer_test};
use crate::editor::timeline_clip::{AudioClipProperties, VideoClip};

#[test]
fn renders_trimmed_audio_with_gaps_gain_and_no_output_files() {
    let _lock = lock_gstreamer_test();
    let root = std::env::temp_dir().join(format!("opencut-audio-{}", Ulid::generate()));
    std::fs::create_dir(&root).unwrap();
    let mut input = vec![0; 44];
    for _ in 0..32_000 {
        input.extend_from_slice(&8000_i16.to_le_bytes());
    }
    std::fs::write(
        root.join("source.wav"),
        opencut_player::transcribe::audio::write_wav_header(input).unwrap(),
    )
    .unwrap();
    let mut timeline = TimelineSerialization::with_test_tracks();
    timeline.settings.frame_rate = FrameRate::new(30, 1);
    let track_id = timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Audio)
        .unwrap()
        .id;
    let asset = media_probe::probe_asset(&root.join("source.wav")).unwrap();
    let asset_id = asset.id;
    let mut asset = asset;
    asset.path = "source.wav".into();
    timeline.assets.push(asset);
    timeline.clips.push(Clip::Audio(AudioClip {
        id: Ulid::generate(),
        track_id,
        asset_id,
        timeline_start: TimelineTime::from_frames(30),
        source_in: TimelineTime::from_frames(15),
        source_out: TimelineTime::from_frames(45),
        audio_properties: AudioClipProperties::default(),
        video_properties: VideoClipProperties::default(),
    }));
    let wav = render_audio_wav(&timeline, &root).unwrap();
    assert_eq!(wav.len(), 44 + 2 * 16_000 * 2);
    let samples: Vec<_> = wav[44..]
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();
    assert!(samples[..15_900].iter().all(|sample| *sample == 0));
    assert!(
        samples[17_000..30_000]
            .iter()
            .any(|sample| sample.abs() > 1000)
    );
    timeline.clips[0]
        .media_mut()
        .unwrap()
        .audio_properties
        .gain_db = -6.020599913;
    let quiet = render_audio_wav(&timeline, &root).unwrap();
    let amplitude = i16::from_le_bytes(quiet[40044..40046].try_into().unwrap());
    assert!((amplitude - 4000).abs() < 10, "{amplitude}");
    let mut other_track = timeline
        .tracks
        .iter()
        .find(|track| track.id == track_id)
        .unwrap()
        .clone();
    other_track.id = Ulid::generate();
    let mut overlapping = timeline.clips[0].clone();
    overlapping.media_mut().unwrap().id = Ulid::generate();
    overlapping.media_mut().unwrap().track_id = other_track.id;
    timeline.tracks.push(other_track);
    timeline.clips.push(overlapping);
    let mixed = render_audio_wav(&timeline, &root).unwrap();
    let amplitude = i16::from_le_bytes(mixed[40044..40046].try_into().unwrap());
    assert!((amplitude - 8000).abs() < 20, "{amplitude}");
    timeline.clips.pop();
    timeline.tracks.pop();
    let mut second = timeline.clips[0].clone();

    second.media_mut().unwrap().id = Ulid::generate();
    second.media_mut().unwrap().timeline_start = TimelineTime::from_frames(75);
    timeline.clips.push(second);
    let gaps = render_audio_wav(&timeline, &root).unwrap();
    assert_eq!(gaps.len(), 44 + 56_000 * 2);
    assert!(gaps[64044..78044].iter().all(|b| *b == 0));
    timeline.clips.pop();
    timeline.clips[0].media_mut().unwrap().timeline_start = TimelineTime::from_frames(499 * 30);
    assert_eq!(
        render_audio_wav(&timeline, &root).unwrap().len(),
        44 + 500 * 16_000 * 2
    );
    timeline.clips[0].media_mut().unwrap().timeline_start = TimelineTime::from_frames(30);
    std::fs::create_dir(root.join("nested")).unwrap();
    timeline
        .save(&root.join("nested/test.timeline.json"))
        .unwrap();
    let saved = TimelineSerialization::load(&root.join("nested/test.timeline.json")).unwrap();
    assert_eq!(render_audio_wav(&saved, &root).unwrap(), quiet);
    std::fs::remove_dir_all(root.join("nested")).unwrap();
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == track_id)
        .unwrap()
        .muted = true;
    assert!(
        render_audio_wav(&timeline, &root)
            .unwrap_err()
            .to_string()
            .contains("no enabled audio")
    );
    timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == track_id)
        .unwrap()
        .muted = false;
    timeline.clips[0].media_mut().unwrap().timeline_start = TimelineTime::from_frames(500 * 30);
    assert!(
        render_audio_wav(&timeline, &root)
            .unwrap_err()
            .to_string()
            .contains("500 seconds")
    );
    timeline.clips[0].media_mut().unwrap().timeline_start = TimelineTime::ZERO;
    std::fs::remove_file(root.join("source.wav")).unwrap();
    assert!(render_audio_wav(&timeline, &root).is_err());
    timeline.clips.clear();
    assert!(
        render_audio_wav(&timeline, &root)
            .unwrap_err()
            .to_string()
            .contains("positive")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn renders_video_audio_without_video_tracks() {
    let _lock = lock_gstreamer_test();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative = PathBuf::from("data/tests/mini测试/地铁-出站-mini-480.mp4");
    let mut asset = media_probe::probe_asset(&root.join(&relative)).unwrap();
    asset.path = relative;
    let asset_id = asset.id;
    let mut timeline = TimelineSerialization::with_test_tracks();
    timeline.settings.frame_rate = FrameRate::new(30, 1);
    timeline.assets.push(asset);
    let track_id = timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .unwrap()
        .id;
    timeline.clips.push(Clip::Video(VideoClip {
        id: Ulid::generate(),
        track_id,
        asset_id,
        timeline_start: TimelineTime::ZERO,
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::from_frames(30),
        audio_properties: AudioClipProperties::default(),
        video_properties: VideoClipProperties::default(),
    }));
    let wav = render_audio_wav(&timeline, root).unwrap();
    assert_eq!(wav.len(), 44 + 32_000);
    assert!(wav[44..].iter().any(|byte| *byte != 0));
}

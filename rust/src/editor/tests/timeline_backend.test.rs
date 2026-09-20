use crate::editor::preview_timeline::TimelinePreviewFrame;
use crate::editor::timeline_backend::{TimelineBackend, TimelineFrame, TimelineLayer};
use anyhow::Result;
use image::{Rgba, RgbaImage};
use opencut_player::timeline::{
    AudioClipProperties, Clip, FrameRate, MediaAsset, MediaClipData, MediaKind, TextClip,
    TextClipProperties, TimelineSerialization, TimelineTime, Track, TrackKind, VideoClipProperties,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};
use ulid::Ulid;

#[test]
fn saving_scroll_preserves_the_preview_and_editing_history() {
    use crate::editor::timeline::TimelineRuntimeState;
    use gpui::{point, px};

    let dir = Temp::new();
    let mut doc = document();
    doc.tracks.push(track(1, TrackKind::Text));
    doc.clips
        .push(text_clip(10, 1, 0, 8, doc.settings.frame_rate));
    let mut timeline =
        TimelineRuntimeState::new("scroll.timeline.json".into(), doc, &dir.0).unwrap();
    let original = wait_for_preview(&timeline.backend, time(0)).unwrap();
    timeline.h_scroll.set_offset(point(px(-120.0), px(0.0)));
    timeline.v_scroll.set_offset(point(px(0.0), px(-40.0)));
    timeline.save_timeline_scroll(&dir.0).unwrap();

    let cached = timeline.backend.preview_frame(time(0)).unwrap().unwrap();
    assert!(Arc::ptr_eq(&original, &cached));
    assert!(timeline.undo_stack.is_empty());
    assert!(timeline.redo_stack.is_empty());
    let saved: TimelineSerialization =
        serde_json::from_slice(&fs::read(dir.0.join(&timeline.path)).unwrap()).unwrap();
    assert_eq!(saved.view.horizontal_scroll, 120.0);
    assert_eq!(saved.view.vertical_scroll, 40.0);
    assert_eq!(saved.view.saved_playhead_frame, time(0));
    timeline.backend.timeline_mut().clips.clear();
    assert!(timeline.backend.preview_frame(time(0)).unwrap().is_none());
}

#[test]
fn preview_reuses_frames_and_replaces_document_snapshots() {
    let dir = Temp::new();
    let mut doc = document();
    doc.tracks.push(track(1, TrackKind::Text));
    doc.clips
        .push(text_clip(10, 1, 0, 8, doc.settings.frame_rate));
    let mut backend = TimelineBackend::new(doc, &dir.0).unwrap();
    let original = wait_for_preview(&backend, time(0)).unwrap();
    assert!(Arc::ptr_eq(
        &original,
        &backend.preview_frame(time(0)).unwrap().unwrap()
    ));
    assert!(backend.preview_frame(time(4)).unwrap().is_none());
    let sought = wait_for_preview(&backend, time(4)).unwrap();
    assert_eq!(sought.frame.timestamp, Duration::from_millis(500));
    let last = wait_for_preview(&backend, time(100)).unwrap();
    assert_eq!(last.frame.timestamp, Duration::from_millis(875));
    assert!(Arc::ptr_eq(
        &last,
        &backend.preview_frame(time(7)).unwrap().unwrap()
    ));
    backend.timeline_mut().clips.clear();
    assert!(backend.preview_frame(time(0)).unwrap().is_none());
    let edited = wait_for_preview(&backend, time(0)).unwrap();
    assert!(edited.frame.layers.is_empty());
    assert_eq!(original.frame.layers.len(), 1);
}

#[test]
fn preview_reports_decode_errors_and_recovers_after_an_edit() {
    let dir = Temp::new();
    let mut doc = document();
    doc.tracks.push(track(1, TrackKind::Video));
    doc.assets.push(asset(100, "missing.png", MediaKind::Image));
    doc.clips.push(media_clip(10, 1, 100, 0, 0, 8));
    let mut backend = TimelineBackend::new(doc, &dir.0).unwrap();
    let error = wait_for_preview(&backend, time(0)).err().unwrap();
    assert!(format!("{error:?}").contains("missing.png"));
    backend.timeline_mut().clips.clear();
    assert!(
        wait_for_preview(&backend, time(0))
            .unwrap()
            .frame
            .layers
            .is_empty()
    );
}

#[test]
fn new_rejects_missing_and_non_directory_media_roots_immediately() {
    let dir = Temp::new();
    let missing = dir.0.join("missing");
    let Err(error) = TimelineBackend::new(document(), &missing) else {
        panic!("missing media root was accepted");
    };
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::NotFound
    );
    let file = dir.0.join("file");
    fs::write(&file, b"").unwrap();
    let Err(error) = TimelineBackend::new(document(), &file) else {
        panic!("non-directory media root was accepted");
    };
    assert!(error.to_string().contains("not a directory"));
}

#[test]
fn metadata_empty_and_audio_only_timelines() {
    let dir = Temp::new();
    let mut doc = document();
    doc.view.saved_playhead_frame = time(42);
    let mut backend = TimelineBackend::open_sync(doc.clone(), &dir.0).unwrap();
    assert_eq!(backend.frame_size(), (16, 12));
    assert_eq!(backend.framerate(), Some(8.0));
    assert_eq!(backend.duration(), Duration::ZERO);
    assert_eq!(backend.position(), Duration::ZERO);
    backend.seek_sync(Duration::MAX).unwrap();
    let empty = backend.get_current_frame().unwrap();
    assert_eq!((empty.width, empty.height), (16, 12));
    assert!(empty.layers.is_empty());
    assert_eq!(empty.timestamp, Duration::ZERO);

    doc.tracks.push(track(1, TrackKind::Audio));
    // Audio requires no media access and still determines duration.
    let Clip::Video(audio) = media_clip(10, 1, 100, 8, 0, 16) else {
        unreachable!();
    };
    doc.clips.push(Clip::Audio(audio));
    let mut backend = TimelineBackend::open_sync(doc, &dir.0).unwrap();
    assert_eq!(backend.duration(), Duration::from_secs(3));
    backend.seek_sync(Duration::MAX).unwrap();
    assert_eq!(backend.position(), Duration::from_millis(2875));
    assert!(backend.get_current_frame().unwrap().layers.is_empty());
}

#[test]
fn text_boundaries_gaps_and_fractional_frame_rate() {
    let dir = Temp::new();
    let mut doc = document();
    doc.settings.frame_rate = FrameRate::new(30_000, 1001);
    doc.tracks.push(track(1, TrackKind::Text));
    doc.clips
        .push(text_clip(10, 1, 2, 2, doc.settings.frame_rate));
    doc.clips
        .push(text_clip(11, 1, 6, 2, doc.settings.frame_rate));
    let rate = doc.settings.frame_rate;
    let mut backend = TimelineBackend::open_sync(doc, &dir.0).unwrap();
    assert_eq!(backend.framerate(), Some(30_000.0 / 1001.0));
    assert_eq!(backend.duration(), rate.duration(time(8)));
    for (frame, ids) in [
        (0, vec![]),
        (2, vec![10]),
        (3, vec![10]),
        (4, vec![]),
        (6, vec![11]),
    ] {
        backend.seek_sync(rate.duration(time(frame))).unwrap();
        let snapshot = backend.get_current_frame().unwrap();
        assert_eq!(snapshot.timestamp, rate.duration(time(frame)));
        assert_eq!(layer_ids(&snapshot), ids);
    }
    backend.seek_sync(Duration::MAX).unwrap();
    assert_eq!(backend.position(), rate.duration(time(7)));
    backend.seek_sync(Duration::from_millis(61)).unwrap();
    assert_eq!(backend.position(), rate.duration(time(2)));
}

#[test]
fn layers_preserve_order_visibility_transforms_and_text() {
    let dir = Temp::new();
    RgbaImage::from_pixel(3, 2, Rgba([40, 80, 120, 128]))
        .save(dir.0.join("source.png"))
        .unwrap();
    let mut doc = document();
    doc.tracks = vec![
        track(1, TrackKind::Text),
        track(2, TrackKind::Video),
        track(3, TrackKind::Video),
    ];
    doc.tracks[0].locked = true;
    doc.tracks[0].muted = true;
    doc.tracks[2].visible = false;
    doc.assets.push(asset(100, "source.png", MediaKind::Image));
    doc.assets.push(asset(101, "missing.png", MediaKind::Image));
    let transform = VideoClipProperties {
        position_x: -20.0,
        position_y: 30.0,
        scale: 0.5,
    };
    let mut lower = media_clip(20, 2, 100, 0, 0, 16);
    lower.media_mut().unwrap().video_properties = transform;
    lower.media_mut().unwrap().audio_properties.muted = true;
    let text = TextClipProperties {
        text: "Timeline text".into(),
        font: "Example Font".into(),
        font_size: 23.0,
        color: 0x80445566,
        position_x: 0.2,
        position_y: 0.8,
    };
    doc.clips = vec![
        text_clip(10, 1, 0, 16, doc.settings.frame_rate),
        lower,
        media_clip(30, 3, 101, 0, 0, 24),
        media_clip(21, 2, 100, 0, 0, 16),
    ];
    let Clip::Text(top) = &mut doc.clips[0] else {
        unreachable!();
    };
    top.properties = text.clone();
    let mut backend = TimelineBackend::open_sync(doc, &dir.0).unwrap();
    assert_eq!(backend.duration(), Duration::from_secs(3));
    let snapshot = backend.get_current_frame().unwrap();
    assert_eq!(layer_ids(&snapshot), vec![20, 21, 10]);
    let TimelineLayer::Image {
        pixels, properties, ..
    } = &snapshot.layers[0]
    else {
        unreachable!();
    };
    assert_eq!(*properties, transform);
    assert_eq!(pixels.dimensions(), (3, 2));
    assert_eq!(pixels.get_pixel(0, 0).0, [40, 80, 120, 128]);
    let TimelineLayer::Image { pixels: second, .. } = &snapshot.layers[1] else {
        unreachable!();
    };
    assert!(Arc::ptr_eq(pixels, second));
    let TimelineLayer::Text { properties, .. } = &snapshot.layers[2] else {
        unreachable!();
    };
    assert_eq!(*properties, text);

    // Cached images and published snapshots survive deletion and backend drop.
    fs::remove_file(dir.0.join("source.png")).unwrap();
    backend.seek_sync(Duration::from_millis(125)).unwrap();
    let current = backend.get_current_frame().unwrap();
    let TimelineLayer::Image { pixels: reused, .. } = &current.layers[0] else {
        unreachable!();
    };
    assert!(Arc::ptr_eq(pixels, reused));
    drop(backend);
    assert_eq!(snapshot.timestamp, Duration::ZERO);
    assert_eq!(layer_ids(&snapshot), vec![20, 21, 10]);
}

#[test]
fn svg_images_resolve_absolute_paths_and_preserve_alpha() {
    let dir = Temp::new();
    let path = dir.0.join("source.svg");
    fs::write(&path, r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="3"><rect width="4" height="3" fill="red" opacity="0.5"/></svg>"#).unwrap();
    let mut doc = document();
    doc.tracks.push(track(1, TrackKind::Video));
    let mut svg = asset(100, "unused", MediaKind::Image);
    svg.path = path;
    doc.assets.push(svg);
    doc.clips.push(media_clip(10, 1, 100, 0, 0, 8));
    let media_root = dir.0.join("not-the-media-root");
    fs::create_dir(&media_root).unwrap();
    let backend = TimelineBackend::open_sync(doc, &media_root).unwrap();
    let frame = backend.get_current_frame().unwrap();
    let TimelineLayer::Image { pixels, .. } = &frame.layers[0] else {
        unreachable!();
    };
    assert_eq!(pixels.dimensions(), (4, 3));
    let pixel = pixels.get_pixel(1, 1).0;
    assert_eq!(pixel[..3], [255, 0, 0]);
    assert!((127..=128).contains(&pixel[3]));
}

#[test]
fn video_trims_rate_mapping_overlaps_and_backward_seeks() {
    let dir = Temp::new();
    write_video(&dir.0);
    let mut doc = document();
    doc.tracks.push(track(1, TrackKind::Video));
    doc.assets.push(asset(100, "source.avi", MediaKind::Video));
    doc.clips = vec![
        media_clip(10, 1, 100, 0, 2, 10),
        media_clip(11, 1, 100, 0, 8, 16),
    ];
    let mut backend = TimelineBackend::open_sync(doc, &dir.0).unwrap();
    let first = backend.get_current_frame().unwrap();
    assert_eq!(video_reds(&first), vec![35, 80]);
    assert!(Arc::ptr_eq(&first, &backend.get_current_frame().unwrap()));
    for (millis, reds) in [
        (125, vec![35, 80]),
        (250, vec![50, 95]),
        (875, vec![80, 125]),
        (250, vec![50, 95]),
        (250, vec![50, 95]),
        (0, vec![35, 80]),
    ] {
        backend.seek_sync(Duration::from_millis(millis)).unwrap();
        assert_eq!(video_reds(&backend.get_current_frame().unwrap()), reds);
    }
    assert_eq!(video_reds(&first), vec![35, 80]);
    backend.seek_sync(Duration::MAX).unwrap();
    assert_eq!(backend.position(), Duration::from_millis(875));
    assert_eq!(
        video_reds(&backend.get_current_frame().unwrap()),
        vec![80, 125]
    );
}

#[test]
fn failed_preparation_preserves_snapshot_and_can_be_retried() {
    let dir = Temp::new();
    write_video(&dir.0);
    let mut doc = document();
    doc.tracks = vec![track(1, TrackKind::Text), track(2, TrackKind::Video)];
    doc.assets.push(asset(100, "broken.avi", MediaKind::Video));
    doc.clips = vec![
        text_clip(10, 1, 0, 8, doc.settings.frame_rate),
        media_clip(20, 2, 100, 8, 0, 8),
    ];
    let mut backend = TimelineBackend::open_sync(doc, &dir.0).unwrap();
    let before = backend.get_current_frame().unwrap();
    for contents in [
        None,
        Some(b"corrupt media".as_slice()),
        Some(b"YUV4MPEG2 W8 H6 F4:1 Ip A1:1 C420jpeg\n".as_slice()),
    ] {
        if let Some(contents) = contents {
            fs::write(dir.0.join("broken.avi"), contents).unwrap();
        }
        let error = backend.seek_sync(Duration::from_secs(1)).unwrap_err();
        assert!(format!("{error:#}").contains("broken.avi"));
        assert_eq!(backend.position(), Duration::ZERO);
        assert!(Arc::ptr_eq(&before, &backend.get_current_frame().unwrap()));
    }
    fs::copy(dir.0.join("source.avi"), dir.0.join("broken.avi")).unwrap();
    backend.seek_sync(Duration::from_secs(1)).unwrap();
    assert_eq!(video_reds(&backend.get_current_frame().unwrap()), vec![20]);
    assert_eq!(layer_ids(&before), vec![10]);
}

#[test]
fn invalid_settings_and_visual_references_are_rejected() {
    let dir = Temp::new();
    let mut cases = Vec::new();
    let mut doc = document();
    doc.settings.width = 0;
    cases.push(doc);
    let mut doc = document();
    doc.settings.height = 0;
    cases.push(doc);
    let mut doc = document();
    doc.settings.frame_rate.numerator = 0;
    cases.push(doc);
    let mut doc = document();
    doc.settings.frame_rate.denominator = 0;
    cases.push(doc);
    let mut doc = document();
    doc.settings.audio_sample_rate = 0;
    cases.push(doc);
    let mut missing_track = document();
    missing_track.clips.push(media_clip(10, 1, 100, 0, 0, 8));
    cases.push(missing_track.clone());
    missing_track.tracks.push(track(1, TrackKind::Video));
    cases.push(missing_track.clone());
    missing_track
        .assets
        .push(asset(100, "audio.wav", MediaKind::Audio));
    cases.push(missing_track.clone());
    missing_track.assets[0].kind = MediaKind::Video;
    missing_track.clips[0]
        .media_mut()
        .unwrap()
        .video_properties
        .scale = f64::NAN;
    cases.push(missing_track);
    let mut wrong_track = document();
    wrong_track.tracks.push(track(1, TrackKind::Video));
    wrong_track
        .clips
        .push(text_clip(10, 1, 0, 8, wrong_track.settings.frame_rate));
    cases.push(wrong_track);
    let expected = [
        "canvas dimensions",
        "canvas dimensions",
        "frame rate",
        "frame rate",
        "audio sample rate",
        "missing track",
        "missing asset",
        "audio asset",
        "transform properties",
        "requires a text track",
    ];
    assert_eq!(cases.len(), expected.len());
    for (doc, expected) in cases.into_iter().zip(expected) {
        let Err(error) = TimelineBackend::new(doc, &dir.0) else {
            panic!("expected rejection for {expected}");
        };
        assert!(error.to_string().contains(expected), "{error:?}");
    }
}

struct Temp(PathBuf);

fn wait_for_preview(
    backend: &TimelineBackend,
    position: TimelineTime,
) -> Result<Arc<TimelinePreviewFrame>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(frame) = backend.preview_frame(position)? {
            return Ok(frame);
        }
        assert!(Instant::now() < deadline, "timeline preview did not finish");
        std::thread::sleep(Duration::from_millis(1));
    }
}

impl Temp {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("opencut-timeline-backend-{}", Ulid::generate()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn document() -> TimelineSerialization {
    let mut doc = TimelineSerialization::default();
    doc.settings.width = 16;
    doc.settings.height = 12;
    doc.settings.frame_rate = FrameRate::new(8, 1);
    doc
}

fn time(frames: i64) -> TimelineTime {
    TimelineTime::from_frames(frames)
}

fn track(id: u128, kind: TrackKind) -> Track {
    Track {
        id: Ulid::from(id),
        name: format!("Track {id}"),
        kind,
        locked: false,
        muted: false,
        visible: true,
    }
}

fn asset(id: u128, path: &str, kind: MediaKind) -> MediaAsset {
    MediaAsset {
        id: Ulid::from(id),
        kind,
        path: PathBuf::from(path),
        name: path.into(),
        duration: 3.0,
        width: 8,
        height: 6,
        framerate: 4.0,
        frame_rate_numerator: 4,
        frame_rate_denominator: 1,
        codec: String::new(),
        has_audio: false,
    }
}

fn media_clip(
    id: u128,
    track: u128,
    asset: u128,
    start: i64,
    source_in: i64,
    source_out: i64,
) -> Clip {
    Clip::Video(MediaClipData {
        id: Ulid::from(id),
        track_id: Ulid::from(track),
        asset_id: Ulid::from(asset),
        timeline_start: time(start),
        source_in: time(source_in),
        source_out: time(source_out),
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    })
}

fn text_clip(id: u128, track: u128, start: i64, length: i64, rate: FrameRate) -> Clip {
    Clip::Text(TextClip {
        id: Ulid::from(id),
        track_id: Ulid::from(track),
        timeline_start: time(start),
        length: rate.duration(time(length)),
        properties: TextClipProperties::default(),
    })
}

fn layer_ids(frame: &TimelineFrame) -> Vec<u128> {
    let mut ids = Vec::new();
    for layer in &frame.layers {
        let id = match layer {
            TimelineLayer::Video { clip_id, .. }
            | TimelineLayer::Image { clip_id, .. }
            | TimelineLayer::Text { clip_id, .. } => *clip_id,
        };
        ids.push(u128::from(id));
    }
    ids
}

fn video_reds(frame: &TimelineFrame) -> Vec<u8> {
    let mut reds = Vec::new();
    for layer in &frame.layers {
        let TimelineLayer::Video { pixels, .. } = layer else {
            panic!("expected video");
        };
        assert_eq!(pixels.dimensions(), (8, 6));
        reds.push(pixels.get_pixel(0, 0).0[0]);
    }
    reds
}

fn write_video(dir: &Path) {
    let mut bytes = Vec::new();
    for frame in 0..12 {
        let image = RgbaImage::from_pixel(8, 6, Rgba([20 + frame * 15, 60, 100, 255]));
        bytes.extend_from_slice(image.as_raw());
    }
    fs::write(dir.join("frames.rgba"), bytes).unwrap();
    let ffmpeg = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/ffmpeg-8.1.2/bin/ffmpeg");
    let output = Command::new(ffmpeg)
        .args([
            "-v",
            "error",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            "8x6",
            "-framerate",
            "4",
            "-i",
        ])
        .arg(dir.join("frames.rgba"))
        .args(["-c:v", "rawvideo", "-pix_fmt", "bgr24"])
        .arg(dir.join("source.avi"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

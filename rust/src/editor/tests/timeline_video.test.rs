use super::*;
use std::time::{Duration, Instant};

#[test]
fn repeated_drag_refreshes_allow_slow_frames_to_finish() {
    ges::init().unwrap();
    let pipeline = gst::parse::launch(
        "videotestsrc name=source pattern=black num-buffers=300 ! video/x-raw,format=RGBA,width=16,height=16,framerate=30/1 ! identity sleep-time=50000 ! appsink name=sink",
    ).unwrap().downcast::<gst::Pipeline>().unwrap();
    let sink = pipeline
        .by_name("sink")
        .unwrap()
        .downcast::<gst_app::AppSink>()
        .unwrap();
    let volume = gst::ElementFactory::make("volume")
        .build()
        .unwrap()
        .dynamic_cast::<gst_audio::StreamVolume>()
        .unwrap();
    let mut playback = VideoBackend::from_pipeline(pipeline.clone(), sink, volume).unwrap();
    pipeline.state(gst::ClockTime::from_seconds(5)).0.unwrap();
    let initial = playback.get_current_frame().unwrap();
    assert_eq!(
        initial.buffer().unwrap().map_readable().unwrap().as_slice()[0],
        0
    );
    pipeline
        .by_name("source")
        .unwrap()
        .set_property_from_str("pattern", "white");

    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(2) {
        try_refresh_timeline_video_frame(&mut playback).unwrap();
        std::thread::sleep(Duration::from_millis(16));
        let frame = playback.get_current_frame().unwrap();
        if frame.buffer().unwrap().map_readable().unwrap().as_slice()[0] == 255 {
            return;
        }
    }
    panic!("new pixels never arrived while drag refreshes continued");
}

#[test]
fn playback_resumes_after_repeated_resize_refreshes() {
    ges::init().unwrap();
    let timeline = ges::Timeline::new_audio_video();
    let clip = ges::TestClip::new().unwrap();
    clip.set_supported_formats(ges::TrackType::VIDEO);
    assert!(clip.set_duration(gst::ClockTime::from_seconds(10)));
    timeline.append_layer().add_clip(&clip).unwrap();
    assert!(timeline.commit());
    let audio_sink = gst::ElementFactory::make("fakesink").build().unwrap();
    let (pipeline, sink) = create_timeline_pipeline_v2(&timeline, &audio_sink).unwrap();
    let volume = gst::ElementFactory::make("volume")
        .build()
        .unwrap()
        .dynamic_cast::<gst_audio::StreamVolume>()
        .unwrap();
    let mut playback = VideoBackend::from_pipeline(pipeline.clone(), sink, volume).unwrap();
    pipeline.state(gst::ClockTime::from_seconds(5)).0.unwrap();
    for round in 0..20 {
        for step in 0..10 {
            clip.set_child_property("width", 160 + round * 10 + step)
                .unwrap();
            clip.set_child_property("height", 90 + round * 10 + step)
                .unwrap();
            // Transform properties apply without a structural timeline commit.
            try_refresh_timeline_video_frame(&mut playback).unwrap();
            std::thread::sleep(Duration::from_millis(16));
        }
        refresh_timeline_video_frame(&mut playback).unwrap();
        let position = playback.position();
        playback.set_paused(false);
        let started = Instant::now();
        while playback.position() <= position + Duration::from_millis(50) {
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "playback stalled after resize round {round}, state {:?}",
                pipeline.state(gst::ClockTime::ZERO)
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        playback.set_paused(true);
        pipeline.state(gst::ClockTime::from_seconds(5)).0.unwrap();
    }
}

#[cfg(target_os = "macos")]
#[test]
fn hevc_with_title_boundaries_uses_software_decoding() {
    use crate::editor::{
        export::ExportOptions,
        export_gstreamer::build_ges_timeline,
        media_probe::probe_video,
        timeline::{TimelineSerialization, TimelineTime},
    };
    ges::init().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/tests");
    let asset_id = ulid::Ulid::generate();
    let mut asset = probe_video(&root.join("4K.MOV"), asset_id).unwrap();
    asset.path = "4K.MOV".into();
    let mut data: TimelineSerialization = serde_json::from_value(serde_json::json!({
        "settings": {"frame_rate":{"numerator":24000,"denominator":1001},"width":320,"height":180,"audio_sample_rate":48000},
        "assets": [asset],
        "tracks": [
            {"id":"01M0PNQ7A7CTYF1YA8J2T5PQW5","name":"Video","kind":"Video","locked":false,"muted":false,"visible":true},
            {"id":"01M10SFVCR94J5XPK9WSXSA3X0","name":"Text","kind":"Text","locked":false,"muted":false,"visible":true},
            {"id":"01M2CXTP09K0KD0VBMEB9T2J4Z","name":"Text 2","kind":"Text","locked":false,"muted":false,"visible":true}
        ],
        "clips": []
    })).unwrap();
    data.clips.push(
        serde_json::from_value(serde_json::json!({"kind":"Video","data":{
            "id":"01M1BDHHMW2HQMDWQWJGVYKW8H","track_id":data.tracks[0].id,"asset_id":asset_id,
            "timeline_start":0,"source_in":0,"source_out":56,
            "video_properties":{"position_x":0.0,"position_y":0.0,"scale":1.0},
            "audio_properties":{"gain_db":0.0,"muted":false}
        }}))
        .unwrap(),
    );
    for (index, start, nanos) in [(1, 4, 1_500_000_000_u64), (2, 30, 140_000_000)] {
        data.clips.push(serde_json::from_value(serde_json::json!({"kind":"Text","data":{
            "id":ulid::Ulid::generate(),"track_id":data.tracks[index].id,"timeline_start":start,
            "length":{"secs":nanos/1_000_000_000,"nanos":nanos%1_000_000_000},
            "properties":{"text":"Title","font":"Sans","font_size":20.0,"color":4294967295_u32,"position_x":0.5,"position_y":0.5}
        }})).unwrap());
    }
    let timeline =
        build_ges_timeline(&data, &root, ExportOptions::from_timeline(&data), false).unwrap();
    let mut backend = TimelineVideoBackend::new(timeline).unwrap();
    let playback = backend.playback_mut();
    playback
        .pipeline()
        .state(gst::ClockTime::from_seconds(10))
        .0
        .unwrap();
    let mut software_decoder = false;
    let mut elements = playback.pipeline().iterate_recurse();
    while let Ok(Some(element)) = elements.next() {
        if let Some(factory) = element.factory() {
            software_decoder |= factory.name() == "avdec_h265";
            assert!(!matches!(factory.name().as_str(), "vtdec" | "vtdec_hw"));
        }
    }
    assert!(
        software_decoder,
        "HEVC title timeline must use the software decoder"
    );
    for frame in [26, 27, 28, 29, 30, 31, 30, 29, 28, 27] {
        playback
            .seek(data.duration(TimelineTime::from_frames(frame)))
            .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        playback
            .pipeline()
            .state(gst::ClockTime::from_seconds(10))
            .0
            .unwrap();
        let sample = playback.get_current_frame().unwrap();
        let pixels = sample.buffer().unwrap().map_readable().unwrap();
        let visible = pixels.as_slice()[..320 * 180]
            .iter()
            .filter(|&&value| value > 32)
            .count();
        assert!(visible > 320 * 180 / 10, "video missing at frame {frame}");
    }
}

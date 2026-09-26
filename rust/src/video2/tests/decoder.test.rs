use super::*;

#[test]
fn obsolete_audio_is_detected_before_its_seek_command_arrives() {
    let shared = Arc::new(Mutex::new(State::new()));
    let (_sender, receiver) = mpsc::channel::<()>();
    let mut control = Control::new(&shared, &receiver);
    assert!(!control.superseded(0));
    lock(&shared).generation = 1;
    assert!(!control.interrupted());
    assert!(control.superseded(0));
    assert!(!control.superseded(1));
}

#[test]
fn cache_uses_exact_half_open_intervals_and_evicts_to_its_budget() {
    let mut cache = FrameCache::default();
    for index in 0..48 {
        let frame = Arc::new(VideoFrame {
            timestamp: Duration::from_millis(index * 40),
            image: ffmpeg::frame::Video::new(ffmpeg::format::Pixel::YUV420P, 2048, 1024),
        });
        cache.insert(frame, Duration::from_millis(index * 40 + 40));
        assert!(cache.bytes <= CACHE_BYTES);
    }
    assert!(cache.get(Duration::ZERO).is_none());
    let target = Duration::from_millis(47 * 40);
    let frame = cache.get(target).expect("newest span retained");
    assert_eq!(frame.timestamp, target);
    assert!(cache.get(target + Duration::from_millis(39)).is_some());
    assert!(cache.get(target + Duration::from_millis(40)).is_none());
    let bytes = cache.bytes;
    cache.insert(frame, target + Duration::from_millis(40));
    assert_eq!(cache.bytes, bytes);
    // No inferred coverage across a discontinuity between decoded segments.
    assert!(cache.get(Duration::from_secs(100)).is_none());
}

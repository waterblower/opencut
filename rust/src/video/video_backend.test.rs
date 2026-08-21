use super::*;
use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};
use url::Url;

#[test]
fn measures_seek_to_half_of_a_video() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/tests/long video.mp4");
    let uri = Url::from_file_path(&source)
        .unwrap_or_else(|_| panic!("could not convert {} to a file URL", source.display()));
    let mut video = VideoBackend::open(&uri).expect("could not open test video");
    video.set_paused(true);
    video
        .pipeline()
        .state(gst::ClockTime::from_seconds(5))
        .0
        .expect("video did not pause before the measured seek");

    let duration = video.duration();
    assert!(!duration.is_zero(), "test video reported a zero duration");
    let target = duration / 2;

    let started_at = Instant::now();
    video
        .seek(target, true)
        .expect("could not seek test video to its midpoint");
    let elapsed = started_at.elapsed();

    eprintln!("VideoBackend::seek to 50% ({target:.2?} of {duration:.2?}) took {elapsed:.2?}");
}

#[test]
fn measures_seek_4k_video() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/tests/4K.mov");
    let uri = Url::from_file_path(&source)
        .unwrap_or_else(|_| panic!("could not convert {} to a file URL", source.display()));
    let mut video = VideoBackend::open(&uri).expect("could not open test video");
    video.set_paused(true);
    video
        .pipeline()
        .state(gst::ClockTime::from_seconds(5))
        .0
        .expect("video did not pause before the measured seek");

    let duration = video.duration();
    assert!(!duration.is_zero(), "test video reported a zero duration");
    let target = duration / 2;

    let started_at = Instant::now();
    video
        .seek(target, false)
        .expect("could not seek test video to its midpoint");
    let elapsed = started_at.elapsed();

    eprintln!("VideoBackend::seek to 50% ({target:.2?} of {duration:.2?}) took {elapsed:.2?}");
}

#[test]
fn adjacent_seek_uses_prefetched_frame() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/tests/4K.mov");
    let uri = Url::from_file_path(&source)
        .unwrap_or_else(|_| panic!("could not convert {} to a file URL", source.display()));
    let mut video = VideoBackend::open(&uri).expect("could not open test video");
    video.set_paused(true);
    video
        .pipeline()
        .state(gst::ClockTime::from_seconds(5))
        .0
        .expect("video did not pause before seeking");

    let target = video.duration() / 2;
    let prefetch_started_at = Instant::now();
    video
        .seek(target, true)
        .expect("could not start the prefetch window");
    let deadline = Instant::now() + Duration::from_secs(30);
    let cached_target = loop {
        let cached_target = video
            .frame_cache
            .lock()
            .frames
            .range(duration_ns(target)..)
            .next()
            .map(|(pts, _)| Duration::from_nanos(*pts));
        if let Some(cached_target) = cached_target {
            break cached_target;
        }
        assert!(
            Instant::now() < deadline,
            "preview pipeline did not cache an adjacent frame"
        );
        thread::sleep(Duration::from_millis(5));
    };

    let started_at = Instant::now();
    video
        .seek(cached_target, true)
        .expect("could not seek to the cached frame");
    let elapsed = started_at.elapsed();

    assert_eq!(*video.pending_cached_seek.lock(), Some(cached_target));
    assert!(
        elapsed < Duration::from_millis(8),
        "cached seek took {elapsed:.2?}"
    );
    eprintln!(
        "prefetch reached {cached_target:.2?} in {:.2?}; cached seek took {elapsed:.2?}",
        prefetch_started_at.elapsed()
    );
}

#[test]
fn cached_frame_must_cover_requested_position() {
    gst::init().expect("could not initialize GStreamer");
    let mut buffer = gst::Buffer::with_size(4).expect("could not create test frame");
    let buffer_mut = buffer
        .get_mut()
        .expect("newly-created test frame should be writable");
    buffer_mut.set_pts(gst::ClockTime::from_mseconds(50));
    buffer_mut.set_duration(gst::ClockTime::from_mseconds(20));
    let sample = gst::Sample::builder().buffer(&buffer).build();
    let mut cache = FrameCache::default();
    let initial = cache.prepare_window(
        Duration::ZERO,
        Duration::from_millis(100),
        Duration::from_millis(50),
    );
    assert!(initial.leading.is_some());
    assert!(initial.trailing.is_some());
    cache.insert(sample);

    assert_eq!(cache.byte_limit, Some(4_000));
    assert!(cache.frame_at(Duration::from_millis(49)).is_none());
    assert!(cache.frame_at(Duration::from_millis(50)).is_some());
    assert!(cache.frame_at(Duration::from_millis(69)).is_some());
    assert!(cache.frame_at(Duration::from_millis(70)).is_none());
}

#[test]
fn cache_recenters_and_only_loads_the_new_edge() {
    gst::init().expect("could not initialize GStreamer");
    let mut cache = FrameCache::default();
    let initial = cache.prepare_window(
        Duration::from_secs(10),
        Duration::from_secs(30),
        Duration::from_secs(20),
    );
    assert!(initial.leading.is_some());
    assert!(initial.trailing.is_some());
    let mut pending = VecDeque::new();
    initial.replace(&mut pending);
    let center_first = pending
        .pop_front()
        .expect("the center-to-forward segment should run first");
    assert_eq!(center_first.start, Duration::from_secs(20));
    assert_eq!(center_first.end, Duration::from_secs(30));
    let leading = pending
        .pop_front()
        .expect("the preceding segment should run second");
    assert_eq!(leading.start, Duration::from_secs(10));
    assert_eq!(leading.end, Duration::from_secs(20));
    for pts in [10, 20, 29] {
        let mut buffer = gst::Buffer::with_size(4).expect("could not create test frame");
        let buffer_mut = buffer
            .get_mut()
            .expect("newly-created test frame should be writable");
        buffer_mut.set_pts(gst::ClockTime::from_seconds(pts));
        buffer_mut.set_duration(gst::ClockTime::from_seconds(1));
        cache.insert(gst::Sample::builder().buffer(&buffer).build());
    }

    assert!(!cache.needs_recenter(Duration::from_millis(24_999)));
    assert!(cache.needs_recenter(Duration::from_secs(25)));
    let forward = cache
        .prepare_window(
            Duration::from_secs(15),
            Duration::from_secs(35),
            Duration::from_secs(25),
        )
        .trailing
        .expect("crossing the forward threshold should extend the trailing edge");
    assert_eq!(forward.start, Duration::from_secs(30));
    assert_eq!(forward.end, Duration::from_secs(35));

    assert!(!cache.needs_recenter(Duration::from_millis(20_001)));
    assert!(cache.needs_recenter(Duration::from_secs(20)));
    let backward = cache
        .prepare_window(
            Duration::from_secs(10),
            Duration::from_secs(30),
            Duration::from_secs(20),
        )
        .leading
        .expect("crossing the backward threshold should extend the leading edge");
    assert_eq!(backward.start, Duration::from_secs(10));
    assert_eq!(backward.end, Duration::from_secs(20));
}

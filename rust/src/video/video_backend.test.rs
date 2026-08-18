use super::*;
use std::{path::Path, time::Instant};
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
        .seek(target, false)
        .expect("could not seek test video to its midpoint");
    let elapsed = started_at.elapsed();

    eprintln!("VideoBackend::seek to 50% ({target:?} of {duration:?}) took {elapsed:?}");
}

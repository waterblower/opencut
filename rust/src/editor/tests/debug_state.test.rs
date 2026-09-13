use super::*;

#[test]
fn frame_dump_measures_visible_luma_without_including_row_padding() {
    gst::init().unwrap();
    let info = VideoInfo::builder(gstreamer_video::VideoFormat::Nv12, 2, 2)
        .build()
        .unwrap();
    let mut buffer = gst::Buffer::with_size(info.size()).unwrap();
    {
        let buffer = buffer.get_mut().unwrap();
        buffer.set_pts(gst::ClockTime::from_seconds(1));
        let mut pixels = buffer.map_writable().unwrap();
        pixels.fill(255);
        let stride = info.stride()[0] as usize;
        for row in 0..2 {
            pixels[row * stride..row * stride + 2].fill(16);
        }
    }
    let sample = gst::Sample::builder()
        .buffer(&buffer)
        .caps(&info.to_caps().unwrap())
        .build();
    let report = frame_snapshot(&sample);
    assert_eq!(report["pts_ns"], 1_000_000_000_u64);
    assert_eq!(report["luma"]["sample_count"], 4);
    assert_eq!(report["luma"]["mean"], 16.0);
    assert_eq!(report["luma"]["histogram"][16], 4);
    assert_eq!(report["luma"]["histogram"][255], 0);
}

#[test]
fn frame_dump_handles_missing_frame_metadata() {
    gst::init().unwrap();
    let sample = gst::Sample::builder().build();
    let report = frame_snapshot(&sample);
    assert_eq!(report["present"], true);
    assert_eq!(report["caps"], Value::Null);
}

#[test]
fn object_dump_excludes_unlisted_file_locations() {
    gst::init().unwrap();
    let source = gst::ElementFactory::make("filesrc")
        .property("location", "/private/test-token-must-not-be-copied")
        .build()
        .unwrap();
    let report = object_properties(source.upcast_ref());
    assert!(!report.to_string().contains("test-token"));
    assert!(report.get("location").is_none());
}

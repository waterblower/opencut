//! macOS integration test for the VideoToolbox GStreamer export path.

use super::*;

#[test]
fn exports_every_video_in_the_mini_fixture_with_videotoolbox() {
    gst::init().unwrap();
    assert!(
        gst::ElementFactory::find("vtenc_h264_hw").is_some(),
        "VideoToolbox export test requires the GStreamer vtenc_h264_hw element"
    );

    integration_tests::export_mini_fixture(
        ExportEncoder::Hardware,
        "assembled-export-videotoolbox.mp4",
    );
}

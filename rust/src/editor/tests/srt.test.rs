use super::*;
use std::time::Duration;

#[test]
fn parses_cues_at_the_requested_frame_rate() {
    let srt = SRT::from_string("1\n00:00:01,000 --> 00:00:02,500\nHello\nworld\n").unwrap();
    let clips = srt_text_clips(&srt, FrameRate::new(24, 1)).unwrap();

    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].timeline_start.frames(), 24);
    assert_eq!(clips[0].length, Duration::from_millis(1_500));
    assert_eq!(clips[0].properties.text, "Hello\nworld");
}

#[test]
fn propagates_invalid_srt_timestamp() {
    let error = SRT::from_string("1\n00:00:01,000 --> invalid\nHello\n").unwrap_err();

    assert!(error.to_string().contains("invalid timestamp: invalid"));
}

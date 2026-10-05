use super::*;
use std::time::Duration;

#[test]
fn parses_cues_at_the_requested_frame_rate() {
    let srt = SRT::from_string("1\n00:00:01,000 --> 00:00:02,500\nHello\nworld\n").unwrap();
    let clips = srt_text_clips(&srt, FrameRate::new(24, 1));

    assert_eq!(clips.len(), 1);
    assert_eq!(i64::from(clips[0].timeline_start), 24);
    assert_eq!(clips[0].duration, Duration::from_millis(1_500));
    assert_eq!(clips[0].properties.text, "Hello\nworld");
}

#[test]
fn propagates_invalid_srt_timestamp() {
    let error = SRT::from_string("1\n00:00:01,000 --> invalid\nHello\n").unwrap_err();

    assert!(error.to_string().contains("invalid timestamp: invalid"));
}

use std::fs;
use std::path::PathBuf;

#[test]
fn writes_utf8_subtitles_to_the_exact_absolute_path_and_overwrites() {
    let directory = test_directory();
    let text = "1\n00:00:00,100 --> 00:00:01,000\n你好 hello\n";
    let mut srt = SRT::from_string(text).unwrap();
    let path = directory.path.join("chosen-name.srt");
    write_srt(&path, &srt).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), srt.to_string());
    srt.subtitles[0].text = "replacement".into();
    write_srt(&path, &srt).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), srt.to_string());
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
    assert!(write_srt(Path::new("relative.srt"), &srt).is_err());
    assert!(write_srt(&directory.path.join("missing/output.srt"), &srt).is_err());
}

struct TestDirectory {
    path: PathBuf,
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn test_directory() -> TestDirectory {
    let path = std::env::temp_dir().join(format!("opencut-srt-test-{}", Ulid::generate()));
    fs::create_dir(&path).unwrap();
    TestDirectory { path }
}

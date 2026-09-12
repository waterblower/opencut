use super::*;
use crate::editor::write_srt;
use std::{fs, path::Path};

#[tokio::test]
async fn rejects_missing_key_media_and_timeline_sources() {
    let directory = test_directory();
    let error = start_transcription(
        directory.path.join("missing.mp4"),
        directory.path.clone(),
        String::new(),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("MiniMax API key"));
    let error = start_transcription(
        directory.path.join("missing.mp4"),
        directory.path.clone(),
        "test-key".into(),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("unreadable_media"));
    let error = start_transcription(
        directory.path.join("source.timeline.json"),
        directory.path.clone(),
        "test-key".into(),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("could not read"));
}

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

use super::*;
use crate::editor::write_srt;
use std::{fs, path::Path};

#[tokio::test]
async fn rejects_missing_key_media_and_timeline_sources() {
    let directory = test_directory();
    let error = start_transcription(directory.path.join("missing.mp4"), String::new())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("MiniMax API key"));
    let error = start_transcription(directory.path.join("missing.mp4"), "test-key".into())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("unreadable_media"));
    let error = start_transcription(
        directory.path.join("source.timeline.json"),
        "test-key".into(),
    )
    .await
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("timeline transcription is not supported")
    );
}

#[test]
fn writes_utf8_subtitles_to_the_exact_absolute_path_and_overwrites() {
    let directory = test_directory();
    let text = "1\n00:00:00,100 --> 00:00:01,000\n你好 hello\n";
    let path = directory.path.join("chosen-name.srt");
    write_srt(&path, text).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), text);
    write_srt(&path, "replacement").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "replacement");
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
    assert!(write_srt(Path::new("relative.srt"), text).is_err());
    assert!(write_srt(&directory.path.join("missing/output.srt"), text).is_err());
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

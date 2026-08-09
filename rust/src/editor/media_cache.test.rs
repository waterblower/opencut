use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn generates_waveform_without_a_subprocess() {
    ffmpeg::init().unwrap();
    let audio_source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data/tests/mini测试/地铁-出站-mini-480.mp4");
    assert!(
        format::input(&audio_source)
            .ok()
            .is_some_and(|input| input.streams().best(Type::Audio).is_some()),
        "test video must contain audio"
    );
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("opencut-media-cache-{unique}"));
    fs::create_dir_all(&directory).unwrap();
    let waveform = directory.join("waveform.ocwf");

    generate_waveform(&audio_source, &waveform).unwrap();

    let waveform_data = load_waveform_file(&audio_source, &waveform).unwrap();
    assert!(waveform_data.sample_rate > 0);
    assert!(waveform_data.total_samples > 0);
    assert!(waveform_data.levels.len() > 1);
    assert_eq!(waveform_data.levels[0].samples_per_peak, 64);
    assert_eq!(
        fs::read(&waveform).unwrap().get(..4),
        Some(WAVEFORM_MAGIC.as_slice())
    );
    assert_eq!(waveform_data.columns(0.0, 1.0, 320).len(), 320);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cache_keys_are_stable_and_source_path_based() {
    let first = media_key(Path::new("media/first.mp4"));
    assert_eq!(first, media_key(Path::new("media/first.mp4")));
    assert_ne!(first, media_key(Path::new("media/second.mp4")));
}
